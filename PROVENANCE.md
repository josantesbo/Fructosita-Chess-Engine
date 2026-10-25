# Fructosita 3.1.0 — publication provenance

The publication source is the project's own `fructosita-3.1.0-source.zip`, which is byte-identical to the git tree of local tag `v3.1.0` on branch `release-3.1.0`. It is not reconstructed from a later engine version.

## Inputs

- Source commit: `28547a13607dfaff4fbf249eba1c4bc7c801a9da` (tree `83c2140898383d6259dd515d4a28f1fb5660c985`)
- Source ZIP: SHA-256 `467328f6074b747643e3277dd23551e1f58df3cdae2522da99f0c214436c216c`
- Released Windows executable: SHA-256 `2be292708060a77f8f002c9164c9317efc819a822c10bc1f6fc51a5d0f920507`
- Recorded bench signatures: depth 10 `b5c6d136e9144e36`, depth 12 `b5db3356890cf707`

## Publication-only differences

All files under `src/`, `testdata/`, plus `Cargo.lock` and `LICENSE`, are byte-for-byte identical to the source archive. Publication-specific changes are confined to:

- `Cargo.toml`: canonical repository URL corrected from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`; version and build settings unchanged.
- `.gitignore`: publication ignore policy adopted from the 3.0.0 GIT-READY reference (the original ignored only `target/`).
- `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `PROVENANCE.md`: publication documentation in the format of the published 3.0.0 release.

This package intentionally does **not** include `.git/`, `.github/`, or `assets/`; the latter two are preserved in the official clone. It contains no git branch, commit, tag, push, or GitHub release operation.

## Validation

During local publication validation on 2026-10-19, this publication tree compiled with `cargo build --release --locked` (Rust 1.96.1); all 93 Rust tests passed; perft 5 from the start position gave 4,865,609 nodes; UCI initialisation (`id name Fructosita 3.1.0`, `uciok`, `readyok`) and a depth-6 search succeeded; and the depth-10 and depth-12 bench signatures (`b5c6d136e9144e36`, 631,758 nodes; `b5db3356890cf707`, 2,128,879 nodes) matched the signatures recorded for the released executable. The historical match results were not rerun. The released executable is the profile-guided build recorded in the project; the validation build is a plain `cargo build` and therefore differs in bytes, not in play.
