#![allow(non_snake_case)]
//! Búsqueda del motor: negamax con poda alfa-beta, Principal Variation
//! Search (PVS), null-move pruning, Late Move Reductions (LMR), extensión
//! por jaque, quiescence search, y profundización iterativa con manejo de
//! tiempo y control mediante `stop`.
//!
//! Todas estas son técnicas públicas y estándar (Chess Programming Wiki),
//! implementadas aquí desde cero, no copiadas de ningún motor existente.

use crate::board::Board;
use crate::tune;
use crate::eval;
use crate::movegen::{generate_legal_captures_into, generate_legal_moves, generate_legal_moves_into};
use crate::moves::Move;
use crate::tt::{TTFlag, TranspositionTable};
use crate::types::{Color, PieceType};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

pub const MATE_SCORE: i32 = 30_000;
pub const MAX_PLY: usize = 128;
const INFINITY: i32 = 32_000;

// Continuation History 1-ply (investigación 10). Flag para la verificación de
// fontanería: con `false`, la evaluación de ordenación vuelve a ser exactamente
// la del baseline (equivalencia bit a bit).
const ENABLE_CONT_HISTORY: bool = true;
// Factor de normalización de escala entre historiales. MEDIDO en nuestro motor
// (no copiado): la tabla de continuation history tiene 18x mas celdas, asi que
// cada celda acumula ~20x menos (medido: 21.1 vs 425.0 de media por jugada
// silenciosa). Sin normalizar, la senal quedaba diluida y solo movia el 3.5% de
// las decisiones por indice (umbral pre-registrado: 5%). Ver investigacion 10.
const CONT_HISTORY_SCALE: i32 = 20;
// Tope de saturación de la tabla i16 (subiteración 10a). Forzado por la
// representación: 1.000.000 no cabe en 16 bits. El valor medio medido por jugada
// silenciosa es ~21, muy por debajo, así que la pérdida de precisión solo afecta
// a las celdas más calientes. Se verifica empíricamente que la señal sobrevive.
const CONT_HISTORY_MAX: i16 = 16_384;
// Build de MEDICIÓN: calcula la doble ordenación (con y sin cont-history) para
// obtener `tier_change_pct` (puerta 2). Cuesta tiempo → OFF en el candidato.


// ---------------------------------------------------------------------------
// EXP-0005 (SEARCH-A) — LMR logarítmica.
// La reducción crece con el producto ln(profundidad)·ln(nº de jugada): reducir
// más cuanto más tarde aparece la jugada Y cuanto más profundo es el nodo (el
// coste de buscar de más crece exponencialmente con la profundidad). La forma
// ln·ln es el concepto público (CPW, "Late Move Reductions"). Los DOS
// coeficientes son propios, fijados por dos anclas y no afinados:
//   (a) d=3, jugada 4 -> R=1: exactamente lo que ya hacía Fructosita, para no
//       cambiar el comportamiento cerca de las hojas;
//   (b) d=12, jugada 20 -> R=3: una jugada tardía en un nodo profundo se busca
//       como mucho 3 plies más corta antes de verificarla.
// Resolviendo R = a + ln d · ln m / c con (a), (b): a ≈ 0,5, c ≈ 3,0.
// ---------------------------------------------------------------------------


/// TM-04: la caída de score que activa el modo pánico es `tune::TM_PANIC_DROP` (30 cp).

// EXP-0014 (SEARCH-D): singular extensions. Profundidad mínima, margen en cp
// por ply de profundidad (a la escala pawn=70: d=10 -> 20 cp, menos de un
// tercio de peón) y guarda de explosión (ply < 2·profundidad raíz).
// G19: SE_MIN_DEPTH y SE_MARGIN son ajustables (`tune::SE_MIN_DEPTH`, `tune::SE_MARGIN`; 8 y 2).

// EXP-0013: con `false` todas las jugadas se puntúan ANTES de buscar la de la
// TT (orden bit a bit igual al de EXP-0011: sirve para verificar el selector y
// el validador). Con `true` solo se generan si la jugada de la TT no corta.
const LAZY_GEN: bool = true;


/// G19: la tabla se calcula al empezar cada búsqueda con los parámetros
/// ajustables (`tune::LMR_BASE_X100`, `tune::LMR_DIV_X100`); por defecto
/// 0,5 y 3,0, exactamente como antes.
fn lmr_table() -> Box<[[i32; 64]; 64]> {
    let base = tune::get(tune::LMR_BASE_X100) as f64 / 100.0;
    let div = tune::get(tune::LMR_DIV_X100) as f64 / 100.0;
    let mut t = Box::new([[0i32; 64]; 64]);
    for (d, row) in t.iter_mut().enumerate().skip(1) {
        for (m, r) in row.iter_mut().enumerate().skip(1) {
            *r = (base + (d as f64).ln() * (m as f64).ln() / div) as i32;
        }
    }
    t
}

