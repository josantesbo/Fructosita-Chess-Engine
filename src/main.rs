//! Fructosita: motor de ajedrez UCI, desarrollado desde cero en Rust.
//!
//! Uso normal (protocolo UCI por stdin/stdout):
//!     fructosita
//!
//! Modos de depuración por línea de comandos (sin UCI):
//!     fructosita perft <profundidad> ["<fen>"]
//!     fructosita perft 5
//!     fructosita perft 4 "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
//!     fructosita selfplay <plies> [movetime_ms] [threads]
//!         Hace que el motor juegue contra sí mismo N medio-movimientos,
//!         verificando en cada paso que el movimiento elegido es legal.
//!         Sirve como prueba de estabilidad de extremo a extremo (hash,
//!         TT, historial de repetición, búsqueda, y desde Lazy SMP,
//!         concurrencia entre hilos) más allá de perft, que solo valida
//!         la generación de movimientos.

mod bitboard;
mod board;
mod book;
mod eval;
mod magic;
mod movegen;
mod moves;
mod perft;
mod search;
mod see;
mod tt;
mod types;
mod uci;
mod zobrist;

use board::Board;
use movegen::generate_legal_moves;
use search::SearchLimits;
use std::env;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tt::TranspositionTable;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "perft" {
        run_cli_perft(&args[2..]);
        return;
    }
    if args.len() > 1 && args[1] == "selfplay" {
        run_selfplay(&args[2..]);
        return;
    }
    if args.len() > 1 && args[1] == "bench-attacks" {
        run_bench_attacks();
        return;
    }

    uci::run();
}

fn run_cli_perft(args: &[String]) {
    let depth: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(5);

    let board = if args.len() > 1 {
        let fen = args[1..].join(" ");
        match Board::from_fen(&fen) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("FEN inválido: {e}");
                std::process::exit(1);
            }
        }
    } else {
        Board::start_pos()
    };

    println!("FEN: {}", board.to_fen());
    let start = std::time::Instant::now();
    let nodes = perft::perft(&board, depth);
    let elapsed = start.elapsed().as_secs_f64();
    let nps = nodes as f64 / elapsed.max(1e-9);
    println!("Perft({depth}) = {nodes}");
    println!("Tiempo: {elapsed:.3}s ({nps:.0} nodos/s)");
}

fn run_selfplay(args: &[String]) {
    let plies: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(40);
    let movetime_ms: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let threads: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let mut board = Board::start_pos();
    let mut history = vec![board.hash];
    let tt = std::sync::Arc::new(TranspositionTable::new(32));
    let stop = std::sync::Arc::new(AtomicBool::new(false));

    println!("Auto-juego: {plies} medios-movimientos, {movetime_ms}ms por jugada, {threads} hilo(s)\n");

    for ply in 1..=plies {
        let legal = generate_legal_moves(&board);
        if legal.is_empty() {
            let result = if board.in_check(board.side_to_move) { "jaque mate" } else { "ahogado" };
            println!("Partida terminada en el medio-movimiento {ply}: {result}");
            break;
        }

        let now = Instant::now();
        let limits = SearchLimits {
            max_depth: 64,
            soft_deadline: now + Duration::from_millis(movetime_ms),
            hard_deadline: now + Duration::from_millis(movetime_ms * 3),
        };
        let (mv, score) = search::lazy_smp_search(board, limits, tt.clone(), history.clone(), stop.clone(), threads);

        if !legal.contains(&mv) {
            panic!("¡BUG CRÍTICO! El motor devolvió un movimiento ilegal: {mv} en posición {}", board.to_fen());
        }

        board = board.make_move(mv);
        history.push(board.hash);
        println!("{ply:>3}. {mv}  (score {score}, fen: {})", board.to_fen());

        if board.halfmove_clock >= 100 {
            println!("Tablas por regla de 50 movimientos en el medio-movimiento {ply}");
            break;
        }
    }

    println!("\nAuto-juego completado sin movimientos ilegales.");
}

/// Micro-benchmark temporal de diagnóstico: compara el cómputo puro de
/// ataques de piezas deslizantes (magic vs. rayos), sin nada más del motor
/// alrededor, usando las mismas tablas reales (`bitboard::tables()`). Solo
/// para investigar dónde se pierde/gana tiempo; no es parte del motor.
fn run_bench_attacks() {
    use bitboard::tables;

    let occupancies: Vec<u64> = {
        let mut v = Vec::new();
        let mut x: u64 = 0x123456789ABCDEF0;
        for _ in 0..2000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            v.push(x & (x >> 1)); // sesgado a disperso, más realista que uniforme
        }
        v
    };

    const ITERS: u32 = 2000;

    let start = Instant::now();
    let mut acc: u64 = 0;
    for _ in 0..ITERS {
        for &occ in &occupancies {
            for sq in 0u8..64 {
                acc ^= tables().rook_attacks(sq, occ);
                acc ^= tables().bishop_attacks(sq, occ);
            }
        }
    }
    let magic_elapsed = start.elapsed();
    let calls = (ITERS as u64) * (occupancies.len() as u64) * 64 * 2;
    println!("magic:  {calls} llamadas en {:.3}s ({:.1}M llamadas/s) [acc={acc}]",
        magic_elapsed.as_secs_f64(), calls as f64 / magic_elapsed.as_secs_f64() / 1e6);

    let start = Instant::now();
    let mut acc: u64 = 0;
    for _ in 0..ITERS {
        for &occ in &occupancies {
            for sq in 0u8..64 {
                acc ^= tables().rook_attacks_ray(sq, occ);
                acc ^= tables().bishop_attacks_ray(sq, occ);
            }
        }
    }
    let ray_elapsed = start.elapsed();
    println!("rayos:  {calls} llamadas en {:.3}s ({:.1}M llamadas/s) [acc={acc}]",
        ray_elapsed.as_secs_f64(), calls as f64 / ray_elapsed.as_secs_f64() / 1e6);

    let ratio = ray_elapsed.as_secs_f64() / magic_elapsed.as_secs_f64();
    println!("magic es {ratio:.2}x respecto a rayos en este micro-benchmark aislado");
}
