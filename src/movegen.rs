//! Generación de movimientos.
//!
//! Estrategia de legalidad: se generan movimientos pseudo-legales (incluido
//! el enroque, que valida sus propias condiciones de jaque) y luego se
//! filtran simulando cada uno con `Board::make_move` y comprobando si el
//! propio rey queda en jaque. Este método "fuerza bruta" es robusto frente a
//! casos difíciles (clavadas, jaques descubiertos por captura al paso, etc.)
//! a cambio de algo de rendimiento, que se puede optimizar más adelante.

use crate::bitboard::{count_bits, get_bit, lsb, pop_lsb, set_bit, tables, Bitboard, EMPTY};
use crate::board::Board;
use crate::moves::{Move, MoveKind};
use crate::types::*;

const PROMOTION_PIECES: [PieceType; 4] = [
    PieceType::Queen,
    PieceType::Rook,
    PieceType::Bishop,
    PieceType::Knight,
];

fn generate_pawn_moves(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let them = us.opposite();
    let occ = board.occupancy();
    let enemy_occ = board.color_occupancy(them);
    let t = tables();

    let mut pawns = board.pieces[us.index()][PieceType::Pawn.index()];
    let (push_delta, start_rank, promo_rank): (i32, u8, u8) = if us == Color::White {
        (8, 1, 7)
    } else {
        (-8, 6, 0)
    };

    while pawns != EMPTY {
        let from = pop_lsb(&mut pawns);

        let to = (from as i32 + push_delta) as Square;
        if !get_bit(occ, to) {
            if rank_of(to) == promo_rank {
                for &p in PROMOTION_PIECES.iter() {
                    moves.push(Move::new(from, to, MoveKind::Promotion(p)));
                }
            } else {
                moves.push(Move::new(from, to, MoveKind::Quiet));
                if rank_of(from) == start_rank {
                    let to2 = (from as i32 + 2 * push_delta) as Square;
                    if !get_bit(occ, to2) {
                        moves.push(Move::new(from, to2, MoveKind::DoublePawnPush));
                    }
                }
            }
        }

        let mut attacks = t.pawn_attacks(us, from) & enemy_occ;
        while attacks != EMPTY {
            let to = pop_lsb(&mut attacks);
            if rank_of(to) == promo_rank {
                for &p in PROMOTION_PIECES.iter() {
                    moves.push(Move::new(from, to, MoveKind::PromotionCapture(p)));
                }
            } else {
                moves.push(Move::new(from, to, MoveKind::Capture));
            }
        }

        if let Some(ep) = board.en_passant {
            if get_bit(t.pawn_attacks(us, from), ep) {
                moves.push(Move::new(from, ep, MoveKind::EnPassantCapture));
            }
        }
    }
}

fn generate_knight_moves(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let own_occ = board.color_occupancy(us);
    let enemy_occ = board.color_occupancy(us.opposite());
    let t = tables();
    let mut knights = board.pieces[us.index()][PieceType::Knight.index()];
    while knights != EMPTY {
        let from = pop_lsb(&mut knights);
        let mut targets = t.knight_attacks(from) & !own_occ;
        while targets != EMPTY {
            let to = pop_lsb(&mut targets);
            let kind = if get_bit(enemy_occ, to) {
                MoveKind::Capture
            } else {
                MoveKind::Quiet
            };
            moves.push(Move::new(from, to, kind));
        }
    }
}

fn generate_king_moves(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let own_occ = board.color_occupancy(us);
    let enemy_occ = board.color_occupancy(us.opposite());
    let t = tables();
    let from = board.king_square(us);
    let mut targets = t.king_attacks(from) & !own_occ;
    while targets != EMPTY {
        let to = pop_lsb(&mut targets);
        let kind = if get_bit(enemy_occ, to) {
            MoveKind::Capture
        } else {
            MoveKind::Quiet
        };
        moves.push(Move::new(from, to, kind));
    }
}

