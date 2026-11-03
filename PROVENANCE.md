# Provenance

Fructosita 3.3.0 is my own work. The engine code, the handcrafted evaluation and its tuned values were written and
measured for this project. No code, weights, tables or training data were taken from other engines. Ideas come
from public sources such as the Chess Programming Wiki and published papers, and were implemented from scratch.

## Release files

- `fructosita-3.3.0-windows-x86_64.exe`: SHA-256 `d39dea24e983ea484fbe13eb30e31ff874f8635d14e0d5aa64683f1912618dc6`. It is built with profile-guided optimisation (profile `fructosita-3.3.0.profdata`, attached to the release), so its bytes differ from a plain build, but it plays the same moves.
- Bench signatures of that executable: depth 10 `b5c4a137e60e39db`, depth 12 `b5eed34762ad6df9`.

## Rebuilding

`cargo build --release --locked` from this repository gives the same bench signatures as the release executable
(checked on 2026-10-27 with Rust 1.96.1: 96 tests pass, perft 5 = 4,865,609).
