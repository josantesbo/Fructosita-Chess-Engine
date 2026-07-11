//! Arnés de posiciones EPD (Extended Position Description): valida en
//! segundos o minutos, a profundidad fija y determinista (mismo espíritu
//! que `bench`), si el motor sigue encontrando la jugada correcta en un
//! conjunto de posiciones de referencia — antes de comprometer horas en un
//! match de fastchess.
//!
//! Soporta el formato EPD estándar (FEN de 4 campos + operaciones `bm`
//! best-move/s, `am` avoid-move/s, `id`), con los movimientos en notación
//! algebraica estándar (SAN, ver `san.rs`) — el mismo formato que usan
//! suites públicas conocidas (WAC, STS, Arasan, etc.). Cualquier archivo
//! `.epd` estándar debería funcionar tal cual, no solo los incluidos en
//! `testdata/epd/`.
//!
//! Deliberadamente NO evalúa "qué tan buena" es una jugada distinta a la
//! esperada: solo compara contra `bm`/`am`. Es una señal binaria y barata
//! (pasa/falla), pensada para detectar regresiones obvias rápido, no para
//! reemplazar el match con fastchess como validación de fuerza.

mod san;

use crate::board::Board;
use crate::movegen::generate_legal_moves;
use crate::moves::Move;
use crate::search;
use crate::tt::TranspositionTable;
use std::fmt;
use std::sync::Arc;

#[derive(Debug)]
pub struct EpdPosition {
    pub fen: String,
    pub id: Option<String>,
    pub best_moves: Vec<Move>,
    pub avoid_moves: Vec<Move>,
}

#[derive(Debug)]
pub enum EpdError {
    MissingFen(String),
    InvalidFen(String),
    UnresolvedMove { token: String, id: Option<String> },
}

impl fmt::Display for EpdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EpdError::MissingFen(line) => write!(f, "línea EPD sin FEN completo: {line}"),
            EpdError::InvalidFen(msg) => write!(f, "FEN inválido en línea EPD: {msg}"),
            EpdError::UnresolvedMove { token, id } => {
                let name = id.as_deref().unwrap_or("<sin id>");
                write!(
                    f,
                    "no se pudo interpretar el movimiento '{token}' en '{name}'"
                )
            }
        }
    }
}

/// Separa las operaciones de una línea EPD por ';', respetando comillas
/// dobles (para que `id "algo; con punto y coma"` no se parta mal — poco
/// común, pero es parte de la especificación).
fn split_operations(rest: &str) -> Vec<String> {
    let mut ops = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in rest.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            ';' if !in_quotes => {
                ops.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        ops.push(current);
    }
    ops
}

pub fn parse_epd_line(line: &str) -> Result<Option<EpdPosition>, EpdError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }

    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        return Err(EpdError::MissingFen(line.to_string()));
    }
    let fen = tokens[0..4].join(" ");
    let board = Board::from_fen(&fen).map_err(|e| EpdError::InvalidFen(format!("{fen} ({e})")))?;
    let legal = generate_legal_moves(&board);

    let rest = tokens[4..].join(" ");
    let mut id = None;
    let mut best_moves = Vec::new();
    let mut avoid_moves = Vec::new();

    for op in split_operations(&rest) {
        let op = op.trim();
        if let Some(v) = op.strip_prefix("id ") {
            id = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = op.strip_prefix("bm ") {
            for tok in v.split_whitespace() {
                let mv = san::match_san(&board, &legal, tok).ok_or_else(|| {
                    EpdError::UnresolvedMove {
                        token: tok.to_string(),
                        id: id.clone(),
                    }
                })?;
                best_moves.push(mv);
            }
        } else if let Some(v) = op.strip_prefix("am ") {
            for tok in v.split_whitespace() {
                let mv = san::match_san(&board, &legal, tok).ok_or_else(|| {
                    EpdError::UnresolvedMove {
                        token: tok.to_string(),
                        id: id.clone(),
                    }
                })?;
                avoid_moves.push(mv);
            }
        }
        // Otras operaciones EPD estándar (c0, ce, acd, etc.) se ignoran a
        // propósito: lo único que este arnés valida es "¿la jugada elegida
        // coincide con bm/evita am?", así que el resto no aporta nada aquí.
    }

    Ok(Some(EpdPosition {
        fen,
        id,
        best_moves,
        avoid_moves,
    }))
}