fn generate_sliding_moves(board: &Board, piece: PieceType, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let own_occ = board.color_occupancy(us);
    let enemy_occ = board.color_occupancy(us.opposite());
    let occ = board.occupancy();
    let t = tables();
    let mut pieces_bb = board.pieces[us.index()][piece.index()];
    while pieces_bb != EMPTY {
        let from = pop_lsb(&mut pieces_bb);
        let attacks = match piece {
            PieceType::Bishop => t.bishop_attacks(from, occ),
            PieceType::Rook => t.rook_attacks(from, occ),
            PieceType::Queen => t.queen_attacks(from, occ),
            _ => unreachable!("generate_sliding_moves solo admite alfil/torre/dama"),
        };
        let mut targets = attacks & !own_occ;
        while targets != EMPTY {
            let to = pop_lsb(&mut targets);
            let kind = if get_bit(enemy_occ, to) {
                MoveKind::Capture
            } else {
                MoveKind::Quiet
            };
            moves.push(Move::new(from, to, kind));
        }
    }
}

fn generate_castling(board: &Board, moves: &mut Vec<Move>) {
    let us = board.side_to_move;
    let occ = board.occupancy();
    let enemy = us.opposite();

    let (king_home, kingside_right, queenside_right, f, g, d, c, b) = match us {
        Color::White => (
            E1,
            board.castling.white_kingside,
            board.castling.white_queenside,
            F1,
            G1,
            D1,
            C1,
            B1,
        ),
        Color::Black => (
            E8,
            board.castling.black_kingside,
            board.castling.black_queenside,
            F8,
            G8,
            D8,
            C8,
            B8,
        ),
    };

    if kingside_right
        && !get_bit(occ, f)
        && !get_bit(occ, g)
        && !board.is_square_attacked(king_home, enemy)
        && !board.is_square_attacked(f, enemy)
        && !board.is_square_attacked(g, enemy)
    {
        moves.push(Move::new(king_home, g, MoveKind::CastleKingside));
    }

    if queenside_right
        && !get_bit(occ, d)
        && !get_bit(occ, c)
        && !get_bit(occ, b)
        && !board.is_square_attacked(king_home, enemy)
        && !board.is_square_attacked(d, enemy)
        && !board.is_square_attacked(c, enemy)
    {
        moves.push(Move::new(king_home, c, MoveKind::CastleQueenside));
    }
}

pub fn generate_pseudo_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(48);
    generate_pawn_moves(board, &mut moves);
    generate_knight_moves(board, &mut moves);
    generate_sliding_moves(board, PieceType::Bishop, &mut moves);
    generate_sliding_moves(board, PieceType::Rook, &mut moves);
    generate_sliding_moves(board, PieceType::Queen, &mut moves);
    generate_king_moves(board, &mut moves);
    generate_castling(board, &mut moves);
    moves
}