#[derive(Debug, Default)]
pub struct SearchStats {
    pub nodes: AtomicU64,
    pub qnodes: AtomicU64,
    pub tt_probes: AtomicU64,
    pub tt_hits: AtomicU64,
    pub tt_cutoffs: AtomicU64,
    pub beta_cutoffs: AtomicU64,
    pub beta_cutoffs_first_move: AtomicU64,
    pub null_move_attempts: AtomicU64,
    pub null_move_cutoffs: AtomicU64,
    pub lmr_attempts: AtomicU64,
    pub lmr_researches: AtomicU64,
    pub pvs_researches: AtomicU64,
    // Instrumentación de Late Move Pruning (investigación 04). Miden el
    // solapamiento del bloque de podas por-jugada; NO alimentan la firma de
    // bench. Ver research/04-late-move-pruning §8.
    pub lmp_prunes: AtomicU64,
    pub lmp_prunes_futility_silent: AtomicU64,
    pub lmp_prunes_beyond_lmr: AtomicU64,
    // Puerta 2 de la investigación 10: cuántas jugadas silenciosas cambian de
    // TIER (T0 completa / T1 reducida por LMR / T2 podada por LMP) al añadir
    // continuation history a la ordenación. Solo con MEASURE_TIER_CHANGES.
    pub ch_quiets_classified: AtomicU64,
    pub ch_tier_changes: AtomicU64,
    pub ch_nonzero_entries: AtomicU64,
    pub ch_quiets_scored: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStatsSnapshot {
    pub nodes: u64,
    pub qnodes: u64,
    pub tt_probes: u64,
    pub tt_hits: u64,
    pub tt_cutoffs: u64,
    pub beta_cutoffs: u64,
    pub beta_cutoffs_first_move: u64,
    pub null_move_attempts: u64,
    pub null_move_cutoffs: u64,
    pub lmr_attempts: u64,
    pub lmr_researches: u64,
    pub pvs_researches: u64,
    pub lmp_prunes: u64,
    pub lmp_prunes_futility_silent: u64,
    pub lmp_prunes_beyond_lmr: u64,
    pub ch_quiets_classified: u64,
    pub ch_tier_changes: u64,
    pub ch_nonzero_entries: u64,
    pub ch_quiets_scored: u64,
}

impl SearchStats {
    pub fn snapshot(&self) -> SearchStatsSnapshot {
        SearchStatsSnapshot {
            nodes: self.nodes.load(Ordering::Relaxed),
            qnodes: self.qnodes.load(Ordering::Relaxed),
            tt_probes: self.tt_probes.load(Ordering::Relaxed),
            tt_hits: self.tt_hits.load(Ordering::Relaxed),
            tt_cutoffs: self.tt_cutoffs.load(Ordering::Relaxed),
            beta_cutoffs: self.beta_cutoffs.load(Ordering::Relaxed),
            beta_cutoffs_first_move: self.beta_cutoffs_first_move.load(Ordering::Relaxed),
            null_move_attempts: self.null_move_attempts.load(Ordering::Relaxed),
            null_move_cutoffs: self.null_move_cutoffs.load(Ordering::Relaxed),
            lmr_attempts: self.lmr_attempts.load(Ordering::Relaxed),
            lmr_researches: self.lmr_researches.load(Ordering::Relaxed),
            pvs_researches: self.pvs_researches.load(Ordering::Relaxed),
            lmp_prunes: self.lmp_prunes.load(Ordering::Relaxed),
            lmp_prunes_futility_silent: self.lmp_prunes_futility_silent.load(Ordering::Relaxed),
            lmp_prunes_beyond_lmr: self.lmp_prunes_beyond_lmr.load(Ordering::Relaxed),
            ch_quiets_classified: self.ch_quiets_classified.load(Ordering::Relaxed),
            ch_tier_changes: self.ch_tier_changes.load(Ordering::Relaxed),
            ch_nonzero_entries: self.ch_nonzero_entries.load(Ordering::Relaxed),
            ch_quiets_scored: self.ch_quiets_scored.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy)]
pub struct SearchLimits {
    pub max_depth: i32,
    pub soft_deadline: Instant,
    pub hard_deadline: Instant,
    /// `go nodes N`: límite de nodos (aproximado a 2.048), o `None`.
    pub max_nodes: Option<u64>,
}

/// EXP-0013: contadores del hilo, sin atómicos en el camino caliente. Se
/// vuelcan a `SearchStats` cada 2.048 nodos y al terminar.
#[derive(Default)]
struct LocalStats {
    nodes: u64,
    qnodes: u64,
    tt_probes: u64,
    tt_hits: u64,
    tt_cutoffs: u64,
    beta_cutoffs: u64,
    beta_cutoffs_first_move: u64,
    null_move_attempts: u64,
    null_move_cutoffs: u64,
    lmr_attempts: u64,
    lmr_researches: u64,
    pvs_researches: u64,
    lmp_prunes: u64,
    lmp_prunes_futility_silent: u64,
    lmp_prunes_beyond_lmr: u64,
}

/// SPEED-B: caché de evaluación estática por hilo. 2^18 entradas de 8 bytes
/// (2 MB): 48 bits altos del hash Zobrist + eval en 16 bits. La eval es una
/// función pura de la posición, así que un acierto devuelve exactamente lo que
/// devolvería `evaluate` (salvo colisión de 48 bits, despreciable).
const EVAL_CACHE_BITS: u32 = 18;
const EVAL_KEY_MASK: u64 = !0xFFFF;

#[inline]
fn cached_eval(board: &Board, ctx: &mut SearchContext) -> i32 {
    let idx = (board.hash as usize) & ((1usize << EVAL_CACHE_BITS) - 1);
    let e = ctx.hist.eval_cache[idx];
    if e != 0 && (e & EVAL_KEY_MASK) == (board.hash & EVAL_KEY_MASK) {
        return (e & 0xFFFF) as u16 as i16 as i32;
    }
    let v = eval::evaluate(board);
    ctx.hist.eval_cache[idx] = (board.hash & EVAL_KEY_MASK) | (v as i16 as u16 as u64);
    v
}

// ---------------------------------------------------------------------------
// EXP-0033 (SEARCH-Q) — historial de corrección de la eval estática.
// La eval HCE tiene sesgos sistemáticos que dependen de la estructura de peones
// (p. ej. infravalora cierto tipo de centro bloqueado). Cada vez que un nodo
// termina con un resultado de búsqueda fiable, la diferencia búsqueda − eval se
// acumula (media móvil ponderada por profundidad) en una tabla indexada por
// (bando al turno, firma de la estructura de peones). La eval estática que usan
// las podas (RFP, null move, futility, razoring, ProbCut, improving) y el
// stand-pat de la quiescencia se corrigen con esa media. Concepto público
// ("static evaluation correction history"); tamaño, grano, pesos y tope propios.
// ---------------------------------------------------------------------------
const CORR_BITS: u32 = 14;
const CORR_GRAIN: i32 = 256;
const CORR_WEIGHT_SCALE: i32 = 256;
const CORR_MAX: i32 = CORR_GRAIN * 48; // tope ±48 cp

#[inline]
fn pawn_key(board: &Board) -> usize {
    let w = board.pieces[0][PieceType::Pawn.index()];
    let b = board.pieces[1][PieceType::Pawn.index()];
    let h = w.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F).rotate_left(29);
    (h >> (64 - CORR_BITS)) as usize
}

#[inline]
fn corrected_eval(board: &Board, raw: i32, ctx: &SearchContext) -> i32 {
    let c = ctx.hist.corr[board.side_to_move.index()][pawn_key(board)] / CORR_GRAIN;
    (raw + c).clamp(-MATE_SCORE + MAX_PLY as i32 + 1, MATE_SCORE - MAX_PLY as i32 - 1)
}

fn update_correction(board: &Board, ctx: &mut SearchContext, depth: i32, diff: i32) {
    let e = &mut ctx.hist.corr[board.side_to_move.index()][pawn_key(board)];
    let w = (depth + 1).min(16);
    let target = diff.clamp(-CORR_MAX / CORR_GRAIN, CORR_MAX / CORR_GRAIN) * CORR_GRAIN;
    *e = ((*e * (CORR_WEIGHT_SCALE - w) + target * w) / CORR_WEIGHT_SCALE).clamp(-CORR_MAX, CORR_MAX);
}
/// EXP-0032: historiales persistentes entre búsquedas. Antes cada `go` creaba
/// historial mariposa y continuation history a cero: lo aprendido en la jugada
/// anterior (qué silenciosas cortan en esta partida) se perdía. Ahora cada hilo
/// devuelve sus tablas a este depósito al terminar y las recupera en la
/// siguiente búsqueda, envejecidas a la mitad (la información vieja sigue
/// ordenando las primeras iteraciones, pero la nueva la domina enseguida).
/// `ucinewgame` vacía el depósito. Los killers no se conservan (son por ply).
struct Histories {
    /// PATCH G19: caché de eval (SPEED-B) persistente: la eval es función pura
    /// de la posición, así que las entradas de jugadas anteriores siguen valiendo.
    eval_cache: [u64; 1 << EVAL_CACHE_BITS],
    butterfly: [[[i32; 64]; 64]; 2],
    cont: [[[[i16; 64]; 6]; 64]; 6],
    /// EXP-0046: correction history (EXP-0033) también persistente, sin
    /// envejecer: el sesgo de la eval para una estructura no caduca por jugada.
    corr: [[i32; 1 << CORR_BITS]; 2],
}

static HISTORY_POOL: std::sync::Mutex<Vec<Option<Box<Histories>>>> = std::sync::Mutex::new(Vec::new());

/// Vacía los historiales guardados (nueva partida).
pub fn clear_histories() {
    if let Ok(mut pool) = HISTORY_POOL.lock() {
        pool.clear();
    }
}

fn take_histories(thread_id: usize) -> Box<Histories> {
    let saved = HISTORY_POOL.lock().ok().and_then(|mut p| p.get_mut(thread_id).and_then(|x| x.take()));
    match saved {
        Some(mut h) => {
            for c in h.butterfly.iter_mut() { for row in c.iter_mut() { for v in row.iter_mut() { *v /= 2; } } }
            for a in h.cont.iter_mut() { for b in a.iter_mut() { for c in b.iter_mut() { for v in c.iter_mut() { *v /= 2; } } } }
            h
        }
        None => {
            // Construcción directa en el heap (evita 2x 300 KB en la pila).
            // SAFETY: `Histories` son solo enteros; todo ceros es un valor válido.
            unsafe { Box::new_zeroed().assume_init() }
        }
    }
}

fn give_back_histories(thread_id: usize, h: Box<Histories>) {
    if let Ok(mut pool) = HISTORY_POOL.lock() {
        if pool.len() <= thread_id {
            pool.resize_with(thread_id + 1, || None);
        }
        pool[thread_id] = Some(h);
    }
}

struct SearchContext<'a> {
    /// G19: tabla LMR de esta búsqueda (parámetros ajustables).
    lmr: Box<[[i32; 64]; 64]>,
    stats: &'a SearchStats,
    ls: LocalStats,
    /// EXP-0014: jugada excluida en la búsqueda de verificación singular de
    /// cada ply (misma posición, todas las jugadas salvo esta).
    excluded: [Option<Move>; MAX_PLY],
    /// Profundidad de la iteración en curso (guarda de las extensiones).
    root_depth: i32,
    /// Nodos de este hilo (no se reinicia al volcar): marca el chequeo de tiempo.
    ticks: u64,
    seldepth: u8,
    stop: &'a AtomicBool,
    hard_deadline: Instant,
    max_nodes: Option<u64>,
    stopped: bool,
    is_main: bool,
    killers: [[Option<Move>; 2]; MAX_PLY],
    /// EXP-0032: historial mariposa + continuation history (persistentes).
    hist: Box<Histories>,
    /// Evaluación estática por ply. La heurística "improving" compara la eval
    /// del nodo actual con la de dos plies antes (mismo bando al turno) para
    /// saber si la posición "va a mejor". Es infraestructura reutilizable por
    /// futuras podas/reducciones (LMP, futility, LMR). Por hilo (vive en el
    /// contexto), sin problema de concurrencia bajo Lazy SMP.
    /// Ver research/02-improving-heuristic.
    eval_stack: [i32; MAX_PLY],
    /// Jugada realizada en cada ply: `(pieza, casilla destino)`. Infraestructura
    /// nueva de la investigación 10, reutilizable después por countermove,
    /// continuation history 2/4-ply y moduladores de LMR.
    move_stack: [(PieceType, u8); MAX_PLY],
    /// Continuation history 1-ply: [pieza_previa][destino_previo][pieza][destino].
    /// 576 KB → **en heap** (`Box`), no en la pila del hilo (los hilos se lanzan
    /// con pila por defecto ~2 MB). Por hilo, como el history mariposa.
    /// SUBITERACIÓN 10a: `i16` en vez de `i32` → 288 KB (mitad). El tope de
    /// saturación baja a 16.384 porque 1.000.000 no cabe en 16 bits: es una
    /// consecuencia FORZADA de la representación, no un reajuste de calibración.
    /// El factor `CONT_HISTORY_SCALE` permanece congelado en 20.
    tt: &'a TranspositionTable,
    /// Hashes de todas las posiciones desde el inicio de la partida (o del
    /// FEN inicial) hasta el nodo actual, inclusive. Crece/decrece con la
    /// recursión: se hace push antes de bajar a un hijo y pop al volver.
    game_history: Vec<u64>,
    /// S2-002: búferes reutilizables por ply (jugadas, puntuaciones, silenciosas
    /// probadas, PV del hijo y lista de la quiescencia). Se toman con
    /// `std::mem::take` y se devuelven al terminar el nodo: una llamada anidada
    /// al mismo ply (verificación singular) recibe un búfer vacío nuevo, así que
    /// el contenido nunca se comparte. Solo evita reservar memoria por nodo; el
    /// orden y el árbol son idénticos.
    move_pool: Vec<Vec<Move>>,
    score_pool: Vec<Vec<i64>>,
    quiet_pool: Vec<Vec<Move>>,
    pv_pool: Vec<Vec<Move>>,
    qs_pool: Vec<Vec<(Move, i32)>>,
    /// S3-002: "¿está en jaque el bando al turno?" ya calculado por el llamador
    /// para el nodo que se va a visitar. Se fija justo antes de la llamada y el
    /// nodo lo consume en su primera instrucción: nunca queda un valor viejo.
    check_hint: Option<bool>,
}

/// S2-002: saca el búfer del ply (vacío) de su depósito.
#[inline]
fn take_buf<T>(pool: &mut [Vec<T>], ply: usize) -> Vec<T> {
    let mut v = std::mem::take(&mut pool[ply]);
    v.clear();
    v
}

