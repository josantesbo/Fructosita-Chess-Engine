<p align="center">
  <img src="assets/Fructosita.png" width="360" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Modern alpha-beta search · Every change measured
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.2.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an open-source chess engine written entirely from scratch in **Rust**. No code, tables or weights were copied from any other engine: every idea comes from public literature (the Chess Programming Wiki, research papers) and was implemented, tuned and measured specifically for Fructosita.

The project follows one rule, **learn from everyone, copy from no one**, and a second one: **no change is kept without evidence**. Every improvement is tested against its parent, and every version that claims more strength is checked on openings that were never used while developing it.

## The author

Fructosita is created by **Antonio Espinosa**, a **chemist and pharmacobiologist** (*Químico Farmacobiólogo*).
The project started on **July 2, 2026** as a personal challenge: to learn, from zero, how a chess engine thinks.

The name comes from the author's undergraduate thesis on **fructose**. That little molecule became the engine's
identity and its logo: a king merging into a hexagonal ring, where chess meets chemistry.

> *Fructosita nació como un reto personal de un químico farmacobiólogo: aprender desde cero cómo piensa un motor
> de ajedrez. El nombre viene de mi tesis de licenciatura sobre la fructosa.* (Antonio Espinosa)

## Fructosita 3.2.0

Fructosita 3.2.0 runs about 1.3 times as many nodes per second with the same tree, adds three small search refinements and now needs a CPU with POPCNT.

### What's new since 3.1.0

- Speed with an identical tree (about 23% less time per position): a move is made only after it survives pruning, check squares are computed once per node, the move picker sorts once, the evaluation counts bits with the POPCNT instruction and uses pre-summed tables, and SEE updates the attackers instead of recomputing them.
- Search: the transposition table keeps a position's good move when the new entry is only an upper bound; late-move reductions take the move's history into account; the engine allows one more iteration when the best move has just changed.
- New requirement: a CPU with POPCNT (any x86-64 since 2008). On an older CPU the engine prints a message and exits.

### Evidence

- Sealed holdout vs 3.1.0: **+62.5 ± 13.9 Elo** (95% CI about +48.6 to +76.5), 1,000 games at 10+0.1 (+350 −172 =478). No time forfeits, illegal moves or errors.
- During development, at 5+0.05 with one thread and 64 MB hash: the speed changes passed SPRT with +49.9 ± 19.2 Elo in 526 games, the search changes with +14.7 ± 9.8 Elo in 2,084 games.

## Strength

| Version | Measured against | Result | Conditions |
|---|---|---|---|
| 1.3.4 |  | Submitted to **CCRL** |  |
| 1.7.0 | 1.3.4 | **+146 Elo** (95% CI +131 to +162) | 1,400 fresh games |
| 2.0.0 | 1.7.0 | **+239 ± 23 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.0.0 | 2.0.0 | **+68.3 ± 15.3 Elo** (95% CI about +53 to +84) | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.1.0 | 3.0.0 | **+21.6 ± 14.4 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.2.0 | 3.1.0 | **+62.5 ± 13.9 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |

Self-play Elo overstates the gap seen in rating lists; independent ratings (CCRL) are the reference.

## Features

**Search**
- Principal Variation Search, iterative deepening, aspiration windows
- Transposition table: lock-less, 16-byte entries, prefetch; TT move tried before move generation; a good move is kept when only an upper bound is stored
- Null-move pruning (adaptive), reverse futility, futility, razoring, ProbCut, late-move pruning, SEE pruning
- Logarithmic late-move reductions (also guided by the move's history), internal iterative reductions, check and **singular extensions**
- History and continuation history, killers, **static-evaluation correction history**
- Search statistics **persist between moves** (history, correction and evaluation caches)
- Lazy SMP, time management with panic time on score drops

**Evaluation (handcrafted, Texel-tuned)**
- Tapered material and piece-square tables (formula plus tuned per-file, per-rank and per-square corrections)
- Safe mobility per piece, threats, hanging pieces and pawns, pawn-push threats, pins
- King safety: attack units, king shield, **safe checks**, weak king-zone squares
- Pawn structure: doubled, isolated, passed pawns (blocked or free, king race, rook behind)
- Outposts, bishop pair, rooks on open files, endgame scaling (insufficient material, opposite bishops, mop-up)

**Engine**
- UCI: `Hash`, `Threads`, `OwnBook`, `BookFile`, `Clear Hash`, `go infinite/ponder/nodes/depth/movetime`
- Polyglot opening book support
- Built-in tools: `perft`, `bench`, EPD test harness, Texel tuner

## Usage

Fructosita is a UCI engine: load the executable in any UCI GUI (Arena, Cute Chess, Banksia GUI, ChessBase, ...).
It identifies itself as `id name Fructosita 3.2.0`.

| Option | Default | Notes |
|---|---|---|
| `Hash` | 64 | MB |
| `Threads` | 1 | Lazy SMP |
| `OwnBook` | true | Only used when `BookFile` is set (Polyglot book); no file, no book |
| `BookFile` | empty | Path of a Polyglot book |
| `Clear Hash` |  | Button |

Other options (`RfpMargin`, `LmrBase`, ...) are internal search parameters exposed for testing and tuning; leave them at their defaults.

```
fructosita.exe              # UCI mode
fructosita.exe bench        # deterministic benchmark (node count and signature)
fructosita.exe perft 5      # move-generator self-check
```

`fructosita.exe bench depth 12` ends with `bench signature b5d576af5da807d9`.

## Building

```
cargo build --release --locked
cargo test --release --locked
```

The binary is written to `target/release/fructosita(.exe)`. Requires a recent stable Rust toolchain.

`.cargo/config.toml` enables POPCNT.

## Requirements

Windows x64 and a CPU with POPCNT (any x86-64 since 2008). On a CPU without POPCNT the engine prints a message and exits.

## Version history

See the **Releases** page for binaries, source archives and SHA-256 checksums of every published version, and
`CHANGELOG.md` for the complete list of internal steps and their measurements.

## Security

Fructosita is a local engine and does not process network input. Opening books, EPD and PGN files from unknown
sources should still be treated as untrusted. See [`SECURITY.md`](SECURITY.md).

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE). © 2026 Antonio Espinosa.
