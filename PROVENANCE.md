# Provenance

Fructosita 3.2.0 is my own work. The engine code, the handcrafted evaluation and its tuned values were written and
measured for this project. No code, weights, tables or training data were taken from other engines. Ideas come
from public sources such as the Chess Programming Wiki and published papers, and were implemented from scratch.

## Release files

- `fructosita-3.2.0-windows-x86_64.exe`: SHA-256 `628bd17972ebd4910b2d4373ec90e89a98cac37cb55a980e902ce0e4549d6416`.
- Bench signatures of that executable: depth 10 `b5c8865adb04a094`, depth 12 `b5d576af5da807d9`.

## Rebuilding

`cargo build --release --locked` from this repository gives the same bench signatures as the release executable
(checked on 2026-10-27 with Rust 1.96.1: 95 tests pass, perft 5 = 4,865,609).
