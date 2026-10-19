# v3.0.0 publication provenance

This tag preserves the Fructosita 3.0.0 engine implementation supplied in the historical release bundle. The source was not reconstructed from a later version of the engine.

## Historical artifacts supplied for publication

- Original outer bundle `v3.0.0.zip` SHA-256: `8fc889453d99072edf163c5ad0d587ba76ed4cddf34c318e421ce226d334cd3c`
- Original supplied source archive SHA-256: `6cf0897d09c16527a0e656cf3e2059fe547a7ace2e333de6548cbd106bca4dd6`
- Historical Windows x86-64 executable SHA-256: `4abede018a485cd468f52c9210a74d521f5b9e0152aa4aef1a3b6207ad4a1007`
- Historical `RELEASE_NOTES.md` SHA-256: `3b1d787fe6e40c4306e3a2e6c175612896b17b14fa6910e7886b322f9baf3457`
- Original supplied `SHA256SUMS.txt` SHA-256: `6161d77c22eac61ba369e319bc51f03b81b9377c1e0811a8f587791124630a69`

The historical bundle contains the Rust source tree, project configuration, test data, license, and accompanying release files. The original artifacts and the later GitHub publication package are distinguished here because the source ZIP was repackaged after documentation was checked.

## Source integrity and publication scope

This record applies to the historical **Fructosita 3.0.0** release. The engine implementation in `src/`, the test data, `Cargo.lock`, and `LICENSE` were checked against the supplied 3.0.0 source archive and found to match byte-for-byte. The source ZIP distributed on GitHub was subsequently checked against the prepared publication snapshot.

The release retains the historical engine implementation; repository documentation and packaging were prepared separately. Details of those adjustments remain traceable in the Git commit history. No results or features from later engine versions are attributed to 3.0.0.

## Strength provenance

The historical `RELEASE_NOTES.md` reports **+68.3 ± 15.3 Elo for Fructosita 3.0.0 against 2.0.0**, from 1,000 games at 10+0.1 using sealed holdout openings (reported 95% confidence interval approximately +53 to +84 Elo).

This is a historical engine-to-engine result, **not an official CCRL rating**. The match was not repeated during publication preparation. Development-step comparisons and SPRT results are not additive.

### CCRL-based strength estimate (unofficial)

The [CCRL Blitz rating list of October 17, 2026](https://computerchess.org.uk/404/rating_list_all.html) records **Fructosita 2.0.0 at 2,921 Elo**. A simple projection using the reported historical improvement of **+68.3 Elo** gives approximately **2,989 Elo (about 2,990)** for 3.0.0 on that scale.

This value is **not an independently measured CCRL rating for Fructosita 3.0.0**, nor a statistical confidence interval for its CCRL performance. The development match and CCRL Blitz used different conditions, so the actual rating could differ. The figure is included solely as an approximate reference pending independent testing of version 3.0.0.

## Validation and benchmark provenance

The original release material reports passing tests and no crashes, illegal moves, or time forfeits in its reported match. Those historical match-reliability claims were not independently retested during publication.

On **October 19, 2026**, the published source was checked locally using Rust 1.96.1:

- `cargo build --release --locked` completed successfully.
- `cargo test --release --locked` passed **91 tests**.
- UCI initialization and depth-5 search completed successfully.
- Three reference perft positions matched their expected counts.
- Benchmark at depth 10 reproduced **631,758 nodes**, signature `b5c6d136e9144e36`.
- Benchmark at depth 12 reproduced **2,128,879 nodes**, signature `b5db3356890cf707`.

No new engine-versus-engine match was run as part of these publication checks.

## Published release package

The release assets were checked before uploading to GitHub. Their recorded SHA-256 hashes are:

- `fructosita-3.0.0-windows-x86_64.exe`: `4abede018a485cd468f52c9210a74d521f5b9e0152aa4aef1a3b6207ad4a1007`
- `fructosita-3.0.0-source.zip`: `03e4e70a1d287a1838035a296dc7bebdbeb3e389d1af107cf03421f8d24b97e8`
- `RELEASE_NOTES.md`: `3b1d787fe6e40c4306e3a2e6c175612896b17b14fa6910e7886b322f9baf3457`
- `SHA256SUMS.txt`: `f0f005e17f739388a28b4108ee024a86afae64ed5fdf5cd82a1382f926cae469`

The published source ZIP was verified against the prepared repository across **34/34 files** before upload. This record refers to the original v3.0.0 release distribution; later documentation-only changes to the repository's `main` branch do not alter the tagged historical release.
