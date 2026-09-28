pub mod san;

use crate::board::Board;
use crate::epd::san::resolve_san_token;
use crate::movegen::generate_legal_moves;
use crate::moves::Move;
use crate::search;
use crate::tt::TranspositionTable;
use std::fs;
use std::sync::Arc;

#[derive(Debug)]
pub struct EpdPosition {
    pub fen: String,
    pub best_moves: Vec<String>,
    pub avoid_moves: Vec<String>,
    pub id: Option<String>,
    pub line_number: usize,
}

#[derive(Debug, Default)]
pub struct EpdSummary {
    pub positions: usize,
    pub passed: usize,
    pub failed: usize,
}

pub fn run_file(path: &str, depth: i32) -> Result<EpdSummary, String> {
    let contents =
        fs::read_to_string(path).map_err(|err| format!("no se pudo leer {path}: {err}"))?;
    let mut summary = EpdSummary::default();

    for (idx, line) in contents.lines().enumerate() {
        let line_number = idx + 1;
        let Some(position) = parse_line(line, line_number)? else {
            continue;
        };
        if position.best_moves.is_empty() && position.avoid_moves.is_empty() {
            println!("line {line_number}: INVALID no bm/am; ignored");
            continue;
        }
        summary.positions += 1;
        match run_position(&position, depth) {
            Ok(result) => {
                if result.passed {
                    summary.passed += 1;
                    println!(
                        "PASS line {line_number} {} bestmove {}",
                        label(&position),
                        result.best_move
                    );
                } else {
                    summary.failed += 1;
                    println!(
                        "FAIL line {line_number} {} bestmove {}: {}",
                        label(&position),
                        result.best_move,
                        result.reason
                    );
                }
            }
            Err(err) => {
                summary.failed += 1;
                println!("FAIL line {line_number} {}: {err}", label(&position));
            }
        }
    }

    Ok(summary)
}

pub struct EpdResult {
    pub passed: bool,
    pub best_move: Move,
    pub reason: String,
}

pub fn run_position(position: &EpdPosition, depth: i32) -> Result<EpdResult, String> {
    let board = Board::from_fen(&position.fen).map_err(|err| format!("FEN inválido: {err}"))?;
    let legal = generate_legal_moves(&board);
    let bm = resolve_tokens(&board, &position.best_moves)?;
    let am = resolve_tokens(&board, &position.avoid_moves)?;
    let tt = Arc::new(TranspositionTable::new(64));
    let (best_move, _score, _stats) =
        search::search_fixed_depth_with_stats(board, depth, tt, vec![board.hash]);
    if !legal.contains(&best_move) {
        return Err(format!(
            "la búsqueda devolvió una jugada ilegal: {best_move}"
        ));
    }
    if !bm.is_empty() && !bm.contains(&best_move) {
        return Ok(EpdResult {
            passed: false,
            best_move,
            reason: format!("no está en bm {:?}", position.best_moves),
        });
    }
    if am.contains(&best_move) {
        return Ok(EpdResult {
            passed: false,
            best_move,
            reason: format!("está en am {:?}", position.avoid_moves),
        });
    }
    Ok(EpdResult {
        passed: true,
        best_move,
        reason: String::new(),
    })
}

fn resolve_tokens(board: &Board, tokens: &[String]) -> Result<Vec<Move>, String> {
    tokens
        .iter()
        .map(|token| {
            resolve_san_token(board, token)
                .ok_or_else(|| format!("no se pudo resolver SAN '{token}'"))
        })
        .collect()
}

pub fn parse_line(line: &str, line_number: usize) -> Result<Option<EpdPosition>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 4 {
        return Err(format!("line {line_number}: EPD incompleto"));
    }
    let fen = format!(
        "{} {} {} {} 0 1",
        fields[0], fields[1], fields[2], fields[3]
    );
    let ops = line.splitn(5, char::is_whitespace).nth(4).unwrap_or("");
    let mut position = EpdPosition {
        fen,
        best_moves: Vec::new(),
        avoid_moves: Vec::new(),
        id: None,
        line_number,
    };
    for op in split_ops(ops) {
        let op = op.trim();
        if let Some(rest) = op.strip_prefix("bm ") {
            position
                .best_moves
                .extend(rest.split_whitespace().map(str::to_string));
        } else if let Some(rest) = op.strip_prefix("am ") {
            position
                .avoid_moves
                .extend(rest.split_whitespace().map(str::to_string));
        } else if let Some(rest) = op.strip_prefix("id ") {
            position.id = Some(parse_id(rest.trim()));
        }
    }
    Ok(Some(position))
}

fn split_ops(ops: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for c in ops.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            ';' if !in_quote => {
                result.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

fn parse_id(rest: &str) -> String {
    if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 {
        rest[1..rest.len() - 1].to_string()
    } else {
        rest.to_string()
    }
}

fn label(position: &EpdPosition) -> String {
    position
        .id
        .clone()
        .unwrap_or_else(|| format!("epd.{}", position.line_number))
}
