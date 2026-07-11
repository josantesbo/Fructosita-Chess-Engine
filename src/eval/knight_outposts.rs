//! Outposts de caballo: bono por un caballo en una casilla avanzada que
//! ningún peón enemigo puede atacar jamás.
//!
//! Teoría: un caballo en un "agujero" (columna sin peón enemigo en las
//! columnas adyacentes que pueda llegar a atacarlo, ni ahora ni avanzando)
//! y suficientemente adentrado en territorio rival no puede ser expulsado
//! por un peón — solo por una pieza, y a menudo ni eso, si el rival no
//! tiene la pieza adecuada disponible. Es uno de los conceptos posicionales
//! más clásicos y ampliamente usados en HCE (Chess Programming Wiki,
//! "Outpost"): premia colocar el caballo donde es estructuralmente
//! imposible de echar con un peón, en vez de premiar solo casillas
//! centrales sin más (ya cubierto, de forma más genérica, por las PST).
//!
//! Deliberadamente aditivo: no toca el cálculo de ningún otro término
//! (material, movilidad, estructura de peones, peones pasados, seguridad
//! del rey) — solo añade una bonificación nueva e independiente, siguiendo
//! el mismo patrón que peones pasados (el único de los cuatro intentos
//! anteriores que sí ganó Elo de forma validada).
//!
//! Valores en fase de candidato: estimaciones conservadoras de partida, a
//! validar con match antes de tocarlas.

use crate::bitboard::{get_bit, pop_lsb, tables, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;

const OUTPOST_MG: i32 = 12;
const OUTPOST_EG: i32 = 6;
const OUTPOST_DEFENDED_BONUS_MG: i32 = 8;
const OUTPOST_DEFENDED_BONUS_EG: i32 = 4;

/// Filas suficientemente adentradas en territorio rival como para que un
/// caballo ahí realmente incomode (chess ranks 4-6, sea cual sea el color).
fn is_outpost_rank(rank: u8, color: Color) -> bool {
    match color {
        Color::White => (3..=5).contains(&rank),
        Color::Black => (2..=4).contains(&rank),
    }
}

/// ¿Puede algún peón enemigo, ya esté ahí ahora o tras avanzar, llegar a
/// atacar `sq`? Solo mira las columnas ADYACENTES (nunca la propia: un
/// peón nunca ataca en línea recta, así que uno en la misma columna que
/// el caballo es irrelevante aquí — a diferencia de la detección de
/// peones pasados, que sí le importa su propia columna).
fn is_hole(sq: Square, color: Color, enemy_pawns: u64) -> bool {
    let file = file_of(sq);
    let rank = rank_of(sq);
    let mut adjacent_files = [None; 2];
    if file > 0 {
        adjacent_files[0] = Some(file - 1);
    }
    if file < 7 {
        adjacent_files[1] = Some(file + 1);
    }

    if color == Color::White {
        for r in (rank + 1)..8 {
            for f in adjacent_files.into_iter().flatten() {
                if get_bit(enemy_pawns, make_square(f, r)) {
                    return false;
                }
            }
        }
    } else {
        for r in 0..rank {
            for f in adjacent_files.into_iter().flatten() {
                if get_bit(enemy_pawns, make_square(f, r)) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn knight_outposts(board: &Board, color: Color) -> Score {
    let own_knights = board.pieces[color.index()][PieceType::Knight.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let own_pawn_attacks = tables().pawn_attack_set(own_pawns, color);

    let mut mg = 0;
    let mut eg = 0;
    let mut bb = own_knights;
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        if is_outpost_rank(rank_of(sq), color) && is_hole(sq, color, enemy_pawns) {
            mg += OUTPOST_MG;
            eg += OUTPOST_EG;
            if get_bit(own_pawn_attacks, sq) {
                mg += OUTPOST_DEFENDED_BONUS_MG;
                eg += OUTPOST_DEFENDED_BONUS_EG;
            }
        }
    }

    Score::new(mg, eg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defended_outpost_gets_both_bonuses() {
        // Nd5: fila 5 (dentro de 4-6), sin peones negros en el tablero que
        // puedan amenazarla nunca; Pc4 la defiende (ataca b5 y d5).
        let board = Board::from_fen("4k3/8/8/3N4/2P5/8/2K5/8 w - - 0 1").unwrap();
        assert_eq!(
            knight_outposts(&board, Color::White),
            Score::new(
                OUTPOST_MG + OUTPOST_DEFENDED_BONUS_MG,
                OUTPOST_EG + OUTPOST_DEFENDED_BONUS_EG
            )
        );
    }

    #[test]
    fn undefended_outpost_gets_only_the_base_bonus() {
        let board = Board::from_fen("4k3/8/8/3N4/8/8/2K5/8 w - - 0 1").unwrap();
        assert_eq!(
            knight_outposts(&board, Color::White),
            Score::new(OUTPOST_MG, OUTPOST_EG)
        );
    }

    #[test]
    fn knight_too_far_back_gets_no_bonus() {
        // Nd3: fila 3, fuera del rango 4-6 para blancas.
        let board = Board::from_fen("4k3/8/8/8/8/3N4/8/4K3 w - - 0 1").unwrap();
        assert_eq!(knight_outposts(&board, Color::White), Score::ZERO);
    }

    #[test]
    fn square_reachable_by_an_enemy_pawn_is_not_an_outpost() {
        // Nd5 en fila válida, pero el peón negro en e6 ya la ataca
        // directamente (columna adyacente, por delante) -- no es agujero.
        let board = Board::from_fen("4k3/8/4p3/3N4/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(knight_outposts(&board, Color::White), Score::ZERO);
    }
}