/// PROPUESTA 33: capturas y promociones pseudo-legales, y NADA MAS.
///
/// Recorre las mismas piezas en el mismo orden que `generate_pseudo_legal_moves`
/// y, dentro de cada pieza, el mismo recorrido de bitboards por LSB, con la
/// mascara restringida a `enemy_occ`. Filtrar una secuencia LSB y recorrer el
/// subconjunto por LSB dan la misma secuencia, asi que el orden relativo de las
/// capturas es identico. `generate_castling` se omite entero: el enroque no es
/// captura ni promocion. Verificado sobre 25.775.209 nodos sin discrepancias
/// (`capt-02-diagnostico-fase1.md` §4).
pub fn generate_pseudo_legal_captures(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(16);
    let us = board.side_to_move;
    let them = us.opposite();
    let occ = board.occupancy();
    let enemy_occ = board.color_occupancy(them);
    let own_occ = board.color_occupancy(us);
    let t = tables();

    // --- peones ---
    let mut pawns = board.pieces[us.index()][PieceType::Pawn.index()];
    let (push_delta, promo_rank): (i32, u8) = if us == Color::White { (8, 7) } else { (-8, 0) };
    while pawns != EMPTY {
        let from = pop_lsb(&mut pawns);
        // promocion por empuje: no es captura, pero SI es promocion
        let to = (from as i32 + push_delta) as Square;
        if !get_bit(occ, to) && rank_of(to) == promo_rank {
            for &p in PROMOTION_PIECES.iter() {
                moves.push(Move::new(from, to, MoveKind::Promotion(p)));
            }
        }
        let mut attacks = t.pawn_attacks(us, from) & enemy_occ;
        while attacks != EMPTY {
            let to = pop_lsb(&mut attacks);
            if rank_of(to) == promo_rank {
                for &p in PROMOTION_PIECES.iter() {
                    moves.push(Move::new(from, to, MoveKind::PromotionCapture(p)));
                }
            } else {
                moves.push(Move::new(from, to, MoveKind::Capture));
            }
        }
        if let Some(ep) = board.en_passant {
            if get_bit(t.pawn_attacks(us, from), ep) {
                moves.push(Move::new(from, ep, MoveKind::EnPassantCapture));
            }
        }
    }
    // --- caballos ---
    let mut knights = board.pieces[us.index()][PieceType::Knight.index()];
    while knights != EMPTY {
        let from = pop_lsb(&mut knights);
        let mut targets = t.knight_attacks(from) & !own_occ & enemy_occ;
        while targets != EMPTY {
            let to = pop_lsb(&mut targets);
            moves.push(Move::new(from, to, MoveKind::Capture));
        }
    }
    // --- deslizantes, en el mismo orden: alfil, torre, dama ---
    for piece in [PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
        let mut pieces_bb = board.pieces[us.index()][piece.index()];
        while pieces_bb != EMPTY {
            let from = pop_lsb(&mut pieces_bb);
            let attacks = match piece {
                PieceType::Bishop => t.bishop_attacks(from, occ),
                PieceType::Rook => t.rook_attacks(from, occ),
                _ => t.queen_attacks(from, occ),
            };
            let mut targets = attacks & !own_occ & enemy_occ;
            while targets != EMPTY {
                let to = pop_lsb(&mut targets);
                moves.push(Move::new(from, to, MoveKind::Capture));
            }
        }
    }
    // --- rey ---
    let from = board.king_square(us);
    let mut targets = t.king_attacks(from) & !own_occ & enemy_occ;
    while targets != EMPTY {
        let to = pop_lsb(&mut targets);
        moves.push(Move::new(from, to, MoveKind::Capture));
    }
    moves
}

/// PROPUESTA 33: capturas legales. Mismo `LegalityContext` de la inv. 30, sin
/// tocarlo, y el filtro en el mismo sitio de la misma cadena.
pub fn generate_legal_captures(board: &Board) -> Vec<Move> {
    let ctx = LegalityContext::new(board);
    generate_pseudo_legal_captures(board)
        .into_iter()
        .filter(|&mv| ctx.is_legal(board, mv))
        .collect()
}

/// Movimientos legales: pseudo-legales filtrados comprobando que el propio
/// rey no quede en jaque tras simular el movimiento.
pub fn generate_legal_moves(board: &Board) -> Vec<Move> {
    // INV. 30: el predicado de legalidad sustituye a `make_move` + `in_check`.
    // El generador pseudo-legal, su orden y la posicion del filtro NO cambian:
    // un `filter` con predicado equivalente sobre la misma secuencia no puede
    // reordenar nada. Especificacion verificada sobre 1.715.545.199 jugadas
    // (legal-02-diagnostico-fase1.md, 0 discrepancias).
    let ctx = LegalityContext::new(board);
    generate_pseudo_legal_moves(board)
        .into_iter()
        .filter(|&mv| ctx.is_legal(board, mv))
        .collect()
}

