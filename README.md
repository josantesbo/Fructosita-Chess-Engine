<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.5.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **1.5.0** is a search-focused milestone built on the CCRL-era 1.3.4 line. It keeps Fructosita as a fully **handcrafted-evaluation (HCE)** engine while substantially modernizing search behavior and time usage.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 1.5.0

### What's new since 1.3.4

The 1.5.0 line introduces two accepted development steps:

- **Time-management iteration guard**: the engine avoids starting a new iteration once more than 60% of the soft time budget has been consumed.
- **Logarithmic late-move reductions (LMR)**.
- **Adaptive null-move pruning**, conditioned on static evaluation and adjusted by depth.
- **Aspiration windows** around the previous iteration score.
- **Transposition-table aging**.

These changes form the modernized search core used by this release.

## Strength evidence

The public CCRL-era **1.3.4** release is being used here as an informal reference of approximately **2500 Elo**.

Development SPRTs for the steps leading to 1.5.0 reported:

| Step | Change | SPRT vs parent |
|---|---|---:|
| 1.4.0 | Time-management iteration guard | **+32.8 ± 23.4 Elo** |
| 1.5.0 | Logarithmic LMR, adaptive null move, aspiration windows, TT aging | **+52.2 ± 32.6 Elo** |

If those parent-relative figures are chained arithmetically onto the informal 2500 reference, they suggest a rough development ballpark near **2585 Elo**. **This is not a CCRL rating and not a direct 1.5.0-vs-1.3.4 measurement.** Sequential SPRT estimates are noisy and should not be treated as perfectly additive; the table is included as development evidence, not as a formal absolute rating.

## Verification

The release-preparation source was derived from the measured **EXP-0005** candidate. The deterministic depth-10 bench for the measured candidate was:

- nodes: **735,391**
- signature: **`b5c623535b1fc8c7`**

Publication-only documentation and canonical repository metadata do not intentionally change engine logic.

### Search

Fructosita 1.5.0 includes:

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
- Static Exchange Evaluation for capture ordering
- Killer heuristic
- History heuristic
- One-ply continuation history
- Transposition-table aging

### Evaluation

Fructosita 1.5.0 retains a handcrafted, tapered evaluation including:

- Material
- Piece-square tables
- Per-rank corrections
- Mobility
- Pawn structure
- Passed pawns
- Basic king safety

### Engine

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
