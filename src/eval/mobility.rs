//! Movilidad: número de casillas atacadas por cada pieza menor/mayor.

use crate::bitboard::{count_bits, pop_lsb, tables, EMPTY};
use crate::board::Board;
use crate::types::*;

pub fn mobility(board: &Board, color: Color) -> (i32, i32) {
    let t = tables();
    let occ = board.occupancy();
    let own = board.color_occupancy(color);
    let mut count = 0i32;

    let mut bb = board.pieces[color.index()][PieceType::Knight.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.knight_attacks(sq) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Bishop.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.bishop_attacks(sq, occ) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Rook.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.rook_attacks(sq, occ) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Queen.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.queen_attacks(sq, occ) & !own) as i32;
    }

    (count * 4, count * 3)
}
