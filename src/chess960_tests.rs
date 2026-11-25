//! Reglas de Chess960 sobre el tablero y el generador. Los valores esperados
//! (enroques legales, número de jugadas y perft 2) salen de python-chess, y
//! los perft de `testdata/chess960_perft.epd`, de Stockfish 11: ninguno de la
//! propia implementación.

use crate::board::Board;
use crate::chess960::start_fen;
use crate::movegen::{find_move, generate_legal_moves};
use crate::perft::perft;
use crate::types::*;

type Case = (&'static str, &'static str, &'static [&'static str], usize, u64);

const CASES: &[Case] = &[
    ("rey ya en su casilla de llegada", "4k3/8/8/8/8/8/8/6KR w H - 0 1", &["g1h1"], 12, 53),
    ("torre ya en su casilla de llegada", "4k3/8/8/8/8/8/8/4KR2 w F - 0 1", &["e1f1"], 14, 47),
    ("rey y torre intercambian casillas", "4k3/8/8/8/8/8/8/5KR1 w G - 0 1", &["f1g1"], 13, 58),
    ("rey y torre se cruzan (largo)", "4k3/8/8/8/8/8/8/RK6 w A - 0 1", &["b1a1"], 12, 53),
    ("ruta de la torre ocupada (d1)", "4k3/8/8/8/8/8/8/RK1N4 w A - 0 1", &[], 15, 70),
    ("llegada del rey ocupada (c1)", "4k3/8/8/8/8/8/8/RKB5 w A - 0 1", &[], 17, 75),
    ("rey en jaque", "4k3/8/8/8/8/8/8/RK4r1 w A - 0 1", &[], 3, 57),
    ("el rey cruza una casilla atacada", "4kr2/8/8/8/8/8/8/4K2R w H - 0 1", &[], 12, 145),
    ("llegada del rey atacada", "4k1r1/8/8/8/8/8/8/4K2R w H - 0 1", &[], 14, 184),
    ("llegada del rey atacada por dama", "4k3/8/8/8/8/8/7q/4K2R w H - 0 1", &[], 5, 107),
    ("solo la ruta de la torre atacada: legal", "1r2k3/8/8/8/8/8/8/R3K3 w A - 0 1", &["e1a1"], 16, 226),
    ("torre atacada en su casilla: legal", "4k2r/8/8/8/8/8/8/4K2R w H - 0 1", &["e1h1"], 15, 171),
    ("la torre deja de tapar la primera fila (rey quieto)", "4k3/8/8/8/8/8/8/qRK5 w B - 0 1", &[], 4, 65),
    ("la torre deja de tapar la primera fila (rey llega)", "4k3/8/8/8/8/8/8/qR2K3 w B - 0 1", &[], 8, 146),
    ("rey lejos de su llegada", "4k3/8/8/8/8/8/8/1KR5 w C - 0 1", &["b1c1"], 17, 73),
    ("la torre que enroca está en la ruta del rey", "4k3/8/8/8/8/8/8/RK3R2 w AF - 0 1", &["b1a1", "b1f1"], 25, 76),
    ("el rey cruza la casilla atacada de su torre", "1rk1r3/8/8/8/8/8/8/1RK1R3 w EBeb - 0 1", &["c1b1"], 23, 442),
    ("negras, rey ya en c8", "1rk5/8/8/8/8/8/8/4K3 b b - 0 1", &["c8b8"], 13, 58),
    ("paso de rey y enroque con las mismas casillas", "4k3/8/8/8/8/8/8/5K1R w H - 0 1", &["f1h1"], 14, 63),
    ("X-FEN: K es la torre exterior", "4k3/8/8/8/8/8/8/1K3R1R w K - 0 1", &[], 24, 73),
    ("Shredder-FEN: torre interior", "4k3/8/8/8/8/8/8/1K3R1R w F - 0 1", &["b1f1"], 25, 76),
];

fn play(b: &Board, mv: &str) -> Board {
    b.make_move(find_move(b, mv).unwrap_or_else(|| panic!("{mv} no es legal en {}", b.to_fen())))
}

fn rights(b: &Board) -> (bool, bool, bool, bool) {
    let c = &b.castling;
    (c.white_kingside, c.white_queenside, c.black_kingside, c.black_queenside)
}

