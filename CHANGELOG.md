# Changelog

This changelog records the public history of Fructosita up to the version represented by this tag.

## 1.3.4 — CCRL edition (2026)

### Changed

- Published the engine version submitted to CCRL.
- Principal Variation Search with null-move pruning, late-move reductions, reverse futility, futility pruning, late-move pruning, and internal iterative reductions.
- Static Exchange Evaluation for capture ordering.
- One-ply continuation history alongside killer and history heuristics.
- Handcrafted tapered evaluation with material, piece-square tables, per-rank corrections, mobility, pawn structure, passed pawns, and basic king safety.
- Deterministic time management with a measured safety reserve.
- UCI options for `Hash`, `Threads`, `OwnBook`, and `BookFile`.

Version 1.3.4 is the public CCRL identity of the engine developed internally as version 1.3.3.

## 1.2.0

- Lazy SMP.
- Color-symmetry fix.

## 1.1.0

- Magic bitboards.

## 1.0.0

- First public release, originally named *Aldosa*.
