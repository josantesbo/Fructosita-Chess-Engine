//! Bucle del protocolo UCI (Universal Chess Interface).
//!
//! La búsqueda corre en un hilo "orquestador" separado del bucle principal
//! de lectura de stdin: así, si llega un comando `stop` mientras el motor
//! está pensando, el hilo principal puede procesarlo de inmediato
//! (activando una bandera atómica que todos los hilos de búsqueda revisan
//! periódicamente) en vez de quedar bloqueado esperando a que termine.
//!
//! Con `Threads > 1` (Lazy SMP), ese hilo orquestador a su vez lanza varios
//! hilos de búsqueda que comparten la misma tabla de transposición
//! (`Arc<TranspositionTable>`, segura para acceso concurrente — ver
//! `tt.rs`) y la misma bandera de `stop`.

use crate::board::Board;
use crate::book::{Book, BookRng};
use crate::movegen::{find_move, generate_legal_moves};
use crate::perft::perft_divide;
use crate::search::{self, SearchLimits};
use crate::time_management::{allocate_time, TimeControlInput};
use crate::tt::TranspositionTable;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const ENGINE_NAME: &str = "Fructosita 1.2.0 Aldosa";
pub const ENGINE_AUTHOR: &str = "Antonio";
const DEFAULT_HASH_MB: usize = 64;
const DEFAULT_THREADS: usize = 1;

struct EngineState {
    board: Board,
    /// Hashes de todas las posiciones desde el inicio de la partida hasta
    /// la posición actual, inclusive. Se usa para detectar repeticiones.
    game_history: Vec<u64>,
    tt: Arc<TranspositionTable>,
    threads: usize,
    stop_flag: Arc<AtomicBool>,
    search_thread: Option<JoinHandle<()>>,
    book: Option<Book>,
    own_book: bool,
    book_rng: BookRng,
}

impl EngineState {
    fn new() -> Self {
        let board = Board::start_pos();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545F4914F6CDD1D);
        EngineState {
            game_history: vec![board.hash],
            board,
            tt: Arc::new(TranspositionTable::new(DEFAULT_HASH_MB)),
            threads: DEFAULT_THREADS,
            stop_flag: Arc::new(AtomicBool::new(false)),
            search_thread: None,
            book: None,
            own_book: true,
            book_rng: BookRng::new(seed),
        }
    }
}

/// Si hay una búsqueda en curso, la detiene y espera a que termine (une el
/// hilo orquestador). Se llama antes de procesar `position`, `go`,
/// `ucinewgame` o `setoption`, para evitar que un comando pise a una
/// búsqueda que aún no ha terminado.
fn ensure_search_finished(state: &mut EngineState) {
    if let Some(handle) = state.search_thread.take() {
        state.stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join();
        state.stop_flag.store(false, Ordering::Relaxed);
    }
}

pub fn run() {
    let stdin = io::stdin();
    let mut state = EngineState::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let first_word = line.split_whitespace().next().unwrap_or("");
        match first_word {
            "uci" => {
                println!("id name {ENGINE_NAME}");
                println!("id author {ENGINE_AUTHOR}");
                println!("option name Hash type spin default {DEFAULT_HASH_MB} min 1 max 4096");
                println!("option name Threads type spin default {DEFAULT_THREADS} min 1 max 64");
                println!("option name OwnBook type check default true");
                println!("option name BookFile type string default <empty>");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                ensure_search_finished(&mut state);
                state.board = Board::start_pos();
                state.game_history = vec![state.board.hash];
                state.tt.clear();
            }
            "position" => {
                ensure_search_finished(&mut state);
                let rest = line["position".len()..].trim();
                handle_position(rest, &mut state);
            }
            "go" => {
                ensure_search_finished(&mut state);
                handle_go(line, &mut state);
            }
            "stop" => state.stop_flag.store(true, Ordering::Relaxed),
            "setoption" => {
                ensure_search_finished(&mut state);
                handle_setoption(line, &mut state);
            }
            "d" => println!("{}", state.board),
            "eval" => print!("{}", crate::eval::trace(&state.board)),
            "perft" => {
                ensure_search_finished(&mut state);
                let depth: u32 = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                run_perft_command(&state.board, depth);
            }
            "quit" => {
                state.stop_flag.store(true, Ordering::Relaxed);
                break;
            }
            "ponderhit" => {}
            _ => {} // Comandos no reconocidos se ignoran, según el protocolo UCI.
        }
        io::stdout().flush().ok();
    }

    if let Some(handle) = state.search_thread.take() {
        state.stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}

fn handle_position(rest: &str, state: &mut EngineState) {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut idx = 0;

    let mut board = if tokens.first() == Some(&"startpos") {
        idx += 1;
        Board::start_pos()
    } else if tokens.first() == Some(&"fen") {
        idx += 1;
        let fen_start = idx;
        while idx < tokens.len() && tokens[idx] != "moves" {
            idx += 1;
        }
        let fen = tokens[fen_start..idx].join(" ");
        match Board::from_fen(&fen) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("info string FEN inválido ({e}); usando posición inicial");
                Board::start_pos()
            }
        }
    } else {
        Board::start_pos()
    };

    let mut history = vec![board.hash];
    if tokens.get(idx) == Some(&"moves") {
        idx += 1;
        for mv_str in &tokens[idx..] {
            match find_move(&board, mv_str) {
                Some(mv) => {
                    board = board.make_move(mv);
                    history.push(board.hash);
                }
                None => {
                    eprintln!("info string movimiento desconocido o ilegal: {mv_str}");
                    break;
                }
            }
        }
    }

    state.board = board;
    state.game_history = history;
}

