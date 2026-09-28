<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Alpha-beta search · UCI
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.3.4-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an original UCI chess engine written from scratch in **Rust**.

Version **1.3.4** is the version submitted to the **CCRL** rating lists. Internally, this engine was developed as version 1.3.3; version 1.3.4 is the public CCRL release identity of the same engine implementation.

Fructosita uses a fully **handcrafted evaluation (HCE)** and a classical alpha-beta search architecture.

The project follows a simple principle:

> **Learn from everyone. Copy from no one.**

Fructosita is independently implemented. Its design is informed by public chess-programming literature and research, while engine-specific source code, tuned tables, and parameter sets are not copied from other engines.

## Author

Fructosita is developed by **Antonio Espinosa**, a *Químico Farmacobiólogo* (chemist–pharmacobiologist).

The project began on July 2, 2026 as a personal challenge to learn from scratch how a chess engine works. The name **Fructosita** comes from the author's undergraduate thesis work involving fructose.

## Fructosita 1.3.4

### Search

- Principal Variation Search
- Iterative deepening
- Null-move pruning
- Late-move reductions
- Reverse futility pruning
- Futility pruning
- Late-move pruning
- Internal iterative reductions
- Delta pruning in quiescence
- Static Exchange Evaluation for capture ordering
- Killer heuristic
- History heuristic
- One-ply continuation history

### Evaluation

Fructosita 1.3.4 uses a handcrafted, tapered evaluation including:

- Material
- Piece-square tables
- Per-rank corrections
- Mobility
- Pawn structure
- Passed pawns
- Basic king safety

Evaluation parameters were tuned specifically for Fructosita.

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

The historical v1.3.4 Windows binary is distributed as a release asset; the repository tag contains the corresponding engine source plus publication documentation and metadata.

## Security

Fructosita is a local chess engine and is not designed to process untrusted network input. Opening books, EPD/PGN files, and other external files should nevertheless be treated as untrusted input.

See [`SECURITY.md`](SECURITY.md) for reporting and safe-use guidance.

## License

Fructosita is licensed under **GPL-3.0-or-later**. See [`LICENSE`](LICENSE).

© 2026 Antonio Espinosa.
