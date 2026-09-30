//! Tabla de transposición (TT): caché de posiciones ya analizadas, indexada
//! por hash Zobrist. Evita re-analizar la misma posición cuando se llega a
//! ella por distintos órdenes de movimientos, y guarda el mejor movimiento
//! encontrado para mejorar el ordenamiento de movimientos en visitas futuras.
//!
//! EXP-0013 (SPEED-A): diseño sin locks. Antes cada acceso tomaba el `Mutex`
//! de un shard y cada entrada ocupaba 24 bytes (con Hash=64 la tabla usaba en
//! realidad 96 MB). Ahora cada entrada son dos `AtomicU64` (16 bytes):
//!   - `data`: jugada, score, profundidad, cota y generación empaquetados;
//!   - `check`: la clave Zobrist XOR `data`.
//! Una lectura válida exige `check ^ data == clave`: si otro hilo escribió a
//! medias (una palabra nueva y otra vieja), la verificación falla y la entrada
//! se trata como ausente, nunca como una mezcla corrupta. Es el esquema
//! "lockless hashing" público (Hyatt y Mann), implementado aquí desde cero.
//! Mismo número de entradas y misma regla de reemplazo que antes: la búsqueda
//! de un hilo es idéntica (misma firma de bench).

use crate::moves::{Move, MoveKind};
use crate::types::PieceType;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TTFlag {
    /// El valor guardado es exacto (se completó una búsqueda con ventana abierta).
    Exact,
    /// El valor guardado es una cota inferior (hubo poda beta: el valor real es >= score).
    LowerBound,
    /// El valor guardado es una cota superior (ningún movimiento mejoró alfa: el valor real es <= score).
    UpperBound,
}

pub struct TTProbe {
    pub depth: i8,
    pub score: i32,
    pub flag: TTFlag,
    pub best_move: Option<Move>,
}

struct Slot {
    check: AtomicU64,
    data: AtomicU64,
}

pub struct TranspositionTable {
    slots: Vec<Slot>,
    total_mask: usize,
    /// Generación actual: se incrementa al empezar cada búsqueda (`go`).
    /// EXP-0005 (envejecimiento): una entrada de una búsqueda anterior se puede
    /// reemplazar aunque sea más profunda.
    generation: AtomicU8,
}

// Disposición de `data` (bits):
//   0..16  jugada (from 6 | to 6 | tipo 4), 16 = hay jugada
//   17..33 score (i16)
//   33..41 profundidad (i8)
//   41..43 cota
//   43..51 generación
//   51     entrada ocupada
const VALID_BIT: u64 = 1 << 51;

fn encode_kind(k: MoveKind) -> u64 {
    let promo = |p: PieceType| match p {
        PieceType::Knight => 0,
        PieceType::Bishop => 1,
        PieceType::Rook => 2,
        _ => 3,
    };
    match k {
        MoveKind::Quiet => 0,
        MoveKind::DoublePawnPush => 1,
        MoveKind::Capture => 2,
        MoveKind::EnPassantCapture => 3,
        MoveKind::CastleKingside => 4,
        MoveKind::CastleQueenside => 5,
        MoveKind::Promotion(p) => 6 + promo(p),
        MoveKind::PromotionCapture(p) => 10 + promo(p),
    }
}

fn decode_kind(c: u64) -> MoveKind {
    let promo = |i: u64| match i {
        0 => PieceType::Knight,
        1 => PieceType::Bishop,
        2 => PieceType::Rook,
        _ => PieceType::Queen,
    };
    match c {
        0 => MoveKind::Quiet,
        1 => MoveKind::DoublePawnPush,
        2 => MoveKind::Capture,
        3 => MoveKind::EnPassantCapture,
        4 => MoveKind::CastleKingside,
        5 => MoveKind::CastleQueenside,
        6..=9 => MoveKind::Promotion(promo(c - 6)),
        _ => MoveKind::PromotionCapture(promo(c - 10)),
    }
}

fn pack(depth: i8, score: i32, flag: TTFlag, best_move: Option<Move>, generation: u8) -> u64 {
    let mv = match best_move {
        Some(m) => (1 << 16) | (m.from as u64) | ((m.to as u64) << 6) | (encode_kind(m.kind) << 12),
        None => 0,
    };
    let flag = match flag {
        TTFlag::Exact => 0u64,
        TTFlag::LowerBound => 1,
        TTFlag::UpperBound => 2,
    };
    mv | (((score as i16) as u16 as u64) << 17)
        | ((depth as u8 as u64) << 33)
        | (flag << 41)
        | ((generation as u64) << 43)
        | VALID_BIT
}

#[inline]
fn unpack_depth(d: u64) -> i8 {
    ((d >> 33) & 0xFF) as u8 as i8
}

#[inline]
fn unpack_generation(d: u64) -> u8 {
    ((d >> 43) & 0xFF) as u8
}

fn unpack(d: u64) -> TTProbe {
    let best_move = if d & (1 << 16) != 0 {
        Some(Move::new((d & 63) as u8, ((d >> 6) & 63) as u8, decode_kind((d >> 12) & 15)))
    } else {
        None
    };
    TTProbe {
        depth: unpack_depth(d),
        score: ((d >> 17) & 0xFFFF) as u16 as i16 as i32,
        flag: match (d >> 41) & 3 {
            0 => TTFlag::Exact,
            1 => TTFlag::LowerBound,
            _ => TTFlag::UpperBound,
        },
        best_move,
    }
}

