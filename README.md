<p align="center">
  <img src="assets/Fructosita.png" width="360" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Modern alpha-beta search · Every change measured
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.3.0-orange" alt="version">
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

## Fructosita 3.3.0

Fructosita 3.3.0 evaluates faster with identical results, retunes 30 search parameters and ships a statically linked executable that no longer needs the Visual C++ runtime.

### What's new since 3.2.0

- Faster evaluation, same results: terms shared by several evaluation features are computed once per position, and everything that depends only on pawns and kings (doubled, isolated and passed pawns, king shield, open files near the king) lives in its own cache. The board keeps occupancy, material and piece-square sums up to date incrementally. Same search tree as 3.2.0 with about 13% less middlegame time.
- Retuned search: 30 parameters tuned together with games (SPSA), 13 of them never tuned before (depth limits of near-leaf pruning, history-reduction thresholds, time allocation). The most visible changes: futility and move-count pruning reach one ply deeper, the base late-move reduction is smaller and move-count pruning starts a little earlier.
- Fix: never two null moves in a row (the second one was scored as a repetition draw without searching).
- Build: profile-guided optimisation (about 6% faster with the same code) and the C runtime linked into the executable, so the Visual C++ redistributable is no longer needed.

### Evidence

- Sealed holdout vs 3.2.0: **+44.0 ± 13.8 Elo**, 1,000 games at 10+0.1, no early stop (+324 −198 =478). No time forfeits or illegal moves.
- During development (SPRT at 5+0.05): speed changes +23.8 ± 12.9 Elo in 1,096 games; retuned search +11.3 ± 8.3 Elo in 3,118 games.
- Gauntlet at 10+0.1 against Cheese 3.3.0, Critter 1.6a and Equinox 3.30, 228 games each, anchored to their CCRL Blitz ratings of 2026-09-26: 3.2.0 about 2975 (95% CI 2929 to 3017), 3.3.0 about 3041 (95% CI 2998 to 3083). Short time control on my own hardware, so this is an estimate and **not a CCRL rating**.

### Known limitations

- With a "moves to go" time control and two moves left, the engine may spend almost the entire clock on the penultimate move.

## Strength

| Version | Measured against | Result | Conditions |
|---|---|---|---|
| 1.3.4 |  | Submitted to **CCRL** |  |
| 1.7.0 | 1.3.4 | **+146 Elo** (95% CI +131 to +162) | 1,400 fresh games |
| 2.0.0 | 1.7.0 | **+239 ± 23 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.0.0 | 2.0.0 | **+68.3 ± 15.3 Elo** (95% CI about +53 to +84) | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.1.0 | 3.0.0 | **+21.6 ± 14.4 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.2.0 | 3.1.0 | **+62.5 ± 13.9 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.3.0 | 3.2.0 | **+44.0 ± 13.8 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |

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
- Pawn and king terms kept in their own cache; occupancy, material and piece-square sums updated incrementally

**Engine**
- UCI: `Hash`, `Threads`, `OwnBook`, `BookFile`, `Clear Hash`, `go infinite/ponder/nodes/depth/movetime`
- Polyglot opening book support
- Built-in tools: `perft`, `bench`, EPD test harness, Texel tuner
- Release executable built with profile-guided optimisation and the C runtime linked in (no Visual C++ redistributable needed)

## Usage

Fructosita is a UCI engine: load the executable in any UCI GUI (Arena, Cute Chess, Banksia GUI, ChessBase, ...).
It identifies itself as `id name Fructosita 3.3.0`.

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

`fructosita.exe bench depth 12` ends with `bench signature b5eed34762ad6df9`.

## Building

```
cargo build --release --locked
cargo test --release --locked
```

The binary is written to `target/release/fructosita(.exe)`. Requires a recent stable Rust toolchain.

`.cargo/config.toml` enables POPCNT and links the C runtime statically. The executable attached to the release is built with profile-guided optimisation; the profile (`fructosita-3.3.0.profdata`) is attached too. A plain `cargo build` plays exactly the same moves, a few percent slower.

## Requirements

Windows x64 and a CPU with POPCNT (a processor that supports the POPCNT instruction). On a CPU without POPCNT the engine prints a message and exits.

## Version history

See the **Releases** page for binaries, source archives and SHA-256 checksums of every published version, and
`CHANGELOG.md` for the complete list of internal steps and their measurements.

## Security

Fructosita is a local engine and does not process network input. Opening books, EPD and PGN files from unknown
sources should still be treated as untrusted. See [`SECURITY.md`](SECURITY.md).

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE). © 2026 Antonio Espinosa.
