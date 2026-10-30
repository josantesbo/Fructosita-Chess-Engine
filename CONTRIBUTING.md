# Contributing

Thank you for your interest in Fructosita.

Fructosita is meant to stay an independently written engine with a handcrafted evaluation. Contributions should keep that identity and must be legally clean.

## Before submitting code

- Do not copy or mechanically translate code, tuned tables, parameter sets or implementation details from another chess engine, unless the license and origin are compatible and we have agreed on it first.
- Prefer your own implementations of public algorithms, papers and general chess-programming literature.
- Explain where non-trivial ideas come from in the pull request.
- Do not commit credentials, tokens, personal paths, datasets with unclear redistribution rights, build directories, PGNs from private experiments or binaries.

## Validation

For code changes, run at least:

```text
cargo test --release --locked
cargo build --release --locked
```

For move-generation or search changes, include `perft` or `bench` results when they apply.

## Licensing

By contributing code that is accepted into this repository, you agree that it may be distributed under the repository's **GPL-3.0-or-later** license, and you confirm that you have the right to submit it under those terms.