fn handle_setoption(line: &str, state: &mut EngineState) {
    // Formato UCI: "setoption name <Nombre> value <Valor>"
    let Some(name_pos) = line.find("name ") else {
        return;
    };
    let rest = &line[name_pos + 5..];
    let Some(value_pos) = rest.find(" value ") else {
        return;
    };
    let name = rest[..value_pos].trim();
    let value = rest[value_pos + 7..].trim();

    if name.eq_ignore_ascii_case("Hash") {
        if let Ok(mb) = value.parse::<usize>() {
            // No hace falta &mut: simplemente apuntamos a una tabla nueva.
            // Cualquier hilo que aún tuviera un Arc a la tabla vieja (no
            // debería, gracias a ensure_search_finished) seguiría viéndola
            // intacta; nadie más la referenciará desde aquí en adelante.
            state.tt = Arc::new(TranspositionTable::new(mb.max(1)));
        }
    } else if name.eq_ignore_ascii_case("Threads") {
        if let Ok(n) = value.parse::<usize>() {
            state.threads = n.clamp(1, 64);
        }
    } else if name.eq_ignore_ascii_case("OwnBook") {
        state.own_book = value.eq_ignore_ascii_case("true");
    } else if name.eq_ignore_ascii_case("BookFile") {
        if value.is_empty() || value == "<empty>" {
            state.book = None;
        } else {
            match Book::load(value) {
                Ok(book) => state.book = Some(book),
                Err(e) => {
                    eprintln!("info string {e}");
                    state.book = None;
                }
            }
        }
    }
}

fn handle_go(line: &str, state: &mut EngineState) {
    let mut it = line.split_whitespace();
    it.next(); // consume "go"
    let tokens: Vec<&str> = it.collect();

    if tokens.first() == Some(&"perft") {
        let depth: u32 = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
        run_perft_command(&state.board, depth);
        return;
    }

    let legal = generate_legal_moves(&state.board);
    if legal.is_empty() {
        println!("bestmove 0000");
        return;
    }

    if state.own_book {
        if let Some(book) = &state.book {
            let candidates = book.probe(&state.board);
            if let Some(mv) = crate::book::choose_weighted(&candidates, &mut state.book_rng) {
                println!(
                    "info string jugada de libro ({} candidata{})",
                    candidates.len(),
                    if candidates.len() == 1 { "" } else { "s" }
                );
                println!("bestmove {mv}");
                return;
            }
        }
    }

    let mut wtime: Option<u64> = None;
    let mut btime: Option<u64> = None;
    let mut winc: Option<u64> = None;
    let mut binc: Option<u64> = None;
    let mut movestogo: Option<u64> = None;
    let mut movetime: Option<u64> = None;
    let mut max_depth: i32 = 64;
    let mut depth_limit: Option<i32> = None;
    let mut infinite = false;
    let mut ponder = false;

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "wtime" => {
                wtime = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "btime" => {
                btime = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "winc" => {
                winc = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "binc" => {
                binc = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "movestogo" => {
                movestogo = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "movetime" => {
                movetime = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            "depth" => {
                depth_limit = tokens.get(i + 1).and_then(|s| s.parse().ok());
                max_depth = depth_limit.unwrap_or(64);
                i += 1;
            }
            "infinite" => infinite = true,
            "ponder" => ponder = true,
            _ => {}
        }
        i += 1;
    }

    let now = Instant::now();
    let allocation = allocate_time(TimeControlInput {
        side_to_move: state.board.side_to_move,
        wtime_ms: wtime,
        btime_ms: btime,
        winc_ms: winc,
        binc_ms: binc,
        movestogo,
        movetime_ms: movetime,
        depth: depth_limit,
        infinite,
        ponder,
    });
    let soft = allocation
        .soft_ms
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(86_400));
    let hard = allocation
        .hard_ms
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(86_400));
    let limits = SearchLimits {
        max_depth,
        soft_deadline: now + soft,
        hard_deadline: now + hard,
    };

    state.stop_flag.store(false, Ordering::Relaxed);
    let stop_flag = Arc::clone(&state.stop_flag);
    let tt = Arc::clone(&state.tt);
    let board = state.board;
    let game_history = state.game_history.clone();
    let threads = state.threads;

    let handle = thread::spawn(move || {
        let (best_move, _score) =
            search::lazy_smp_search(board, limits, tt, game_history, stop_flag, threads);
        println!("bestmove {best_move}");
        io::stdout().flush().ok();
    });
    state.search_thread = Some(handle);
}

fn run_perft_command(board: &Board, depth: u32) {
    let start = std::time::Instant::now();
    let mut total = 0u64;
    for (mv, count) in perft_divide(board, depth) {
        println!("{mv}: {count}");
        total += count;
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!();
    println!("Nodes searched: {total}");
    println!("Time: {elapsed:.3}s");
}
