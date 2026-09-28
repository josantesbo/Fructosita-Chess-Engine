//! Búsqueda del motor: negamax con poda alfa-beta, Principal Variation
//! Search (PVS), null-move pruning, Late Move Reductions (LMR), extensión
//! por jaque, quiescence search, y profundización iterativa con manejo de
//! tiempo y control mediante `stop`.
//!
//! Todas estas son técnicas públicas y estándar (Chess Programming Wiki),
//! implementadas aquí desde cero, no copiadas de ningún motor existente.

use crate::board::Board;
use crate::eval;
use crate::movegen::generate_legal_moves;
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
const MEASURE_TIER_CHANGES: bool = false;

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
const LMR_BASE: f64 = 0.5;
const LMR_DIVISOR: f64 = 3.0;

fn lmr_table() -> &'static [[i32; 64]; 64] {
    static TABLE: std::sync::OnceLock<[[i32; 64]; 64]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [[0i32; 64]; 64];
        for (d, row) in t.iter_mut().enumerate().skip(1) {
            for (m, r) in row.iter_mut().enumerate().skip(1) {
                *r = (LMR_BASE + (d as f64).ln() * (m as f64).ln() / LMR_DIVISOR) as i32;
            }
        }
        t
    })
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
}

struct SearchContext<'a> {
    stats: &'a SearchStats,
    seldepth: u8,
    stop: &'a AtomicBool,
    hard_deadline: Instant,
    stopped: bool,
    is_main: bool,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history_heuristic: [[[i32; 64]; 64]; 2],
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
    cont_history: Box<[[[[i16; 64]; 6]; 64]; 6]>,
    tt: &'a TranspositionTable,
    /// Hashes de todas las posiciones desde el inicio de la partida (o del
    /// FEN inicial) hasta el nodo actual, inclusive. Crece/decrece con la
    /// recursión: se hace push antes de bajar a un hijo y pop al volver.
    game_history: Vec<u64>,
}