impl<'a> SearchContext<'a> {
    fn store_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != Some(mv) {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = Some(mv);
        }
    }

    fn bump_history(&mut self, color: Color, mv: Move, depth: i32) {
        let entry = &mut self.hist.butterfly[color.index()][mv.from as usize][mv.to as usize];
        *entry += depth * depth;
        if *entry > 1_000_000 {
            for c in self.hist.butterfly.iter_mut() {
                for row in c.iter_mut() {
                    for v in row.iter_mut() {
                        *v /= 2;
                    }
                }
            }
        }
    }

    /// SEARCH-B: castigo simétrico al bonus. Una silenciosa que se buscó ANTES
    /// de la que cortó y no cortó recibe −depth²: el historial pasa a medir
    /// "corta frente a no corta" y no solo "cuántas veces cortó", así que las
    /// jugadas que fallan de forma repetida bajan en la ordenación (y la LMR las
    /// reduce antes). Suelo en −1.000.000, simétrico al techo del bonus.
    fn malus_history(&mut self, color: Color, mv: Move, depth: i32) {
        let entry = &mut self.hist.butterfly[color.index()][mv.from as usize][mv.to as usize];
        *entry = (*entry - depth * depth).max(-1_000_000);
    }

    /// Incrementa el historial de continuación para (jugada previa → jugada
    /// actual). Misma regla y mismo tope que el history mariposa, para que ambos
    /// vivan en el mismo rango.
    fn bump_cont_history(&mut self, ply: usize, piece: PieceType, to: u8, depth: i32) {
        if ply == 0 {
            return; // sin jugada previa
        }
        let (pp, pt) = self.move_stack[ply - 1];
        let entry =
            &mut self.hist.cont[pp.index()][pt as usize][piece.index()][to as usize];
        *entry = entry.saturating_add((depth * depth) as i16);
        if *entry > CONT_HISTORY_MAX {
            for a in self.hist.cont.iter_mut() {
                for b in a.iter_mut() {
                    for c in b.iter_mut() {
                        for v in c.iter_mut() {
                            *v /= 2;
                        }
                    }
                }
            }
        }
    }

    /// Valor de continuation history para una jugada silenciosa en este ply.
    fn cont_history_score(&self, ply: usize, piece: PieceType, to: u8) -> i32 {
        if ply == 0 || !ENABLE_CONT_HISTORY {
            return 0;
        }
        let (pp, pt) = self.move_stack[ply - 1];
        self.hist.cont[pp.index()][pt as usize][piece.index()][to as usize] as i32
    }

    fn flush_stats(&mut self) {
        let l = std::mem::take(&mut self.ls);
        let st = self.stats;
        st.nodes.fetch_add(l.nodes, Ordering::Relaxed);
        st.qnodes.fetch_add(l.qnodes, Ordering::Relaxed);
        st.tt_probes.fetch_add(l.tt_probes, Ordering::Relaxed);
        st.tt_hits.fetch_add(l.tt_hits, Ordering::Relaxed);
        st.tt_cutoffs.fetch_add(l.tt_cutoffs, Ordering::Relaxed);
        st.beta_cutoffs.fetch_add(l.beta_cutoffs, Ordering::Relaxed);
        st.beta_cutoffs_first_move.fetch_add(l.beta_cutoffs_first_move, Ordering::Relaxed);
        st.null_move_attempts.fetch_add(l.null_move_attempts, Ordering::Relaxed);
        st.null_move_cutoffs.fetch_add(l.null_move_cutoffs, Ordering::Relaxed);
        st.lmr_attempts.fetch_add(l.lmr_attempts, Ordering::Relaxed);
        st.lmr_researches.fetch_add(l.lmr_researches, Ordering::Relaxed);
        st.pvs_researches.fetch_add(l.pvs_researches, Ordering::Relaxed);
        st.lmp_prunes.fetch_add(l.lmp_prunes, Ordering::Relaxed);
        st.lmp_prunes_futility_silent.fetch_add(l.lmp_prunes_futility_silent, Ordering::Relaxed);
        st.lmp_prunes_beyond_lmr.fetch_add(l.lmp_prunes_beyond_lmr, Ordering::Relaxed);
    }

    fn check_time(&mut self) {
        if self.ticks.is_multiple_of(2048) {
            self.flush_stats();
            let over_nodes = self
                .max_nodes
                .is_some_and(|n| self.stats.nodes.load(Ordering::Relaxed) >= n);
            if over_nodes || self.stop.load(Ordering::Relaxed) || Instant::now() >= self.hard_deadline {
                self.stopped = true;
            }
        }
    }
}

fn has_non_pawn_material(board: &Board, color: Color) -> bool {
    let idx = color.index();
    board.pieces[idx][PieceType::Knight.index()] != 0
        || board.pieces[idx][PieceType::Bishop.index()] != 0
        || board.pieces[idx][PieceType::Rook.index()] != 0
        || board.pieces[idx][PieceType::Queen.index()] != 0
}

/// Ajusta un score de mate para que sea relativo al *nodo raíz* antes de
/// guardarlo en la TT (así "mate en 3" sigue significando lo mismo sin
/// importar desde qué profundidad de la búsqueda se reutilice la entrada).
fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_SCORE - MAX_PLY as i32 {
        score + ply as i32
    } else if score <= -MATE_SCORE + MAX_PLY as i32 {
        score - ply as i32
    } else {
        score
    }
}

/// Inverso de `score_to_tt`: convierte un score guardado (relativo a la
/// raíz) de vuelta a relativo al nodo actual.
fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_SCORE - MAX_PLY as i32 {
        score - ply as i32
    } else if score <= -MATE_SCORE + MAX_PLY as i32 {
        score + ply as i32
    } else {
        score
    }
}

fn is_repetition_or_fifty(board: &Board, ctx: &SearchContext) -> bool {
    if board.halfmove_clock >= 100 {
        // A legal checkmate ends the game before the defender can claim a draw.
        return !board.in_check(board.side_to_move) || !generate_legal_moves(board).is_empty();
    }
    let hist = &ctx.game_history;
    let len = hist.len();
    if len < 2 {
        return false;
    }
    let current = hist[len - 1];
    let limit = (board.halfmove_clock as usize).min(len - 1);
    let mut i = 2;
    while i <= limit {
        if hist[len - 1 - i] == current {
            return true;
        }
        i += 2;
    }
    false
}

fn move_score(
    mv: &Move,
    board: &Board,
    tt_move: Option<Move>,
    ply: usize,
    ctx: &SearchContext,
) -> i32 {
    if tt_move == Some(*mv) {
        return 2_000_000;
    }
    if let Some(promo) = mv.promotion() {
        let base = if promo == PieceType::Queen {
            1_800_000
        } else {
            100_000
        };
        return base + if mv.is_capture() { 10_000 } else { 0 };
    }
    if mv.is_capture() {
        // SEE es más preciso que MVV-LVA: considera la secuencia completa
        // de recapturas, no solo "víctima menos atacante". Las capturas
        // rentables (SEE >= 0) se buscan primero que cualquier jugada
        // silenciosa; las que pierden material se ordenan después de los
        // killers/historial, pero antes de las jugadas silenciosas neutras
        // (siguen mereciendo consideración, solo con menos prioridad).
        let see_value = crate::see::see(board, mv);
        if see_value >= 0 {
            1_000_000 + see_value
        } else {
            see_value
        }
    } else if ctx.killers[ply][0] == Some(*mv) {
        90_000
    } else if ctx.killers[ply][1] == Some(*mv) {
        80_000
    } else {
        // Jugada silenciosa: history mariposa COMBINADO con continuation history
        // (investigación 10). La división entre 2 es una DECISIÓN DE INGENIERÍA
        // PROVISIONAL para conservar el rango [.., 1.000.000] y no mover ningún
        // tier de ordenación; NO es parte de la hipótesis y puede reajustarse.
        let h = ctx.hist.butterfly[board.side_to_move.index()][mv.from as usize]
            [mv.to as usize];
        if !ENABLE_CONT_HISTORY {
            return h;
        }
        let piece = match board.mailbox[mv.from as usize] {
            Some(p) => p.kind,
            None => return h,
        };
        // Normalizar cont_history a la escala del history y promediar. El cap
        // mantiene el rango en [.., 1.000.000] -> ningun tier de ordenacion se
        // mueve (decision de ingenieria, no hipotesis; ver investigacion 10 §5.3).
        let ch = ctx.cont_history_score(ply, piece, mv.to);
        let ch_scaled = (ch * CONT_HISTORY_SCALE).min(1_000_000);
        (h + ch_scaled) / 2
    }
}

/// S2-009: "¿da jaque?" calculado una vez por jugada y solo si hace falta.
#[inline]
fn gives_check_cached(
    cache: &mut Option<bool>,
    info: &mut Option<crate::board::CheckInfo>,
    board: &Board,
    mv: Move,
) -> bool {
    // S3-002: las casillas de jaque del nodo se calculan una vez (perezoso) y
    // cada jugada se resuelve con dos AND; solo los casos raros usan el camino exacto.
    *cache.get_or_insert_with(|| {
        let ci = info.get_or_insert_with(|| board.check_info());
        board.gives_check_with(mv, ci)
    })
}

/// S2-003: umbral de historial "bueno" para reducir menos (medido, ver LMR).
const LMR_HIST_GOOD: i32 = 1_000;

/// S2-003: la misma puntuación que `move_score` da a una silenciosa que no es
/// killer ni jugada de la TT (historial mariposa combinado con continuation
/// history). La lista ya consumió su puntuación en la selección perezosa.
fn quiet_order_score(mv: &Move, board: &Board, ply: usize, ctx: &SearchContext) -> i32 {
    let h = ctx.hist.butterfly[board.side_to_move.index()][mv.from as usize][mv.to as usize];
    if !ENABLE_CONT_HISTORY {
        return h;
    }
    let piece = match board.mailbox[mv.from as usize] {
        Some(p) => p.kind,
        None => return h,
    };
    let ch = ctx.cont_history_score(ply, piece, mv.to);
    (h + (ch * CONT_HISTORY_SCALE).min(1_000_000)) / 2
}

/// EXP-0013: quita de la lista la jugada que ya se buscó primero (la de la
/// TT) y puntúa el resto con `move_score`.
fn score_moves(
    moves: &mut Vec<Move>,
    exclude: Option<Move>,
    board: &Board,
    tt_move: Option<Move>,
    ply: usize,
    ctx: &SearchContext,
    out: &mut Vec<i64>,
) {
    if let Some(x) = exclude {
        moves.retain(|&m| m != x);
    }
    out.clear();
    // S3-002: clave = puntuación * 256 + (255 − índice). El máximo de la clave es
    // la mayor puntuación y, en empate, el menor índice: el mismo orden que la
    // selección anterior (máximo, primero en caso de empate). Hay <= 218 jugadas.
    out.extend(
        moves
            .iter()
            .enumerate()
            .map(|(j, m)| (move_score(m, board, tt_move, ply, ctx) as i64) * 256 + (255 - j as i64)),
    );
}

