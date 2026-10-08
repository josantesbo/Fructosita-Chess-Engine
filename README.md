<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-2.3.1-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **2.3.1** is an intermediate milestone after the 2.0.0 release. It adds search-side correction history, persistent search statistics between moves, a broader handcrafted king-attack and positional evaluation, and a persistent evaluation cache. Fructosita remains a fully **handcrafted-evaluation (HCE)** engine.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 2.3.1

### Main changes since 2.0.0

The supplied historical release material records:

- **2.1.0 — static-evaluation correction history:** a pawn-structure keyed correction learns the systematic difference between static evaluation and search results, then refines the evaluation used by pruning decisions;
- **2.2.0 — persistent search statistics:** history, continuation history, and correction history are retained between moves instead of being reset at every `go`; `ucinewgame` clears the retained state;
- **2.3.0 — expanded evaluation:** safe checks, undefended king-zone squares, pawn-push threats, hanging pawns, rook behind passed pawn, minor behind pawn, pins, rook-on-queen threats, knight/pawn-count interaction, and joint re-tuning;
- **2.3.1 — persistent evaluation cache:** the per-thread static-evaluation cache is kept between moves as a speed-only change, with identical search according to the historical release notes.

## Strength evidence

The supplied historical release notes report a direct development calibration match:

- **Fructosita 2.3.1 vs 2.0.0: +52.5 ± 16.5 Elo**
- **800 games**
- **10+0.1** time control
- development openings
- calibration match

The same material records these parent-relative development results at 5+0.05:

- 2.1.0 vs parent: **+17.7 ± 13.8 Elo**
- 2.2.0 vs parent: **+72 ± 32 Elo** — early stop
- 2.3.0 vs parent: **+31.6 ± 21.7 Elo**

These parent-relative values must **not** be added together. The direct 2.3.1-vs-2.0.0 calibration match is separate evidence.

No absolute Elo rating is assigned to 2.3.1 by this publication package.

## Historical measured-candidate identity

The supplied release notes identify the measured candidate as **EXP-0051** and report that the 2.3.1 build differs only in its version string. They record a depth-10 bench signature of:

```text
b5c4f767593552bf
```

Publication validation should reproduce this signature before the release is finalized.

## Search

Fructosita 2.3.1 includes:

- Principal Variation Search
- Iterative deepening
- Aspiration windows
- Adaptive null-move pruning
- Logarithmic late-move reductions
- Reverse futility pruning
- Futility pruning
- Late-move pruning
- Internal iterative reductions
- Razoring
- ProbCut
- Delta pruning in quiescence
- Static Exchange Evaluation and SEE-based capture pruning
- Killer, history, history-malus and continuation-history heuristics
- Pawn-structure keyed static-evaluation correction history
- Persistent history, continuation-history and correction-history statistics between moves
- Lock-less transposition table with aging and prefetch
- TT probing in quiescence
- TT-refined static evaluation
- TT move validation/search before full move generation
- Lazy move selection
- Per-thread evaluation cache retained between moves

## Evaluation

Fructosita 2.3.1 uses a handcrafted tapered evaluation including:

- Material
- Piece-square tables with file/rank and per-square tuning
- Safe per-piece mobility
- Pawn structure and passed pawns
- Blocked/free passed-pawn terms
- King safety, king shield and pawn storm
- King-zone attack pressure
- Safe-check opportunities by piece type
- Undefended king-zone squares
- Bishop-pair bonus
- Rook bonuses on open and semi-open files
- Threats and hanging pieces, including hanging pawns
- Pawn-push threats
- Knight outposts
- Rook behind passed pawn
- Minor piece behind own pawn
- Pins against the king
- Rook-on-queen threats
- Knight value interaction with own pawn count
- Endgame-specific knowledge

## Engine and UCI

- UCI protocol
- Lazy SMP
- Configurable hash table
- Configurable thread count
- Polyglot opening-book support
- `go nodes`
- `go infinite` / `go ponder` stop handling
- `ponderhit`
- `Clear Hash`
- Built-in `perft`
- Built-in `bench`
- EPD testing tools
- Texel tuning tools

## Usage

Fructosita is a UCI engine and should normally be loaded into a compatible UCI chess GUI.

It also provides command-line diagnostic tools:

```text
fructosita.exe
fructosita.exe bench
fructosita.exe perft 5
```

## Building from source

A recent stable Rust toolchain is required.

```bash
cargo build --release --locked
```

The executable will be generated in `target/release/`.

## Release integrity

Official release assets include a `SHA256SUMS.txt` file. Verify downloaded binaries before use when integrity matters.

## Security

Fructosita is a local chess engine and is not designed to process untrusted network input. Opening books, EPD/PGN files, and other external files should nevertheless be treated as untrusted input.

See [`SECURITY.md`](SECURITY.md) for reporting and safe-use guidance.

## License

Fructosita is licensed under **GPL-3.0-or-later**. See [`LICENSE`](LICENSE).

© 2026 Antonio Espinosa.
