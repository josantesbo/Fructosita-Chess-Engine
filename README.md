<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.0.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **3.0.0** builds on the search and evaluation work of the 2.x series. Its main additions are singular extensions with multi-cut and SPSA-tuned search parameters. Fructosita remains a **handcrafted-evaluation (HCE)** engine.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 3.0.0

### Main changes since 2.3.1

- **Singular extensions with multi-cut:** search extensions for moves that stand out against the alternatives, with multi-cut logic in the search.
- **SPSA-tuned search parameters:** tuning of selected search settings, including late-move reductions, pruning, and aspiration-window parameters.
- **Configurable search options:** 17 search-related UCI `spin` options, allowing the tuned parameters to be inspected and adjusted.
- **Existing features retained:** correction history, persistent search statistics, the evaluation cache, and the expanded handcrafted king-safety, threat, and positional evaluation introduced in the 2.x series.

These changes follow the historical 3.0.0 release material and the published source. The search additions are part of the engine implementation; they are not separate claims of measured Elo gain.

## Strength evidence

The original 3.0.0 release notes report a direct development comparison:

- **Fructosita 3.0.0 vs 2.0.0: +68.3 ± 15.3 Elo** (reported 95% confidence interval: approximately +53 to +84 Elo)
- **1,000 games**
- **10+0.1** time control
- Reserved (sealed holdout) opening positions

This match was **not rerun** when the historical version was prepared for GitHub. It is a relative result against Fructosita 2.0.0, **not an official CCRL rating** for 3.0.0. Results from separate development tests should not be added together.

### Approximate CCRL Blitz strength (unofficial)

The [CCRL Blitz rating list dated October 17, 2026](https://computerchess.org.uk/404/rating_list_all.html) lists **Fructosita 2.0.0 at 2,921 Elo**. Adding the historical 3.0.0-versus-2.0.0 improvement of 68.3 Elo gives a **rough projection of approximately 2,990 CCRL Blitz Elo for Fructosita 3.0.0**.

This is an illustrative estimate, **not a measured or official rating for 3.0.0**. The historical match used different testing conditions, so its Elo gain may not transfer directly to CCRL Blitz. Independent CCRL testing of 3.0.0 is needed to establish its actual rating.

## Search

Fructosita 3.0.0 includes:

- Principal Variation Search and iterative deepening
- Aspiration windows
- Adaptive null-move pruning
- Late-move reductions and late-move pruning
- Reverse futility pruning, futility pruning, and razoring
- ProbCut and singular extensions with multi-cut
- Internal iterative reductions
- Static Exchange Evaluation and SEE-based capture pruning
- Killer, history, history-malus, and continuation-history heuristics
- Pawn-structure keyed static-evaluation correction history
- Persistent history, continuation-history, and correction-history statistics between moves
- Lock-less transposition table with aging and prefetch
- Transposition-table probing in quiescence
- Lazy move selection and a per-thread evaluation cache retained between moves
- SPSA-tuned search parameters exposed through UCI options

## Evaluation

Fructosita uses handcrafted tapered evaluation, including:

- Material and piece-square tables
- Piece mobility
- Pawn structure, passed pawns, and related positional terms
- King safety, king shield, pawn storms, and king-zone attacks
- Safe checks and undefended king-zone squares
- Bishop pair, rook files, and knight outposts
- Threats, hanging pieces, pawn-push threats, and pins
- Endgame-specific evaluation terms

## Engine and UCI

- UCI protocol
- Lazy SMP
- Configurable hash table and thread count
- Polyglot opening-book support
- `go nodes`, `go infinite`, and `go ponder`
- `ponderhit` and `Clear Hash`
- Built-in `perft` and `bench`
- EPD testing and Texel tuning tools

## Validation

During local publication checks on **October 19, 2026**:

- The release build completed successfully.
- **91 Rust tests passed.**
- UCI initialization and a depth-5 search completed successfully.
- Three reference perft positions matched their expected node counts.
- Historical benchmark signatures and node counts were reproduced:

| Depth | Nodes | Signature |
| --- | ---: | --- |
| 10 | 631,758 | `b5c6d136e9144e36` |
| 12 | 2,128,879 | `b5db3356890cf707` |

These checks confirm the tested build and diagnostic behavior. They do not independently reproduce the historical 1,000-game strength result.

## Usage

Fructosita is a UCI engine and should normally be loaded into a compatible chess GUI.

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
cargo test --release --locked
```

The executable is generated in `target/release/` (`fructosita.exe` on Windows). A local build is not guaranteed to be byte-for-byte identical to the original Windows release binary.

## Downloads and release integrity

The [Fructosita 3.0.0 release](https://github.com/josantesbo/Fructosita-Chess-Engine/releases/tag/v3.0.0) provides the historical Windows x86-64 executable, a source ZIP, release notes, and `SHA256SUMS.txt`.

Verify downloaded release files against the published checksums when integrity matters. See [`PROVENANCE.md`](PROVENANCE.md) for the origins of the historical package and the publication verification record.

## Security

Fructosita is a local chess engine and is not designed to process untrusted network input. Opening books, EPD/PGN files, and other external files should nevertheless be treated as untrusted input.

See [`SECURITY.md`](SECURITY.md) for reporting and safe-use guidance.

## License

Fructosita is licensed under **GPL-3.0-or-later**. See [`LICENSE`](LICENSE).

© 2026 Antonio Espinosa.
