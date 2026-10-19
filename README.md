<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

# Fructosita 3.0.0

**Fructosita** is an UCI chess engine written in Rust by Antonio Espinosa.

I started Fructosita as a personal project to better understand how chess engines work and how small improvements in search and evaluation can make a difference on the board.

The engine uses a handcrafted evaluation (HCE) alongside an alpha-beta search. My goal is to keep improving its playing strength, efficiency, and reliability while maintaining its own identity.

**Learn from everyone. Copy from no one.**

## What's new in Fructosita 3.0.0?

The main focus of version 3.0.0 is search efficiency. Rather than simply searching deeper everywhere, I wanted Fructosita to make better decisions about where to spend its time.

The most important improvements include:

- **Singular extensions:** Promising moves can receive additional search depth when they stand out from the alternatives. This implementation also includes multi-cut techniques.
- **SPSA parameter tuning:** Several search parameters were tuned through experiments, including settings related to late-move reductions (LMR), pruning, and aspiration windows.
- **More UCI options:** Fructosita exposes 17 configurable search parameters, making it easier to experiment with different settings.
- **Improvements carried forward:** Correction history, persistent search statistics, the evaluation cache, and previous king-safety and positional evaluation improvements remain part of the engine.

These additions build on the work introduced throughout the 2.x series without abandoning Fructosita's handcrafted evaluation.

## Playing strength

During the original development tests, Fructosita 3.0.0 achieved a reported improvement of:

**+68.3 ± 15.3 Elo against Fructosita 2.0.0**

The result came from 1,000 games at a time control of 10+0.1, using a reserved set of opening positions.

The reported 95% confidence interval was approximately +53 to +84 Elo.

For context, Fructosita 2.0.0 has appeared on the CCRL Blitz rating list at 2,921 Elo. Combining that rating with the historical improvement suggests an approximate projection of 2,990 Elo for version 3.0.0.

This projection is **not an official CCRL rating for Fructosita 3.0.0**. Results may differ under independent testing conditions.

## Testing and validation

Before publishing this historical version, I rebuilt and tested the source code.

The validation performed on October 19, 2026 included:

- Successful optimized release compilation.
- All 91 Rust tests passing.
- Successful UCI initialization and search.
- Three perft reference positions producing the expected results.
- Exact reproduction of the historical depth-10 and depth-12 benchmark results.

| Depth | Nodes | Benchmark signature |
|---|---:|---|
| 10 | 631,758 | `b5c6d136e9144e36` |
| 12 | 2,128,879 | `b5db3356890cf707` |

The original 1,000-game strength comparison was not rerun during publication preparation. Its results are preserved as historical development evidence.

## Building from source

Fructosita requires a working Rust toolchain and Cargo.

From the repository root, compile the optimized executable:

```powershell
cargo build --release --locked
```

To run the automated tests:

```powershell
cargo test --release --locked
```

On Windows, the compiled engine can be found at:

`target/release/fructosita.exe`

A locally compiled executable may differ from the original historical release binary, depending on the compiler and build environment.

## Using Fructosita

Fructosita follows the Universal Chess Interface (UCI) protocol and can be used with compatible chess graphical interfaces.

You can also launch it from a Windows terminal:

```powershell
.\target\release\fructosita.exe
```

Some basic UCI commands are:

- `uci` — Display engine information and available options.
- `isready` — Check whether the engine is ready.
- `ucinewgame` — Initialize a new game.
- `position startpos` — Set up the starting position.
- `go depth 10` — Search to depth 10.
- `quit` — Exit the engine.

Built-in diagnostic commands include:

```powershell
.\target\release\fructosita.exe perft 5
.\target\release\fructosita.exe bench depth 10
```

## Download Fructosita 3.0.0

The historical release includes a Windows x86-64 executable, a verified source archive, original release notes, and SHA-256 checksums.

**Release:** https://github.com/josantesbo/Fructosita-Chess-Engine/releases/tag/v3.0.0

The original Windows executable has been preserved rather than replaced with a newly compiled binary.

## License

Fructosita is distributed under the **GPL-3.0-or-later** license. See `LICENSE` for details.

Historical source provenance, original archive checksums, and publication-specific documentation changes are recorded in `PROVENANCE.md`.

## Looking ahead

Fructosita is still evolving. There is plenty of room to improve its search, evaluation, speed, and overall reliability.

I want each version to represent meaningful progress, supported by testing rather than just a higher version number.

Thank you to everyone who has downloaded, tested, or followed the project, and especially to the independent engine-testing community for helping developers better understand their engines.

**Antonio Espinosa**  
Fructosita Chess Engine

*Learn from everyone. Copy from no one.*