/// EXP-0013: selección perezosa (máximo, el primero en caso de empate).
/// S3-002: híbrida. Las primeras jugadas se eligen recorriendo las claves (en un
/// nodo de corte bastan una o dos); a partir de la cuarta, el resto se ordena
/// UNA vez de mayor a menor y se consume en secuencia. Con claves distintas
/// (el índice va dentro) el orden es exactamente el de la selección repetida.
/// `i64::MIN` marca una clave ya usada y nunca es una clave real.
#[derive(Default)]
struct Picker {
    picks: u32,
    sorted: bool,
    pos: usize,
}

const PICK_SCANS: u32 = 3;

#[inline]
fn pick_next(keys: &mut [i64], st: &mut Picker) -> Option<usize> {
    if !st.sorted {
        if st.picks < PICK_SCANS {
            st.picks += 1;
            let mut best = i64::MIN;
            let mut at = 0usize;
            for (j, &k) in keys.iter().enumerate() {
                if k > best {
                    best = k;
                    at = j;
                }
            }
            if best == i64::MIN {
                return None;
            }
            keys[at] = i64::MIN;
            return Some(255 - (best & 255) as usize);
        }
        keys.sort_unstable_by(|a, b| b.cmp(a));
        st.sorted = true;
        st.pos = 0;
    }
    match keys.get(st.pos) {
        Some(&k) if k != i64::MIN => {
            st.pos += 1;
            Some(255 - (k & 255) as usize)
        }
        _ => None,
    }
}

/// Filtra y ordena las capturas candidatas de quiescence usando SEE: las
/// capturas que pierden material (SEE < 0) se descartan directamente (poda
/// estándar de quiescence), y las que quedan se ordenan de más a menos
/// rentables. SEE se calcula una sola vez por movimiento y se reutiliza
/// tanto para filtrar como para ordenar.
/// Devuelve `(jugada, SEE)`. El SEE ya se calculaba aquí y se descartaba; ahora se
/// propaga para que el delta pruning lo reutilice SIN recalcularlo. El orden y el
/// conjunto de jugadas devueltas son EXACTAMENTE los mismos que en v6.
/// En jaque el SEE no se calcula (se devuelven todas las evasiones) y el
/// acompañante es 0, valor que nunca se usa porque el delta pruning excluye jaque.
fn filter_and_order_quiescence_moves(moves: &[Move], board: &Board, in_check: bool, scored: &mut Vec<(Move, i32)>) {
    scored.clear();
    if in_check {
        // En jaque hay que considerar todas las evasiones legales, no solo
        // capturas: no podemos permitirnos descartar la única jugada legal
        // solo porque su SEE sea negativo.
        scored.extend(moves.iter().map(|&m| (m, 0)));
        return;
    }
    scored.extend(moves
        .iter()
        .filter_map(|&m| {
            if m.promotion() == Some(PieceType::Queen) {
                return Some((m, i32::MAX));
            }
            if !m.is_capture() {
                return None;
            }
            let s = crate::see::see(board, &m);
            if s >= 0 {
                Some((m, s))
            } else {
                None
            }
        }));
    scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
}

