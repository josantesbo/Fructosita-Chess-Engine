//! Evaluación clásica hecha a mano (sin red neuronal).
//!
//! Devuelve una puntuación en centipeones **relativa a quien tiene el
//! turno** (convención estándar para negamax: positivo = bueno para quien
//! mueve). Combina, cada uno en su propio submódulo:
//!   - `material`: material + tablas posicionales (PST) con "tapered eval"
//!     (interpola entre valores de medio juego y de final).
//!   - `mobility`: movilidad (nº de casillas atacadas).
//!   - `pawn_structure`: estructura de peones (doblados, aislados, pasados).
//!   - `king_safety`: columnas abiertas cerca del rey.
//!   - `rook_activity`: bono de torre en columna abierta/semi-abierta.
//!
//! Se organiza como un término por archivo (en vez de un único eval.rs
//! monolítico) a propósito: el plan de evaluación artesanal (HCE) de
//! Fructosita es seguir añadiendo términos durante bastante tiempo antes de
//! considerar NNUE, y cada término nuevo debe poder añadirse, probarse y
//! revisarse de forma aislada sin inflar un solo archivo.

mod king_safety;
mod material;
mod mobility;
mod pawn_structure;
mod rook_activity;

use crate::board::Board;
use crate::types::Color;
use king_safety::king_safety;
use material::{game_phase, material_and_pst, MAX_PHASE};
use mobility::mobility;
use pawn_structure::pawn_structure;
use rook_activity::rook_activity;

pub use material::piece_value;

/// Puntuación relativa a quien tiene el turno (positivo = bueno para el que mueve).
pub fn evaluate(board: &Board) -> i32 {
    let (w_mg, w_eg) = material_and_pst(board, Color::White);
    let (b_mg, b_eg) = material_and_pst(board, Color::Black);
    let (wm_mg, wm_eg) = mobility(board, Color::White);
    let (bm_mg, bm_eg) = mobility(board, Color::Black);
    let (wp_mg, wp_eg) = pawn_structure(board, Color::White);
    let (bp_mg, bp_eg) = pawn_structure(board, Color::Black);
    let wk = king_safety(board, Color::White);
    let bk = king_safety(board, Color::Black);
    let (wr_mg, wr_eg) = rook_activity(board, Color::White);
    let (br_mg, br_eg) = rook_activity(board, Color::Black);

    let mg = (w_mg + wm_mg + wp_mg + wk + wr_mg) - (b_mg + bm_mg + bp_mg + bk + br_mg);
    let eg = (w_eg + wm_eg + wp_eg + wr_eg) - (b_eg + bm_eg + bp_eg + br_eg);

    let phase = game_phase(board);
    let score = (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE;

    // Convertimos primero a la perspectiva de quien mueve...
    let relative = if board.side_to_move == Color::White {
        score
    } else {
        -score
    };

    // ...y SOLO DESPUÉS sumamos el bono de tempo: así queda garantizado que
    // beneficia a quien tiene el turno sin importar su color. Sumarlo antes
    // de la conversión (como se hacía originalmente) lo convertía en una
    // penalización para las negras en vez de un bono — bug real, detectado
    // por el test `evaluation_is_color_symmetric`.
    relative + 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_is_roughly_balanced() {
        let b = Board::start_pos();
        let score = evaluate(&b);
        // Simétrica salvo el bono de tempo: debe estar cerca de 0, nunca
        // desbalanceada como si faltara una pieza (~esto sería cientos de cp).
        assert!(
            score.abs() < 50,
            "eval de posición inicial fuera de rango: {score}"
        );
    }

    #[test]
    fn missing_queen_is_heavily_penalized() {
        let with_queen = Board::start_pos();
        let without_queen =
            Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        // A las negras les falta la dama: para las blancas (quien mueve) debe evaluarse muy a favor.
        assert!(evaluate(&without_queen) > evaluate(&with_queen) + 700);
    }

    #[test]
    fn passed_pawn_close_to_promotion_is_valuable() {
        let far = Board::from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 0 1").unwrap();
        let close = Board::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(evaluate(&close) > evaluate(&far));
    }

    /// Convierte un FEN en su "espejo": tablero volteado verticalmente,
    /// colores de cada pieza intercambiados, turno intercambiado, derechos
    /// de enroque intercambiados y columna de captura al paso reflejada.
    /// Herramienta solo para tests, deliberadamente independiente del
    /// código de `Board`/`eval` (opera como texto sobre el FEN) para no
    /// compartir ningún supuesto con el código que está validando.
    fn mirror_fen(fen: &str) -> String {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        let ranks: Vec<&str> = parts[0].split('/').collect();
        let swap_case = |c: char| {
            if c.is_uppercase() {
                c.to_ascii_lowercase()
            } else if c.is_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c
            }
        };
        let placement: Vec<String> = ranks
            .iter()
            .rev()
            .map(|r| r.chars().map(swap_case).collect())
            .collect();
        let turn = if parts[1] == "w" { "b" } else { "w" };
        let castling: String = if parts[2] == "-" {
            "-".to_string()
        } else {
            parts[2].chars().map(swap_case).collect()
        };
        let ep = if parts[3] == "-" {
            "-".to_string()
        } else {
            let mut chars = parts[3].chars();
            let file = chars.next().unwrap();
            let rank = chars.next().unwrap().to_digit(10).unwrap();
            format!("{file}{}", 9 - rank)
        };
        format!(
            "{} {} {} {} {} {}",
            placement.join("/"),
            turn,
            castling,
            ep,
            parts.get(4).unwrap_or(&"0"),
            parts.get(5).unwrap_or(&"1")
        )
    }

    #[test]
    fn evaluation_is_color_symmetric() {
        // Para varias posiciones reales (con piezas dispersas, enroque
        // disponible, y captura al paso disponible), la evaluación de la
        // posición original y la de su espejo deben coincidir EXACTAMENTE.
        // Esto ejercita material, PST, movilidad, estructura de peones y
        // seguridad del rey a la vez, y probaría cualquier sesgo oculto
        // hacia un color en cualquiera de esos componentes — no solo en la
        // posición inicial (que es simétrica por construcción y no probaría
        // nada), sino en posiciones asimétricas reales.
        let positions = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r1bqk2r/2pp1ppp/p1n2n2/1pb1p3/4P3/1B3N2/PPPP1PPP/RNBQ1RK1 w kq - 0 8",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 4",
        ];
        for fen in positions {
            let board = Board::from_fen(fen).unwrap();
            let mirrored = Board::from_fen(&mirror_fen(fen)).unwrap();
            assert_eq!(
                evaluate(&board),
                evaluate(&mirrored),
                "evaluación no simétrica entre colores para: {fen}"
            );
        }
    }
}
