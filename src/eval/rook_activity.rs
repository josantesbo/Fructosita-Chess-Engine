//! Actividad de las torres: bono por estar en una columna abierta o
//! semi-abierta.
//!
//! Justificación teórica: una torre en una columna sin peones propios tiene
//! el camino libre para avanzar hasta la última fila o hasta cualquier
//! debilidad enemiga en esa columna, sin que sus propios peones le bloqueen
//! el paso. Si además no hay peones enemigos en esa columna (columna
//! "abierta", no solo "semi-abierta"), la torre puede llegar directamente a
//! la última fila del rival. Es uno de los términos de evaluación clásicos
//! mejor documentados (Chess Programming Wiki: "Rook on open file") y está
//! presente en prácticamente todo motor HCE conocido, precisamente porque
//! codifica un principio posicional muy básico y muy estable: la torre es
//! la pieza que más depende de líneas abiertas para ser efectiva.
//!
//! Valores en fase de candidato: no vienen de ningún motor existente, son
//! una estimación conservadora de partida (menor que el bono típico citado
//! en la literatura, ~25-50cp para columna abierta) a validar con match
//! antes de tocarlos. Aporte pequeño en el final porque ahí el rey y los
//! peones suelen dominar la evaluación mucho más que la actividad de torre.

use crate::bitboard::{file_mask, pop_lsb, EMPTY};
use crate::board::Board;
use crate::types::*;

const OPEN_FILE_MG: i32 = 20;
const OPEN_FILE_EG: i32 = 10;
const SEMI_OPEN_FILE_MG: i32 = 10;
const SEMI_OPEN_FILE_EG: i32 = 5;

pub fn rook_activity(board: &Board, color: Color) -> (i32, i32) {
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let mut mg = 0;
    let mut eg = 0;

    let mut rooks = board.pieces[color.index()][PieceType::Rook.index()];
    while rooks != EMPTY {
        let sq = pop_lsb(&mut rooks);
        let mask = file_mask(file_of(sq));
        let no_own_pawns = own_pawns & mask == EMPTY;
        if no_own_pawns {
            if enemy_pawns & mask == EMPTY {
                mg += OPEN_FILE_MG;
                eg += OPEN_FILE_EG;
            } else {
                mg += SEMI_OPEN_FILE_MG;
                eg += SEMI_OPEN_FILE_EG;
            }
        }
    }

    (mg, eg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rook_on_fully_open_file_gets_the_larger_bonus() {
        // Columna d sin ningún peón, de ningún bando.
        let b = Board::from_fen("4k3/8/8/8/8/8/8/3RK3 w - - 0 1").unwrap();
        assert_eq!(
            rook_activity(&b, Color::White),
            (OPEN_FILE_MG, OPEN_FILE_EG)
        );
    }

    #[test]
    fn rook_on_semi_open_file_gets_the_smaller_bonus() {
        // Sin peón propio en la columna d, pero hay un peón negro en d7.
        let b = Board::from_fen("4k3/3p4/8/8/8/8/8/3RK3 w - - 0 1").unwrap();
        assert_eq!(
            rook_activity(&b, Color::White),
            (SEMI_OPEN_FILE_MG, SEMI_OPEN_FILE_EG)
        );
    }

    #[test]
    fn rook_behind_own_pawn_gets_no_bonus() {
        // Peón propio en d3, delante de la torre en su misma columna.
        let b = Board::from_fen("4k3/8/8/8/8/3P4/8/3RK3 w - - 0 1").unwrap();
        assert_eq!(rook_activity(&b, Color::White), (0, 0));
    }

    #[test]
    fn two_rooks_on_open_files_stack_the_bonus() {
        // Torres en a1 (columna abierta) y d1 (semi-abierta, peón negro en d7).
        let b = Board::from_fen("4k3/3p4/8/8/8/8/8/R2RK3 w - - 0 1").unwrap();
        assert_eq!(
            rook_activity(&b, Color::White),
            (
                OPEN_FILE_MG + SEMI_OPEN_FILE_MG,
                OPEN_FILE_EG + SEMI_OPEN_FILE_EG
            )
        );
    }

    #[test]
    fn side_without_rooks_scores_zero() {
        let b = Board::from_fen("4k3/8/8/8/8/8/8/3RK3 w - - 0 1").unwrap();
        assert_eq!(rook_activity(&b, Color::Black), (0, 0));
    }
}