fn quiescence(
    board: &Board,
    mut alpha: i32,
    beta: i32,
    ply: usize,
    ctx: &mut SearchContext,
) -> i32 {
    let check_hint = ctx.check_hint.take();
    ctx.check_time();
    if ctx.stopped {
        return 0;
    }
    ctx.ls.qnodes += 1;
    { ctx.ls.nodes += 1; ctx.ticks += 1 };
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    // SEARCH-B: consulta de la TT en quiescencia. Cualquier entrada guardada
    // por la búsqueda principal (profundidad >= 0 >= la de quiescencia) con una
    // cota utilizable resuelve el nodo sin generar ni ordenar capturas. La
    // quiescencia no escribe en la TT (no se añade presión sobre la tabla).
    if let Some(entry) = ctx.tt.probe(board.hash) {
        let score = score_from_tt(entry.score, ply);
        let usable = match entry.flag {
            TTFlag::Exact => true,
            TTFlag::LowerBound => score >= beta,
            TTFlag::UpperBound => score <= alpha,
        };
        if usable {
            return score;
        }
    }

    let in_check = check_hint.unwrap_or_else(|| board.in_check(board.side_to_move));
    let stand_pat = if !in_check || ply >= MAX_PLY {
        let raw = cached_eval(board, ctx);
        corrected_eval(board, raw, ctx)
    } else {
        0
    };

    if !in_check {
        if stand_pat >= beta {
            return stand_pat;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
    }
    if ply >= MAX_PLY {
        return stand_pat;
    }

    // PROPUESTA 33: sin jaque, la quiescencia solo necesita capturas y
    // promociones. Se genera exactamente ese subconjunto, en el mismo orden, y
    // se pasa por LA MISMA funcion de filtrado y ordenacion. En jaque el camino
    // queda intacto: hacen falta todas las evasiones.
    // S2-002: misma generación y mismo filtro/orden, sobre búferes del ply.
    let mut gen = take_buf(&mut ctx.move_pool, ply);
    let mut moves = take_buf(&mut ctx.qs_pool, ply);
    if in_check {
        generate_legal_moves_into(board, &mut gen);
        if gen.is_empty() {
            ctx.move_pool[ply] = gen;
            ctx.qs_pool[ply] = moves;
            return -MATE_SCORE + ply as i32;
        }
    } else {
        generate_legal_captures_into(board, &mut gen);
    }
    filter_and_order_quiescence_moves(&gen, board, in_check, &mut moves);
    ctx.move_pool[ply] = gen;

    // -------------------------------------------------------------------
    // DELTA PRUNING (investigación 16), aislado.
    // En quiescencia y SIN jaque, una captura que ni en el mejor caso puede
    // alcanzar alpha no se busca. La ganancia se estima con el SEE que
    // `filter_and_order_quiescence_moves` YA calculó — coste cero.
    //
    // Guardas: nunca en jaque (se buscan todas las evasiones), nunca sobre
    // promociones (el filtro las prioriza aparte), y nunca con alpha en rango
    // de mate: el margen 09 midió que ~0,1% de las capturas podables conducen
    // a mate forzado. Ver analysis/margin-study-09-delta-pruning.md §4.
    //
    // NADA MÁS cambia: SEE, ordenación, stand-pat, ventana, profundidad de
    // quiescencia y generación de capturas quedan intactos.
    // -------------------------------------------------------------------
    let delta_margin: i32 = tune::get(tune::DELTA_MARGIN);

    let mut best = if in_check { -INFINITY } else { alpha };
    for k in 0..moves.len() {
        let (mv, see_v) = moves[k];
        if !in_check
            && mv.promotion().is_none()
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && stand_pat.saturating_add(see_v).saturating_add(delta_margin) <= alpha
        {
            continue;
        }
        let next = board.make_move(mv);
        // S3-002: adelantar la entrada de la TT que el hijo consultará.
        ctx.tt.prefetch(next.hash);
        let score = -quiescence(&next, -beta, -alpha, ply + 1, ctx);
        if ctx.stopped {
            return 0;
        }
        if score > best {
            best = score;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
    }
    ctx.qs_pool[ply] = moves;
    if in_check {
        best.max(alpha)
    } else {
        alpha
    }
}

#[allow(clippy::too_many_arguments)]
fn negamax(
    board: &Board,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    ply: usize,
    pv: &mut Vec<Move>,
    ctx: &mut SearchContext,
) -> i32 {
    let check_hint = ctx.check_hint.take();
    pv.clear();

    ctx.check_time();
    if ctx.stopped {
        return 0;
    }

    if ply > 0 && is_repetition_or_fifty(board, ctx) {
        return 0;
    }
    if ply >= MAX_PLY {
        return cached_eval(board, ctx);
    }

    let in_check = check_hint.unwrap_or_else(|| board.in_check(board.side_to_move));
    let mut depth = depth;
    if in_check {
        depth += 1;
    }

    if depth <= 0 {
        ctx.check_hint = Some(in_check);
        return quiescence(board, alpha, beta, ply, ctx);
    }

    { ctx.ls.nodes += 1; ctx.ticks += 1 };
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    // EXP-0014: en la búsqueda de verificación singular la entrada de la TT
    // describe el nodo COMPLETO (con la jugada excluida): no puede cortar.
    let excluded = ctx.excluded[ply];
    ctx.ls.tt_probes += 1;
    let tt_probe = ctx.tt.probe(board.hash);
    let mut tt_move = None;
    if let Some(entry) = &tt_probe {
        ctx.ls.tt_hits += 1;
        tt_move = entry.best_move;
        if entry.depth as i32 >= depth && ply > 0 && excluded.is_none() {
            let score = score_from_tt(entry.score, ply);
            let usable = match entry.flag {
                TTFlag::Exact => true,
                TTFlag::LowerBound => score >= beta,
                TTFlag::UpperBound => score <= alpha,
            };
            if usable {
                ctx.ls.tt_cutoffs += 1;
                return score;
            }
        }
    }

    // ---------------------------------------------------------------------
    // IIR — Internal Iterative Reductions (investigación 15).
    // Si no hay jugada de la TT, este nodo está mal ordenado: el estudio de
    // margen 07 midió −9,1 pp de cortes al primer movimiento a profundidad >= 4.
    // Se busca 1 ply más superficial; la entrada que deje en la TT hará que la
    // siguiente visita venga ordenada.
    //
    // AISLAMIENTO DELIBERADO: `iir_depth` se usa SOLO en las llamadas recursivas
    // del bucle de movimientos. RFP, futility, LMP, el null-move y la condición
    // de disparo de LMR siguen usando `depth` SIN REDUCIR. Así, si el experimento
    // gana Elo, es atribuible a IIR y no a "más poda". Ver margin-study-07-iir §7.
    //
    // `!in_check` es obligatorio: en jaque `depth` ya subió por la extensión, y
    // restarle 1 la anularía exactamente (18% de los candidatos). Ver §6.
    const IIR_MIN_DEPTH: i32 = 4;
    let iir_depth = if tt_move.is_none() && !in_check && depth >= IIR_MIN_DEPTH {
        depth - 1
    } else {
        depth
    };

    // Evaluación estática del nodo. Hasta ahora solo se calculaba en la
    // quiescencia; RFP, el gating del null-move (a futuro) y la heurística
    // "improving" la necesitan aquí. Ver research/01 y research/02.
    // SEARCH-B: la eval estática se afina con la cota de la TT cuando esta dice
    // más que la heurística: una cota inferior por encima de la eval, una
    // superior por debajo, o un valor exacto, son información de búsqueda real
    // sobre ESTE nodo. Así RFP, null move y futility deciden con mejor
    // estimación. Nunca con scores de mate.
    let uncorrected = cached_eval(board, ctx);
    let raw_eval = if in_check { uncorrected } else { corrected_eval(board, uncorrected, ctx) };
    let static_eval = match &tt_probe {
        Some(e) if e.score.abs() < MATE_SCORE - MAX_PLY as i32 => {
            let tts = e.score;
            match e.flag {
                TTFlag::Exact => tts,
                TTFlag::LowerBound if tts > raw_eval => tts,
                TTFlag::UpperBound if tts < raw_eval => tts,
                _ => raw_eval,
            }
        }
        _ => raw_eval,
    };
    ctx.eval_stack[ply] = static_eval;

    // Heurística "improving": ¿la eval estática del bando al turno es mejor
    // ahora que en su turno anterior (dos plies atrás, mismo bando)? Si la
    // posición "va a peor", es poco probable que esconda una salvación súbita,
    // así que se puede podar algo más; si "va a mejor", conviene ser prudente.
    // Es un modulador barato (una comparación de enteros), no una poda nueva.
    // Convención del hueco (ply<2): false — lo desconocido se trata como "no
    // mejora" (prudente). Ver research/02-improving-heuristic.
    let improving = !in_check && ply >= 2 && static_eval > ctx.eval_stack[ply - 2];

    // Reverse Futility Pruning (static null move): en un nodo non-PV, sin
    // jaque y a profundidad baja, si la eval estática ya supera beta por un
    // margen que crece con la profundidad, asumimos que el nodo fallará alto y
    // cortamos sin buscar. Guardas: non-PV vía ventana nula (beta-alpha==1),
    // no en jaque, profundidad <= RFP_MAX_DEPTH, y beta fuera de rango de mate
    // (no arriesgar cerca de mates forzados). El margen se MODULA con
    // "improving": cuando la posición NO mejora se exige menos colchón (se poda
    // algo más). Constantes de partida, a afinar por SPRT — NO copiadas.
    const RFP_MAX_DEPTH: i32 = 6;
    let RFP_MARGIN: i32 = tune::get(tune::RFP_MARGIN); // centipeones por unidad de profundidad
    let RFP_IMPROVING_BONUS: i32 = tune::get(tune::RFP_IMPROVING_BONUS); // colchón extra cuando la posición mejora
    let rfp_margin = RFP_MARGIN - if improving { 0 } else { RFP_IMPROVING_BONUS };
    if beta - alpha == 1
        && !in_check
        && excluded.is_none()
        && depth <= RFP_MAX_DEPTH
        && beta < MATE_SCORE - MAX_PLY as i32
        && static_eval - rfp_margin * depth >= beta
    {
        return static_eval;
    }

    // Null-move pruning: si "pasar el turno" sigue dando una posición tan
    // buena que supera beta, es muy probable que la posición real también lo
    // haga, así que podamos esta rama. Se evita en jaque, en profundidades
    // bajas, y sin material mayor propio (riesgo de zugzwang).
    //
    // EXP-0005 (SEARCH-A): tres cambios sobre el null move original.
    //  1. Solo si la eval estática ya alcanza beta: si ni siquiera "estando
    //     aquí" llegamos a beta, pasar el turno casi nunca corta y el intento es
    //     trabajo perdido.
    //  2. Solo en nodos non-PV (ventana nula): en la PV queremos el valor
    //     exacto, no un corte especulativo.
    //  3. Reducción adaptativa R = 3 + depth/4 + min((eval-beta)/200, 2): hasta
    //     depth 7 es la R=3 de siempre; más profundo, y cuanto más sobra sobre
    //     beta, más barata es la verificación. 200 cp ≈ dos peones de colchón
    //     por ply extra; tope 2 para no degenerar en quiescencia.
    // Un score de mate devuelto por la búsqueda nula no es fiable (la posición
    // tras pasar es ilegal en ajedrez real): se devuelve beta en ese caso.
    let is_pv = beta - alpha > 1;
    if !is_pv
        && !in_check
        && excluded.is_none()
        && depth >= 3
        && ply > 0
        && static_eval >= beta
        && beta.abs() < MATE_SCORE - MAX_PLY as i32
        && has_non_pawn_material(board, board.side_to_move)
    {
        ctx.ls.null_move_attempts += 1;
        let null_r = 3 + depth / 4 + ((static_eval - beta) / tune::get(tune::NULL_DIV)).min(2);
        let null_board = board.make_null_move();
        ctx.tt.prefetch(null_board.hash);
        ctx.game_history.push(null_board.hash);
        let mut child_pv = take_buf(&mut ctx.pv_pool, ply + 1);
        let score = -negamax(
            &null_board,
            depth - 1 - null_r,
            -beta,
            -beta + 1,
            ply + 1,
            &mut child_pv,
            ctx,
        );
        ctx.pv_pool[ply + 1] = child_pv;
        ctx.game_history.pop();
        if ctx.stopped {
            return 0;
        }
        if score >= beta {
            ctx.ls.null_move_cutoffs += 1;
            return if score >= MATE_SCORE - MAX_PLY as i32 { beta } else { score };
        }
    }

    // SEARCH-E — RAZORING. En non-PV, sin jaque y a profundidad <= 2, si la
    // eval estática está tan por debajo de alpha que ni una ganancia típica de
    // un par de plies la acerca, se verifica con quiescencia (capturas): si
    // tampoco supera alpha, el nodo falla bajo sin generar silenciosas.
    // Margen propio: 250 cp por ply (≈ 2,7 peones a la escala pawn=93).
    let RAZOR_MARGIN: i32 = tune::get(tune::RAZOR_MARGIN);
    if !is_pv
        && !in_check
        && excluded.is_none()
        && depth <= 2
        && alpha.abs() < MATE_SCORE - MAX_PLY as i32
        && static_eval + RAZOR_MARGIN * depth < alpha
    {
        ctx.check_hint = Some(false);
        let q = quiescence(board, alpha, alpha + 1, ply, ctx);
        if ctx.stopped {
            return 0;
        }
        if q <= alpha {
            return q;
        }
    }

    // SEARCH-E — PROBCUT. En non-PV, a profundidad >= 5 y con beta lejos del
    // mate, una captura (SEE suficiente) que en una búsqueda reducida
    // (depth − 4) supera beta + 150 cp casi seguro supera beta a profundidad
    // completa: el nodo se corta. Primero se filtra con quiescencia (barata).
    // Concepto público (Buro, "ProbCut"); margen y reducción propios.
    let PROBCUT_MARGIN: i32 = tune::get(tune::PROBCUT_MARGIN);
    const PROBCUT_MIN_DEPTH: i32 = 5;
    if !is_pv
        && !in_check
        && excluded.is_none()
        && depth >= PROBCUT_MIN_DEPTH
        && beta.abs() < MATE_SCORE - MAX_PLY as i32
    {
        let pc_beta = beta + PROBCUT_MARGIN;
        let mut caps = take_buf(&mut ctx.move_pool, ply);
        generate_legal_captures_into(board, &mut caps);
        let mut child_pv = take_buf(&mut ctx.pv_pool, ply + 1);
        for k in 0..caps.len() {
            let mv = caps[k];
            if mv.promotion().is_some() && mv.promotion() != Some(PieceType::Queen) {
                continue;
            }
            if static_eval + crate::see::see(board, &mv) < pc_beta - 100 {
                continue;
            }
            let next = board.make_move(mv);
            ctx.tt.prefetch(next.hash);
            ctx.game_history.push(next.hash);
            let mut v = -quiescence(&next, -pc_beta, -pc_beta + 1, ply + 1, ctx);
            if !ctx.stopped && v >= pc_beta {
                v = -negamax(&next, depth - 4, -pc_beta, -pc_beta + 1, ply + 1, &mut child_pv, ctx);
            }
            ctx.game_history.pop();
            if ctx.stopped {
                return 0;
            }
            if v >= pc_beta {
                ctx.tt.store(board.hash, depth - 3, score_to_tt(v, ply), TTFlag::LowerBound, Some(mv));
                ctx.move_pool[ply] = caps;
                ctx.pv_pool[ply + 1] = child_pv;
                return v;
            }
        }
        ctx.move_pool[ply] = caps;
        ctx.pv_pool[ply + 1] = child_pv;
    }

    // EXP-0013 (SPEED-A): la jugada de la TT se busca ANTES de generar y
    // ordenar el resto. En la mayoría de nodos de corte es la que corta, así
    // que la generación, el SEE de cada captura y la ordenación se ahorran.
    // Se valida (pseudo-legal + no deja el rey en jaque) porque una colisión de
    // hash puede traer una jugada de otra posición. El resto se elige por
    // selección perezosa (máximo con desempate por orden de generación): el
    // mismo orden que la ordenación estable anterior, sin ordenar la lista.
    let us = board.side_to_move;
    let tt_first = if excluded.is_some() {
        None
    } else {
        tt_move.filter(|&m| {
            crate::movegen::is_pseudo_legal(board, m) && !board.make_move(m).in_check(us)
        })
    };
    let mut moves: Vec<Move> = Vec::new();
    let mut scores: Vec<i64> = Vec::new();
    let mut generated = false;
    if !LAZY_GEN || tt_first.is_none() {
        moves = take_buf(&mut ctx.move_pool, ply);
        generate_legal_moves_into(board, &mut moves);
        if moves.is_empty() {
            ctx.move_pool[ply] = moves;
            return if in_check {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }
        scores = take_buf(&mut ctx.score_pool, ply);
        score_moves(&mut moves, excluded.or(tt_first), board, tt_move, ply, ctx, &mut scores);
        generated = true;
    }

    // -------------------------------------------------------------------
    // EXP-0014 (SEARCH-D): SINGULAR EXTENSIONS. Si la jugada de la TT viene
    // de una búsqueda casi tan profunda con cota inferior (o exacta), se
    // comprueba si es la ÚNICA buena: se busca el resto de jugadas a media
    // profundidad con ventana nula en s_beta = score_tt − margen. Si todas
    // fallan por debajo, la jugada es "singular" (la posición depende de ella)
    // y se extiende 1 ply. Si el resto ya alcanza s_beta >= beta, hay varias
    // jugadas que cortan (multi-cut) y el nodo se resuelve con s_beta.
    // Concepto público (CPW "Singular Extensions"); umbrales propios.
    // -------------------------------------------------------------------
    let mut singular_ext = 0;
    if let (Some(ttm), Some(entry)) = (tt_first, &tt_probe) {
        if ply > 0
            && depth >= tune::get(tune::SE_MIN_DEPTH)
            && entry.depth as i32 >= depth - 3
            && matches!(entry.flag, TTFlag::LowerBound | TTFlag::Exact)
            && entry.score.abs() < MATE_SCORE - MAX_PLY as i32
            && (ply as i32) < 2 * ctx.root_depth
        {
            let s_beta = entry.score - tune::get(tune::SE_MARGIN) * depth;
            let s_depth = (depth - 1) / 2;
            ctx.excluded[ply] = Some(ttm);
            let mut tmp_pv = Vec::new();
            let v = negamax(board, s_depth, s_beta - 1, s_beta, ply, &mut tmp_pv, ctx);
            ctx.excluded[ply] = None;
            if ctx.stopped {
                return 0;
            }
            if v < s_beta {
                singular_ext = 1;
            } else if s_beta >= beta {
                return s_beta;
            }
        }
    }

    let mut best_score = -INFINITY;
    let mut best_move: Option<Move> = None;
    let alpha_orig = alpha;

    // Futility pruning (frontier / extended): poda por el lado de ALPHA, el
    // complemento simétrico de RFP (que poda por beta). En un nodo non-PV, sin
    // jaque y cerca de las hojas, una jugada silenciosa que no puede alcanzar
    // alpha ni con el mejor resultado plausible (eval estática + margen) se
    // salta sin buscarla. El margen crece con la profundidad y se modula con
    // "improving" (menos colchón si la posición no mejora → podar algo más).
    // Variante conservadora (decisión conjunta): NO se poda una jugada que da
    // JAQUE (requiere `make` + comprobar `in_check` del hijo). La primera jugada
    // nunca se poda (siempre se busca al menos una). Guarda anti-mate: no podar
    // si alpha está en rango de mate. Constantes de partida, a afinar por SPRT —
    // NO copiadas. Ver research/03-futility-pruning.
    const FUTILITY_MAX_DEPTH: i32 = 4;
    let FUTILITY_BASE: i32 = tune::get(tune::FUTILITY_BASE); // cp
    let FUTILITY_STEP: i32 = tune::get(tune::FUTILITY_STEP); // cp por unidad de profundidad
    let FUTILITY_IMPROVING_BONUS: i32 = tune::get(tune::FUTILITY_IMPROVING_BONUS); // colchón extra cuando mejora

    // Late Move Pruning (move-count): poda por CONTEO de jugadas, no por eval.
    // Cerca de las hojas, una vez probadas suficientes silenciosas, las
    // restantes —peor ordenadas— se saltan. Cubre el caso que futility no ve
    // (posición no hundida, muchas silenciosas). El umbral crece con la
    // profundidad y se modula con "improving". Detalle de Fructosita: las
    // capturas perdedoras (SEE<0) quedan AL FINAL de la lista, así que se usa
    // `continue` solo-silenciosas, NUNCA `break`. Constantes de partida a afinar
    // por SPRT — NO copiadas. Ver research/04-late-move-pruning §8.
    const LMP_MAX_DEPTH: i32 = 4;
    let LMP_BASE: i32 = tune::get(tune::LMP_BASE);
    const LMP_C: i32 = 1;
    const LMP_IMPROVING_BONUS: i32 = 1; // en unidades de conteo
    let lmp_count =
        LMP_BASE + LMP_C * depth * depth - if improving { 0 } else { LMP_IMPROVING_BONUS };

    // Nº de jugadas silenciosas ya buscadas en este nodo (para el umbral LMP).
    let mut quiets_searched: i32 = 0;
    // SEARCH-B: silenciosas realmente buscadas en este nodo (para el malus).
    let mut quiets_tried: Vec<Move> = take_buf(&mut ctx.quiet_pool, ply);
    let mut child_pv = take_buf(&mut ctx.pv_pool, ply + 1);

    let mut next_index = 0usize;
    let mut check_info: Option<crate::board::CheckInfo> = None;
    // S3-002: selección híbrida (ver `pick_next`).
    let mut picker = Picker::default();
    loop {
        let mv = if next_index == 0 && tt_first.is_some() {
            tt_first.unwrap()
        } else {
            if !generated {
                moves = take_buf(&mut ctx.move_pool, ply);
                generate_legal_moves_into(board, &mut moves);
                scores = take_buf(&mut ctx.score_pool, ply);
                score_moves(&mut moves, excluded.or(tt_first), board, tt_move, ply, ctx, &mut scores);
                generated = true;
            }
            match pick_next(&mut scores, &mut picker) {
                Some(j) => moves[j],
                None => break,
            }
        };
        let i = next_index;
        next_index += 1;
        // S2-009: la jugada solo se ejecuta si sobrevive a las podas; "da jaque"
        // se calcula sin copiar el tablero y una sola vez (perezoso).
        let mut gc: Option<bool> = None;
        let is_quiet = !mv.is_capture() && mv.promotion().is_none();

        // SEARCH-B: poda SEE de capturas perdedoras cerca de las hojas. En
        // non-PV, sin jaque, depth <= 4, una captura que pierde más de un peón
        // (100 en unidades SEE) POR ply restante no se busca: con tan pocos
        // plies no hay tiempo de recuperar ese material. Nunca la primera
        // jugada, nunca promociones, nunca si da jaque ni con alpha de mate.
        const SEE_PRUNE_MAX_DEPTH: i32 = 4;
        let SEE_PRUNE_MARGIN: i32 = tune::get(tune::SEE_PRUNE_MARGIN);
        if i > 0
            && beta - alpha == 1
            && !in_check
            && depth <= SEE_PRUNE_MAX_DEPTH
            && mv.is_capture()
            && mv.promotion().is_none()
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && !gives_check_cached(&mut gc, &mut check_info, board, mv)
            && crate::see::see(board, &mv) < -SEE_PRUNE_MARGIN * depth
        {
            continue;
        }

        // Guarda de futility, por jugada (alpha puede haber subido en el bucle).
        // `!next.in_check(...)` (no da jaque) va al final por su coste.
        if i > 0
            && beta - alpha == 1
            && !in_check
            && depth <= FUTILITY_MAX_DEPTH
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && is_quiet
        {
            let margin = FUTILITY_BASE + FUTILITY_STEP * depth
                - if improving { 0 } else { FUTILITY_IMPROVING_BONUS };
            if static_eval + margin <= alpha && !gives_check_cached(&mut gc, &mut check_info, board, mv) {
                continue;
            }
        }

        // Guarda de LMP (después de futility, antes de LMR): si ya se han buscado
        // suficientes silenciosas, se salta esta. Instrumentación (investigación
        // 04): se contabiliza cada poda, cuántas ocurren donde futility NO habría
        // disparado (`futility_silent`) y cuántas caen en el rango que LMR habría
        // reducido (`beyond_lmr`, i>=4). Los contadores solo miden; NO deciden.
        if i > 0
            && beta - alpha == 1
            && !in_check
            && depth <= LMP_MAX_DEPTH
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && is_quiet
            && quiets_searched >= lmp_count
            && !gives_check_cached(&mut gc, &mut check_info, board, mv)
        {
            ctx.ls.lmp_prunes += 1;
            let f_margin = FUTILITY_BASE + FUTILITY_STEP * depth
                - if improving { 0 } else { FUTILITY_IMPROVING_BONUS };
            if depth > FUTILITY_MAX_DEPTH || static_eval + f_margin > alpha {
                ctx.ls.lmp_prunes_futility_silent += 1;
            }
            if depth >= 3 && i >= 4 {
                ctx.ls.lmp_prunes_beyond_lmr += 1;
            }
            continue;
        }

        let next = board.make_move(mv);
        ctx.tt.prefetch(next.hash);

        if is_quiet {
            quiets_searched += 1;
            quiets_tried.push(mv);
        }

        if let Some(p) = board.mailbox[mv.from as usize] {
            ctx.move_stack[ply] = (p.kind, mv.to);
        }
        ctx.game_history.push(next.hash);

        let score = if i == 0 {
            let ext = if tt_first == Some(mv) { singular_ext } else { 0 };
            ctx.check_hint = gc;
            -negamax(&next, iir_depth - 1 + ext, -beta, -alpha, ply + 1, &mut child_pv, ctx)
        } else {
            // EXP-0005 (SEARCH-A): LMR logarítmica (ver `lmr_table`). Se aplica
            // a silenciosas desde la 3ª jugada en non-PV y desde la 4ª en PV,
            // nunca si la jugada da jaque. Moduladores de una unidad, cada uno
            // con su motivo: +1 si la posición no mejora (improving=false:
            // menos probable que una jugada tardía la salve); −1 en PV (la
            // línea principal merece precisión); −1 para killers (ya cortaron
            // en un hermano). Se limita para que el hijo quede con depth >= 1.
            let lmr_min_index = if is_pv { 3 } else { 2 };
            let reduction = if depth >= 3
                && i >= lmr_min_index
                && is_quiet
                && !in_check
                && !gives_check_cached(&mut gc, &mut check_info, board, mv)
            {
                ctx.ls.lmr_attempts += 1;
                let mut r = ctx.lmr[(depth as usize).min(63)][i.min(63)];
                if !improving {
                    r += 1;
                }
                if is_pv {
                    r -= 1;
                }
                let is_killer = ctx.killers[ply][0] == Some(mv) || ctx.killers[ply][1] == Some(mv);
                if is_killer {
                    r -= 1;
                } else {
                    // S2-003: la puntuación de ordenación de la silenciosa
                    // (historial mariposa + continuation history) predice si la
                    // búsqueda reducida fallará alto: medido en 120 posiciones
                    // DEV a d11, 0,7 % con puntuación negativa (60 % de las
                    // jugadas reducidas) frente a 3,4 % en [100, 1000) y 5,5 %
                    // en [1000, 10000). Una unidad más de reducción cuando el
                    // historial es negativo, una menos cuando es >= 1000.
                    let hs = quiet_order_score(&mv, board, ply, ctx);
                    if hs < 0 {
                        r += 1;
                    } else if hs >= LMR_HIST_GOOD {
                        r -= 1;
                    }
                }
                r.clamp(0, (iir_depth - 2).max(0))
            } else {
                0
            };
            ctx.check_hint = gc;
            let mut s = -negamax(
                &next,
                iir_depth - 1 - reduction,
                -alpha - 1,
                -alpha,
                ply + 1,
                &mut child_pv,
                ctx,
            );
            if !ctx.stopped && s > alpha && reduction > 0 {
                ctx.ls.lmr_researches += 1;
                ctx.check_hint = gc;
                s = -negamax(
                    &next,
                    iir_depth - 1,
                    -alpha - 1,
                    -alpha,
                    ply + 1,
                    &mut child_pv,
                    ctx,
                );
            }
            if !ctx.stopped && s > alpha && s < beta {
                ctx.ls.pvs_researches += 1;
                ctx.check_hint = gc;
                s = -negamax(&next, iir_depth - 1, -beta, -alpha, ply + 1, &mut child_pv, ctx);
            }
            s
        };

        ctx.game_history.pop();

        if ctx.stopped {
            return 0;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(mv);
            pv.clear();
            pv.push(mv);
            pv.extend_from_slice(&child_pv);
        }
        if best_score > alpha {
            alpha = best_score;
        }
        if alpha >= beta {
            ctx.ls.beta_cutoffs += 1;
            if i == 0 {
                ctx.ls.beta_cutoffs_first_move += 1;
            }
            if !mv.is_capture() {
                ctx.store_killer(ply, mv);
                ctx.bump_history(board.side_to_move, mv, depth);
                for &q in quiets_tried.iter() {
                    if q != mv {
                        ctx.malus_history(board.side_to_move, q, depth);
                    }
                }
                if let Some(p) = board.mailbox[mv.from as usize] {
                    ctx.bump_cont_history(ply, p.kind, mv.to, depth);
                }
            }
            break;
        }
    }

    // S2-002: devolver los búferes del ply a sus depósitos.
    if generated {
        ctx.move_pool[ply] = moves;
        ctx.score_pool[ply] = scores;
    }
    ctx.quiet_pool[ply] = quiets_tried;
    ctx.pv_pool[ply + 1] = child_pv;

    let flag = if best_score <= alpha_orig {
        TTFlag::UpperBound
    } else if best_score >= beta {
        TTFlag::LowerBound
    } else {
        TTFlag::Exact
    };
    // EXP-0033: actualizar la corrección cuando el resultado es informativo:
    // sin jaque, la mejor jugada no es captura ni promoción (la eval estática no
    // mide material en vuelo), sin mate, y la cota apunta en la dirección de la
    // diferencia (una cota superior solo dice algo si está por debajo de la eval).
    if !in_check
        && excluded.is_none()
        && best_score.abs() < MATE_SCORE - MAX_PLY as i32
        && best_move.is_none_or(|m| !m.is_capture() && m.promotion().is_none())
        && !(flag == TTFlag::LowerBound && best_score <= raw_eval)
        && !(flag == TTFlag::UpperBound && best_score >= raw_eval)
    {
        update_correction(board, ctx, depth, best_score - uncorrected);
    }

    // La entrada guarda `iir_depth`, que es la profundidad REALMENTE buscada.
    // Guardar `depth` afirmaría más profundidad de la que se exploró y produciría
    // cortes de TT basados en una búsqueda más superficial: sería un fallo de
    // corrección, no una variante. Cuando IIR no dispara, `iir_depth == depth`.
    // EXP-0014: una búsqueda con jugada excluida no describe el nodo: no se guarda.
    if excluded.is_some() {
        return best_score;
    }
    ctx.tt.store(
        board.hash,
        iir_depth,
        score_to_tt(best_score, ply),
        flag,
        best_move,
    );

    best_score
}

fn format_score(score: i32) -> String {
    if score.abs() >= MATE_SCORE - MAX_PLY as i32 {
        let mate_in = if score > 0 {
            (MATE_SCORE - score + 1) / 2
        } else {
            -((MATE_SCORE + score + 1) / 2)
        };
        format!("mate {mate_in}")
    } else {
        format!("cp {score}")
    }
}

fn print_info(depth: i32, seldepth: u8, score: i32, nodes: u64, elapsed_ms: u128, pv: &[Move]) {
    let ms = elapsed_ms.max(1);
    let nps = (nodes as u128 * 1000) / ms;
    let pv_str = pv
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "info depth {depth} seldepth {seldepth} score {} nodes {nodes} nps {nps} time {ms} pv {pv_str}",
        format_score(score)
    );
    let _ = std::io::stdout().flush();
}

struct ThreadResult {
    depth: i32,
    score: i32,
    best_move: Move,
}

/// Una sola búsqueda de profundización iterativa, ejecutada por UN hilo.
/// Comparte `tt` y `nodes` (contador atómico) con los demás hilos de la
/// misma llamada a `lazy_smp_search`; el resto del estado (killers,
/// historial, etc.) es privado de este hilo. Solo el hilo principal
/// (`is_main`) imprime líneas `info` — los demás buscan en silencio,
/// aportando exclusivamente a través de la tabla de transposición
/// compartida (de ahí el nombre "lazy": no hay reparto explícito del árbol
/// de búsqueda entre hilos, cada uno explora por su cuenta y comparte
/// implícitamente lo que descubre vía la TT).
#[allow(clippy::too_many_arguments)]
fn iterative_deepening_one_thread(
    board: &Board,
    limits: &SearchLimits,
    tt: &TranspositionTable,
    game_history: Vec<u64>,
    stop: &AtomicBool,
    stats: &SearchStats,
    is_main: bool,
    thread_id: usize,
) -> ThreadResult {
    let root_moves = generate_legal_moves(board);
    let mut best_move = root_moves[0];
    let mut best_score: i32 = 0;
    let mut last_completed_depth = 0;

    let mut ctx = SearchContext {
        lmr: lmr_table(),
        stats,
        ls: LocalStats::default(),
        excluded: [None; MAX_PLY],
        root_depth: 1,
        ticks: 0,
        seldepth: 0,
        stop,
        hard_deadline: limits.hard_deadline,
        max_nodes: limits.max_nodes,
        stopped: false,
        is_main,
        killers: [[None; 2]; MAX_PLY],
        hist: take_histories(thread_id),
        eval_stack: [0; MAX_PLY],
        move_stack: [(PieceType::Pawn, 0); MAX_PLY],
        tt,
        game_history,
        move_pool: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(64)).collect(),
        score_pool: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(64)).collect(),
        quiet_pool: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(64)).collect(),
        pv_pool: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(MAX_PLY)).collect(),
        qs_pool: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(32)).collect(),
        check_hint: None,
    };

    let start = Instant::now();
    let mut depth = 1;
    let mut score_drop = 0i32;
    // S2-010: ¿cambió la mejor jugada en la última iteración completada?
    let mut best_changed = false;
    loop {
        if depth > limits.max_depth {
            break;
        }
        let mut pv = Vec::new();
        ctx.root_depth = depth;
        // EXP-0005 (SEARCH-A): aspiration windows. Desde depth 5, la iteración
        // se busca con una ventana de ±ASP_DELTA alrededor del score anterior:
        // entre iteraciones consecutivas el score suele moverse poco, y una
        // ventana estrecha poda más. Si el resultado cae fuera, se ensancha
        // SOLO el lado que falló y el margen se duplica; por encima de
        // ASP_MAX_DELTA se pasa a ventana infinita. Nunca con scores de mate.
        // 25 cp = un cuarto de peón: menor que la oscilación típica entre
        // iteraciones de una posición tranquila, mayor que el ruido de tempo.
        const ASP_MIN_DEPTH: i32 = 5;
        let ASP_DELTA: i32 = tune::get(tune::ASP_DELTA);
        const ASP_MAX_DELTA: i32 = 800;
        let score = if depth >= ASP_MIN_DEPTH && best_score.abs() < MATE_SCORE - MAX_PLY as i32 {
            let mut delta = ASP_DELTA;
            let mut alpha = best_score - delta;
            let mut beta = best_score + delta;
            loop {
                let s = negamax(board, depth, alpha, beta, 0, &mut pv, &mut ctx);
                if ctx.stopped {
                    break s;
                }
                if s <= alpha {
                    alpha = (s - delta).max(-INFINITY);
                } else if s >= beta {
                    beta = (s + delta).min(INFINITY);
                } else {
                    break s;
                }
                delta *= 2;
                if delta > ASP_MAX_DELTA {
                    alpha = -INFINITY;
                    beta = INFINITY;
                }
            }
        } else {
            negamax(board, depth, -INFINITY, INFINITY, 0, &mut pv, &mut ctx)
        };
        let completed = !ctx.stopped;

        if completed || depth == 1 {
            if !pv.is_empty() {
                best_changed = completed && depth >= 6 && pv[0] != best_move;
                best_move = pv[0];
                // TM-04: caída del score frente a la iteración completada anterior.
                if completed && depth >= 6 {
                    score_drop = best_score - score;
                }
                best_score = score;
            }
            last_completed_depth = depth;
            ctx.flush_stats();
            if ctx.is_main {
                print_info(
                    depth,
                    ctx.seldepth,
                    score,
                    ctx.stats.nodes.load(Ordering::Relaxed),
                    start.elapsed().as_millis(),
                    &pv,
                );
            }
        }

        if ctx.stopped {
            break;
        }
        // TM-04 ("pánico"): si la última iteración completada perdió >= 30 cp
        // frente a la anterior, la posición se complica: se permite empezar otra
        // iteración hasta el 100 % del presupuesto blando en vez del 60 %
        // (el umbral se estira ×5/3; nunca más allá del plazo duro).
        let soft = if score_drop >= tune::get(tune::TM_PANIC_DROP) {
            let base = limits.soft_deadline.saturating_duration_since(start);
            (start + base * 5 / 3).min(limits.hard_deadline)
        } else if best_changed {
            // S2-010: la mejor jugada acaba de cambiar: la posición no está
            // resuelta. Se permite empezar otra iteración hasta 4/3 del umbral
            // (nunca más allá del plazo duro). Sin recorte en jugadas estables
            // (la causa probable del fracaso de TM-03 en el reto 24H).
            let base = limits.soft_deadline.saturating_duration_since(start);
            (start + base * 4 / 3).min(limits.hard_deadline)
        } else {
            limits.soft_deadline
        };
        if Instant::now() >= soft {
            break;
        }
        depth += 1;
    }

    ctx.flush_stats();
    let SearchContext { hist, .. } = ctx;
    give_back_histories(thread_id, hist);
    ThreadResult {
        depth: last_completed_depth,
        score: best_score,
        best_move,
    }
}