/// Datos de legalidad del nodo. Se calculan UNA vez por llamada a
/// `generate_legal_moves` y sirven para todas sus jugadas.
pub struct LegalityContext {
    us: Color,
    them: Color,
    ksq: Square,
    occ: Bitboard,
    /// numero de jaques (0, 1 o 2)
    ncheck: u32,
    /// piezas propias absolutamente clavadas
    pinned: Bitboard,
    /// para cada casilla clavada, la recta rey-clavador (incluyendo al clavador)
    pin_ray: [Bitboard; 64],
    /// casillas que resuelven un jaque simple (capturar o bloquear)
    checkmask: Bitboard,
}

/// Casillas ESTRICTAMENTE entre `a` y `b` si estan en linea o diagonal; 0 si no.
#[inline]
fn between_sq(a: Square, b: Square) -> Bitboard {
    let (af, ar) = (file_of(a) as i32, rank_of(a) as i32);
    let (bf, br) = (file_of(b) as i32, rank_of(b) as i32);
    let (df, dr) = (bf - af, br - ar);
    if !(df == 0 || dr == 0 || df.abs() == dr.abs()) {
        return EMPTY;
    }
    let (sf, sr) = (df.signum(), dr.signum());
    let mut bb = EMPTY;
    let (mut f, mut r) = (af + sf, ar + sr);
    while (f, r) != (bf, br) {
        if !(0..8).contains(&f) || !(0..8).contains(&r) {
            return EMPTY;
        }
        bb = set_bit(bb, make_square(f as u8, r as u8));
        f += sf;
        r += sr;
    }
    bb
}

/// Piezas de `by` que atacan `sq` con la ocupacion `occ` dada.
#[inline]
fn attackers_to(board: &Board, sq: Square, by: Color, occ: Bitboard) -> Bitboard {
    let t = tables();
    let i = by.index();
    let mut a = t.pawn_attacks(by.opposite(), sq) & board.pieces[i][PieceType::Pawn.index()];
    a |= t.knight_attacks(sq) & board.pieces[i][PieceType::Knight.index()];
    a |= t.king_attacks(sq) & board.pieces[i][PieceType::King.index()];
    let bq = board.pieces[i][PieceType::Bishop.index()] | board.pieces[i][PieceType::Queen.index()];
    a |= t.bishop_attacks(sq, occ) & bq;
    let rq = board.pieces[i][PieceType::Rook.index()] | board.pieces[i][PieceType::Queen.index()];
    a |= t.rook_attacks(sq, occ) & rq;
    a
}

impl LegalityContext {
    pub fn new(board: &Board) -> Self {
        let t = tables();
        let us = board.side_to_move;
        let them = us.opposite();
        let ksq = board.king_square(us);
        let occ = board.occupancy();
        let ui = us.index();
        let mut own = EMPTY;
        for pt in 0..6 {
            own |= board.pieces[ui][pt];
        }

        let checkers = attackers_to(board, ksq, them, occ);
        let ncheck = count_bits(checkers);

        // Clavadas: rayos desde el rey sobre tablero VACIO hasta deslizantes
        // enemigas. Con `occ` se perderian los clavadores tapados.
        let ei = them.index();
        let enemy_bq =
            board.pieces[ei][PieceType::Bishop.index()] | board.pieces[ei][PieceType::Queen.index()];
        let enemy_rq =
            board.pieces[ei][PieceType::Rook.index()] | board.pieces[ei][PieceType::Queen.index()];
        let mut snipers =
            (t.bishop_attacks(ksq, EMPTY) & enemy_bq) | (t.rook_attacks(ksq, EMPTY) & enemy_rq);
        let mut pinned = EMPTY;
        let mut pin_ray = [EMPTY; 64];
        while snipers != EMPTY {
            let s = pop_lsb(&mut snipers);
            let btw = between_sq(ksq, s);
            let blockers = btw & occ;
            if count_bits(blockers) == 1 && (blockers & own) != EMPTY {
                let sq = lsb(blockers);
                pinned = set_bit(pinned, sq);
                pin_ray[sq as usize] = btw | set_bit(EMPTY, s);
            }
        }

        let checkmask = if ncheck == 1 {
            let c = lsb(checkers);
            checkers | between_sq(ksq, c)
        } else {
            !EMPTY
        };

        LegalityContext {
            us,
            them,
            ksq,
            occ,
            ncheck,
            pinned,
            pin_ray,
            checkmask,
        }
    }

