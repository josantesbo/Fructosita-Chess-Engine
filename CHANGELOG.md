# Changelog

## 3.2.0: Much faster, slightly better search (2026-10-01)

- Speed with an identical tree (about 23% less time per position): a move is made only after it survives pruning, check squares are computed once per node, the move picker sorts once, the evaluation counts bits with the POPCNT instruction and uses pre-summed tables, and SEE updates the attackers instead of recomputing them.
- Search: the transposition table keeps a position's good move when the new entry is only an upper bound; late-move reductions take the move's history into account; the engine allows one more iteration when the best move has just changed.
- New requirement: a CPU with POPCNT (any x86-64 since 2008). On an older CPU the engine prints a message and exits.

Evidence:

- Sealed holdout vs 3.1.0: **+62.5 ± 13.9 Elo** (95% CI about +48.6 to +76.5), 1,000 games at 10+0.1 (+350 −172 =478). No time forfeits, illegal moves or errors.
- During development, at 5+0.05 with one thread and 64 MB hash: the speed changes passed SPRT with +49.9 ± 19.2 Elo in 526 games, the search changes with +14.7 ± 9.8 Elo in 2,084 games.

Bench signatures: depth 10 `b5c8865adb04a094`; depth 12 `b5d576af5da807d9`. Windows x64 and a CPU with POPCNT (any x86-64 since 2008). On a CPU without POPCNT the engine prints a message and exits.

## 3.1.0: Faster search, same tree (2026-09-30)

- Per-ply search buffers: move lists and other per-node buffers are reused ply by ply instead of being rebuilt. The search tree is identical to 3.0.0 (same bench signatures) and each search takes about 12% less time.
- Fix: a checkmate delivered when the fifty-move counter reaches its limit is now scored as mate, not as a draw.

Evidence:

- Sealed holdout vs 3.0.0: **+21.6 ± 14.4 Elo** (95% CI about +7 to +36), 1,000 games at 10+0.1 on openings that were never used during development (+284 −222 =494). No time forfeits, illegal moves or crashes.
- During development: SPRT vs 3.0.0 at 5+0.05, +24.3 ± 13.4 Elo, H1 accepted after 1,162 games; calibration match at 10+0.1, +20.0 ± 15.7 Elo in 800 games.

Bench signatures: depth 10 `b5c6d136e9144e36`; depth 12 `b5db3356890cf707`. Windows x64.

## 3.0.0: Singular extensions and SPSA-tuned search (2026)

- Singular extensions with multi-cut (internal step 2.4.0, +18.8 ± 14.5 Elo vs its parent).
- Search parameters (LMR, late-move pruning, razoring, delta and SEE pruning, aspiration windows) tuned with SPSA on real games (+12.8 ± 10.2 Elo vs its parent); selected values exposed as UCI `spin` options.
- **+68.3 ± 15.3 Elo vs 2.0.0**, 1,000 games at 10+0.1 on sealed holdout openings; no crashes, illegal moves or time forfeits.
- Bench signatures: depth 10 `b5c6d136e9144e36`; depth 12 `b5db3356890cf707`.

## 2.3.1: Learning search and a sharper king attack (2026)

- Evaluation cache kept between moves (speed only, identical search).
- **+52.5 ± 16.5 Elo vs 2.0.0**, 800 games at 10+0.1 (development openings, calibration match).
- Bench signature at depth 10: `b5c4f767593552bf`.

## 2.3.0: Expanded king attack and positional evaluation (2026)

- Safe checks by piece type and undefended king-zone squares.
- Pawn-push threats and hanging pawns.
- Rook behind a passed pawn and minor piece behind a pawn.
- Pins, rook-on-queen threats, knight value by pawn count.
- Joint re-tuning of the evaluation. SPRT vs parent at 5+0.05: **+31.6 ± 21.7 Elo**.

## 2.2.0: Persistent search statistics (2026)

- History, continuation history and correction history persist between moves instead of being reset at each `go`; `ucinewgame` clears them.
- SPRT vs parent at 5+0.05: **+72 ± 32 Elo** (early stop).

