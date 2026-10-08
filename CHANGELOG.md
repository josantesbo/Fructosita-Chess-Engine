# Changelog

This changelog records the public history of Fructosita up to the version represented by this tag.

## 2.3.1 — Learning search and a sharper king attack (2026)

- Direct development calibration result vs 2.0.0: **+52.5 ± 16.5 Elo** across **800 games at 10+0.1**.
- Evaluation cache retained between moves; the supplied historical release notes describe this as a speed-only change with identical search.
- Historical measured-candidate identity: **EXP-0051**.
- Historical depth-10 bench signature: **`b5c4f767593552bf`**.

## 2.3.0 — Expanded king attack and positional evaluation (2026)

- Added safe checks by piece type and undefended king-zone squares.
- Added pawn-push threats and hanging-pawn terms.
- Added rook-behind-passed-pawn and minor-behind-pawn terms.
- Added pins, rook-on-queen threats, and knight/pawn-count interaction.
- Joint re-tuning of the expanded handcrafted evaluation.
- Reported parent-relative development SPRT at 5+0.05: **+31.6 ± 21.7 Elo**.

## 2.2.0 — Persistent search statistics (2026)

- History, continuation history, and correction history persist between moves instead of being reset at each `go`.
- `ucinewgame` clears the retained search statistics.
- Reported parent-relative development SPRT at 5+0.05: **+72 ± 32 Elo**, early stop.

## 2.1.0 — Static-evaluation correction history (2026)

- Added pawn-structure keyed correction history that learns the systematic difference between static evaluation and search results.
- The corrected static evaluation is used by pruning decisions and quiescence stand-pat.
- Reported parent-relative development SPRT at 5+0.05: **+17.7 ± 13.8 Elo**.

The 2.1.0–2.3.0 parent-relative development results are not additive. The 2.3.1-vs-2.0.0 result is a separate direct calibration match using development openings.

## 2.0.0 — First major release (2026)

- First release validated on a sealed holdout prepared before experimentation.
- Direct result vs 1.7.0: **+239.2 ± 23.0 Elo** across **1,000 games at 10+0.1**.
- Reliability in that holdout: **0 crashes, 0 illegal moves, 0 time forfeits**.
- Historical release notes report approximately **+30% nodes per second** and **−52% time to depth 12** vs 1.7.0.
- Added per-thread evaluation cache.
- Added per-square PST corrections on top of the file/rank tuning line.
- Added Razoring and ProbCut.
- Added panic-time behavior after a sufficiently large iteration-to-iteration score drop.
- UCI robustness additions include `go nodes`, `go infinite` / `go ponder` stop handling, `ponderhit`, and `Clear Hash`.

## 1.11.0 — Piece-square-table refinement (2026)

- Piece-square-table tuning by file and rank.
- Reported parent-relative early-stopped SPRT: **+91.7 ± 41.2 Elo**.
- Historical measured-candidate identity: EXP-0023, depth-10 bench **482,000 nodes**, signature **`b5c444e86376ebbe`**.

## 1.10.0 — King and pawn evaluation milestone (2026)

- King shield and pawn-storm evaluation.
- Blocked and free passed-pawn terms.
- Reported parent-relative early-stopped SPRT: **+81.4 ± 37.0 Elo**.

## 1.9.0 — Expanded handcrafted evaluation (2026)

- Endgame knowledge.
- Safe per-piece mobility.
- Threats and hanging-piece terms.
- Knight outposts.
- Texel tuning of evaluation parameters.
- Reported parent-relative early-stopped SPRT: **+93.5 ± 41.9 Elo**.

## 1.8.0 — Search and transposition-table speed milestone (2026)

- Lock-less transposition table.
- TT prefetch.
- TT move searched before full move generation.
- Lazy move selection.
- Reported parent-relative early-stopped SPRT: **+45.9 ± 27.2 Elo**.

The 1.8.0–1.11.0 parent-relative SPRT values are development estimates and must not be treated as additive. The 2.0.0-vs-1.7.0 result above is a separate direct sealed-holdout comparison.

## 1.7.0 — Search and king-attack improvements (2026)

- History malus for previously searched quiet moves after a cutoff.
- Transposition-table probing in quiescence search.
- SEE pruning of sufficiently losing captures.
- Static-evaluation refinement using suitable transposition-table information.
- Preserves the evaluation improvements introduced in the intermediate 1.6.0 step.
- Reported development result vs 1.6.0 parent: **+81.9 ± 36.8 Elo**.
- Historical material reported **+146.3 Elo** vs the 1.3.4 code line across 1,400 fresh self-play games; the original match log/PGN was not included in the supplied 1.7.0 publication bundle.

## 1.6.0 — Internal evaluation milestone (2026)

- King-zone attack evaluation.
- Bishop-pair bonus.
- Rook bonuses on open and semi-open files.
- Reported development result vs parent: **+25.6 ± 19.5 Elo**.

## 1.5.0 — Modern search core (2026)

- Logarithmic late-move reductions.
- Adaptive null-move pruning conditioned on static evaluation and depth.
- Aspiration windows around the previous iteration score.
- Transposition-table aging.
- Time-management iteration guard introduced in the intermediate 1.4.0 step.
- Reported 1.5.0 vs 1.4.0 parent: **+52.2 ± 32.6 Elo**.
- Measured-candidate depth-10 bench: **735,391 nodes**, signature **`b5c623535b1fc8c7`**.

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
