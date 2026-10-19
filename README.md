<p align="center">
  <img src="assets/Fructosita.png" width="320" alt="Fructosita Chess Engine">
</p>

# Fructosita 3.0.0

**Fructosita** is an original UCI chess engine written in Rust by Antonio Espinosa. This historical release uses a handcrafted evaluation (HCE) and an alpha-beta search. The project motto is **Learn from everyone. Copy from no one.**

This publication preserves the engine sources supplied with the original v3.0.0 release bundle. The `assets/` and `.github/` directories remain managed by the canonical GitHub repository and are not bundled in this publication workspace.

## Changes since 2.3.1

- Singular extensions with multi-cut in search (2.4.0 development step).
- SPSA-tuned search parameters, including LMR and pruning-related parameters.
- Selected search parameters exposed as UCI `spin` options, with the historical tuned defaults.
- Existing correction history, persistent statistics, evaluation cache, and handcrafted king-safety and threat evaluation remain part of this release.

These descriptions are based on the original historical `RELEASE_NOTES.md` and supplied source code; they are not independently measured in this preparation step.

## Validation evidence reported in the historical release

- **3.0.0 vs 2.0.0:** +68.3 ± 15.3 Elo (reported 95% interval approximately +53 to +84), 1,000 games at 10+0.1 on sealed holdout openings.
- The historical notes report 91 passing tests, perft 5/5, and no crashes, illegal moves, or time forfeits during that match.
- Depth-10 bench: 631,758 nodes, signature `b5c6d136e9144e36`.
- Depth-12 bench: 2,128,879 nodes, signature `b5db3356890cf707`.

**Important:** The Elo and historical match-reliability claims originate from the supplied release package and have not been independently rerun. The Rust tests, perft, UCI functionality, and depth-10/depth-12 bench node counts and signatures were independently reproduced during local publication validation on 2026-10-19. The Elo figure is a relative engine-vs-engine experiment, **not an official CCRL rating**. Development-step SPRT results are not additive.

## Build

Requires a working Rust toolchain. From the repository root:

```powershell
cargo build --release --locked
cargo test --release --locked
```

The engine executable is normally at `target/release/fructosita.exe` on Windows. Do not assume binary equivalence with the historical distribution until the builds are compared and validated.

## Use

Fructosita implements the UCI protocol and can be configured in a compatible chess GUI. To launch in a terminal:

```powershell
.\target\release\fructosita.exe
```

UCI commands include `uci`, `isready`, `ucinewgame`, `position startpos`, `go depth 10`, and `quit`.

Additional built-in diagnostics in the historical source include:

```powershell
.\target\release\fructosita.exe perft 5
.\target\release\fructosita.exe bench depth 10
```

Check exact output and supported options during publication validation.

## Provenance and licensing

The canonical project repository is [josantesbo/Fructosita-Chess-Engine](https://github.com/josantesbo/Fructosita-Chess-Engine). See `PROVENANCE.md` for the historical input hashes and the only publication-specific changes. The source is distributed under GPL-3.0-or-later; see `LICENSE`.