    pub fn is_legal(&self, board: &Board, mv: Move) -> bool {
        let to_bb = set_bit(EMPTY, mv.to);

        if mv.kind == MoveKind::EnPassantCapture {
            // Desaparecen DOS peones de la misma fila: la logica de clavadas no
            // lo ve. Prueba explicita con la ocupacion modificada.
            let cap_sq = match self.us {
                Color::White => mv.to - 8,
                Color::Black => mv.to + 8,
            };
            let occ2 =
                (self.occ & !set_bit(EMPTY, mv.from) & !set_bit(EMPTY, cap_sq)) | to_bb;
            return (attackers_to(board, self.ksq, self.them, occ2) & !set_bit(EMPTY, cap_sq))
                == EMPTY;
        }

        if mv.from == self.ksq {
            if mv.is_castle() {
                // `generate_castling` ya verifico origen, transito y destino.
                return true;
            }
            // El rey se RETIRA de la ocupacion: si no, se tapa a si mismo el rayo
            // y una huida sobre la linea del jaque se declararia legal.
            let occ2 = self.occ & !set_bit(EMPTY, self.ksq);
            return attackers_to(board, mv.to, self.them, occ2) == EMPTY;
        }

        if self.ncheck >= 2 {
            return false; // jaque doble: solo el rey puede mover
        }
        let from_bb = set_bit(EMPTY, mv.from);
        let ok_pin =
            (self.pinned & from_bb) == EMPTY || (self.pin_ray[mv.from as usize] & to_bb) != EMPTY;
        let ok_check = self.ncheck == 0 || (self.checkmask & to_bb) != EMPTY;
        ok_pin && ok_check
    }
}

/// EXP-0013: ¿es `mv` pseudo-legal en `board`, exactamente con el mismo tipo
/// que le daría el generador? Sirve para validar la jugada de la TT antes de
/// buscarla sin generar la lista. Los enroques devuelven `false` (se dejan al
/// camino normal, que los genera y valida). La legalidad respecto al jaque se
/// comprueba aparte, con `make_move` + `in_check`.
pub fn is_pseudo_legal(board: &Board, mv: Move) -> bool {
    let us = board.side_to_move;
    let Some(piece) = board.mailbox[mv.from as usize] else {
        return false;
    };
    if piece.color != us || mv.from == mv.to {
        return false;
    }
    let target = board.mailbox[mv.to as usize];
    if let Some(t) = target {
        if t.color == us || t.kind == PieceType::King {
            return false;
        }
    }
    let t = tables();
    let occ = board.occupancy();
    let to_bb = set_bit(EMPTY, mv.to);
    let (push, start_rank, promo_rank): (i32, u8, u8) =
        if us == Color::White { (8, 1, 7) } else { (-8, 6, 0) };
    let is_pawn = piece.kind == PieceType::Pawn;
    let single = mv.from as i32 + push;
    let piece_attacks = || match piece.kind {
        PieceType::Knight => t.knight_attacks(mv.from),
        PieceType::Bishop => t.bishop_attacks(mv.from, occ),
        PieceType::Rook => t.rook_attacks(mv.from, occ),
        PieceType::Queen => t.queen_attacks(mv.from, occ),
        PieceType::King => t.king_attacks(mv.from),
        PieceType::Pawn => EMPTY,
    };
    match mv.kind {
        MoveKind::Quiet => {
            if target.is_some() {
                return false;
            }
            if is_pawn {
                mv.to as i32 == single && rank_of(mv.to) != promo_rank
            } else {
                piece_attacks() & to_bb != EMPTY
            }
        }
        MoveKind::DoublePawnPush => {
            is_pawn
                && rank_of(mv.from) == start_rank
                && mv.to as i32 == mv.from as i32 + 2 * push
                && target.is_none()
                && board.mailbox[single as usize].is_none()
        }
        MoveKind::Capture => {
            if target.is_none() {
                return false;
            }
            if is_pawn {
                t.pawn_attacks(us, mv.from) & to_bb != EMPTY && rank_of(mv.to) != promo_rank
            } else {
                piece_attacks() & to_bb != EMPTY
            }
        }
        MoveKind::EnPassantCapture => {
            is_pawn
                && board.en_passant == Some(mv.to)
                && target.is_none()
                && t.pawn_attacks(us, mv.from) & to_bb != EMPTY
        }
        MoveKind::CastleKingside | MoveKind::CastleQueenside => false,
        MoveKind::Promotion(p) => {
            is_pawn
                && p != PieceType::Pawn
                && p != PieceType::King
                && target.is_none()
                && mv.to as i32 == single
                && rank_of(mv.to) == promo_rank
        }
        MoveKind::PromotionCapture(p) => {
            is_pawn
                && p != PieceType::Pawn
                && p != PieceType::King
                && target.is_some()
                && rank_of(mv.to) == promo_rank
                && t.pawn_attacks(us, mv.from) & to_bb != EMPTY
        }
    }
}

