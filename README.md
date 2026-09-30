<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.11.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **1.11.0** extends the 1.7.0 line through four documented development steps: faster transposition-table and move-selection machinery, expanded handcrafted evaluation, additional pawn/king terms, and piece-square-table tuning by file and rank. Fructosita remains a fully **handcrafted-evaluation (HCE)** engine.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 1.11.0

### What's new since 1.7.0

The historical development path supplied with this version records:

- **1.8.0 — search/TT speed work:** lock-less transposition table, TT prefetch, searching the TT move before full move generation, and lazy move selection.
- **1.9.0 — evaluation expansion:** endgame knowledge, safe per-piece mobility, threats, hanging pieces, knight outposts, and Texel tuning.
- **1.10.0 — king and pawn evaluation:** king shield and pawn storm terms, plus blocked/free passed-pawn evaluation.
- **1.11.0 — PST refinement:** piece-square tables tuned by file and rank.

These features are present in the supplied 1.11.0 source tree.

## Strength evidence

Historical release-preparation material supplied with 1.11.0 reports the following **parent-relative, early-stopped SPRT estimates**:

| Step | Main change | Reported result vs parent |
|---|---|---:|
| 1.8.0 | Lock-less TT, prefetch, TT-first move handling, lazy move selection | **+45.9 ± 27.2 Elo** |
| 1.9.0 | Expanded evaluation and Texel tuning | **+93.5 ± 41.9 Elo** |
| 1.10.0 | King shield/storm and passed-pawn refinements | **+81.4 ± 37.0 Elo** |
| 1.11.0 | File/rank PST tuning | **+91.7 ± 41.2 Elo** |

These figures **must not be added together** and are not an absolute rating measurement. No independent 1.11.0-vs-1.7.0 holdout result is included in the supplied 1.11.0 publication bundle, so this release does not assign a derived absolute Elo rating from these SPRTs.

The CCRL-era 1.3.4 value of roughly **2500 Elo** remains only an informal historical reference and is not used here to claim a formal rating for 1.11.0.

## Historical bench identity

The supplied release-preparation notes identify this version with measured experiment **EXP-0023** and report the deterministic depth-10 bench as:

- nodes: **482,000**
- signature: **`b5c444e86376ebbe`**

This value should be reproduced from the publication tree before the tag is pushed.

## Search

Fructosita 1.11.0 includes:

- Principal Variation Search
- Iterative deepening
- Aspiration windows
- Adaptive null-move pruning
- Logarithmic late-move reductions
- Reverse futility pruning
- Futility pruning
- Late-move pruning
- Internal iterative reductions
- Delta pruning in quiescence
- Static Exchange Evaluation and SEE-based capture pruning
- Killer, history, history-malus and continuation-history heuristics
- Lock-less transposition table with aging and prefetch
- TT probing in quiescence
- TT-refined static evaluation
- TT move validation and search before full move generation
- Lazy move selection

## Evaluation

Fructosita 1.11.0 uses a handcrafted tapered evaluation including:

- Material
- Piece-square tables with file/rank tuning
- Mobility, including safe per-piece mobility
- Pawn structure and passed pawns
- Blocked/free passed-pawn terms
- King safety, king shield and pawn storm
- King-zone attack pressure
- Bishop-pair bonus
- Rook bonuses on open and semi-open files
- Threats and hanging pieces
- Knight outposts
- Endgame-specific knowledge

## Engine

- UCI protocol
- Lazy SMP
- Configurable hash table
- Configurable thread count
- Polyglot opening-book support
- Deterministic time management
- Built-in `perft`
- Built-in `bench`
- EPD testing tools
- Texel tuning tools

## UCI options

Fructosita supports options including:

- `Hash`
- `Threads`
- `OwnBook`
- `BookFile`

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
