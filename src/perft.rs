//! Perft (performance test / enumeración de nodos): cuenta cuántas
//! posiciones hoja existen a una profundidad dada. Es la herramienta
//! estándar para validar que la generación de movimientos es correcta,
//! comparando contra valores de referencia públicos y bien conocidos.

use crate::board::Board;
use crate::movegen::generate_legal_moves;

pub fn perft(board: &Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = generate_legal_moves(board);
    if depth == 1 {
        // Bulk counting: con generación legal, cada movimiento de hoja
        // cuenta como exactamente 1 nodo, sin necesidad de recursar más.
        return moves.len() as u64;
    }
    let mut nodes = 0u64;
    for mv in moves {
        let next = board.make_move(mv);
        nodes += perft(&next, depth - 1);
    }
    nodes
}

/// Cuenta nodos por cada movimiento de la raíz por separado (estándar
/// "perft divide", usado para localizar en qué rama está un posible bug).
pub fn perft_divide(board: &Board, depth: u32) -> Vec<(String, u64)> {
    generate_legal_moves(board)
        .into_iter()
        .map(|mv| {
            let next = board.make_move(mv);
            let count = if depth == 0 {
                1
            } else {
                perft(&next, depth - 1)
            };
            (mv.to_string(), count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valores de referencia confirmados (Chess Programming Wiki / Steven
    // Edwards), estándar de facto en la comunidad para validar movegen.

    #[test]
    fn perft_startpos() {
        let b = Board::start_pos();
        assert_eq!(perft(&b, 1), 20);
        assert_eq!(perft(&b, 2), 400);
        assert_eq!(perft(&b, 3), 8902);
        assert_eq!(perft(&b, 4), 197281);
    }

    #[test]
    fn perft_kiwipete() {
        // Posición "Kiwipete" de Peter McKenzie: diseñada específicamente
        // para atrapar bugs de enroque, captura al paso y promociones.
        let b =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
                .unwrap();
        assert_eq!(perft(&b, 1), 48);
        assert_eq!(perft(&b, 2), 2039);
        assert_eq!(perft(&b, 3), 97862);
    }

    #[test]
    fn perft_position3() {
        // Posición de final de partida, rica en jaques y capturas al paso.
        let b = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&b, 1), 14);
        assert_eq!(perft(&b, 2), 191);
        assert_eq!(perft(&b, 3), 2812);
        assert_eq!(perft(&b, 4), 43238);
    }
}