/// Lazy SMP: lanza `threads` búsquedas independientes sobre la misma
/// posición, compartiendo la tabla de transposición (vía `Arc`) y la
/// bandera de `stop`. Al terminar, se usa el resultado del hilo que llegó
/// más profundo (en empate, se prefiere el hilo principal) — un resultado
/// completado a mayor profundidad es, en general, más confiable.
pub fn lazy_smp_search(
    board: Board,
    limits: SearchLimits,
    tt: Arc<TranspositionTable>,
    game_history: Vec<u64>,
    stop: Arc<AtomicBool>,
    threads: usize,
) -> (Move, i32) {
    let threads = threads.max(1);
    let stats = Arc::new(SearchStats::default());
    tt.new_search(); // EXP-0005: envejecimiento de la TT

    if threads == 1 {
        let result =
            iterative_deepening_one_thread(&board, &limits, &tt, game_history, &stop, &stats, true, 0);
        return (result.best_move, result.score);
    }

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for t in 0..threads {
            let tt = &tt;
            let stop = &stop;
            let stats = &stats;
            let history = game_history.clone();
            handles.push(scope.spawn(move || {
                iterative_deepening_one_thread(&board, &limits, tt, history, stop, stats, t == 0, t)
            }));
        }

        let mut results: Vec<ThreadResult> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        // Empatar en profundidad prefiere al hilo principal (índice 0): lo
        // conseguimos ordenando de forma estable y comparando solo por
        // profundidad (sort_by_key es estable, así que el primer índice con
        // la profundidad máxima que aparezca primero en el vector gana).
        results
            .iter()
            .enumerate()
            .max_by_key(|(i, r)| (r.depth, if *i == 0 { 1 } else { 0 }))
            .map(|(i, _)| i)
            .map(|i| {
                let r = results.remove(i);
                (r.best_move, r.score)
            })
            .unwrap()
    })
}