impl TranspositionTable {
    pub fn new(mb: usize) -> Self {
        let bytes = mb.max(1) * 1024 * 1024;
        let wanted = (bytes / std::mem::size_of::<Slot>()).max(1024);
        // La mayor potencia de dos que cabe en lo pedido: nunca se excede el
        // tamaño que fija la opción UCI `Hash`.
        let total = if wanted.is_power_of_two() { wanted } else { wanted.next_power_of_two() / 2 };
        let slots = (0..total)
            .map(|_| Slot { check: AtomicU64::new(0), data: AtomicU64::new(0) })
            .collect();
        TranspositionTable {
            slots,
            total_mask: total - 1,
            generation: AtomicU8::new(0),
        }
    }

    /// Marca el comienzo de una búsqueda nueva (EXP-0005).
    pub fn new_search(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    fn slot(&self, key: u64) -> &Slot {
        &self.slots[(key as usize) & self.total_mask]
    }

    /// Adelanta a la caché la línea de la entrada de `key`. No cambia nada del
    /// resultado: solo oculta la latencia de memoria del `probe` que vendrá.
    #[inline(always)]
    pub fn prefetch(&self, key: u64) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
            _mm_prefetch(self.slot(key) as *const Slot as *const i8, _MM_HINT_T0);
        }
    }

    /// Vacía todas las entradas. Solo se llama sin búsqueda en curso.
    pub fn clear(&self) {
        for s in &self.slots {
            s.check.store(0, Ordering::Relaxed);
            s.data.store(0, Ordering::Relaxed);
        }
    }

    #[inline]
    fn read(&self, key: u64) -> Option<u64> {
        let s = self.slot(key);
        let data = s.data.load(Ordering::Relaxed);
        let check = s.check.load(Ordering::Relaxed);
        if data & VALID_BIT != 0 && check ^ data == key {
            Some(data)
        } else {
            None
        }
    }

    pub fn probe(&self, key: u64) -> Option<TTProbe> {
        self.read(key).map(unpack)
    }

    pub fn store(&self, key: u64, depth: i32, score: i32, flag: TTFlag, best_move: Option<Move>) {
        let depth = depth.clamp(0, i8::MAX as i32) as i8;
        let generation = self.generation.load(Ordering::Relaxed);
        let s = self.slot(key);
        let old = s.data.load(Ordering::Relaxed);
        let replace = if old & VALID_BIT == 0 {
            true
        } else {
            let old_key = s.check.load(Ordering::Relaxed) ^ old;
            old_key == key || unpack_generation(old) != generation || unpack_depth(old) <= depth
        };
        if replace {
            let data = pack(depth, score, flag, best_move, generation);
            s.data.store(data, Ordering::Relaxed);
            s.check.store(key ^ data, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn store_and_probe_roundtrip() {
        let tt = TranspositionTable::new(1);
        assert!(tt.probe(12345).is_none());
        tt.store(12345, 5, 100, TTFlag::Exact, None);
        let probe = tt.probe(12345).unwrap();
        assert_eq!(probe.depth, 5);
        assert_eq!(probe.score, 100);
        assert_eq!(probe.flag, TTFlag::Exact);
    }

    #[test]
    fn stale_deep_entry_is_replaced_after_new_search() {
        // EXP-0005: misma casilla, entrada profunda de una búsqueda anterior.
        let tt = TranspositionTable::new(1);
        let a = 7u64;
        let b = a + (tt.total_mask as u64 + 1); // colisiona en la misma casilla
        tt.store(a, 20, 1, TTFlag::Exact, None);
        tt.store(b, 3, 2, TTFlag::Exact, None);
        assert!(tt.probe(a).is_some(), "misma generación: la profunda se conserva");
        tt.new_search();
        tt.store(b, 3, 2, TTFlag::Exact, None);
        assert_eq!(tt.probe(b).unwrap().score, 2, "generación nueva: se reemplaza");
    }

    #[test]
    fn clear_empties_table() {
        let tt = TranspositionTable::new(1);
        tt.store(1, 1, 1, TTFlag::Exact, None);
        tt.clear();
        assert!(tt.probe(1).is_none());
    }

    #[test]
    fn concurrent_access_from_many_threads_never_panics_or_corrupts() {
        // Machaca la misma tabla compartida desde muchos hilos a la vez,
        // con muchas claves distintas colisionando deliberadamente en pocos
        // shards (tabla pequeña a propósito). La propiedad que importa no es
        // "todo hilo siempre encuentra su propio dato" (con una tabla tan
        // pequeña y tantas escrituras, es normal que unas entradas
        // reemplacen a otras) sino: nunca debe entrar en pánico, y si un
        // `probe` SÍ encuentra coincidencia de clave, los datos deben ser
        // exactamente los que se guardaron para esa clave (nunca una mezcla
        // corrupta de dos escrituras distintas).
        let tt = Arc::new(TranspositionTable::new(1));
        let mut handles = Vec::new();
        for t in 0..8u64 {
            let tt = Arc::clone(&tt);
            handles.push(thread::spawn(move || {
                for i in 0..20_000u64 {
                    let key = (t * 1_000_003) ^ i;
                    let depth = ((i % 30) + 1) as i32;
                    let score = (key % 1000) as i32 - 500;
                    tt.store(key, depth, score, TTFlag::Exact, None);
                    if let Some(probe) = tt.probe(key) {
                        // Si la clave coincide exactamente, el score guardado
                        // para ESA clave siempre se deriva determinísticamente
                        // de la clave misma (ver arriba), así que podemos
                        // verificar que no está corrupto.
                        assert_eq!(probe.score, (key % 1000) as i32 - 500);
                    }
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }
}
