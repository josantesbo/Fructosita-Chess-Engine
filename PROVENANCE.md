# v1.5.0 publication provenance

This tag preserves the Fructosita 1.5.0 engine implementation prepared from the measured **EXP-0005** candidate while adding publication documentation and correcting canonical repository metadata.

## Historical artifacts supplied for publication

- Windows x86_64 binary SHA-256: `9f8f60b2a46165079ee4267fbb06cb5eab378e562f2370a43aec41ebaae69849`
- Original v1.5.0 source archive supplied by the release-preparation bundle SHA-256: `c34d290bb4b7a5e0043d35d19bbd8ce76d4587cd391b2adb1248e967782c69c5`

## Measured-candidate verification

The supplied release notes record the EXP-0005 depth-10 deterministic bench as:

- nodes: **735,391**
- signature: **`b5c623535b1fc8c7`**

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text are preserved from the supplied v1.5.0 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata was changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- README, changelog, security, contribution, provenance, and ignore files were added or updated;
- the project logo was added under `assets/`.

No engine logic was intentionally changed by these publication edits.

## Strength-note provenance

The parent-relative SPRT figures published for this version are:

- 1.4.0 vs parent: **+32.8 ± 23.4 Elo**;
- 1.5.0 vs parent: **+52.2 ± 32.6 Elo**.

Any approximate absolute Elo figure derived from the user's informal 2500-Elo reference for 1.3.4 is explicitly labeled as an estimate and is not a CCRL rating or direct 1.5.0-vs-1.3.4 measurement.
