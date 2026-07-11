//! Subcomandos de línea de comandos (fuera del protocolo UCI): `perft`,
//! `bench`, `selfplay`, `bench-attacks`, `epd`. Se mantienen separados de
//! `main.rs` (que solo hace el despacho inicial) y del bucle UCI (`uci.rs`)
//! porque no son parte del protocolo del motor: son herramientas de
//! diagnóstico y validación reproducible para desarrollo/CI.

use crate::bitboard::tables;
use crate::board::Board;
use crate::epd;
use crate::movegen::generate_legal_moves;
use crate::perft;
use crate::search::{self, SearchLimits};
use crate::tt::TranspositionTable;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub fn run_bench(args: &[String]) {
    let depth: i32 = if args.first().map(|s| s.as_str()) == Some("depth") {
        args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5)
    } else {
        5
    };
    let hash_mb = 64usize;
    let threads = 1usize;
    let fens = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3",
        "r3k2r/ppp2ppp/2n5/3q4/3P4/2N1BQ2/PPP2PPP/R3K2R w KQkq - 0 12",
        "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        "4k3/6P1/8/8/8/8/8/4K3 w - - 0 1",
        "8/8/8/3k4/8/3K4/4P3/8 w - - 0 1",
    ];

    let start = Instant::now();
    let mut total = search::SearchStatsSnapshot::default();
    let mut signature =
        0x9E37_79B9_7F4A_7C15u64 ^ depth as u64 ^ ((hash_mb as u64) << 32) ^ threads as u64;

    for fen in fens {
        let board = match Board::from_fen(fen) {
            Ok(board) => board,
            Err(err) => {
                eprintln!("FEN inválido en bench: {err}");
                std::process::exit(1);
            }
        };
        let tt = std::sync::Arc::new(TranspositionTable::new(hash_mb));
        let (best_move, score, stats) =
            search::search_fixed_depth_with_stats(board, depth, tt, vec![board.hash]);
        total.nodes += stats.nodes;
        total.qnodes += stats.qnodes;
        total.tt_probes += stats.tt_probes;
        total.tt_hits += stats.tt_hits;
        total.tt_cutoffs += stats.tt_cutoffs;
        total.beta_cutoffs += stats.beta_cutoffs;
        total.beta_cutoffs_first_move += stats.beta_cutoffs_first_move;
        total.null_move_attempts += stats.null_move_attempts;
        total.null_move_cutoffs += stats.null_move_cutoffs;
        total.lmr_attempts += stats.lmr_attempts;
        total.lmr_researches += stats.lmr_researches;
        total.pvs_researches += stats.pvs_researches;
        signature = signature.rotate_left(7)
            ^ board.hash
            ^ ((best_move.from as u64) << 8 | best_move.to as u64)
            ^ ((score as i64 as u64) << 1)
            ^ stats.nodes;
    }

    let time_ms = start.elapsed().as_millis().max(1);
    let nps = (total.nodes as u128 * 1000) / time_ms;
    println!("bench positions {}", fens.len());
    println!("bench depth {depth}");
    println!("bench nodes {}", total.nodes);
    println!("bench time_ms {time_ms}");
    println!("bench nps {nps}");
    println!("bench hash_mb {hash_mb}");
    println!("bench threads {threads}");
    println!("bench qnodes {}", total.qnodes);
    println!("bench tt_probes {}", total.tt_probes);
    println!("bench tt_hits {}", total.tt_hits);
    println!("bench tt_cutoffs {}", total.tt_cutoffs);
    println!("bench beta_cutoffs {}", total.beta_cutoffs);
    println!(
        "bench beta_cutoffs_first_move {}",
        total.beta_cutoffs_first_move
    );
    let move_ordering_pct = if total.beta_cutoffs > 0 {
        (total.beta_cutoffs_first_move as f64 / total.beta_cutoffs as f64) * 100.0
    } else {
        0.0
    };
    println!("bench move_ordering_pct {move_ordering_pct:.2}");
    println!("bench null_move_attempts {}", total.null_move_attempts);
    println!("bench null_move_cutoffs {}", total.null_move_cutoffs);
    println!("bench lmr_attempts {}", total.lmr_attempts);
    println!("bench lmr_researches {}", total.lmr_researches);
    println!("bench pvs_researches {}", total.pvs_researches);
    println!("bench signature {signature:016x}");
}

