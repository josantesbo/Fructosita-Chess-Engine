//! Seguridad del rey: heurística ligera sobre columnas abiertas/semi-abiertas.

use crate::bitboard::{file_mask, EMPTY};
use crate::board::Board;
use crate::types::*;

/// Heurística ligera: penaliza columnas abiertas/semi-abiertas junto al rey
/// cuando este todavía está cerca de su casa (aprox. "sigue enrocado o sin
/// desarrollar"). No pretende ser un modelo completo de seguridad del rey;
/// eso se refina más adelante con datos reales.
pub fn king_safety(board: &Board, color: Color) -> i32 {
    let king_sq = board.king_square(color);
    let file = file_of(king_sq);
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];

    let near_home = match color {
        Color::White => rank_of(king_sq) <= 1,
        Color::Black => rank_of(king_sq) >= 6,
    };
    if !near_home {
        return 0;
    }

    let mut score = 0;
    let lo = file.saturating_sub(1);
    let hi = (file + 1).min(7);
    for f in lo..=hi {
        if own_pawns & file_mask(f) == EMPTY {
            score -= 15;
        }
    }
    score
}
