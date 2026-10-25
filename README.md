<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

# Fructosita 3.1.0

**Fructosita** is an original UCI chess engine written in Rust by Antonio Espinosa. This release uses a handcrafted evaluation (HCE) and an alpha-beta search. The project motto is **Learn from everyone. Copy from no one.**

Fructosita 3.1.0 searches exactly the same tree as 3.0.0 about 12% faster, and fixes the scoring of a checkmate delivered on the fifty-move boundary. The `assets/` and `.github/` directories remain managed by the canonical GitHub repository and are not bundled in this publication workspace.

## Changes since 3.0.0

- **Per-ply search buffers:** move lists and other per-node buffers are reused ply by ply instead of being rebuilt. The search tree is identical to 3.0.0 (same bench signatures) with about 12% less time per search.
- **Fix:** a checkmate delivered when the fifty-move counter reaches its limit is scored as mate, not as a draw.

## Validation evidence

- Development SPRT vs 3.0.0 (5+0.05): **+24.3 ± 13.4 Elo**, H1 accepted, 1,162 games.
- Development calibration vs 3.0.0 (10+0.1): +20.0 ± 15.7 Elo, 800 games.
- **Sealed holdout vs 3.0.0: +21.6 ± 14.4 Elo** (95% CI ≈ +7 to +36), 1,000 games at 10+0.1 on reserved openings never used during development (+284 −222 =494); 0 time forfeits, 0 illegal moves, 0 crashes.

**Important:** the match results above come from the project's development records. During local publication validation on 2026-10-19, this publication tree compiled with `cargo build --release --locked` (Rust 1.96.1); all 93 Rust tests passed; perft 5 from the start position gave 4,865,609 nodes; UCI initialisation (`id name Fructosita 3.1.0`, `uciok`, `readyok`) and a depth-6 search succeeded; and the depth-10 and depth-12 bench signatures (`b5c6d136e9144e36`, 631,758 nodes; `b5db3356890cf707`, 2,128,879 nodes) matched the signatures recorded for the released executable. The historical match results were not rerun. Elo figures are relative engine-vs-engine experiments, **not official CCRL ratings**. Development-step SPRT results are not additive.

## Requirements

Windows x64. The engine identifies itself as `id name Fructosita 3.1.0`.

## Build

Requires a working Rust toolchain. From the repository root:

```powershell
cargo build --release --locked
cargo test --release --locked
```

The engine executable is normally at `target/release/fructosita.exe` on Windows.

## Use

Fructosita implements the UCI protocol and can be loaded in any compatible GUI or tournament manager. To launch in a terminal:

```powershell
.\target\release\fructosita.exe
```

Main UCI options:

| Option | Default | Notes |
|---|---|---|
| `Hash` | 64 | MB |
| `Threads` | 1 | Lazy SMP |
| `OwnBook` | true | Only used when `BookFile` is set (Polyglot book); no file, no book |
| `BookFile` | empty | Path of a Polyglot book |
| `Clear Hash` | - | Button |

Other options (`RfpMargin`, `LmrBase`, ...) are internal search parameters exposed for tuning; leave them at their defaults. Built-in diagnostics:

```powershell
.\target\release\fructosita.exe perft 5
.\target\release\fructosita.exe bench depth 12
```

`bench depth 12` must end with `bench signature b5db3356890cf707`.

## Provenance and licensing

The canonical project repository is [josantesbo/Fructosita-Chess-Engine](https://github.com/josantesbo/Fructosita-Chess-Engine). See `PROVENANCE.md` for the input hashes and the only publication-specific changes. The source is distributed under GPL-3.0-or-later; see `LICENSE`.