## 2.1.0: Static-evaluation correction history (2026)

- A correction keyed by pawn structure learns how far the static evaluation is from what the search finds, and corrects the evaluation used by pruning and by the quiescence stand-pat.
- SPRT vs parent at 5+0.05: **+17.7 ± 13.8 Elo**.

The 2.1.0 to 2.3.0 results are measured against each parent and must not be added together. The 2.3.1 result vs 2.0.0 is a separate direct match.

## 2.0.0: First major release (2026)

- **+239.2 ± 23.0 Elo vs 1.7.0**, 1,000 games at 10+0.1 on sealed holdout openings prepared before any experiment; no crashes, illegal moves or time forfeits.
- About 30% more nodes per second and 52% less time to depth 12 than 1.7.0.
- Per-thread evaluation cache; per-square PST corrections on top of the file and rank tuning.
- Razoring and ProbCut; panic time after a large score drop between iterations.
- UCI: `go nodes`, `go infinite` and `go ponder` wait for `stop`, `ponderhit`, `Clear Hash`.

## 1.11.0: Piece-square-table refinement (2026)

- Piece-square tables tuned by file and rank. Early-stopped SPRT vs parent: **+91.7 ± 41.2 Elo**.
- Depth-10 bench: 482,000 nodes, signature `b5c444e86376ebbe`.

## 1.10.0: King and pawn evaluation (2026)

- King shield and pawn storm; blocked and free passed pawns. Early-stopped SPRT vs parent: **+81.4 ± 37.0 Elo**.

## 1.9.0: Expanded handcrafted evaluation (2026)

- Endgame knowledge, safe mobility per piece, threats and hanging pieces, knight outposts, Texel tuning. Early-stopped SPRT vs parent: **+93.5 ± 41.9 Elo**.

## 1.8.0: Search and transposition-table speed (2026)

- Lock-less transposition table, TT prefetch, TT move searched before full move generation, lazy move selection. Early-stopped SPRT vs parent: **+45.9 ± 27.2 Elo**.

The 1.8.0 to 1.11.0 results are development estimates against each parent and must not be added together. The 2.0.0 result vs 1.7.0 is a separate direct comparison.

## 1.7.0: Search and king-attack improvements (2026)

- History malus for quiet moves searched before a cutoff.
- Transposition-table probing in quiescence search.
- SEE pruning of clearly losing captures.
- Static evaluation refined with suitable transposition-table information.
- Keeps the evaluation work of the internal 1.6.0 step. Result vs 1.6.0: **+81.9 ± 36.8 Elo**.
- **+146.3 Elo vs the 1.3.4 code line** over 1,400 fresh self-play games.

## 1.6.0: Internal evaluation step (2026)

- King-zone attack evaluation, bishop pair, rooks on open and semi-open files. Result vs parent: **+25.6 ± 19.5 Elo**.

## 1.5.0: Modern search core (2026)

- Logarithmic late-move reductions.
- Adaptive null-move pruning conditioned on static evaluation and depth.
- Aspiration windows around the previous iteration's score.
- Transposition-table aging.
- Time-management guard from the internal 1.4.0 step.
- Result vs 1.4.0: **+52.2 ± 32.6 Elo**. Depth-10 bench: 735,391 nodes, signature `b5c623535b1fc8c7`.

## 1.4.0: Internal step (2026)

- Time management: no new iteration once more than 60% of the soft time budget is used. SPRT vs parent: **+32.8 ± 23.4 Elo**.

## 1.3.4: CCRL edition (2026)

- Public CCRL identity of the engine developed internally as 1.3.3.
- Principal Variation Search, null-move pruning, LMR, futility-family pruning, SEE, continuation history, Lazy SMP, Polyglot support and a handcrafted tapered evaluation.

## 1.2.0

- Lazy SMP.
- Colour-symmetry fix.

## 1.1.0

- Magic bitboards.

## 1.0.0

- First public release, originally named *Aldosa*.