impl<'a> SearchContext<'a> {
    fn store_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != Some(mv) {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = Some(mv);
        }
    }

    fn bump_history(&mut self, color: Color, mv: Move, depth: i32) {
        let entry = &mut self.history_heuristic[color.index()][mv.from as usize][mv.to as usize];
        *entry += depth * depth;
        if *entry > 1_000_000 {
            for c in self.history_heuristic.iter_mut() {
                for row in c.iter_mut() {
                    for v in row.iter_mut() {
                        *v /= 2;
                    }
                }
            }
        }
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
            &mut self.cont_history[pp.index()][pt as usize][piece.index()][to as usize];
        *entry = entry.saturating_add((depth * depth) as i16);
        if *entry > CONT_HISTORY_MAX {
            for a in self.cont_history.iter_mut() {
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
        self.cont_history[pp.index()][pt as usize][piece.index()][to as usize] as i32
    }

    fn check_time(&mut self) {
        if self
            .stats
            .nodes
            .load(Ordering::Relaxed)
            .is_multiple_of(2048)
            && (self.stop.load(Ordering::Relaxed) || Instant::now() >= self.hard_deadline)
        {
            self.stopped = true;
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
        return true;
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
        let h = ctx.history_heuristic[board.side_to_move.index()][mv.from as usize]
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
fn filter_and_order_quiescence_moves(moves: Vec<Move>, board: &Board, in_check: bool) -> Vec<(Move, i32)> {
    if in_check {
        // En jaque hay que considerar todas las evasiones legales, no solo
        // capturas: no podemos permitirnos descartar la única jugada legal
        // solo porque su SEE sea negativo.
        return moves.into_iter().map(|m| (m, 0)).collect();
    }
    let mut scored: Vec<(Move, i32)> = moves
        .into_iter()
        .filter_map(|m| {
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
        })
        .collect();
    scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    scored
}

fn quiescence(
    board: &Board,
    mut alpha: i32,
    beta: i32,
    ply: usize,
    ctx: &mut SearchContext,
) -> i32 {
    ctx.check_time();
    if ctx.stopped {
        return 0;
    }
    ctx.stats.qnodes.fetch_add(1, Ordering::Relaxed);
    ctx.stats.nodes.fetch_add(1, Ordering::Relaxed);
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    let in_check = board.in_check(board.side_to_move);
    let stand_pat = if !in_check || ply >= MAX_PLY {
        eval::evaluate(board)
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
    let moves = if in_check {
        let all_legal = generate_legal_moves(board);
        if all_legal.is_empty() {
            return -MATE_SCORE + ply as i32;
        }
        filter_and_order_quiescence_moves(all_legal, board, true)
    } else {
        filter_and_order_quiescence_moves(
            crate::movegen::generate_legal_captures(board),
            board,
            false,
        )
    };

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
    const DELTA_MARGIN: i32 = 200;

    let mut best = if in_check { -INFINITY } else { alpha };
    for (mv, see_v) in moves {
        if !in_check
            && mv.promotion().is_none()
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && stand_pat.saturating_add(see_v).saturating_add(DELTA_MARGIN) <= alpha
        {
            continue;
        }
        let next = board.make_move(mv);
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
    pv.clear();

    ctx.check_time();
    if ctx.stopped {
        return 0;
    }

    if ply > 0 && is_repetition_or_fifty(board, ctx) {
        return 0;
    }
    if ply >= MAX_PLY {
        return eval::evaluate(board);
    }

    let in_check = board.in_check(board.side_to_move);
    let mut depth = depth;
    if in_check {
        depth += 1;
    }

    if depth <= 0 {
        return quiescence(board, alpha, beta, ply, ctx);
    }

    ctx.stats.nodes.fetch_add(1, Ordering::Relaxed);
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    ctx.stats.tt_probes.fetch_add(1, Ordering::Relaxed);
    let tt_probe = ctx.tt.probe(board.hash);
    let mut tt_move = None;
    if let Some(entry) = &tt_probe {
        ctx.stats.tt_hits.fetch_add(1, Ordering::Relaxed);
        tt_move = entry.best_move;
        if entry.depth as i32 >= depth && ply > 0 {
            let score = score_from_tt(entry.score, ply);
            let usable = match entry.flag {
                TTFlag::Exact => true,
                TTFlag::LowerBound => score >= beta,
                TTFlag::UpperBound => score <= alpha,
            };
            if usable {
                ctx.stats.tt_cutoffs.fetch_add(1, Ordering::Relaxed);
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
    let static_eval = eval::evaluate(board);
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
    const RFP_MARGIN: i32 = 80; // centipeones por unidad de profundidad
    const RFP_IMPROVING_BONUS: i32 = 25; // colchón extra cuando la posición mejora
    let rfp_margin = RFP_MARGIN - if improving { 0 } else { RFP_IMPROVING_BONUS };
    if beta - alpha == 1
        && !in_check
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
        && depth >= 3
        && ply > 0
        && static_eval >= beta
        && beta.abs() < MATE_SCORE - MAX_PLY as i32
        && has_non_pawn_material(board, board.side_to_move)
    {
        ctx.stats.null_move_attempts.fetch_add(1, Ordering::Relaxed);
        let null_r = 3 + depth / 4 + ((static_eval - beta) / 200).min(2);
        let null_board = board.make_null_move();
        ctx.game_history.push(null_board.hash);
        let mut child_pv = Vec::new();
        let score = -negamax(
            &null_board,
            depth - 1 - null_r,
            -beta,
            -beta + 1,
            ply + 1,
            &mut child_pv,
            ctx,
        );
        ctx.game_history.pop();
        if ctx.stopped {
            return 0;
        }
        if score >= beta {
            ctx.stats.null_move_cutoffs.fetch_add(1, Ordering::Relaxed);
            return if score >= MATE_SCORE - MAX_PLY as i32 { beta } else { score };
        }
    }

    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return if in_check {
            -MATE_SCORE + ply as i32
        } else {
            0
        };
    }
    // === MEDICIÓN (puerta 2, investigación 10): tier_change_pct ===
    // Se calculan las DOS ordenaciones (con y sin continuation history) y se
    // clasifica cada silenciosa en T0 (completa) / T1 (LMR) / T2 (LMP) bajo cada
    // una. Solo en build de medición: el candidato no paga este coste.
    if MEASURE_TIER_CHANGES && depth >= 3 {
        // Literales espejo de LMP_BASE/LMP_C/LMP_IMPROVING_BONUS (declaradas más
        // abajo en la función). Solo código de medición.
        let lmp_c = 3 + depth * depth - if improving { 0 } else { 1 };
        let mut with_ch: Vec<(i32, Move)> = moves
            .iter()
            .map(|m| (move_score(m, board, tt_move, ply, ctx), *m))
            .collect();
        let mut no_ch: Vec<(i32, Move)> = moves
            .iter()
            .map(|m| {
                let base = if m.is_capture() || m.promotion().is_some() {
                    move_score(m, board, tt_move, ply, ctx)
                } else if ctx.killers[ply][0] == Some(*m) || ctx.killers[ply][1] == Some(*m) {
                    move_score(m, board, tt_move, ply, ctx)
                } else {
                    ctx.history_heuristic[board.side_to_move.index()][m.from as usize]
                        [m.to as usize]
                };
                (base, *m)
            })
            .collect();
        with_ch.sort_by_key(|(sc, _)| std::cmp::Reverse(*sc));
        no_ch.sort_by_key(|(sc, _)| std::cmp::Reverse(*sc));
        let tier_of = |list: &Vec<(i32, Move)>, target: &Move| -> Option<u8> {
            let mut q = 0i32;
            for (i, (_, m)) in list.iter().enumerate() {
                let is_q = !m.is_capture() && m.promotion().is_none();
                if m == target {
                    if !is_q {
                        return None;
                    }
                    return Some(if q >= lmp_c {
                        2
                    } else if i >= 4 {
                        1
                    } else {
                        0
                    });
                }
                if is_q {
                    q += 1;
                }
            }
            None
        };
        for mv in moves.iter() {
            if mv.is_capture() || mv.promotion().is_some() {
                continue;
            }
            // Diagnóstico de escala: magnitud típica de cada historial.
            if let Some(pc) = board.mailbox[mv.from as usize] {
                let hv = ctx.history_heuristic[board.side_to_move.index()][mv.from as usize]
                    [mv.to as usize];
                let cv = ctx.cont_history_score(ply, pc.kind, mv.to);
                ctx.stats
                    .ch_quiets_scored
                    .fetch_add(hv as u64, Ordering::Relaxed);
                ctx.stats
                    .ch_nonzero_entries
                    .fetch_add(cv as u64, Ordering::Relaxed);
            }
            if let (Some(a), Some(b)) = (tier_of(&with_ch, mv), tier_of(&no_ch, mv)) {
                ctx.stats.ch_quiets_classified.fetch_add(1, Ordering::Relaxed);
                if a != b {
                    ctx.stats.ch_tier_changes.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    moves.sort_by_cached_key(|mv| std::cmp::Reverse(move_score(mv, board, tt_move, ply, ctx)));

    let mut best_score = -INFINITY;
    let mut best_move = moves[0];
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
    const FUTILITY_BASE: i32 = 100; // cp
    const FUTILITY_STEP: i32 = 100; // cp por unidad de profundidad
    const FUTILITY_IMPROVING_BONUS: i32 = 50; // colchón extra cuando mejora

    // Late Move Pruning (move-count): poda por CONTEO de jugadas, no por eval.
    // Cerca de las hojas, una vez probadas suficientes silenciosas, las
    // restantes —peor ordenadas— se saltan. Cubre el caso que futility no ve
    // (posición no hundida, muchas silenciosas). El umbral crece con la
    // profundidad y se modula con "improving". Detalle de Fructosita: las
    // capturas perdedoras (SEE<0) quedan AL FINAL de la lista, así que se usa
    // `continue` solo-silenciosas, NUNCA `break`. Constantes de partida a afinar
    // por SPRT — NO copiadas. Ver research/04-late-move-pruning §8.
    const LMP_MAX_DEPTH: i32 = 4;
    const LMP_BASE: i32 = 3;
    const LMP_C: i32 = 1;
    const LMP_IMPROVING_BONUS: i32 = 1; // en unidades de conteo
    let lmp_count =
        LMP_BASE + LMP_C * depth * depth - if improving { 0 } else { LMP_IMPROVING_BONUS };

    // Nº de jugadas silenciosas ya buscadas en este nodo (para el umbral LMP).
    let mut quiets_searched: i32 = 0;

    for (i, &mv) in moves.iter().enumerate() {
        let next = board.make_move(mv);
        let is_quiet = !mv.is_capture() && mv.promotion().is_none();

        // Guarda de futility, por jugada (alpha puede haber subido en el bucle).
        // `!next.in_check(...)` (no da jaque) va al final por su coste.
        if i > 0
            && beta - alpha == 1
            && !in_check
            && depth <= FUTILITY_MAX_DEPTH
            && alpha > -MATE_SCORE + MAX_PLY as i32
            && is_quiet
            && !next.in_check(next.side_to_move)
        {
            let margin = FUTILITY_BASE + FUTILITY_STEP * depth
                - if improving { 0 } else { FUTILITY_IMPROVING_BONUS };
            if static_eval + margin <= alpha {
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
            && !next.in_check(next.side_to_move)
        {
            ctx.stats.lmp_prunes.fetch_add(1, Ordering::Relaxed);
            let f_margin = FUTILITY_BASE + FUTILITY_STEP * depth
                - if improving { 0 } else { FUTILITY_IMPROVING_BONUS };
            if depth > FUTILITY_MAX_DEPTH || static_eval + f_margin > alpha {
                ctx.stats
                    .lmp_prunes_futility_silent
                    .fetch_add(1, Ordering::Relaxed);
            }
            if depth >= 3 && i >= 4 {
                ctx.stats
                    .lmp_prunes_beyond_lmr
                    .fetch_add(1, Ordering::Relaxed);
            }
            continue;
        }

        if is_quiet {
            quiets_searched += 1;
        }

        if let Some(p) = board.mailbox[mv.from as usize] {
            ctx.move_stack[ply] = (p.kind, mv.to);
        }
        ctx.game_history.push(next.hash);
        let mut child_pv = Vec::new();

        let score = if i == 0 {
            -negamax(&next, iir_depth - 1, -beta, -alpha, ply + 1, &mut child_pv, ctx)
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
                && !next.in_check(next.side_to_move)
            {
                ctx.stats.lmr_attempts.fetch_add(1, Ordering::Relaxed);
                let mut r = lmr_table()[(depth as usize).min(63)][i.min(63)];
                if !improving {
                    r += 1;
                }
                if is_pv {
                    r -= 1;
                }
                if ctx.killers[ply][0] == Some(mv) || ctx.killers[ply][1] == Some(mv) {
                    r -= 1;
                }
                r.clamp(0, (iir_depth - 2).max(0))
            } else {
                0
            };
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
                ctx.stats.lmr_researches.fetch_add(1, Ordering::Relaxed);
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
                ctx.stats.pvs_researches.fetch_add(1, Ordering::Relaxed);
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
            best_move = mv;
            pv.clear();
            pv.push(mv);
            pv.extend(child_pv);
        }
        if best_score > alpha {
            alpha = best_score;
        }
        if alpha >= beta {
            ctx.stats.beta_cutoffs.fetch_add(1, Ordering::Relaxed);
            if i == 0 {
                ctx.stats
                    .beta_cutoffs_first_move
                    .fetch_add(1, Ordering::Relaxed);
            }
            if !mv.is_capture() {
                ctx.store_killer(ply, mv);
                ctx.bump_history(board.side_to_move, mv, depth);
                if let Some(p) = board.mailbox[mv.from as usize] {
                    ctx.bump_cont_history(ply, p.kind, mv.to, depth);
                }
            }
            break;
        }
    }

    let flag = if best_score <= alpha_orig {
        TTFlag::UpperBound
    } else if best_score >= beta {
        TTFlag::LowerBound
    } else {
        TTFlag::Exact
    };
    // La entrada guarda `iir_depth`, que es la profundidad REALMENTE buscada.
    // Guardar `depth` afirmaría más profundidad de la que se exploró y produciría
    // cortes de TT basados en una búsqueda más superficial: sería un fallo de
    // corrección, no una variante. Cuando IIR no dispara, `iir_depth == depth`.
    ctx.tt.store(
        board.hash,
        iir_depth,
        score_to_tt(best_score, ply),
        flag,
        Some(best_move),
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
) -> ThreadResult {
    let root_moves = generate_legal_moves(board);
    let mut best_move = root_moves[0];
    let mut best_score: i32 = 0;
    let mut last_completed_depth = 0;

    let mut ctx = SearchContext {
        stats,
        seldepth: 0,
        stop,
        hard_deadline: limits.hard_deadline,
        stopped: false,
        is_main,
        killers: [[None; 2]; MAX_PLY],
        history_heuristic: [[[0; 64]; 64]; 2],
        eval_stack: [0; MAX_PLY],
        move_stack: [(PieceType::Pawn, 0); MAX_PLY],
        cont_history: Box::new([[[[0i16; 64]; 6]; 64]; 6]),
        tt,
        game_history,
    };

    let start = Instant::now();
    let mut depth = 1;
    loop {
        if depth > limits.max_depth {
            break;
        }
        let mut pv = Vec::new();
        // EXP-0005 (SEARCH-A): aspiration windows. Desde depth 5, la iteración
        // se busca con una ventana de ±ASP_DELTA alrededor del score anterior:
        // entre iteraciones consecutivas el score suele moverse poco, y una
        // ventana estrecha poda más. Si el resultado cae fuera, se ensancha
        // SOLO el lado que falló y el margen se duplica; por encima de
        // ASP_MAX_DELTA se pasa a ventana infinita. Nunca con scores de mate.
        // 25 cp = un cuarto de peón: menor que la oscilación típica entre
        // iteraciones de una posición tranquila, mayor que el ruido de tempo.
        const ASP_MIN_DEPTH: i32 = 5;
        const ASP_DELTA: i32 = 25;
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
                best_move = pv[0];
                best_score = score;
            }
            last_completed_depth = depth;
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
        if Instant::now() >= limits.soft_deadline {
            break;
        }
        depth += 1;
    }

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
            iterative_deepening_one_thread(&board, &limits, &tt, game_history, &stop, &stats, true);
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
                iterative_deepening_one_thread(&board, &limits, tt, history, stop, stats, t == 0)
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
    };
    let result =
        iterative_deepening_one_thread(&board, &limits, &tt, game_history, &stop, &stats, false);
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
        };
        let (mv, _) = lazy_smp_search(board, limits, tt, vec![board.hash], stop, 4);
        assert!(generate_legal_moves(&board).contains(&mv));
    }
}
