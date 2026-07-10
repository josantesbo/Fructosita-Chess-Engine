//! Generación de movimientos.
//!
//! Estrategia de legalidad: se generan movimientos pseudo-legales (incluido
//! el enroque, que valida sus propias condiciones de jaque) y luego se
//! filtran simulando cada uno con `Board::make_move` y comprobando si el
//! propio rey queda en jaque. Este método "fuerza bruta" es robusto frente a
//! casos difíciles (clavadas, jaques descubiertos por captura al paso, etc.)
//! a cambio de algo de rendimiento, que se puede optimizar más adelante.

use crate::bitboard::{get_bit, pop_lsb, tables, EMPTY};
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

/// Movimientos legales: pseudo-legales filtrados comprobando que el propio
/// rey no quede en jaque tras simular el movimiento.
pub fn generate_legal_moves(board: &Board) -> Vec<Move> {
    let us = board.side_to_move;
    generate_pseudo_legal_moves(board)
        .into_iter()
        .filter(|&mv| !board.make_move(mv).in_check(us))
        .collect()
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
