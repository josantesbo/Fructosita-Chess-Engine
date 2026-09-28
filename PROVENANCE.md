# v1.7.0 publication provenance

This tag preserves the Fructosita 1.7.0 engine implementation supplied in the historical release bundle while adding publication documentation and correcting canonical repository metadata.

## Historical artifacts supplied for publication

- Outer historical bundle `v1.7.0.zip` SHA-256: `ea5c1b0e922ce860943fd30f6b51d1e81fc91d0c5fb8b78f2e9f16f5cbd6e779`
- Windows x86_64 binary SHA-256: `6fcddddbd710e80068cbf41975032cf85b33554f2621ec1256474a429295ab07`
- Original supplied v1.7.0 source archive SHA-256: `c009feefcc00d70e442503c7ecc8fc30fbe62c42c0f0c2756ee03144927eb98e`

The original source archive contains the actual Rust source tree (`src/`), `testdata/`, `Cargo.toml`, `Cargo.lock`, and the GPL license.

## Chronological scope

Publication documentation for this tag is intentionally limited to information available up to **Fructosita 1.7.0**. No later Fructosita version, later experiment, or later result is intentionally referenced.

A review of the supplied 1.7.0 source found no explicit references to later numbered Fructosita versions. Historical technical comments in the source were therefore preserved rather than rewritten merely for style.

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text are preserved from the supplied v1.7.0 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata was changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- README, changelog, security, contribution, provenance, and ignore files were added or updated;
- the existing project artwork under `assets/` is intended to be preserved from the canonical repository during the controlled copy step.

No engine logic was intentionally changed by these publication edits.

## Strength-note provenance

The supplied historical `RELEASE_NOTES.md` reports:

- 1.4.0 vs parent: **+32.8 ± 23.4 Elo**;
- 1.5.0 vs parent: **+52.2 ± 32.6 Elo**;
- 1.6.0 vs parent: **+25.6 ± 19.5 Elo**;
- 1.7.0 vs parent: **+81.9 ± 36.8 Elo**;
- 1.7.0 vs the 1.3.4 code line: **+146.3 Elo**, reported 95% CI **+131.0 to +162.1**, across **1,400 fresh self-play games**.

The supplied publication bundle does not contain the original match log or PGN for the 1,400-game comparison. Accordingly, the direct-match figure is described as a historically reported result and not as an independently re-audited result.

Any absolute Elo estimate derived from the informal ~2500-Elo reference associated with 1.3.4 is explicitly labeled as a development estimate and not a CCRL rating.
