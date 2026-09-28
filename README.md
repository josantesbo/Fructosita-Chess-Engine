<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.7.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **1.7.0** extends the search-focused 1.5.0 line with evaluation improvements introduced in the intermediate 1.6.0 step and additional search refinements in 1.7.0. Fructosita remains a fully **handcrafted-evaluation (HCE)** engine.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 1.7.0

### What's new since 1.5.0

The development path to 1.7.0 introduced:

- **King-zone attack evaluation**, rewarding coordinated pressure around the enemy king.
- **Bishop-pair evaluation**.
- **Rook bonuses on open and semi-open files**.
- **History malus** for previously searched quiet moves that fail to produce a cutoff.
- **Transposition-table probing in quiescence search**.
- **SEE pruning of sufficiently losing captures**.
- **TT-refined static evaluation** in the main search.

The earlier 1.5.0 search improvements remain part of this release, including logarithmic LMR, adaptive null-move pruning, aspiration windows, time-management iteration control, and transposition-table aging.

## Strength evidence

The public CCRL-era **1.3.4** release is used here only as an informal reference of approximately **2500 Elo**.

The historical release-preparation material supplied for 1.7.0 reports these parent-relative development results:

| Step | Main change | Reported result vs parent |
|---|---|---:|
| 1.4.0 | Time-management iteration guard | **+32.8 ± 23.4 Elo** |
| 1.5.0 | Logarithmic LMR, adaptive null move, aspiration windows, TT aging | **+52.2 ± 32.6 Elo** |
| 1.6.0 | King-zone attack evaluation, bishop pair, rook file bonuses | **+25.6 ± 19.5 Elo** |
| 1.7.0 | History malus, qsearch TT probe, SEE pruning, TT-refined static eval | **+81.9 ± 36.8 Elo** |

The same historical material also reports a separate **1,400-game self-play comparison of 1.7.0 against the 1.3.4 code line**, measuring **+146.3 Elo** with a reported **95% confidence interval of +131.0 to +162.1 Elo**. The supplied publication bundle does not include the original match log or PGN, so this figure is preserved as a historically reported development result rather than independently re-audited match evidence.

Using the informal ~2500 reference for 1.3.4 together with that direct self-play gap suggests a rough development ballpark around **2645–2650 Elo**. **This is not a CCRL rating and should not be presented as one.** Self-play gaps also do not necessarily transfer directly to independent rating lists.

## Search

Fructosita 1.7.0 includes:

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
- Static Exchange Evaluation
- SEE-based capture pruning
- Killer heuristic
- History heuristic and history malus
- One-ply continuation history
- Transposition-table aging
- Transposition-table probing in quiescence
- TT-refined static evaluation

## Evaluation

Fructosita 1.7.0 uses a handcrafted tapered evaluation including:

- Material
- Piece-square tables
- Per-rank corrections
- Mobility
- Pawn structure
- Passed pawns
- King safety
- King-zone attack pressure
- Bishop-pair bonus
- Rook bonuses on open and semi-open files

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
