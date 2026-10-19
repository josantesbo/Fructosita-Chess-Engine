# Fructosita 3.0.0 — historical publication provenance

The publication source is the supplied `v3.0.0.zip` bundle, preserved unchanged. It is not reconstructed from a later engine version.

## Historical inputs

- Original outer bundle `v3.0.0.zip`: SHA-256 `8fc889453d99072edf163c5ad0d587ba76ed4cddf34c318e421ce226d334cd3c`
- Original source ZIP: SHA-256 `6cf0897d09c16527a0e656cf3e2059fe547a7ace2e333de6548cbd106bca4dd6`
- Original Windows executable: SHA-256 `4abede018a485cd468f52c9210a74d521f5b9e0152aa4aef1a3b6207ad4a1007`
- Original `RELEASE_NOTES.md`: SHA-256 `3b1d787fe6e40c4306e3a2e6c175612896b17b14fa6910e7886b322f9baf3457`
- Original `SHA256SUMS.txt`: SHA-256 `6161d77c22eac61ba369e319bc51f03b81b9377c1e0811a8f587791124630a69`

The original bundle carries 29 regular source files under `fructosita-3.0.0/` and four regular files in its outer distribution directory.

## Publication-only differences

The source implementation under `src/`, `testdata/`, `Cargo.lock` and `LICENSE` is byte-for-byte identical to the supplied source archive. Publication-specific changes are confined to:

- `Cargo.toml`: canonical repository URL corrected from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`; version and build settings unchanged.
- `.gitignore`: adopted publication ignore policy from the supplied 2.3.1 GIT-READY reference.
- `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `PROVENANCE.md`: publication documentation added from or adapted using the historical 2.3.1 GIT-READY format.

This package intentionally does **not** include `.git/`, `.github/`, or `assets/`; the latter two are to be preserved in the official clone at the controlled copying stage. It contains no git branch, commit, tag, push, or GitHub release operation.

## Historical results and status

The supplied v3.0.0 release notes report +68.3 ± 15.3 Elo against 2.0.0 over 1,000 games at 10+0.1 with sealed holdout openings. It is a reported relative comparison and not a CCRL rating. Depth-10 signature `b5c6d136e9144e36` and depth-12 signature `b5db3356890cf707` are historical claims pending local reproduction.

The original GIT-READY workspace was prepared without fresh execution tests. During subsequent publication validation on 2026-10-19, the source compiled successfully with Rust 1.96.1; all 91 Rust tests passed; three independent perft cases matched their reference node counts; UCI initialization and a depth-5 search succeeded; and historical depth-10 and depth-12 bench node counts and signatures were reproduced. The historical 1,000-game Elo result was not rerun. No fresh PGN match validation was performed.
