# v1.11.0 publication provenance

This tag preserves the Fructosita 1.11.0 engine implementation supplied in the historical release bundle while adding publication documentation and correcting canonical repository metadata.

## Historical artifacts supplied for publication

- Outer historical bundle `v1.11.0.zip` SHA-256: `b59876bf9dfec1dde074ef266e91f92a44029ed03d434de90e0d3ad14ee09579`
- Windows x86_64 binary SHA-256: `6221ca2c9e96dd9ca0c3bb776014754e118c6c09c5a5107dc966d8050d19c2fe`
- Original supplied v1.11.0 source archive SHA-256: `e9a2783ec258d3df10590d2c1cad7552c159945b4699678afbdd376415c0f68f`

The original source archive contains the actual Rust source tree (`src/`), `testdata/`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, and the GPL license.

## Chronological scope

Publication documentation for this tag is intentionally limited to information available up to **Fructosita 1.11.0**. No later Fructosita version, later experiment, or later result is intentionally referenced.

The supplied historical `RELEASE_NOTES.md` contained explicit information about a later release and later validation result. Those forward references were intentionally excluded from the 1.11.0 publication documentation to preserve chronological integrity.

A review of the supplied 1.11.0 source tree found no explicit references to later numbered Fructosita versions. Historical technical comments in the source were therefore preserved rather than rewritten merely for style.

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text are preserved from the supplied v1.11.0 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata is changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- README, changelog, security, contribution, provenance, and ignore files are added or updated for publication;
- the existing project artwork under `assets/` is intended to be preserved from the canonical repository during the controlled copy step.

No engine logic is intentionally changed by these publication edits.

## Strength-note provenance

The supplied historical `RELEASE_NOTES.md` reports these parent-relative, early-stopped SPRT estimates:

- 1.8.0 vs parent: **+45.9 ± 27.2 Elo**;
- 1.9.0 vs parent: **+93.5 ± 41.9 Elo**;
- 1.10.0 vs parent: **+81.4 ± 37.0 Elo**;
- 1.11.0 vs parent: **+91.7 ± 41.2 Elo**.

The same historical notes explicitly warn that these estimates must not be added together. The supplied 1.11.0 bundle does not provide an independent direct holdout result for the complete 1.7.0→1.11.0 chain, so no derived absolute Elo rating is asserted for 1.11.0.

## Historical bench provenance

The supplied release-preparation notes state that 1.11.0 was built from measured experiment **EXP-0023** with only the version string changed and report a deterministic depth-10 bench of **482,000 nodes** with signature **`b5c444e86376ebbe`**. This should be reproduced from the publication tree before release.
