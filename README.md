# Fructosita 🍬♟️

[![CI](https://github.com/josantesbo/Fructosita-Chess-Engine/actions/workflows/ci.yml/badge.svg)](https://github.com/josantesbo/Fructosita-Chess-Engine/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)
![License](https://img.shields.io/badge/License-GPL--3.0-blue)
![UCI](https://img.shields.io/badge/Protocol-UCI-success)
![Status](https://img.shields.io/badge/Status-Active-green)

**Fructosita** is an open-source UCI chess engine written entirely from scratch in **Rust** — no code cloned or derived from any other engine.

The project began on **July 2, 2026** as a personal initiative to learn chess engine development from scratch. The name *Fructosita* was inspired by the author's undergraduate thesis on **fructose**, eventually becoming the engine's permanent identity.

The long-term goal is to evolve Fructosita from a classical handcrafted engine into a modern competitive chess engine featuring advanced search techniques and, eventually, an NNUE evaluation — with sights set on genuine competitiveness in the CCRL and TCEC community.

---

# Features

### Search

- Alpha-Beta Negamax
- Iterative Deepening
- Quiescence Search
- Principal Variation Search (PVS)
- Null Move Pruning
- Late Move Reductions (LMR)
- Check Extensions
- Killer Moves & History Heuristic
- Lazy SMP (multi-threaded search, shared transposition table)
- Aspiration Windows (planned)

### Move Generation

- Bitboards
- Legal move generation (validated against public perft reference values)
- Incremental Zobrist hashing
- Static Exchange Evaluation (SEE), including x-ray attacks and promotion handling
- Magic Bitboards (bishops & rooks), self-generated and independently verified

### Position Evaluation

- Material evaluation
- Tapered Piece-Square Tables (middlegame/endgame)
- Mobility
- King safety
- Pawn structure (doubled, isolated, passed pawns)
- Verified color-symmetric (no hidden bias toward either side)

### Opening

- Polyglot opening book support (`.bin`), verified against `python-chess`

### Engine

- UCI compatible, including `Hash`, `Threads`, `OwnBook`, and `BookFile` options
- Transposition table (thread-safe, shared across search threads)
- Multi-platform builds (Windows / Linux / macOS)
- Continuous Integration via GitHub Actions

---

# What's New in v1.2.0

- ✅ **Lazy SMP**: multiple search threads share one transposition table (`setoption name Threads value N`) for real multi-core scaling.
- ✅ **Fixed a real color-symmetry bug**: the tempo bonus was applied before converting to the side-to-move's perspective, turning into a penalty for Black instead of a bonus. Caught by a dedicated mirror-position test (`evaluation_is_color_symmetric`) and now permanently guarded by it.
- ✅ 52 automated tests passing (up from 48), including a concurrency stress test (8 threads, 160,000 operations on a shared table with no corruption).

## What's New in v1.1.0

- ✅ Magic Bitboards for bishops and rooks.
- ✅ Reproducible magic number generator (`src/bin/find_magics.rs`), included so anyone can regenerate and verify the numbers independently.
- ✅ Verified against the previous ray-based implementation: exhaustive check across every occupancy subset per square, plus 20,000 random full-board occupancies per square.
- ✅ Perft results identical to the previous version.

---

# Performance

Current engine status:

| Feature | Status |
|---------|--------|
| UCI Protocol | ✅ |
| Legal Move Generation | ✅ |
| Zobrist Hashing | ✅ |
| Polyglot Book | ✅ |
| SEE | ✅ |
| Magic Bitboards | ✅ |
| Lazy SMP | ✅ |
| Automated Tests | **52 / 52 Passed** |
| Perft Validation | ✅ |

**Estimated strength:** development estimate only, not yet confirmed by
independent testing — internal self-play and games against known engines
(Krevete, ChessGM, Teki 2, all rated ~2330-2500) suggest a range in the
low-to-mid 2000s. Formal CCRL testing is the next step to get a real number.

---

# Architecture

```
                 UCI
                  │
                  ▼
             Search Engine ◄──── Lazy SMP (N threads)
                  │
      ┌───────────┼───────────────┐
      ▼           ▼               ▼
 Position    Move Generation   Opening Book
 Evaluation  (Magic Bitboards)  (Polyglot)
      │           │
      ▼           ▼
 Piece-Square  Static Exchange
   Tables      Evaluation (SEE)
      │           │
      └─────┬─────┘
            ▼
      Bitboard Core
            │
            ▼
     Zobrist Hashing
            │
            ▼
  Transposition Table
   (shared, thread-safe)
```

---

# Testing

Fructosita includes an extensive automated test suite covering:

- Move generation (validated against public perft reference values)
- Magic Bitboards (exhaustive + randomized validation against the reference ray-based implementation)
- Zobrist hashing (incremental updates checked against from-scratch recomputation)
- Polyglot support (validated against a book generated independently with `python-chess`)
- Static Exchange Evaluation (including x-ray attacks, en passant, and promotion edge cases)
- Evaluation color symmetry (mirror-position invariant)
- Search correctness (forced mates, avoiding bad trades)
- Concurrency (shared transposition table under multi-threaded stress)
- Perft validation

Current status:

```
52 tests passed
0 failed
```

---

# Building

Clone the repository:

```bash
git clone https://github.com/josantesbo/Fructosita-Chess-Engine.git
cd Fructosita-Chess-Engine
```

Compile:

```bash
cargo build --release
```

Run (UCI mode over stdin/stdout):

```bash
./target/release/fructosita
```

To use multiple threads or your own opening book, set these via UCI:

```
setoption name Threads value 6
setoption name BookFile value /path/to/your/book.bin
```

---

# Roadmap

## Search

- [x] Alpha-Beta
- [x] Iterative Deepening
- [x] Quiescence Search
- [x] Transposition Table
- [x] Principal Variation Search
- [x] Null Move Pruning
- [x] Late Move Reductions
- [x] History Heuristic
- [x] Killer Moves
- [x] Lazy SMP
- [ ] Aspiration Windows
- [ ] MultiPV
- [ ] Singular Extensions / ProbCut

---

## Evaluation

- [x] Material
- [x] Piece-Square Tables (tapered)
- [x] Mobility
- [x] King Safety
- [x] Pawn Structure (doubled, isolated, passed)
- [ ] Space Evaluation
- [ ] Texel Tuning
- [ ] NNUE

---

## Future

- [ ] Syzygy Tablebases
- [ ] Refined Time Management (PV stability, score trend)
- [ ] CCRL Testing
- [ ] OpenBench Testing
- [ ] SPRT Regression Testing
- [ ] TCEC qualification (long-term goal)

---

# Why "Fructosita"?

The engine's name originated during the author's undergraduate research on **fructose**.

Originally intended as a temporary codename, it eventually became the official identity of the project.

---

# Releases

Every tagged release automatically generates binaries for:

- Windows
- Linux
- macOS

using GitHub Actions.

---

# License

This project is released under the **GNU General Public License v3.0** (GPL-3.0) — see [`LICENSE`](LICENSE) for the full text. This is the same license used by Stockfish, Lc0, and the large majority of engines in the CCRL/TCEC community: anyone can use, study, and modify Fructosita freely, provided that distributed modified versions also share their source code under the same license.

---

# Acknowledgements

Fructosita is developed as a personal learning project exploring modern chess engine design, in close collaboration with Claude (Anthropic).

The project draws inspiration from the broader computer chess community, open research (Chess Programming Wiki), and the public, standardized formats it interoperates with (Polyglot opening books, verified against `python-chess`). All search, evaluation, and magic bitboard code is original — no source code was copied from any other engine.

---

*"Every strong chess engine started with a legal move generator."* ♟️
