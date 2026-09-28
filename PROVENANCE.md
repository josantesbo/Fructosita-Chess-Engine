# v1.3.4 publication provenance

This tag preserves the Fructosita 1.3.4 engine implementation used for the CCRL-era release while adding publication documentation and correcting repository metadata.

## Historical artifacts supplied for publication

- Historical Windows AVX2 binary SHA-256: `43e8bc039400135ccee3aec8f3d5779823322e31b01f87e84a8637307aa1dde7`
- Historical source archive supplied by the release-preparation bundle SHA-256: `4a12e250105ae12b2b94a493734aee642472044b6389450890fbad1949867607`

## Publication-only changes

The engine implementation under `src/`, `Cargo.lock`, `testdata/`, and the GPL license text are preserved from the supplied v1.3.4 source archive.

For publication in the canonical repository:

- `Cargo.toml` repository metadata was changed from `jordiqui/Fructosita-Chess-Engine` to `josantesbo/Fructosita-Chess-Engine`;
- README, changelog, security, contribution, provenance, and ignore files were added or updated;
- the project logo was added under `assets/`.

No engine logic was intentionally changed by these publication edits.