#[test]
fn castling_edge_cases_match_reference() {
    for &(name, fen, castles, moves, perft2) in CASES {
        let b = Board::from_fen(fen).unwrap();
        let legal = generate_legal_moves(&b);
        // Siempre en notación Chess960 (rey captura torre), como la referencia.
        let rook_of = |m: &crate::moves::Move| b.castle_rooks[b.side_to_move.index() * 2 + usize::from(m.kind == crate::moves::MoveKind::CastleQueenside)];
        let mut ours: Vec<String> = legal
            .iter()
            .filter(|m| m.is_castle())
            .map(|m| format!("{}{}", square_to_str(m.from), square_to_str(rook_of(m))))
            .collect();
        ours.sort();
        assert_eq!(ours, castles, "{name}: {fen}");
        assert_eq!(legal.len(), moves, "{name}: {fen}");
        assert_eq!(perft(&b, 2), perft2, "{name}: {fen}");
    }
}

/// Igualdad completa con el tablero reconstruido desde su propio FEN.
fn assert_consistent(b: &Board) {
    let r = Board::from_fen(&b.to_fen()).unwrap_or_else(|e| panic!("{e}: {}", b.to_fen()));
    assert_eq!(b.hash, r.hash, "hash: {}", b.to_fen());
    assert_eq!(b.castling, r.castling, "derechos: {}", b.to_fen());
    assert_eq!(b.pieces, r.pieces, "piezas: {}", b.to_fen());
    assert_eq!(b.mailbox, r.mailbox, "mailbox: {}", b.to_fen());
    assert_eq!(b.occ, r.occ, "ocupación: {}", b.to_fen());
    assert_eq!(b.psq, r.psq, "material+PST: {}", b.to_fen());
    assert_eq!(
        (b.en_passant, b.halfmove_clock, b.fullmove_number),
        (r.en_passant, r.halfmove_clock, r.fullmove_number)
    );
    let alive = rights(b);
    for (i, on) in [alive.0, alive.1, alive.2, alive.3].into_iter().enumerate() {
        if on {
            assert_eq!(b.castle_rooks[i], r.castle_rooks[i], "torre de enroque {i}: {}", b.to_fen());
            // Un derecho vivo implica rey en su primera fila y torre propia en su casilla.
            let color = if i < 2 { Color::White } else { Color::Black };
            assert_eq!(b.mailbox[b.castle_rooks[i] as usize], Some(Piece::new(color, PieceType::Rook)));
            assert_eq!(rank_of(b.king_square(color)), if i < 2 { 0 } else { 7 });
        }
    }
}

fn walk(b: &Board, depth: u32, n: &mut u64) {
    assert_consistent(b);
    if depth == 0 {
        return;
    }
    let ci = b.check_info();
    for mv in generate_legal_moves(b) {
        let next = b.make_move(mv);
        // La jugada se relee desde su texto UCI y los datos de jaque coinciden.
        assert_eq!(find_move(b, &b.move_uci(mv)), Some(mv), "{} en {}", b.move_uci(mv), b.to_fen());
        assert_eq!(b.gives_check_with(mv, &ci), next.in_check(next.side_to_move));
        assert!(!next.in_check(b.side_to_move));
        *n += 1;
        walk(&next, depth - 1, n);
    }
}

#[test]
fn state_is_consistent_after_every_move() {
    let mut n = 0;
    for &(_, fen, ..) in CASES {
        walk(&Board::from_fen(fen).unwrap(), 3, &mut n);
    }
    for idx in (0..960).step_by(97) {
        walk(&Board::from_fen(&start_fen(idx).unwrap()).unwrap(), 3, &mut n);
    }
    for line in include_str!("../testdata/chess960_perft.epd").lines().take(40) {
        walk(&Board::from_fen(line.split(" ;").next().unwrap()).unwrap(), 2, &mut n);
    }
    assert!(n > 100_000, "{n}");
}

#[test]
fn perft_matches_stockfish_reference() {
    let mut positions = 0;
    for line in include_str!("../testdata/chess960_perft.epd").lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.split(" ;");
        let b = Board::from_fen(parts.next().unwrap()).unwrap();
        for field in parts {
            let (d, count) = field.split_once(' ').unwrap();
            let depth: u32 = d[1..].parse().unwrap();
            if depth <= 3 || positions < 12 {
                assert_eq!(perft(&b, depth), count.trim().parse::<u64>().unwrap(), "{line}");
            }
        }
        positions += 1;
    }
    assert!(positions >= 100);
}

