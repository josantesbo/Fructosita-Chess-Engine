<p align="center">
  <img src="assets/Fructosita.png" width="360" alt="Fructosita Chess Engine">
</p>

<h1 align="center">Fructosita</h1>

<p align="center">
  <b>An original UCI chess engine written from scratch in Rust</b><br>
  Handcrafted evaluation · Modern alpha-beta search · Every change measured
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.4.0-orange" alt="version">
  <img src="https://img.shields.io/badge/Rust-2021-orange" alt="Rust">
  <img src="https://img.shields.io/badge/protocol-UCI-success" alt="UCI">
  <img src="https://img.shields.io/badge/evaluation-HCE-blue" alt="HCE">
  <img src="https://img.shields.io/badge/variant-Chess960-blueviolet" alt="Chess960">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License">
</p>

---

## About

**Fructosita** is an open-source chess engine written entirely from scratch in **Rust**. It plays standard chess and Chess960. No code, tables or weights were copied from any other engine: every idea comes from public literature (the Chess Programming Wiki, research papers) and was implemented, tuned and measured specifically for Fructosita.

The project follows one rule, **learn from everyone, copy from no one**, and a second one: **no change is kept without evidence**. Every improvement is tested against its parent, and every version that claims more strength is checked on openings that were never used while developing it.

## The author

Fructosita is created by **Antonio Espinosa**, a **chemist and pharmacobiologist** (*Químico Farmacobiólogo*).
The project started on **July 2, 2026** as a personal challenge: to learn, from zero, how a chess engine thinks.

The name comes from the author's undergraduate thesis on **fructose**. That little molecule became the engine's
identity and its logo: a king merging into a hexagonal ring, where chess meets chemistry.

> *Fructosita nació como un reto personal de un químico farmacobiólogo: aprender desde cero cómo piensa un motor
> de ajedrez. El nombre viene de mi tesis de licenciatura sobre la fructosa.* (Antonio Espinosa)

## Fructosita 3.4.0

Fructosita 3.4.0 plays Chess960 (Fischer Random) from the same executable and orders moves better in both modes.

### What's new since 3.3.0

- Chess960: standard UCI option `UCI_Chess960`, positions in X-FEN or Shredder-FEN, castling with any king and rook placement, and `frc N` prints start position N. The rules were checked by perft against two independent references on all 960 start positions and on thousands of test positions without a mismatch, and in more than 4,000 games against itself and other engines without a single illegal move. In standard chess the engine behaves the same with the option on or off.
- Search, both modes: quiet moves tried before the refuting move also lose points in the continuation history; the static evaluation is also corrected by each side's piece placement (before, only by pawns); and the move history distinguishes whether the piece leaves an attacked square and whether it lands on one.
- Profile-guided optimisation and static linking, as in 3.3.0.

### Evidence

- Sealed holdout vs 3.3.0, standard chess: **+18.1 ± 13.9 Elo**, 1,000 games at 10+0.1, no early stop (+266 −214 =520). No time forfeits or illegal moves.
- Sealed Chess960 positions vs the first Chess960 build (3.3.0 search): **+59.3 ± 15.3 Elo**, 1,000 games at 10+0.1 (+344 −175 =481), no incidents. That reference was not built with PGO, so about 8 to 10 Elo of the figure comes from the build.
- During development (SPRT at 5+0.05): continuation-history malus plus piece-placement correction +22.5 ± 13.0 Elo in 1,252 games; threat-aware move history +10.8 ± 8.1 Elo in 3,302 games.
- Gauntlet at 10+0.1, 200 games per opponent and mode. Standard: +158 vs Cheese 3.3.0, +106 vs Bobcat 8.0, −151 vs Critter 1.6a, +235 vs Spike 1.4.2, +326 vs Daydreamer 2.0.0-pre2; performance about 3064 (95% CI 3038 to 3091) with the CCRL Blitz ratings of Cheese (2959) and Critter (3163) of 2026-11-21. Chess960: +83, +160, −217, +228, +207 against the same engines; performance about 3096 (95% CI 3074 to 3118) with the CCRL 40/2 FRC list of 2026-10-04. Short time control on my own hardware: **not a CCRL rating**, and the two lists are different scales.
- Estimated rating: CCRL Blitz rates 3.3.0 at **3114** (840 games, list of 2026-11-21). Adding the gain measured over 3.3.0, I expect 3.4.0 around **3130** on that list. This is my estimate, **not a CCRL rating**.

### Known limitations

- With a "moves to go" time control and two moves left, the engine can spend almost the whole clock on the penultimate move. It does not lose on time (300 games at 40 moves in 10 s, repeating, without incident), but the last move of the control may get very little time.
- Twelve other ideas were tested and rejected in the same stage, including an automatic retune of 32 parameters that played worse.

## Strength

