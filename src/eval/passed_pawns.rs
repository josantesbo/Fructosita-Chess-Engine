//! Peones pasados: detección y bonificación enriquecida.
//!
//! Un peón pasado (sin peón enemigo en su columna ni en las adyacentes,
//! por delante de él) es uno de los activos más valiosos del final de
//! partida: puede decidir la partida por sí solo si el rey rival no llega
//! a tiempo a detenerlo. Más allá del bono simple por avance (ya validado
//! antes de esta ampliación), añade tres refinamientos clásicos y bien
//! establecidos:
//!   - Distancia de los reyes a la casilla de coronación: cuanto más lejos
//!     esté el rey rival y más cerca el propio, más peligroso es el peón
//!     en la práctica (solo aporta a `eg`; se desvanece solo en
//!     mediojuego gracias a la interpolación de fase que ya hace
//!     `evaluate()`, sin necesidad de lógica adicional aquí).
//!   - Peones pasados conectados (columna adyacente, como mucho una fila
//!     de diferencia): mucho más fuertes que uno aislado, porque se
//!     protegen mutuamente el avance.
//!   - Peón pasado protegido por otro peón propio.
//!
//! Valores en fase de candidato: estimaciones de partida conservadoras
//! (no vienen de ningún motor existente), a validar con match antes de
//! tocarlas — mismo criterio que el resto de términos nuevos del proyecto.

