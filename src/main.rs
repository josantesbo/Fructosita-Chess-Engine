//! Fructosita: motor de ajedrez UCI, desarrollado desde cero en Rust.
//!
//! Uso normal (protocolo UCI por stdin/stdout):
//!     fructosita
//!
//! Modos de depuración por línea de comandos (sin UCI), implementados en
//! `commands.rs`:
//!     fructosita perft <profundidad> ["<fen>"]
//!     fructosita perft 5
//!     fructosita perft 4 "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
//!     fructosita bench [depth <profundidad>]
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
mod commands;
mod eval;
mod magic;
mod magic_constants;
mod movegen;
mod moves;
mod perft;
mod polyglot_random;
mod search;
mod see;
mod time_management;
mod tt;
mod types;
mod uci;
mod zobrist;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "perft" {
        commands::run_cli_perft(&args[2..]);
        return;
    }
    if args.len() > 1 && args[1] == "bench" {
        commands::run_bench(&args[2..]);
        return;
    }
    if args.len() > 1 && args[1] == "selfplay" {
        commands::run_selfplay(&args[2..]);
        return;
    }
    if args.len() > 1 && args[1] == "bench-attacks" {
        commands::run_bench_attacks();
        return;
    }

    uci::run();
}
