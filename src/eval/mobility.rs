//! Movilidad segura: número de casillas atacadas por cada pieza menor/mayor,
//! excluyendo las casillas atacadas por un peón enemigo.
//!
//! Teoría: una casilla donde un peón enemigo puede capturar la pieza gratis
//! no es "actividad" en ningún sentido útil — es simplemente ponerse en la
//! mira. Contarla igual que cualquier otra casilla libre infla el término
//! con ruido que no refleja actividad real, diluyendo la señal que sí
//! importa (casillas genuinamente disponibles). Prácticamente todo HCE
//! fuerte conocido excluye estas casillas del cómputo de movilidad
//! precisamente por esto — es uno de los ajustes más básicos y mejor
//! establecidos sobre la movilidad ingenua ("cuenta todo lo que se ataca").

use crate::bitboard::{count_bits, pop_lsb, tables, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;

pub fn mobility(board: &Board, color: Color) -> Score {
    let t = tables();
    let occ = board.occupancy();
    let own = board.color_occupancy(color);
    let enemy = color.opposite();
    let enemy_pawns = board.pieces[enemy.index()][PieceType::Pawn.index()];
    let enemy_pawn_attacks = t.pawn_attack_set(enemy_pawns, enemy);
    let safe = !own & !enemy_pawn_attacks;
    let mut count = 0i32;

    let mut bb = board.pieces[color.index()][PieceType::Knight.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.knight_attacks(sq) & safe) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Bishop.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.bishop_attacks(sq, occ) & safe) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Rook.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.rook_attacks(sq, occ) & safe) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Queen.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.queen_attacks(sq, occ) & safe) as i32;
    }

    Score::new(count * 4, count * 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_defended_by_enemy_pawn_does_not_count_as_mobility() {
        // Caballo blanco en c3: entre sus 8 casillas de salto está d5, que
        // un peón negro en e6 vigila (e6 ataca d5 y f5). Sin la exclusión,
        // d5 contaría como movilidad igual que cualquier otra casilla.
        let with_defender = Board::from_fen("4k3/8/4p3/8/8/2N5/8/4K3 w - - 0 1").unwrap();
        let without_defender = Board::from_fen("4k3/8/8/8/8/2N5/8/4K3 w - - 0 1").unwrap();

        let mg_with = mobility(&with_defender, Color::White).mg;
        let mg_without = mobility(&without_defender, Color::White).mg;

        // Con el peón vigilando d5, la movilidad "segura" debe ser
        // estrictamente menor que sin él (exactamente una casilla menos
        // contada, multiplicada por el peso de movilidad de mediojuego).
        assert!(
            mg_with < mg_without,
            "d5 vigilada por el peón debería restar movilidad segura: {mg_with} vs {mg_without}"
        );
    }

    #[test]
    fn own_pawns_do_not_affect_own_mobility_computation() {
        // La exclusión es sobre peones ENEMIGOS; los propios ya se excluyen
        // por ser "own occupancy", no por este mecanismo. Verificamos que
        // no hay doble conteo raro: caballo con y sin un peón PROPIO en una
        // casilla que ni siquiera es uno de sus 8 destinos posibles (h2 no
        // está entre a2/a4/b1/b5/d1/d5/e2/e4) da la misma movilidad.
        let board_a = Board::from_fen("4k3/8/8/8/8/2N5/8/4K3 w - - 0 1").unwrap();
        let board_b = Board::from_fen("4k3/8/8/8/8/2N5/7P/4K3 w - - 0 1").unwrap();
        assert_eq!(
            mobility(&board_a, Color::White).mg,
            mobility(&board_b, Color::White).mg
        );
    }
}