/// Parsea un archivo EPD completo (una posición por línea). Las líneas que
/// fallan se reportan como errores en vez de abortar todo el archivo: una
/// entrada mal formada en una suite de terceros no debería impedir correr
/// las demás.
pub fn parse_epd_file(contents: &str) -> (Vec<EpdPosition>, Vec<EpdError>) {
    let mut positions = Vec::new();
    let mut errors = Vec::new();
    for line in contents.lines() {
        match parse_epd_line(line) {
            Ok(Some(pos)) => positions.push(pos),
            Ok(None) => {}
            Err(e) => errors.push(e),
        }
    }
    (positions, errors)
}

pub struct EpdOutcome {
    pub id: Option<String>,
    pub fen: String,
    pub chosen: Move,
    pub passed: bool,
    pub nodes: u64,
}

pub struct EpdSummary {
    pub total: usize,
    pub passed: usize,
    pub outcomes: Vec<EpdOutcome>,
}

/// Corre cada posición a profundidad fija (determinista, mismo espíritu
/// que `bench`: comparable entre máquinas y entre commits sin depender de
/// la velocidad del hardware) y compara la jugada elegida contra `bm`/`am`.
pub fn run_suite(positions: &[EpdPosition], depth: i32) -> EpdSummary {
    let mut outcomes = Vec::with_capacity(positions.len());
    let mut passed = 0;

    for pos in positions {
        let board = Board::from_fen(&pos.fen).expect("ya validado durante el parseo");
        let tt = Arc::new(TranspositionTable::new(16));
        let (chosen, _score, stats) =
            search::search_fixed_depth_with_stats(board, depth, tt, vec![board.hash]);

        let respects_best = pos.best_moves.is_empty() || pos.best_moves.contains(&chosen);
        let respects_avoid = !pos.avoid_moves.contains(&chosen);
        let this_passed = respects_best && respects_avoid;
        if this_passed {
            passed += 1;
        }

        outcomes.push(EpdOutcome {
            id: pos.id.clone(),
            fen: pos.fen.clone(),
            chosen,
            passed: this_passed,
            nodes: stats.nodes,
        });
    }

    EpdSummary {
        total: positions.len(),
        passed,
        outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bm_and_id() {
        let line = r#"6k1/5ppp/8/8/8/8/8/4R1K1 w - - bm Re8+; id "back rank mate";"#;
        let pos = parse_epd_line(line).unwrap().unwrap();
        assert_eq!(pos.id.as_deref(), Some("back rank mate"));
        assert_eq!(pos.best_moves.len(), 1);
        assert_eq!(pos.best_moves[0].to_string(), "e1e8");
        assert!(pos.avoid_moves.is_empty());
    }

    #[test]
    fn parses_am() {
        let line = r#"3rk3/8/8/8/8/8/8/3QK3 w - - am Qxd8; id "avoid bad trade";"#;
        let pos = parse_epd_line(line).unwrap().unwrap();
        assert_eq!(pos.avoid_moves.len(), 1);
        assert_eq!(pos.avoid_moves[0].to_string(), "d1d8");
        assert!(pos.best_moves.is_empty());
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        assert!(parse_epd_line("").unwrap().is_none());
        assert!(parse_epd_line("   ").unwrap().is_none());
        assert!(parse_epd_line("# comentario").unwrap().is_none());
    }

    #[test]
    fn unresolved_move_is_reported_not_panicked() {
        let line = r#"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - bm Qh5;"#;
        let err = parse_epd_line(line).unwrap_err();
        assert!(matches!(err, EpdError::UnresolvedMove { .. }));
    }

    #[test]
    fn run_suite_reports_pass_on_solvable_mate_in_one() {
        let (positions, errors) =
            parse_epd_file(r#"6k1/5ppp/8/8/8/8/8/4R1K1 w - - bm Re8+; id "m1";"#);
        assert!(errors.is_empty());
        let summary = run_suite(&positions, 4);
        assert_eq!(summary.total, 1);
        assert_eq!(summary.passed, 1);
        assert!(summary.outcomes[0].passed);
    }

    #[test]
    fn malformed_line_does_not_abort_the_rest_of_the_file() {
        let contents = "esto no es un FEN\n6k1/5ppp/8/8/8/8/8/4R1K1 w - - bm Re8+; id \"m1\";\n";
        let (positions, errors) = parse_epd_file(contents);
        assert_eq!(positions.len(), 1);
        assert_eq!(errors.len(), 1);
    }
}
