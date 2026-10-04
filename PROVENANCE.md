# v2.0.0 publication provenance

This tag preserves the Fructosita 2.0.0 engine implementation supplied in the historical release bundle while adding publication documentation and correcting canonical repository metadata.

## Historical artifacts supplied for publication

- Outer historical bundle `v2.0.0.zip` SHA-256: `5cb8c407a17aa56b992dab0fa1bf81a82e06da4ebeb5242e365fa798ff1c5ea4`
- Historical Windows x86_64 binary SHA-256: `3927506d0b958d6ef1aab790020e57fc64013e06330216121f533863e262ed0c`
- Original supplied v2.0.0 source archive SHA-256: `b5945fc66732ea6bc7230fd02d29973dd85e6306f87cfaa2bfd2149b580f1144`

The original source archive contains the actual Rust source tree (`src/`), `testdata/`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, and the GPL license.

## Chronological scope

Publication documentation for this tag is intentionally limited to information available up to **Fructosita 2.0.0**. No later Fructosita version, later experiment, or later result is intentionally referenced.

No explicit references to numbered Fructosita versions later than 2.0.0 were found in the supplied Rust source tree. Historical technical comments in the source are preserved rather than rewritten merely for style.

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text is preserved from the supplied v2.0.0 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata is changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- README, changelog, security, contribution, provenance, and ignore files are added or updated for publication;
- existing project artwork under `assets/` and repository automation under `.github/` are intended to be preserved from the canonical repository during the controlled copy step.

No engine logic is intentionally changed by these publication edits.

## Strength provenance

The supplied historical `RELEASE_NOTES.md` reports a direct **2.0.0 vs 1.7.0** sealed-holdout result of **+239.2 ± 23.0 Elo** over **1,000 games at 10+0.1**, with **0 crashes, 0 illegal moves, and 0 time forfeits**.

The same notes report approximately **+30% nodes per second** and **−52% time to depth 12** relative to 1.7.0.

Using the previously documented informal 1.7.0 ballpark of roughly 2645–2650 Elo gives an informal 2.0.0 development ballpark of roughly **2884–2889 Elo**. This derived figure is not a CCRL rating and is not presented as an independently measured absolute rating.

## Source-change provenance vs 1.11.0

A source-tree comparison against the published 1.11.0 snapshot shows substantive changes in:

- `src/commands.rs`
- `src/eval.rs`
- `src/movegen.rs`
- `src/search.rs`
- `src/texel.rs`
- `src/uci.rs`

The supplied 2.0.0 source contains the release-note features including per-thread evaluation caching, per-square PST correction, Razoring, ProbCut, panic-time logic, `go nodes`, `ponderhit`, `Clear Hash`, and stronger handling for `go infinite` / `go ponder`.

## Bench note

The supplied 2.0.0 release notes do not provide a frozen deterministic bench node count or signature. A local publication validation should therefore record the bench output produced by the Git-ready tree, but it should not be described as matching a historical bench identity unless separate evidence is supplied.
