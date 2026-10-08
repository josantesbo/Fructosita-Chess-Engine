# v2.3.1 publication provenance

This tag preserves the Fructosita 2.3.1 engine implementation supplied in the historical release bundle while adding publication documentation and correcting canonical repository metadata.

## Historical artifacts supplied for publication

- Outer historical bundle `v2.3.1.zip` SHA-256: `c84173c3a138f9e0c481d66d49e763f5197323af6bbfbf428be67c690515d623`
- Historical Windows x86_64 binary SHA-256: `d1b1f794c039e649477d51bc90f7771f980b00e54cee48ee7040fb9602e5cda3`
- Original supplied v2.3.1 source archive SHA-256: `064d5efdd00fe99777cb8d556dd2b629b77cb2d31091d47993d477bfa7d75dd6`
- Historical `RELEASE_NOTES.md` SHA-256: `ba17f0b6133befcc0a5c5ed4fc0b105825bf565ff5e5497b15a57ec9971382be`

The original source archive contains the actual Rust source tree (`src/`), `testdata/`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, and the GPL license.

## Chronological scope

Publication documentation for this tag is intentionally limited to information available up to **Fructosita 2.3.1**. No numbered Fructosita version later than 2.3.1 is intentionally referenced.

No explicit references to numbered Fructosita versions later than 2.3.1 were found in the supplied Rust source tree. Historical technical comments and experiment identifiers in the source are preserved rather than rewritten merely for style.

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text is preserved byte-for-byte from the supplied v2.3.1 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata is changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- the canonical repository `.gitignore` policy is retained instead of the minimal historical `target/` entry;
- README, changelog, security, contribution, and provenance documentation are added or updated for publication;
- existing project artwork under `assets/` and repository automation under `.github/` are intended to be preserved from the canonical repository during the controlled copy step.

No engine logic is intentionally changed by these publication edits.

## Strength provenance

The supplied historical `RELEASE_NOTES.md` reports a direct **2.3.1 vs 2.0.0** development calibration result of **+52.5 ± 16.5 Elo** over **800 games at 10+0.1**, using development openings.

The same notes report these parent-relative development SPRTs at **5+0.05**:

- 2.1.0 vs parent: **+17.7 ± 13.8 Elo**
- 2.2.0 vs parent: **+72 ± 32 Elo**, early stop
- 2.3.0 vs parent: **+31.6 ± 21.7 Elo**

These parent-relative results must not be added together. The direct 2.3.1-vs-2.0.0 calibration match is separate evidence.

No absolute Elo rating is assigned to 2.3.1 by this publication package.

## Source-change provenance vs 2.0.0

A source-tree comparison against the published 2.0.0 snapshot shows substantive Rust-source changes in:

- `src/commands.rs`
- `src/eval.rs`
- `src/search.rs`
- `src/texel.rs`
- `src/uci.rs`

The supplied 2.3.1 source contains the release-note features including static-evaluation correction history, persistent search statistics between moves, expanded king-attack and positional evaluation terms, joint evaluation re-tuning, and a persistent evaluation cache.

## Bench provenance

The supplied historical release notes identify the measured candidate as **EXP-0051** and state that the 2.3.1 source was built from the exact measured source with only the version string changed.

They report a depth-10 bench signature of **`b5c4f767593552bf`**.

This signature should be reproduced from the Git-ready publication tree before the tag is published. The historical release notes do not state a bench node count, so a locally produced node count should be recorded as publication validation rather than claimed as a supplied historical value.