/// Busca, entre los movimientos legales, el que corresponde a la notación
/// UCI dada (p. ej. "e2e4", "e7e8q"). Útil para procesar `position ... moves ...`.
pub fn find_move(board: &Board, uci_str: &str) -> Option<Move> {
    generate_legal_moves(board)
        .into_iter()
        .find(|mv| mv.to_string() == uci_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_has_20_legal_moves() {
        let b = Board::start_pos();
        assert_eq!(generate_legal_moves(&b).len(), 20);
    }

    #[test]
    fn pinned_knight_has_no_legal_moves() {
        // Rey blanco e2, caballo blanco e3, torre negra e8: el caballo está
        // clavado en la columna e. Como el caballo no puede moverse en línea
        // recta, una clavada absoluta lo deja sin ningún movimiento legal.
        let b = Board::from_fen("k3r3/8/8/8/8/4N3/4K3/8 w - - 0 1").unwrap();
        let legal = generate_legal_moves(&b);
        let e3 = str_to_square("e3").unwrap();
        assert!(legal.iter().all(|mv| mv.from != e3));
    }

    #[test]
    fn en_passant_discovered_check_is_illegal() {
        // Rey blanco e5, peón blanco d5, peón negro acaba de jugar c7-c5,
        // torre negra a5: capturar dxc6 al paso destaparía la 5ª fila
        // completa y dejaría al propio rey en jaque, así que debe ser ilegal.
        let b = Board::from_fen("4k3/8/8/r1pPK3/8/8/8/8 w - c6 0 1").unwrap();
        let legal = generate_legal_moves(&b);
        assert!(legal.iter().all(|mv| mv.kind != MoveKind::EnPassantCapture));
    }
}

#[cfg(test)]
mod pseudo_legal_tests {
    use super::*;

    /// Para cada posición: toda jugada legal es pseudo-legal según el
    /// validador, y ninguna jugada legal de OTRA posición que no sea
    /// pseudo-legal aquí pasa el filtro (se contrasta con la lista generada).
    #[test]
    fn validator_matches_generator() {
        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        ];
        let boards: Vec<Board> = fens.iter().map(|f| Board::from_fen(f).unwrap()).collect();
        for b in &boards {
            let legal = generate_legal_moves(b);
            for &mv in &legal {
                if !mv.is_castle() {
                    assert!(is_pseudo_legal(b, mv), "legal no aceptada: {mv} en {}", b.to_fen());
                }
            }
            for other in &boards {
                for mv in generate_legal_moves(other) {
                    if is_pseudo_legal(b, mv) && !b.make_move(mv).in_check(b.side_to_move) {
                        assert!(legal.contains(&mv), "aceptada no legal: {mv} en {}", b.to_fen());
                    }
                }
            }
        }
    }
}
