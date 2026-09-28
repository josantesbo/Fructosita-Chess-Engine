# Changelog

This changelog records the public history of Fructosita up to the version represented by this tag.

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

Using the CCRL-era 1.3.4 estimate of roughly 2500 Elo as an informal reference gives a rough chained development estimate around 2585 Elo. This is not a direct rating measurement and the sequential SPRT results must not be treated as perfectly additive.

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
