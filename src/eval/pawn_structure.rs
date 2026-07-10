//! Estructura de peones: doblados, aislados, y pasados.

use crate::bitboard::{count_bits, file_mask, get_bit, pop_lsb, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;

pub fn pawn_structure(board: &Board, color: Color) -> Score {
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let mut mg = 0;
    let mut eg = 0;

    for file in 0u8..8 {
        let count_on_file = count_bits(own_pawns & file_mask(file)) as i32;
        if count_on_file > 1 {
            mg -= 10 * (count_on_file - 1);
            eg -= 20 * (count_on_file - 1);
        }
        if count_on_file > 0 {
            let mut adjacent: u64 = 0;
            if file > 0 {
                adjacent |= file_mask(file - 1);
            }
            if file < 7 {
                adjacent |= file_mask(file + 1);
            }
            if own_pawns & adjacent == EMPTY {
                mg -= 12;
                eg -= 16;
            }
        }
    }

    // Peones pasados: bonus creciente según lo avanzados que estén.
    let mut bb = own_pawns;
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        let file = file_of(sq);
        let rank = rank_of(sq);
        let lo_file = file.saturating_sub(1);
        let hi_file = (file + 1).min(7);

        let mut blocked = false;
        if color == Color::White {
            for r in (rank + 1)..8 {
                for f in lo_file..=hi_file {
                    if get_bit(enemy_pawns, make_square(f, r)) {
                        blocked = true;
                    }
                }
            }
        } else {
            for r in 0..rank {
                for f in lo_file..=hi_file {
                    if get_bit(enemy_pawns, make_square(f, r)) {
                        blocked = true;
                    }
                }
            }
        }
        if !blocked {
            let advance = if color == Color::White {
                rank
            } else {
                7 - rank
            };
            let bonus = (advance as i32) * (advance as i32) * 3;
            mg += bonus / 2;
            eg += bonus;
        }
    }

    Score::new(mg, eg)
}