use crate::bitboard::{get_bit, pop_lsb, tables, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;

const ADVANCE_WEIGHT: i32 = 3;
const KING_DISTANCE_WEIGHT: i32 = 5; // por unidad de distancia Chebyshev, solo eg
const CONNECTED_BONUS_MG: i32 = 10;
const CONNECTED_BONUS_EG: i32 = 20;
const PROTECTED_BONUS_MG: i32 = 8;
const PROTECTED_BONUS_EG: i32 = 16;

#[inline(always)]
fn chebyshev_distance(a: Square, b: Square) -> i32 {
    let file_diff = (file_of(a) as i32 - file_of(b) as i32).abs();
    let rank_diff = (rank_of(a) as i32 - rank_of(b) as i32).abs();
    file_diff.max(rank_diff)
}

/// ¿Puede algún peón enemigo, ya sea capturando o simplemente bloqueando,
/// detener a este peón antes de coronar? Comprueba su propia columna y las
/// adyacentes, por delante de él, en busca de cualquier peón enemigo.
fn is_passed(sq: Square, color: Color, enemy_pawns: u64) -> bool {
    let file = file_of(sq);
    let rank = rank_of(sq);
    let lo_file = file.saturating_sub(1);
    let hi_file = (file + 1).min(7);

    if color == Color::White {
        for r in (rank + 1)..8 {
            for f in lo_file..=hi_file {
                if get_bit(enemy_pawns, make_square(f, r)) {
                    return false;
                }
            }
        }
    } else {
        for r in 0..rank {
            for f in lo_file..=hi_file {
                if get_bit(enemy_pawns, make_square(f, r)) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn passed_pawns(board: &Board, color: Color) -> Score {
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let own_king = board.king_square(color);
    let enemy_king = board.king_square(color.opposite());
    let own_pawn_attacks = tables().pawn_attack_set(own_pawns, color);

    // Primero se identifican TODOS los peones pasados propios: hace falta
    // el conjunto completo antes de poder evaluar "conectados" (¿el de la
    // columna vecina también es pasado?).
    let mut passed_squares: Vec<Square> = Vec::new();
    let mut bb = own_pawns;
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        if is_passed(sq, color, enemy_pawns) {
            passed_squares.push(sq);
        }
    }

    let mut mg = 0;
    let mut eg = 0;

    for &sq in &passed_squares {
        let file = file_of(sq);
        let rank = rank_of(sq);
        let advance = if color == Color::White {
            rank
        } else {
            7 - rank
        };
        let bonus = (advance as i32) * (advance as i32) * ADVANCE_WEIGHT;
        mg += bonus / 2;
        eg += bonus;

        let promo_sq = make_square(file, if color == Color::White { 7 } else { 0 });
        let own_dist = chebyshev_distance(own_king, promo_sq);
        let enemy_dist = chebyshev_distance(enemy_king, promo_sq);
        eg += (enemy_dist - own_dist) * KING_DISTANCE_WEIGHT;

        let connected = passed_squares.iter().any(|&other| {
            other != sq && file_of(other).abs_diff(file) == 1 && rank_of(other).abs_diff(rank) <= 1
        });
        if connected {
            mg += CONNECTED_BONUS_MG;
            eg += CONNECTED_BONUS_EG;
        }

        if get_bit(own_pawn_attacks, sq) {
            mg += PROTECTED_BONUS_MG;
            eg += PROTECTED_BONUS_EG;
        }
    }

    Score::new(mg, eg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closer_own_king_gives_a_bigger_endgame_bonus() {
        // Mismo peón (a2), mismo rey rival (h8); solo cambia dónde está el
        // rey propio. El bono de avance (mg y parte de eg) es idéntico en
        // ambas posiciones -- la única diferencia posible es el término de
        // distancia de reyes, que solo aporta a eg.
        let king_near = Board::from_fen("7k/8/8/8/8/2K5/P7/8 w - - 0 1").unwrap();
        let king_far = Board::from_fen("7k/8/8/8/8/8/P7/7K w - - 0 1").unwrap();

        let near = passed_pawns(&king_near, Color::White);
        let far = passed_pawns(&king_far, Color::White);

        assert_eq!(near.mg, far.mg, "el avance no debería cambiar aquí");
        assert_eq!(
            near.eg - far.eg,
            10,
            "distancia Chebyshev: rey propio 5 vs 7 casillas de la corona, rival fijo en 7 -> diferencia (7-5)*5 - (7-7)*5 = 10"
        );
    }

    #[test]
    fn connected_passed_pawns_get_a_bonus_beyond_their_individual_value() {
        // Reyes en la fila 1, lejos de la fila 8: la distancia Chebyshev a
        // d8 y a e8 queda dominada por la distancia de fila (idéntica para
        // ambas columnas), así que el término de distancia de reyes no
        // introduce ninguna diferencia entre "juntos" y "por separado" --
        // el único efecto que puede diferir es el bono de "conectados".
        let both = Board::from_fen("8/8/8/3PP3/8/8/8/k6K w - - 0 1").unwrap();
        let only_d5 = Board::from_fen("8/8/8/3P4/8/8/8/k6K w - - 0 1").unwrap();
        let only_e5 = Board::from_fen("8/8/8/4P3/8/8/8/k6K w - - 0 1").unwrap();

        let both_score = passed_pawns(&both, Color::White);
        let independent_mg =
            passed_pawns(&only_d5, Color::White).mg + passed_pawns(&only_e5, Color::White).mg;
        let independent_eg =
            passed_pawns(&only_d5, Color::White).eg + passed_pawns(&only_e5, Color::White).eg;

        assert_eq!(both_score.mg - independent_mg, 2 * CONNECTED_BONUS_MG);
        assert_eq!(both_score.eg - independent_eg, 2 * CONNECTED_BONUS_EG);
    }

    #[test]
    fn protected_passed_pawn_gets_a_bonus() {
        // c4 defiende d5 (ataque diagonal hacia adelante); un peón negro en
        // b5 bloquea a c4 (que por tanto no es pasado él mismo, y tampoco
        // cae dentro del cono de d5), así que la única diferencia entre
        // estas dos posiciones es exactamente el bono de "protegido".
        let protected = Board::from_fen("k7/8/8/1p1P4/2P5/8/8/7K w - - 0 1").unwrap();
        let unprotected = Board::from_fen("k7/8/8/3P4/8/8/8/7K w - - 0 1").unwrap();

        let diff_mg =
            passed_pawns(&protected, Color::White).mg - passed_pawns(&unprotected, Color::White).mg;
        let diff_eg =
            passed_pawns(&protected, Color::White).eg - passed_pawns(&unprotected, Color::White).eg;

        assert_eq!(diff_mg, PROTECTED_BONUS_MG);
        assert_eq!(diff_eg, PROTECTED_BONUS_EG);
    }

    #[test]
    fn blocked_pawn_is_not_considered_passed() {
        let blocked = Board::from_fen("4k3/8/4p3/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(passed_pawns(&blocked, Color::White), Score::ZERO);
    }
}