#[test]
fn castling_rights_follow_the_rooks() {
    // Rey b1, torres a1 y f1; rey negro g8, torres a8 y h8.
    let b = Board::from_fen("r5kr/8/8/8/8/8/8/RK3R2 w AFha - 0 1").unwrap();
    assert_eq!(b.castle_rooks, [F1, A1, H8, A8]);
    // La torre de f1 se va: solo se pierde el corto blanco, aunque vuelva.
    let x = play(&b, "f1f5");
    assert_eq!(rights(&x), (false, true, true, true));
    let back = play(&play(&x, "g8g7"), "f5f1");
    assert_eq!(rights(&back), (false, true, false, false));
    assert_ne!(back.hash, Board::from_fen("r6r/6k1/8/8/8/8/8/RK3R2 b AF - 0 1").unwrap().hash);
    assert_eq!(back.hash, Board::from_fen("r6r/6k1/8/8/8/8/8/RK3R2 b A - 0 1").unwrap().hash);
    // Capturar una torre en su casilla quita ese derecho a su dueño (y nada más).
    assert_eq!(rights(&play(&b, "a1a8")), (true, false, true, false));
    assert_eq!(rights(&play(&play(&b, "a1a7"), "h8h1")), (true, false, false, true));
    assert_eq!(rights(&play(&play(&play(&b, "a1a7"), "h8h1"), "f1h1")), (false, false, false, true));
    // Mover el rey pierde los dos; enrocar también.
    assert_eq!(rights(&play(&b, "b1b2")), (false, false, true, true));
    let short = play(&b, "b1f1");
    assert_eq!(rights(&short), (false, false, true, true));
    assert_eq!(short.to_fen(), "r5kr/8/8/8/8/8/8/R4RK1 b ha - 1 1");
    assert_eq!(play(&b, "b1a1").to_fen(), "r5kr/8/8/8/8/8/8/2KR1R2 b ha - 1 1");
}

#[test]
fn uci_castling_notation() {
    // Disposición estándar: e1g1 por defecto; se acepta también e1h1.
    let std = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    let oo = find_move(&std, "e1g1").unwrap();
    assert!(oo.is_castle());
    assert_eq!(std.move_uci(oo), "e1g1");
    assert_eq!(find_move(&std, "e1h1"), Some(oo));
    assert_eq!(std.to_fen(), "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert_eq!(Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w HAha - 0 1").unwrap().hash, std.hash);
    // Disposición Chess960: f1g1 es un paso de rey y f1h1 el enroque.
    let frc = Board::from_fen("4k3/8/8/8/8/8/8/5K1R w H - 0 1").unwrap();
    let step = find_move(&frc, "f1g1").unwrap();
    let castle = find_move(&frc, "f1h1").unwrap();
    assert!(!step.is_castle() && castle.is_castle());
    assert_eq!((castle.from, castle.to), (step.from, step.to));
    assert_ne!(frc.make_move(step).hash, frc.make_move(castle).hash);
    assert_eq!(frc.make_move(castle).to_fen(), "4k3/8/8/8/8/8/8/5RK1 b - - 1 1");
    assert_eq!(frc.to_fen(), "4k3/8/8/8/8/8/8/5K1R w H - 0 1");
    // Un derecho imposible (sin torre o con el rey fuera de su fila) se descarta.
    assert_eq!(rights(&Board::from_fen("4k3/8/8/8/8/8/8/4K3 w KQkq - 0 1").unwrap()), (false, false, false, false));
    assert_eq!(rights(&Board::from_fen("4k2r/8/8/8/8/8/4K3/R6R w KQk - 0 1").unwrap()), (false, false, true, false));
}

#[test]
fn repetition_keys_in_chess960() {
    // Los caballos van y vuelven: misma clave. Una torre que va y vuelve
    // pierde su derecho: clave distinta aunque las piezas coincidan.
    let start = Board::from_fen(&start_fen(0).unwrap()).unwrap(); // BBQNNRKR
    let mut b = start;
    for mv in ["d1c3", "d8c6", "c3d1", "c6d8"] {
        b = play(&b, mv);
    }
    assert_eq!(b.hash, start.hash);
    assert_eq!(b.halfmove_clock, 4);
    let open = Board::from_fen("bbqnnrkr/ppppppp1/8/7p/7P/8/PPPPPPP1/BBQNNRKR w HFhf - 0 2").unwrap();
    let mut c = open;
    for mv in ["h1h3", "h8h6", "h3h1", "h6h8"] {
        c = play(&c, mv);
    }
    assert_eq!(c.pieces, open.pieces);
    assert_ne!(c.hash, open.hash);
    assert_eq!(c.to_fen(), "bbqnnrkr/ppppppp1/8/7p/7P/8/PPPPPPP1/BBQNNRKR w Ff - 4 4");
}
