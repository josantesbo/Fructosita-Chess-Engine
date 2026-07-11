//! Estructura de peones: doblados y aislados. Los peones pasados tienen su
//! propio término, más rico, en `passed_pawns.rs`.

use crate::bitboard::{count_bits, file_mask, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;

pub fn pawn_structure(board: &Board, color: Color) -> Score {
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
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

    Score::new(mg, eg)
}
