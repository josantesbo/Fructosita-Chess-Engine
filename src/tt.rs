//! Tabla de transposición (TT): caché de posiciones ya analizadas, indexada
//! por hash Zobrist. Evita re-analizar la misma posición cuando se llega a
//! ella por distintos órdenes de movimientos, y guarda el mejor movimiento
//! encontrado para mejorar el ordenamiento de movimientos en visitas futuras.
//!
//! Diseño para Lazy SMP: la tabla se divide en muchos "shards" (fragmentos)
//! independientes, cada uno protegido por su propio `Mutex`. Varios hilos de
//! búsqueda pueden compartir la MISMA tabla (vía `Arc<TranspositionTable>`)
//! y probarla/escribirla concurrentemente: solo se bloquean entre sí si dos
//! hilos golpean el mismo shard exactamente al mismo tiempo, algo raro con
//! suficientes shards. Se eligió esta técnica (candados finos) en vez del
//! "lockless hashing" de motores como Stockfish (empaquetar la entrada en
//! enteros atómicos, tolerando lecturas parcialmente corruptas gracias a la
//! verificación de la clave) porque es igual de segura en la práctica para
//! la cantidad de hilos que tiene sentido usar en una laptop normal, y con
//! muchísimo menor riesgo de bugs sutiles de concurrencia.

use crate::moves::Move;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TTFlag {
    /// El valor guardado es exacto (se completó una búsqueda con ventana abierta).
    Exact,
    /// El valor guardado es una cota inferior (hubo poda beta: el valor real es >= score).
    LowerBound,
    /// El valor guardado es una cota superior (ningún movimiento mejoró alfa: el valor real es <= score).
    UpperBound,
}

#[derive(Clone, Copy)]
struct TTEntry {
    key: u64,
    depth: i8,
    score: i32,
    flag: TTFlag,
    best_move: Option<Move>,
}

pub struct TTProbe {
    pub depth: i8,
    pub score: i32,
    pub flag: TTFlag,
    pub best_move: Option<Move>,
}

/// Cantidad de fragmentos independientes. Con este número, incluso con
/// varios hilos golpeando la tabla constantemente, la probabilidad de que
/// dos caigan en el mismo fragmento a la vez es baja.
const NUM_SHARDS: usize = 1024;

struct Shard {
    entries: Mutex<Vec<Option<TTEntry>>>,
}

pub struct TranspositionTable {
    shards: Vec<Shard>,
    entries_per_shard: usize,
    total_mask: usize,
}

impl TranspositionTable {
    pub fn new(mb: usize) -> Self {
        let bytes = mb.max(1) * 1024 * 1024;
        let entry_size = std::mem::size_of::<Option<TTEntry>>().max(1);
        let mut total_entries = (bytes / entry_size).max(NUM_SHARDS);
        total_entries = total_entries.next_power_of_two();
        // No pasarnos de más del doble del tamaño pedido.
        if total_entries > (bytes / entry_size).max(NUM_SHARDS) * 2 {
            total_entries /= 2;
        }
        let num_shards = NUM_SHARDS.min(total_entries);
        let entries_per_shard = (total_entries / num_shards).max(1);
        let actual_total = entries_per_shard * num_shards;

        let shards = (0..num_shards)
            .map(|_| Shard {
                entries: Mutex::new(vec![None; entries_per_shard]),
            })
            .collect();

        TranspositionTable {
            shards,
            entries_per_shard,
            total_mask: actual_total - 1,
        }
    }

    #[inline(always)]
    fn locate(&self, key: u64) -> (usize, usize) {
        let global = (key as usize) & self.total_mask;
        (
            global / self.entries_per_shard,
            global % self.entries_per_shard,
        )
    }

    /// Vacía todas las entradas. Seguro de llamar con la tabla compartida
    /// (`&self`, gracias a los `Mutex` internos), pero en la práctica el
    /// motor solo lo hace cuando no hay ninguna búsqueda en curso.
    pub fn clear(&self) {
        for shard in &self.shards {
            let mut entries = shard.entries.lock().unwrap();
            for e in entries.iter_mut() {
                *e = None;
            }
        }
    }

    pub fn probe(&self, key: u64) -> Option<TTProbe> {
        let (shard_idx, local_idx) = self.locate(key);
        let entries = self.shards[shard_idx].entries.lock().unwrap();
        if let Some(entry) = &entries[local_idx] {
            if entry.key == key {
                return Some(TTProbe {
                    depth: entry.depth,
                    score: entry.score,
                    flag: entry.flag,
                    best_move: entry.best_move,
                });
            }
        }
        None
    }

    pub fn store(&self, key: u64, depth: i32, score: i32, flag: TTFlag, best_move: Option<Move>) {
        let (shard_idx, local_idx) = self.locate(key);
        let depth = depth.clamp(0, i8::MAX as i32) as i8;
        let mut entries = self.shards[shard_idx].entries.lock().unwrap();
        let replace = match &entries[local_idx] {
            None => true,
            Some(existing) => existing.key == key || existing.depth <= depth,
        };
        if replace {
            entries[local_idx] = Some(TTEntry {
                key,
                depth,
                score,
                flag,
                best_move,
            });
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
