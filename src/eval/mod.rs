//! Evaluación clásica hecha a mano (sin red neuronal).
//!
//! Devuelve una puntuación en centipeones **relativa a quien tiene el
//! turno** (convención estándar para negamax: positivo = bueno para quien
//! mueve). Combina, cada uno en su propio submódulo, un [`Score`] (par
//! mediojuego/final) por término:
//!   - `material`: material + tablas posicionales (PST) con "tapered eval"
//!     (interpola entre valores de medio juego y de final).
//!   - `mobility`: movilidad (nº de casillas atacadas). Una versión
//!     "segura" (excluyendo casillas vigiladas por un peón enemigo) se
//!     probó y se revirtió: perdió con claridad contra el baseline
//!     (LOS 99.9% a favor del baseline, ver historial de git) — la teoría
//!     era razonable, pero probablemente exigía recalibrar el peso por
//!     casilla junto con el cambio, no solo excluir casillas.
//!   - `pawn_structure`: doblados y aislados.
//!   - `passed_pawns`: avance, distancia de reyes, conectados, protegidos.
//!     Validado por match: +40.13 Elo, LOS 97.50 % (ver el propio módulo).
//!   - `king_safety`: columnas abiertas cerca del rey.
//!   - `knight_outposts`: caballo en agujero avanzado, con bono extra si
//!     está defendido por un peón propio.
//!
//! Se organiza como un término por archivo (en vez de un único eval.rs
//! monolítico) a propósito: el plan de evaluación artesanal (HCE) de
//! Fructosita es seguir añadiendo términos durante bastante tiempo antes de
//! considerar NNUE, y cada término nuevo debe poder añadirse, probarse y
//! revisarse de forma aislada sin inflar un solo archivo.
//!
//! # Cómo añadir un término nuevo
//! 1. Crear `src/eval/mi_termino.rs` con una función
//!    `pub fn mi_termino(board: &Board, color: Color) -> Score` y sus
//!    propios tests unitarios (ver `rook_activity.rs` en el historial de
//!    git para un ejemplo ya usado, aunque revertido por no validar en
//!    match — sigue siendo un buen modelo de cómo estructurar uno nuevo).
//! 2. Declarar `mod mi_termino;` más abajo e importar la función.
//! 3. Sumarla dentro de `evaluate_breakdown` (una sola vez: se resta
//!    automáticamente para el bando contrario al construir `Breakdown`).
//! 4. `cargo test`, `cargo bench`/`fructosita bench`, y solo si el match
//!    contra la versión sin el término muestra una mejora estadísticamente
//!    clara, se conserva — si no, se revierte sin excepción (ver README).
//!
//! `evaluate()` y `trace()` comparten una única función interna
//! (`evaluate_breakdown`) para que ambos vean siempre exactamente los
//! mismos números: si mañana se añade un término y alguien olvida
//! actualizar el desglose de diagnóstico, no puede desincronizarse de lo
//! que el motor realmente usa para jugar (el mismo tipo de bug que ya
//! encontramos una vez entre `eval` y `see.rs`).

mod king_safety;
mod knight_outposts;
mod material;
mod mobility;
mod passed_pawns;
mod pawn_structure;
mod score;

use crate::board::Board;
use crate::types::Color;
use king_safety::king_safety;
use knight_outposts::knight_outposts;
pub use material::piece_value;
use material::{game_phase, material_and_pst, MAX_PHASE};
use mobility::mobility;
use passed_pawns::passed_pawns;
use pawn_structure::pawn_structure;
pub use score::Score;

/// Desglose completo de una evaluación: cada término, por bando, más el
/// resultado final. Pensado para diagnóstico (comando UCI `eval`) y para
/// que los tests puedan comprobar términos individuales sin repetir la
/// lógica de combinación.
pub struct Breakdown {
    pub material: (Score, Score), // (blancas, negras)
    pub mobility: (Score, Score),
    pub pawn_structure: (Score, Score),
    pub passed_pawns: (Score, Score),
    pub king_safety: (Score, Score),
    pub knight_outposts: (Score, Score),
    pub phase: i32,
    /// Score final en centipeones, idéntico al que devuelve `evaluate()`
    /// para el mismo tablero (relativo a quien tiene el turno, con tempo).
    pub score: i32,
}