pub fn run_cli_perft(args: &[String]) {
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

pub fn run_selfplay(args: &[String]) {
    let plies: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(40);
    let movetime_ms: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let threads: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let mut board = Board::start_pos();
    let mut history = vec![board.hash];
    let tt = std::sync::Arc::new(TranspositionTable::new(32));
    let stop = std::sync::Arc::new(AtomicBool::new(false));

    println!(
        "Auto-juego: {plies} medios-movimientos, {movetime_ms}ms por jugada, {threads} hilo(s)\n"
    );

    for ply in 1..=plies {
        let legal = generate_legal_moves(&board);
        if legal.is_empty() {
            let result = if board.in_check(board.side_to_move) {
                "jaque mate"
            } else {
                "ahogado"
            };
            println!("Partida terminada en el medio-movimiento {ply}: {result}");
            break;
        }

        let now = Instant::now();
        let limits = SearchLimits {
            max_depth: 64,
            soft_deadline: now + Duration::from_millis(movetime_ms),
            hard_deadline: now + Duration::from_millis(movetime_ms * 3),
        };
        let (mv, score) = search::lazy_smp_search(
            board,
            limits,
            tt.clone(),
            history.clone(),
            stop.clone(),
            threads,
        );

        if !legal.contains(&mv) {
            panic!(
                "¡BUG CRÍTICO! El motor devolvió un movimiento ilegal: {mv} en posición {}",
                board.to_fen()
            );
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
pub fn run_bench_attacks() {
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
    println!(
        "magic:  {calls} llamadas en {:.3}s ({:.1}M llamadas/s) [acc={acc}]",
        magic_elapsed.as_secs_f64(),
        calls as f64 / magic_elapsed.as_secs_f64() / 1e6
    );

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
    println!(
        "rayos:  {calls} llamadas en {:.3}s ({:.1}M llamadas/s) [acc={acc}]",
        ray_elapsed.as_secs_f64(),
        calls as f64 / ray_elapsed.as_secs_f64() / 1e6
    );

    let ratio = ray_elapsed.as_secs_f64() / magic_elapsed.as_secs_f64();
    println!("magic es {ratio:.2}x respecto a rayos en este micro-benchmark aislado");
}

/// Corre una suite EPD (ver `epd.rs`) a profundidad fija y determinista.
/// Sale con código de proceso 1 si hubo errores de parseo o al menos una
/// posición falló, para que se pueda enganchar directamente en CI.
pub fn run_epd(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("uso: fructosita epd <archivo.epd> [depth <profundidad>]");
        std::process::exit(1);
    };
    let depth: i32 = if args.get(1).map(|s| s.as_str()) == Some("depth") {
        args.get(2).and_then(|s| s.parse().ok()).unwrap_or(6)
    } else {
        6
    };

    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no se pudo leer '{path}': {e}");
            std::process::exit(1);
        }
    };

    let (positions, parse_errors) = epd::parse_epd_file(&contents);
    for err in &parse_errors {
        eprintln!("error de parseo: {err}");
    }
    if positions.is_empty() {
        eprintln!("ninguna posición válida en '{path}'");
        std::process::exit(1);
    }

    println!("epd suite {path}");
    println!("epd depth {depth}");

    let start = Instant::now();
    let summary = epd::run_suite(&positions, depth);
    let elapsed = start.elapsed().as_secs_f64();

    let mut total_nodes = 0u64;
    for outcome in &summary.outcomes {
        let status = if outcome.passed { "PASS" } else { "FAIL" };
        let name = outcome.id.as_deref().unwrap_or("<sin id>");
        total_nodes += outcome.nodes;
        println!(
            "[{status}] {name}: jugada {} (nodos {}) — {}",
            outcome.chosen, outcome.nodes, outcome.fen
        );
    }

    println!(
        "epd result {}/{} ({:.1}%) en {:.2}s, {total_nodes} nodos totales",
        summary.passed,
        summary.total,
        100.0 * summary.passed as f64 / summary.total as f64,
        elapsed
    );

    if !parse_errors.is_empty() || summary.passed < summary.total {
        std::process::exit(1);
    }
}
