<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-2.0.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **2.0.0** is the first major release and consolidates the 1.8.0–1.11.0 development line with additional search, evaluation, speed, time-management, and UCI-robustness work. Fructosita remains a fully **handcrafted-evaluation (HCE)** engine.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 2.0.0

### Main changes since 1.7.0

The historical 2.0.0 release material records:

- lock-less transposition table with 16-byte entries and prefetch;
- TT move searched before full move generation and lazy move selection;
- per-thread static-evaluation cache;
- expanded endgame, mobility, threat, hanging-piece, outpost, king-safety and passed-pawn evaluation;
- piece-square-table tuning by file/rank and per square;
- **Razoring** and **ProbCut**;
- panic-time extension when the score drops between completed iterations;
- UCI robustness for `go infinite`, `go ponder`, `ponderhit`, `go nodes`, and `Clear Hash`.

## Strength evidence

The supplied historical release notes report a direct sealed-holdout comparison:

- **Fructosita 2.0.0 vs 1.7.0: +239.2 ± 23.0 Elo**
- **1,000 games**
- **10+0.1** time control
- sealed holdout openings created before the experiments and opened once for the final test
- **0 crashes, 0 illegal moves, 0 time forfeits**

The same material reports approximately **+30% nodes per second** and **−52% time to depth 12** relative to 1.7.0.

Using the previously documented informal 1.7.0 ballpark of roughly **2645–2650 Elo**, the direct +239.2 result implies an **informal development ballpark of about 2884–2889 Elo** for 2.0.0. This is **not a CCRL rating** and should not be presented as an independently measured absolute rating.

## Search

Fructosita 2.0.0 includes:

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
- Lock-less transposition table with aging and prefetch
- TT probing in quiescence
- TT-refined static evaluation
- TT move validation/search before full move generation
- Lazy move selection
- Per-thread evaluation cache

## Evaluation

Fructosita 2.0.0 uses a handcrafted tapered evaluation including:

- Material
- Piece-square tables with file/rank and per-square tuning
- Safe per-piece mobility
- Pawn structure and passed pawns
- Blocked/free passed-pawn terms
- King safety, king shield and pawn storm
- King-zone attack pressure
- Bishop-pair bonus
- Rook bonuses on open and semi-open files
- Threats and hanging pieces
- Knight outposts
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