fn evaluate_breakdown(board: &Board) -> Breakdown {
    let material = (
        material_and_pst(board, Color::White),
        material_and_pst(board, Color::Black),
    );
    let mobility = (mobility(board, Color::White), mobility(board, Color::Black));
    let pawn_structure = (
        pawn_structure(board, Color::White),
        pawn_structure(board, Color::Black),
    );
    let passed_pawns = (
        passed_pawns(board, Color::White),
        passed_pawns(board, Color::Black),
    );
    let king_safety = (
        king_safety(board, Color::White),
        king_safety(board, Color::Black),
    );
    let knight_outposts = (
        knight_outposts(board, Color::White),
        knight_outposts(board, Color::Black),
    );

    let total = (material.0
        + mobility.0
        + pawn_structure.0
        + passed_pawns.0
        + king_safety.0
        + knight_outposts.0)
        - (material.1
            + mobility.1
            + pawn_structure.1
            + passed_pawns.1
            + king_safety.1
            + knight_outposts.1);

    let phase = game_phase(board);
    let tapered = total.interpolate(phase, MAX_PHASE);

    // Convertimos primero a la perspectiva de quien mueve...
    let relative = if board.side_to_move == Color::White {
        tapered
    } else {
        -tapered
    };

    // ...y SOLO DESPUÉS sumamos el bono de tempo: así queda garantizado que
    // beneficia a quien tiene el turno sin importar su color. Sumarlo antes
    // de la conversión (como se hacía originalmente) lo convertía en una
    // penalización para las negras en vez de un bono — bug real, detectado
    // por el test `evaluation_is_color_symmetric`.
    let score = relative + 10;

    Breakdown {
        material,
        mobility,
        pawn_structure,
        passed_pawns,
        king_safety,
        knight_outposts,
        phase,
        score,
    }
}

/// Puntuación relativa a quien tiene el turno (positivo = bueno para el que mueve).
pub fn evaluate(board: &Board) -> i32 {
    evaluate_breakdown(board).score
}

/// Desglose legible en texto de la evaluación de `board`, término por
/// término y bando por bando: para el comando de diagnóstico UCI `eval`
/// (ver `uci.rs`), pensado para verificar "a ojo" que un término nuevo
/// aporta lo que se espera antes de someterlo a bench/match.
pub fn trace(board: &Board) -> String {
    let b = evaluate_breakdown(board);
    let mut out = String::new();
    out.push_str("Término            MG(W)   MG(B)   EG(W)   EG(B)\n");
    let row = |out: &mut String, name: &str, w: Score, black: Score| {
        out.push_str(&format!(
            "{name:<18}{:>6}  {:>6}  {:>6}  {:>6}\n",
            w.mg, black.mg, w.eg, black.eg
        ));
    };
    row(&mut out, "Material + PST", b.material.0, b.material.1);
    row(&mut out, "Movilidad", b.mobility.0, b.mobility.1);
    row(
        &mut out,
        "Estructura peones",
        b.pawn_structure.0,
        b.pawn_structure.1,
    );
    row(
        &mut out,
        "Peones pasados",
        b.passed_pawns.0,
        b.passed_pawns.1,
    );
    row(
        &mut out,
        "Seguridad del rey",
        b.king_safety.0,
        b.king_safety.1,
    );
    row(
        &mut out,
        "Outposts de caballo",
        b.knight_outposts.0,
        b.knight_outposts.1,
    );
    out.push_str(&format!("Fase de partida: {}/{}\n", b.phase, MAX_PHASE));
    out.push_str(&format!(
        "Score final (perspectiva de quien mueve, con tempo): {} cp\n",
        b.score
    ));
    out
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

    #[test]
    fn trace_score_always_matches_evaluate() {
        // trace() y evaluate() comparten evaluate_breakdown(): este test
        // documenta esa garantía y la protege de una futura refactorización
        // que accidentalmente les haga calcular el número por caminos
        // distintos (el mismo tipo de bug de "dos fuentes de verdad" que ya
        // corregimos una vez entre `eval` y `see.rs`).
        let positions = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r1bqk2r/2pp1ppp/p1n2n2/1pb1p3/4P3/1B3N2/PPPP1PPP/RNBQ1RK1 w kq - 0 8",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ];
        for fen in positions {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(evaluate_breakdown(&board).score, evaluate(&board));
            // trace() no debe entrar en pánico y debe mencionar cada término.
            let text = trace(&board);
            assert!(text.contains("Material"));
            assert!(text.contains("Movilidad"));
            assert!(text.contains("Seguridad del rey"));
        }
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
