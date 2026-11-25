# Provenance

Fructosita 3.4.0 is my own work. The engine code, the handcrafted evaluation and its tuned values were written and
measured for this project. No code, weights, tables or training data were taken from other engines. Ideas come
from public sources such as the Chess Programming Wiki and published papers, and were implemented from scratch.

## Release files

- `fructosita-3.4.0-windows-x86_64.exe`: SHA-256 `c3ca3ba9e1c634fbc95f1ceccc69dd6b74582331fce32b07a374587f52c43c0d`. It is built with profile-guided optimisation (profile `fructosita-3.4.0.profdata`, attached to the release), so its bytes differ from a plain build, but it plays the same moves.
- Bench signatures of that executable: depth 10 `b5cb09b5e62201e0`, depth 12 `b5d90d2a54f488fd`.

## Rebuilding

`cargo build --release --locked` from this repository gives the same bench signatures as the release executable
(checked on 2026-10-27 with Rust 1.96.1: 105 tests pass, perft 5 = 4,865,609).
