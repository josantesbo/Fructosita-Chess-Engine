use crate::board::Board;
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
        total.ch_quiets_scored += stats.ch_quiets_scored;
        total.ch_nonzero_entries += stats.ch_nonzero_entries;
        total.ch_quiets_classified += stats.ch_quiets_classified;
        total.ch_tier_changes += stats.ch_tier_changes;
        total.lmp_prunes += stats.lmp_prunes;
        total.lmp_prunes_futility_silent += stats.lmp_prunes_futility_silent;
        total.lmp_prunes_beyond_lmr += stats.lmp_prunes_beyond_lmr;
        signature = signature.rotate_left(7)
            ^ board.hash
            ^ ((best_move.from as u64) << 8 | best_move.to as u64)
            ^ ((score as i64 as u64) << 1)
            ^ stats.nodes;
    }

    let time_ms = start.elapsed().as_millis().max(1);
    let nps = (total.nodes as u128 * 1000) / time_ms;
    let move_ordering_pct = if total.beta_cutoffs == 0 {
        0.0
    } else {
        100.0 * total.beta_cutoffs_first_move as f64 / total.beta_cutoffs as f64
    };
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
    println!("bench move_ordering_pct {move_ordering_pct:.2}");
    println!("bench null_move_attempts {}", total.null_move_attempts);
    println!("bench null_move_cutoffs {}", total.null_move_cutoffs);
    println!("bench lmr_attempts {}", total.lmr_attempts);
    println!("bench lmr_researches {}", total.lmr_researches);
    println!("bench pvs_researches {}", total.pvs_researches);
    println!("ch sum_history {}", total.ch_quiets_scored);
    println!("ch sum_conthist {}", total.ch_nonzero_entries);
    println!("ch quiets_classified {}", total.ch_quiets_classified);
    println!("ch tier_changes {}", total.ch_tier_changes);
    let tcp = if total.ch_quiets_classified == 0 {
        0.0
    } else {
        100.0 * total.ch_tier_changes as f64 / total.ch_quiets_classified as f64
    };
    println!("ch tier_change_pct {tcp:.2}");
    println!("bench lmp_prunes {}", total.lmp_prunes);
    println!(
        "bench lmp_prunes_futility_silent {}",
        total.lmp_prunes_futility_silent
    );
    println!("bench lmp_prunes_beyond_lmr {}", total.lmp_prunes_beyond_lmr);
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
            max_nodes: None,
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
    use crate::bitboard::tables;

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

pub fn run_epd(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("uso: fructosita epd <archivo.epd> [depth N]");
        std::process::exit(1);
    };
    let depth = if args.get(1).map(String::as_str) == Some("depth") {
        args.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(8)
    } else {
        8
    };
    println!("epd file {path}");
    println!("epd depth {depth}");
    match crate::epd::run_file(path, depth) {
        Ok(summary) => {
            println!("epd positions {}", summary.positions);
            println!("epd passed {}", summary.passed);
            println!("epd failed {}", summary.failed);
            if summary.failed > 0 {
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("epd error: {err}");
            std::process::exit(1);
        }
    }
}

/// Extrae posiciones de un PGN para el afinado Texel (ver `texel.rs`).
/// El dataset (una línea `fen;resultado;eval_blancas` por posición) sale por
/// stdout para poder redirigirlo; el resumen va por stderr.
pub fn run_texel_extract(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("uso: fructosita texel-extract <archivo.pgn> [skip N] [stride S]");
        std::process::exit(1);
    };
    let mut opts = crate::texel::ExtractOptions {
        skip_opening_plies: 8,
        stride: 1,
    };
    let mut i = 1;
    while i + 1 < args.len() + 1 {
        match (args.get(i).map(String::as_str), args.get(i + 1)) {
            (Some("skip"), Some(n)) => {
                opts.skip_opening_plies = n.parse().unwrap_or(opts.skip_opening_plies);
                i += 2;
            }
            (Some("stride"), Some(n)) => {
                opts.stride = n.parse::<usize>().unwrap_or(opts.stride).max(1);
                i += 2;
            }
            _ => break,
        }
    }
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("no se pudo leer {path}: {err}");
            std::process::exit(1);
        }
    };
    let summary = crate::texel::extract(&contents, &opts);
    eprintln!("texel games {}", summary.games);
    eprintln!("texel games_failed {}", summary.games_failed);
    eprintln!("texel positions {}", summary.positions);
    eprintln!(
        "texel opts skip={} stride={}",
        opts.skip_opening_plies, opts.stride
    );
}

/// Afinado Texel: carga el dataset, ajusta K con los pesos actuales, y corre
/// el descenso por coordenadas. Imprime el progreso por stderr y los pesos
/// finales por stdout.
pub fn run_texel_tune(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("uso: fructosita texel-tune <dataset.csv>");
        std::process::exit(1);
    };
    let dataset = match crate::texel::load_dataset(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("texel-tune: {e}");
            std::process::exit(1);
        }
    };
    let start = crate::eval::EvalParams::default();
    eprintln!("posiciones {}", dataset.entries.len());
    let k = crate::texel::fit_k(&dataset, &start);
    eprintln!("K {k:.4} (fijada durante el afinado)");
    let opt = |name: &str| -> Option<f64> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
    };
    let from = opt("only_from").map(|x| x as usize).unwrap_or(0);
    let steps: Vec<i32> = args
        .iter()
        .position(|a| a == "steps")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_else(|| vec![8, 4, 2, 1]);
    eprintln!("only_from {from} steps {steps:?}");
    let (tuned, e0, e1) = crate::texel::tune_range(&dataset, &start, k, from, &steps);
    println!("K {k:.4}");
    println!("E inicial {e0:.6}");
    println!("E final   {e1:.6}");
    println!("pesos afinados:");
    println!("{tuned:#?}");
}

/// Calcula el error Texel de los pesos por defecto de ESTE binario sobre un
/// dataset, con una K dada. Sirve para evaluar unos pesos sobre un conjunto
/// de posiciones que no participó en su afinado (validación train/test).
pub fn run_texel_error(args: &[String]) {
    let (Some(path), Some(kflag), Some(kval)) = (args.first(), args.get(1), args.get(2)) else {
        eprintln!("uso: fructosita texel-error <dataset.csv> k <K>");
        std::process::exit(1);
    };
    if kflag != "k" {
        eprintln!("uso: fructosita texel-error <dataset.csv> k <K>");
        std::process::exit(1);
    }
    let k: f64 = kval.parse().unwrap_or_else(|_| {
        eprintln!("K inválida: {kval}");
        std::process::exit(1);
    });
    let dataset = match crate::texel::load_dataset(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("texel-error: {e}");
            std::process::exit(1);
        }
    };
    let params = crate::eval::EvalParams::default();
    let e = crate::texel::error(&dataset, &params, k);
    println!("posiciones {}", dataset.entries.len());
    println!("K {k:.4}");
    println!("E {e:.6}");
}