| Version | Measured against | Result | Conditions |
|---|---|---|---|
| 1.3.4 |  | Submitted to **CCRL** |  |
| 1.7.0 | 1.3.4 | **+146 Elo** (95% CI +131 to +162) | 1,400 fresh games |
| 2.0.0 | 1.7.0 | **+239 ± 23 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.0.0 | 2.0.0 | **+68.3 ± 15.3 Elo** (95% CI about +53 to +84) | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.1.0 | 3.0.0 | **+21.6 ± 14.4 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.2.0 | 3.1.0 | **+62.5 ± 13.9 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.3.0 | 3.2.0 | **+44.0 ± 13.8 Elo** | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.4.0 | 3.3.0 | **+18.1 ± 13.9 Elo** (standard) | 1,000 games, 10+0.1, **sealed holdout openings** |
| 3.4.0 | first Chess960 build | **+59.3 ± 15.3 Elo** (Chess960) | 1,000 games, 10+0.1, **sealed Chess960 positions** |

Self-play Elo overstates the gap seen in rating lists; independent ratings (CCRL) are the reference. CCRL Blitz, list of 2026-11-21: 2.0.0 **2930**, 3.0.0 **3017**, 3.3.0 **3114**.

## Features

**Search**
- Principal Variation Search, iterative deepening, aspiration windows
- Transposition table: lock-less, 16-byte entries, prefetch; TT move tried before move generation; a good move is kept when only an upper bound is stored
- Null-move pruning (adaptive), reverse futility, futility, razoring, ProbCut, late-move pruning, SEE pruning
- Logarithmic late-move reductions (also guided by the move's history), internal iterative reductions, check and **singular extensions**
- History and continuation history (with a malus for quiet moves tried before the cutoff), killers, **static-evaluation correction history** keyed by pawns and by each side's piece placement
- Move history that knows whether the piece leaves or lands on an attacked square
- Search statistics **persist between moves** (history, correction and evaluation caches)
- Lazy SMP, time management with panic time on score drops

**Evaluation (handcrafted, Texel-tuned)**
- Tapered material and piece-square tables (formula plus tuned per-file, per-rank and per-square corrections)
- Safe mobility per piece, threats, hanging pieces and pawns, pawn-push threats, pins
- King safety: attack units, king shield, **safe checks**, weak king-zone squares
- Pawn structure: doubled, isolated, passed pawns (blocked or free, king race, rook behind)
- Outposts, bishop pair, rooks on open files, endgame scaling (insufficient material, opposite bishops, mop-up)
- Pawn and king terms kept in their own cache; occupancy, material and piece-square sums updated incrementally

**Engine**
- UCI: `Hash`, `Threads`, `OwnBook`, `BookFile`, `Clear Hash`, `UCI_Chess960`, `go infinite/ponder/nodes/depth/movetime`
- **Chess960** (Fischer Random) in the same executable: X-FEN and Shredder-FEN, castling with any king and rook placement
- Polyglot opening book support
- Built-in tools: `perft`, `bench`, EPD test harness, Texel tuner, `frc N` (Chess960 start positions)
- Release executable built with profile-guided optimisation and the C runtime linked in (no Visual C++ redistributable needed)

## Usage

Fructosita is a UCI engine: load the executable in any UCI GUI (Arena, Cute Chess, Banksia GUI, ChessBase, ...).
It identifies itself as `id name Fructosita 3.4.0`.

| Option | Default | Notes |
|---|---|---|
| `Hash` | 64 | MB |
| `Threads` | 1 | Lazy SMP |
| `OwnBook` | true | Only used when `BookFile` is set (Polyglot book); no file, no book |
| `BookFile` | empty | Path of a Polyglot book |
| `UCI_Chess960` | false | Chess960 notation (castling as king takes rook); GUIs set it automatically |
| `Clear Hash` |  | Button |

Other options (`RfpMargin`, `LmrBase`, ...) are internal search parameters exposed for testing and tuning; leave them at their defaults.

```
fructosita.exe              # UCI mode
fructosita.exe bench        # deterministic benchmark (node count and signature)
fructosita.exe perft 5      # move-generator self-check
```

`fructosita.exe bench depth 12` ends with `bench signature b5d90d2a54f488fd`.

### Chess960

- Standard `UCI_Chess960` option; positions in X-FEN (`KQkq` = outermost rook) or Shredder-FEN (`HAha`, file letters).
- With the option on, castling is sent and read as king to rook square (`e1h1`); with it off and a standard position, as usual (`e1g1`).
- `fructosita.exe frc N` prints start position N (0 to 959, Scharnagl numbering; 518 is the standard position).

## Building

```
cargo build --release --locked
cargo test --release --locked
```

The binary is written to `target/release/fructosita(.exe)`. Requires a recent stable Rust toolchain.

`.cargo/config.toml` enables POPCNT and links the C runtime statically. The executable attached to the release is built with profile-guided optimisation; the profile (`fructosita-3.4.0.profdata`) is attached too. A plain `cargo build` plays exactly the same moves, a few percent slower.

## Requirements

Windows x64 and a CPU with POPCNT (a processor that supports the POPCNT instruction). On a CPU without POPCNT the engine prints a message and exits.

## Version history

See the **Releases** page for binaries, source archives and SHA-256 checksums of every published version, and
`CHANGELOG.md` for the complete list of internal steps and their measurements.

## Security

Fructosita is a local engine and does not process network input. Opening books, EPD and PGN files from unknown
sources should still be treated as untrusted. See [`SECURITY.md`](SECURITY.md).

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE). © 2026 Antonio Espinosa.
