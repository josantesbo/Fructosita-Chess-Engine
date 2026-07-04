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

#[derive(Clone, Copy)]
pub struct SearchLimits {
    pub max_depth: i32,
    pub soft_deadline: Instant,
    pub hard_deadline: Instant,
}

struct SearchContext<'a> {
    nodes: &'a AtomicU64,
    seldepth: u8,
    stop: &'a AtomicBool,
    hard_deadline: Instant,
    stopped: bool,
    is_main: bool,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history_heuristic: [[[i32; 64]; 64]; 2],
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

    fn check_time(&mut self) {
        if self.nodes.load(Ordering::Relaxed) % 2048 == 0
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

fn move_score(mv: &Move, board: &Board, tt_move: Option<Move>, ply: usize, ctx: &SearchContext) -> i32 {
    if tt_move == Some(*mv) {
        return 2_000_000;
    }
    if let Some(promo) = mv.promotion() {
        let base = if promo == PieceType::Queen { 1_800_000 } else { 100_000 };
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
        ctx.history_heuristic[board.side_to_move.index()][mv.from as usize][mv.to as usize]
    }
}

/// Filtra y ordena las capturas candidatas de quiescence usando SEE: las
/// capturas que pierden material (SEE < 0) se descartan directamente (poda
/// estándar de quiescence), y las que quedan se ordenan de más a menos
/// rentables. SEE se calcula una sola vez por movimiento y se reutiliza
/// tanto para filtrar como para ordenar.
fn filter_and_order_quiescence_moves(moves: Vec<Move>, board: &Board, in_check: bool) -> Vec<Move> {
    if in_check {
        // En jaque hay que considerar todas las evasiones legales, no solo
        // capturas: no podemos permitirnos descartar la única jugada legal
        // solo porque su SEE sea negativo.
        return moves;
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
    scored.into_iter().map(|(m, _)| m).collect()
}

fn quiescence(board: &Board, mut alpha: i32, beta: i32, ply: usize, ctx: &mut SearchContext) -> i32 {
    ctx.check_time();
    if ctx.stopped {
        return 0;
    }
    ctx.nodes.fetch_add(1, Ordering::Relaxed);
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    let in_check = board.in_check(board.side_to_move);
    let stand_pat = eval::evaluate(board);

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

    let all_legal = generate_legal_moves(board);
    if in_check && all_legal.is_empty() {
        return -MATE_SCORE + ply as i32;
    }

    let moves = filter_and_order_quiescence_moves(all_legal, board, in_check);

    let mut best = if in_check { -INFINITY } else { alpha };
    for mv in moves {
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

    ctx.nodes.fetch_add(1, Ordering::Relaxed);
    if ply as u8 > ctx.seldepth {
        ctx.seldepth = ply as u8;
    }

    let tt_probe = ctx.tt.probe(board.hash);
    let mut tt_move = None;
    if let Some(entry) = &tt_probe {
        tt_move = entry.best_move;
        if entry.depth as i32 >= depth && ply > 0 {
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
    }

    // Null-move pruning: si "pasar el turno" sigue dando una posición tan
    // buena que supera beta, es muy probable que la posición real también lo
    // haga, así que podamos esta rama. Se evita en jaque, en profundidades
    // bajas, y sin material mayor propio (riesgo de zugzwang).
    if !in_check && depth >= 3 && ply > 0 && has_non_pawn_material(board, board.side_to_move) {
        let null_board = board.make_null_move();
        ctx.game_history.push(null_board.hash);
        let mut child_pv = Vec::new();
        let score = -negamax(&null_board, depth - 4, -beta, -beta + 1, ply + 1, &mut child_pv, ctx);
        ctx.game_history.pop();
        if ctx.stopped {
            return 0;
        }
        if score >= beta {
            return score;
        }
    }

    let mut moves = generate_legal_moves(board);
    if moves.is_empty() {
        return if in_check { -MATE_SCORE + ply as i32 } else { 0 };
    }
    moves.sort_by_key(|mv| std::cmp::Reverse(move_score(mv, board, tt_move, ply, ctx)));

    let mut best_score = -INFINITY;
    let mut best_move = moves[0];
    let alpha_orig = alpha;

    for (i, &mv) in moves.iter().enumerate() {
        let next = board.make_move(mv);
        ctx.game_history.push(next.hash);
        let mut child_pv = Vec::new();

        let score = if i == 0 {
            -negamax(&next, depth - 1, -beta, -alpha, ply + 1, &mut child_pv, ctx)
        } else {
            let reduction = if depth >= 3 && i >= 4 && !mv.is_capture() && mv.promotion().is_none() && !in_check {
                if i >= 10 { 2 } else { 1 }
            } else {
                0
            };
            let mut s = -negamax(&next, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, &mut child_pv, ctx);
            if !ctx.stopped && s > alpha && reduction > 0 {
                s = -negamax(&next, depth - 1, -alpha - 1, -alpha, ply + 1, &mut child_pv, ctx);
            }
            if !ctx.stopped && s > alpha && s < beta {
                s = -negamax(&next, depth - 1, -beta, -alpha, ply + 1, &mut child_pv, ctx);
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
            if !mv.is_capture() {
                ctx.store_killer(ply, mv);
                ctx.bump_history(board.side_to_move, mv, depth);
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
    ctx.tt.store(board.hash, depth, score_to_tt(best_score, ply), flag, Some(best_move));

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
    let pv_str = pv.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ");
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
    nodes: &AtomicU64,
    is_main: bool,
) -> ThreadResult {
    let root_moves = generate_legal_moves(board);
    let mut best_move = root_moves[0];
    let mut best_score = 0;
    let mut last_completed_depth = 0;

    let mut ctx = SearchContext {
        nodes,
        seldepth: 0,
        stop,
        hard_deadline: limits.hard_deadline,
        stopped: false,
        is_main,
        killers: [[None; 2]; MAX_PLY],
        history_heuristic: [[[0; 64]; 64]; 2],
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
        let score = negamax(board, depth, -INFINITY, INFINITY, 0, &mut pv, &mut ctx);
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
                    ctx.nodes.load(Ordering::Relaxed),
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

    ThreadResult { depth: last_completed_depth, score: best_score, best_move }
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
    let nodes = Arc::new(AtomicU64::new(0));

    if threads == 1 {
        let result = iterative_deepening_one_thread(&board, &limits, &tt, game_history, &stop, &nodes, true);
        return (result.best_move, result.score);
    }

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for t in 0..threads {
            let tt = &tt;
            let stop = &stop;
            let nodes = &nodes;
            let history = game_history.clone();
            handles.push(scope.spawn(move || {
                iterative_deepening_one_thread(&board, &limits, tt, history, stop, nodes, t == 0)
            }));
        }

        let mut results: Vec<ThreadResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();
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
        assert!(score >= MATE_SCORE - MAX_PLY as i32, "no se detectó mate, score={score}");

        let board = Board::from_fen(fen).unwrap();
        let after = board.make_move(mv);
        assert!(after.in_check(after.side_to_move), "el movimiento no da jaque: {mv}");
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
        assert!(score > 300, "score inesperadamente bajo tras ganar una torre limpia: {score}");
    }

    #[test]
    fn avoids_hanging_the_queen_to_a_defended_piece() {
        // Aquí la torre negra en d8 SÍ está defendida por su propio rey en
        // e8 (casillas adyacentes): capturarla con la dama perdería dama por
        // torre tras la recaptura del rey. El motor no debe caer en esta trampa.
        let (mv, _score) = search_fixed_depth("3rk3/8/8/8/8/8/8/3QK3 w - - 0 1", 4);
        assert_ne!(mv.to_string(), "d1d8", "el motor cambió dama por torre innecesariamente");
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
        assert!(score >= MATE_SCORE - MAX_PLY as i32, "no se detectó mate con 4 hilos, score={score}");
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
