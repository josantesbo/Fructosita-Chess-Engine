# Changelog

This changelog records the public history of Fructosita up to the version represented by this tag.

## 1.7.0 — Search and king-attack improvements (2026)

### Added / changed

- History malus for previously searched quiet moves after a cutoff.
- Transposition-table probing in quiescence search.
- SEE pruning of sufficiently losing captures.
- Static-evaluation refinement using suitable transposition-table information.
- Preserves the evaluation improvements introduced in the intermediate 1.6.0 step.

### Development strength evidence

Historical release-preparation material reports:

- 1.6.0 vs 1.5.0 parent: **+25.6 ± 19.5 Elo**.
- 1.7.0 vs 1.6.0 parent: **+81.9 ± 36.8 Elo**.
- 1.7.0 vs the 1.3.4 code line: **+146.3 Elo**, reported 95% CI **+131.0 to +162.1**, over **1,400 fresh self-play games**.

The supplied 1.7.0 publication bundle does not include the original log or PGN for the 1,400-game comparison, so that result is preserved as historically reported evidence rather than independently re-audited match data.

Using the informal ~2500 Elo reference associated with the CCRL-era 1.3.4 release would place the reported self-play gap roughly in the **2645–2650 Elo** development ballpark. This is **not a CCRL rating**.

## 1.6.0 — Internal evaluation milestone (2026)

- King-zone attack evaluation.
- Bishop-pair bonus.
- Rook bonuses on open and semi-open files.
- Reported development result vs parent: **+25.6 ± 19.5 Elo**.

## 1.5.0 — Modern search core (2026)

### Added / changed

- Logarithmic late-move reductions.
- Adaptive null-move pruning conditioned on static evaluation and depth.
- Aspiration windows around the previous iteration score.
- Transposition-table aging.
- Time-management iteration guard introduced in the intermediate 1.4.0 step.

### Development strength evidence

- 1.4.0 vs 1.3.4 parent: **+32.8 ± 23.4 Elo**.
- 1.5.0 vs 1.4.0 parent: **+52.2 ± 32.6 Elo**.

The CCRL-era 1.3.4 estimate of roughly 2500 Elo was used only as an informal reference. Chained parent-relative SPRT values are not a direct rating measurement and should not be treated as perfectly additive.

### Verification

Measured-candidate depth-10 bench: **735,391 nodes**, signature **`b5c623535b1fc8c7`**.

## 1.4.0 — Internal milestone (2026)

- Time management: do not begin a new iteration once more than 60% of the soft time budget has been consumed.
- Development SPRT vs parent: **+32.8 ± 23.4 Elo**.

## 1.3.4 — CCRL edition (2026)

- Public CCRL identity of the engine developed internally as 1.3.3.
- Principal Variation Search, null-move pruning, LMR, futility-family pruning, SEE, continuation history, Lazy SMP, Polyglot support, and handcrafted tapered evaluation.

## 1.2.0

- Lazy SMP.
- Color-symmetry fix.

## 1.1.0

- Magic bitboards.

## 1.0.0

- First public release, originally named *Aldosa*.
