# Contributing

Thank you for your interest in Fructosita.

Fructosita is intended to remain an independently implemented handcrafted-evaluation chess engine. Contributions should preserve that identity and must be legally clean.

## Before submitting code

- Do not copy or mechanically translate code, tuned tables, parameter sets, or implementation details from another chess engine unless the license and provenance are explicitly compatible and the maintainer has agreed to that direction.
- Prefer independently designed implementations based on public algorithms, papers, and general chess-programming literature.
- Explain the provenance of non-trivial ideas in the pull request.
- Do not commit credentials, tokens, personal paths, datasets with unclear redistribution rights, build directories, PGNs from private experiments, or generated binaries unless explicitly requested.

## Validation

For code changes, run at least:

```text
cargo test --release --locked
cargo build --release --locked
```

For move-generation or search changes, include relevant `perft` or `bench` evidence when applicable.

## Licensing

By contributing code that is accepted into this repository, you agree that your contribution may be distributed under the repository's **GPL-3.0-or-later** license, and you confirm that you have the right to submit it under those terms.