pub fn search_fixed_depth_with_stats(
    board: Board,
    depth: i32,
    tt: Arc<TranspositionTable>,
    game_history: Vec<u64>,
) -> (Move, i32, SearchStatsSnapshot) {
    let stop = AtomicBool::new(false);
    let stats = SearchStats::default();
    let limits = SearchLimits {
        max_depth: depth,
        soft_deadline: Instant::now() + std::time::Duration::from_secs(86_400),
        hard_deadline: Instant::now() + std::time::Duration::from_secs(86_400),
        max_nodes: None,
    };
    // Bench/EPD: cada posición parte de historiales vacíos (firma reproducible).
    clear_histories();
    let result =
        iterative_deepening_one_thread(&board, &limits, &tt, game_history, &stop, &stats, false, 0);
    (result.best_move, result.score, stats.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tt::TranspositionTable;
    use std::time::Duration;

    fn search_fixed_depth(fen: &str, depth: i32) -> (Move, i32) {
        let board = Board::from_fen(fen).unwrap();
        let tt = Arc::new(TranspositionTable::new(16));
        let stop = Arc::new(AtomicBool::new(false));
        let limits = SearchLimits {
            max_depth: depth,
            soft_deadline: Instant::now() + Duration::from_secs(30),
            hard_deadline: Instant::now() + Duration::from_secs(30),
            max_nodes: None,
        };
        lazy_smp_search(board, limits, tt, vec![board.hash], stop, 1)
    }

    #[test]
    fn finds_mate_in_one() {
        // Mate de la fila de atrás: el rey negro está encerrado por sus
        // propios peones y la torre blanca entra en la 8ª fila sin que haya
        // ninguna escapatoria. En vez de asumir cuál es "la" jugada de mate
        // (podría haber más de una en otras posiciones), verificamos
        // directamente la semántica: el movimiento debe dar jaque y dejar
        // al rival sin ningún movimiento legal.
        let fen = "6k1/5ppp/8/8/8/8/8/4R1K1 w - - 0 1";
        let (mv, score) = search_fixed_depth(fen, 4);
        assert!(
            score >= MATE_SCORE - MAX_PLY as i32,
            "no se detectó mate, score={score}"
        );

        let board = Board::from_fen(fen).unwrap();
        let after = board.make_move(mv);
        assert!(
            after.in_check(after.side_to_move),
            "el movimiento no da jaque: {mv}"
        );
        assert!(
            generate_legal_moves(&after).is_empty(),
            "el movimiento no es mate, el rival todavía tiene jugadas: {mv}"
        );
    }

    #[test]
    fn captures_free_rook() {
        // Torre negra en d8 realmente indefensa (el rey negro está en h8,
        // lejos): la dama blanca en d1 puede capturarla sin compensación
        // para las negras. A diferencia de una torre "protegida" por su
        // propio rey adyacente, aquí no hay ninguna razón para no capturar.
        let (mv, score) = search_fixed_depth("3r3k/8/8/8/8/8/8/3QK3 w - - 0 1", 3);
        assert_eq!(mv.to_string(), "d1d8");
        assert!(
            score > 300,
            "score inesperadamente bajo tras ganar una torre limpia: {score}"
        );
    }

    #[test]
    fn mating_move_takes_precedence_over_fifty_move_draw() {
        for clock in [99, 149] {
            let fen = format!("6k1/5ppp/8/8/8/8/8/4R1K1 w - - {clock} 1");
            let (mv, score) = search_fixed_depth(&fen, 4);
            let board = Board::from_fen(&fen).unwrap();
            let after = board.make_move(mv);
            assert!(after.in_check(after.side_to_move));
            assert!(generate_legal_moves(&after).is_empty(), "missed mate with clock {clock}: {mv}");
            assert!(score >= MATE_SCORE - MAX_PLY as i32, "mate scored as {score}");
        }
    }

    #[test]
    fn fifty_move_draw_still_applies_when_there_is_no_mate() {
        let (_, score) = search_fixed_depth("7k/8/8/8/8/8/8/4R1K1 w - - 99 1", 3);
        assert_eq!(score, 0);
    }

    #[test]
    fn avoids_hanging_the_queen_to_a_defended_piece() {
        // Aquí la torre negra en d8 SÍ está defendida por su propio rey en
        // e8 (casillas adyacentes): capturarla con la dama perdería dama por
        // torre tras la recaptura del rey. El motor no debe caer en esta trampa.
        let (mv, _score) = search_fixed_depth("3rk3/8/8/8/8/8/8/3QK3 w - - 0 1", 4);
        assert_ne!(
            mv.to_string(),
            "d1d8",
            "el motor cambió dama por torre innecesariamente"
        );
    }

    #[test]
    fn stops_promptly_when_flag_set() {
        let board = Board::start_pos();
        let tt = Arc::new(TranspositionTable::new(16));
        let stop = Arc::new(AtomicBool::new(true)); // ya detenido desde el principio
        let limits = SearchLimits {
            max_depth: 64,
            soft_deadline: Instant::now() + Duration::from_secs(30),
            hard_deadline: Instant::now() + Duration::from_secs(30),
            max_nodes: None,
        };
        let (mv, _) = lazy_smp_search(board, limits, tt, vec![board.hash], stop, 1);
        // Debe devolver un movimiento legal de la posición inicial pese a
        // estar "detenido" desde el inicio (gracias a la garantía de profundidad 1).
        assert!(generate_legal_moves(&board).contains(&mv));
    }

    #[test]
    fn multithreaded_search_still_finds_mate() {
        // Con varios hilos compartiendo la misma TT, el resultado debe
        // seguir siendo correcto: mismo test de mate que arriba, pero
        // forzando 4 hilos en vez de 1. Verifica tanto la corrección del
        // resultado como que lanzar/unir los hilos no entra en pánico.
        let fen = "6k1/5ppp/8/8/8/8/8/4R1K1 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let tt = Arc::new(TranspositionTable::new(16));
        let stop = Arc::new(AtomicBool::new(false));
        let limits = SearchLimits {
            max_depth: 4,
            soft_deadline: Instant::now() + Duration::from_secs(30),
            hard_deadline: Instant::now() + Duration::from_secs(30),
            max_nodes: None,
        };
        let (mv, score) = lazy_smp_search(board, limits, tt, vec![board.hash], stop, 4);
        assert!(
            score >= MATE_SCORE - MAX_PLY as i32,
            "no se detectó mate con 4 hilos, score={score}"
        );
        let after = board.make_move(mv);
        assert!(after.in_check(after.side_to_move));
        assert!(generate_legal_moves(&after).is_empty());
    }

    #[test]
    fn multithreaded_search_stops_promptly() {
        let board = Board::start_pos();
        let tt = Arc::new(TranspositionTable::new(16));
        let stop = Arc::new(AtomicBool::new(true));
        let limits = SearchLimits {
            max_depth: 64,
            soft_deadline: Instant::now() + Duration::from_secs(30),
            hard_deadline: Instant::now() + Duration::from_secs(30),
            max_nodes: None,
        };
        let (mv, _) = lazy_smp_search(board, limits, tt, vec![board.hash], stop, 4);
        assert!(generate_legal_moves(&board).contains(&mv));
    }
}
