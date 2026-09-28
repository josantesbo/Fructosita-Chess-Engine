//! Extracción de posiciones para el afinado Texel (fase P1C: preparación de
//! datos, sin cambio de fuerza).
//!
//! El método Texel afina los pesos de la evaluación minimizando el error
//! entre la predicción de la eval estática (pasada por una sigmoide) y el
//! resultado real de la partida de la que salió cada posición. Este módulo
//! cubre el primer paso: convertir PGNs de partidas reales (p. ej. la salida
//! de un match de fastchess) en un dataset plano `fen;resultado;eval`.
//!
//! Qué hace:
//!   - Parsea PGN estándar: cabeceras, comentarios `{...}`, variantes
//!     `(...)` (se ignoran), NAGs `$n`, números de jugada y resultado.
//!   - Acepta movetext tanto en SAN ("Nf3", "exd5", "O-O") — resuelto con
//!     el mismo `epd::san` ya probado por el arnés EPD — como en formato
//!     UCI ("g1f3"), para poder procesar también registros no-PGN-puros.
//!   - Filtra posiciones "ruidosas", los filtros clásicos y baratos del
//!     método: se saltan las primeras jugadas (libro/teoría), las
//!     posiciones donde el bando al turno está en jaque y aquellas cuya
//!     jugada siguiente fue captura o promoción (proxy barato de "posición
//!     no tranquila": la eval estática ahí no es representativa).
//!   - Emite por stdout una línea CSV por posición aceptada:
//!     `fen;resultado;eval_blancas` donde `resultado` es 1.0/0.5/0.0 desde
//!     la perspectiva de las blancas y `eval_blancas` la evaluación
//!     estática actual también desde las blancas (útil para ajustar la K
//!     de la sigmoide sin re-evaluar).
//!
//! Qué NO hace: no toca la evaluación ni la búsqueda (solo las *lee*), no
//! afina pesos (eso es el optimizador, fase posterior), y no pretende un
//! parser PGN completo para anotaciones exóticas — cubre el PGN que
//! producen fastchess/cutechess y herramientas similares.

use crate::board::Board;
use crate::epd::san::resolve_san_token;
use crate::eval;
use crate::movegen::generate_legal_moves;
use crate::moves::Move;
use crate::types::Color;

/// Resultado de una partida desde la perspectiva de las blancas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameResult {
    WhiteWin,
    Draw,
    BlackWin,
}

impl GameResult {
    fn from_tag(tag: &str) -> Option<GameResult> {
        match tag {
            "1-0" => Some(GameResult::WhiteWin),
            "0-1" => Some(GameResult::BlackWin),
            "1/2-1/2" => Some(GameResult::Draw),
            _ => None,
        }
    }

    fn as_white_score(self) -> f64 {
        match self {
            GameResult::WhiteWin => 1.0,
            GameResult::Draw => 0.5,
            GameResult::BlackWin => 0.0,
        }
    }
}

#[derive(Debug)]
pub struct PgnGame {
    pub result: GameResult,
    pub tokens: Vec<String>,
}

/// Separa un archivo PGN en partidas: para cada una, su `[Result ...]` y la
/// lista de tokens de movimiento ya limpia (sin comentarios, variantes,
/// NAGs, números de jugada ni token de resultado).
pub fn parse_pgn(contents: &str) -> Vec<PgnGame> {
    // Los PGN exportados desde Windows/ChessBase suelen empezar con BOM
    // (U+FEFF), que no es whitespace: sin quitarlo, la primera cabecera no
    // se reconocería como tal y la primera partida se perdería.
    let contents = contents.trim_start_matches('\u{feff}');
    let mut games = Vec::new();
    let mut result: Option<GameResult> = None;
    let mut tokens: Vec<String> = Vec::new();
    let mut in_comment = false;
    let mut variation_depth = 0usize;

    let flush =
        |result: &mut Option<GameResult>, tokens: &mut Vec<String>, games: &mut Vec<PgnGame>| {
            if let Some(r) = result.take() {
                if !tokens.is_empty() {
                    games.push(PgnGame {
                        result: r,
                        tokens: std::mem::take(tokens),
                    });
                    return;
                }
            }
            tokens.clear();
        };

    for line in contents.lines() {
        let trimmed = line.trim();
        if !in_comment && trimmed.starts_with('[') {
            // Una cabecera tras movetext acumulado = empieza otra partida.
            if !tokens.is_empty() {
                flush(&mut result, &mut tokens, &mut games);
            }
            if let Some(rest) = trimmed.strip_prefix("[Result \"") {
                if let Some(end) = rest.find('"') {
                    result = GameResult::from_tag(&rest[..end]);
                }
            }
            continue;
        }

        let mut token = String::new();
        for c in trimmed.chars() {
            if in_comment {
                if c == '}' {
                    in_comment = false;
                }
                continue;
            }
            match c {
                '{' => {
                    in_comment = true;
                    push_token(&mut token, &mut tokens, variation_depth);
                }
                '(' => {
                    push_token(&mut token, &mut tokens, variation_depth);
                    variation_depth += 1;
                }
                ')' => {
                    push_token(&mut token, &mut tokens, variation_depth);
                    variation_depth = variation_depth.saturating_sub(1);
                }
                ';' => {
                    // comentario hasta fin de línea
                    push_token(&mut token, &mut tokens, variation_depth);
                    break;
                }
                c if c.is_whitespace() => push_token(&mut token, &mut tokens, variation_depth),
                _ => token.push(c),
            }
        }
        push_token(&mut token, &mut tokens, variation_depth);
    }
    flush(&mut result, &mut tokens, &mut games);
    games
}

fn push_token(token: &mut String, tokens: &mut Vec<String>, variation_depth: usize) {
    if token.is_empty() {
        return;
    }
    let t = std::mem::take(token);
    if variation_depth > 0 {
        return; // dentro de una variante: se ignora
    }
    if t.starts_with('$') {
        return; // NAG
    }
    if t == "1-0" || t == "0-1" || t == "1/2-1/2" || t == "*" {
        return; // resultado: ya viene de la cabecera
    }
    // números de jugada: "1.", "1...", "23."
    let core = t.trim_end_matches('.');
    if !core.is_empty() && core.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    // "1.e4" pegado: separa el prefijo numérico
    if let Some(dot) = t.rfind('.') {
        let (num, mv) = t.split_at(dot + 1);
        let num_core = num.trim_end_matches('.');
        if !num_core.is_empty() && num_core.chars().all(|c| c.is_ascii_digit()) && !mv.is_empty() {
            tokens.push(mv.to_string());
            return;
        }
    }
    tokens.push(t);
}

/// Resuelve un token de movimiento: primero como SAN (vía `epd::san`), y si
/// no, como movimiento en formato UCI ("e2e4", "a7a8q") comparando contra la
/// lista de movimientos legales.
fn resolve_token(board: &Board, token: &str) -> Option<Move> {
    if let Some(mv) = resolve_san_token(board, token) {
        return Some(mv);
    }
    let legal = generate_legal_moves(board);
    legal.into_iter().find(|m| m.to_string() == token)
}

pub struct ExtractOptions {
    /// Medias-jugadas iniciales a saltar (libro/teoría).
    pub skip_opening_plies: usize,
    /// Emitir solo una de cada `stride` posiciones aceptadas (1 = todas).
    pub stride: usize,
}

pub struct ExtractSummary {
    pub games: usize,
    pub games_failed: usize,
    pub positions: usize,
}

/// Recorre las partidas y escribe `fen;resultado;eval_blancas` por stdout.
pub fn extract(contents: &str, opts: &ExtractOptions) -> ExtractSummary {
    let games = parse_pgn(contents);
    let mut summary = ExtractSummary {
        games: games.len(),
        games_failed: 0,
        positions: 0,
    };
    let mut accepted = 0usize;

    'games: for game in &games {
        let mut board = Board::start_pos();
        let score = game.result.as_white_score();
        for (ply, token) in game.tokens.iter().enumerate() {
            let Some(mv) = resolve_token(&board, token) else {
                // Movimiento irresoluble: partida corrupta o parser
                // insuficiente. Se descarta la partida entera para no
                // etiquetar posiciones con un resultado que quizá no les
                // corresponde.
                summary.games_failed += 1;
                continue 'games;
            };
            let quiet = ply >= opts.skip_opening_plies
                && !mv.is_capture()
                && mv.promotion().is_none()
                && !board.in_check(board.side_to_move);
            if quiet {
                accepted += 1;
                if accepted.is_multiple_of(opts.stride) {
                    let relative = eval::evaluate(&board);
                    let white_eval = if board.side_to_move == Color::White {
                        relative
                    } else {
                        -relative
                    };
                    println!("{};{:.1};{}", board.to_fen(), score, white_eval);
                    summary.positions += 1;
                }
            }
            board = board.make_move(mv);
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// Afinador (fase 2): descenso por coordenadas sobre los pesos de EvalParams
// minimizando el error sigmoide contra los resultados reales del dataset.
// ---------------------------------------------------------------------------

use crate::eval::{evaluate_with, EvalParams};

pub struct Dataset {
    pub entries: Vec<(Board, f64)>,
}

/// Carga un dataset `fen;resultado;eval` (la columna eval se ignora: el
/// afinador siempre re-evalúa con los pesos candidatos).
pub fn load_dataset(path: &str) -> Result<Dataset, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
    let mut entries = Vec::new();
    for (idx, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split(';');
        let (Some(fen), Some(result)) = (parts.next(), parts.next()) else {
            return Err(format!("línea {} malformada", idx + 1));
        };
        let board = Board::from_fen(fen).map_err(|e| format!("línea {}: {e}", idx + 1))?;
        let result: f64 = result
            .parse()
            .map_err(|e| format!("línea {}: resultado inválido: {e}", idx + 1))?;
        entries.push((board, result));
    }
    if entries.is_empty() {
        return Err("dataset vacío".to_string());
    }
    Ok(Dataset { entries })
}

fn sigmoid(eval_cp: f64, k: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-k * eval_cp / 400.0))
}

/// Error cuadrático medio del dataset con estos pesos y esta K. La eval se
/// convierte a perspectiva de las blancas porque el resultado (1/0.5/0) lo
/// está.
pub fn error(dataset: &Dataset, params: &EvalParams, k: f64) -> f64 {
    let mut total = 0.0;
    for (board, result) in &dataset.entries {
        let relative = evaluate_with(board, params);
        let white = if board.side_to_move == Color::White {
            relative
        } else {
            -relative
        };
        let diff = result - sigmoid(white as f64, k);
        total += diff * diff;
    }
    total / dataset.entries.len() as f64
}

/// Ajusta K por sección dorada con los pesos actuales (K queda fija durante
/// el afinado de pesos: es la escala centipeones→probabilidad, y dejarla
/// libre a la vez que los pesos permitiría compensaciones espurias).
pub fn fit_k(dataset: &Dataset, params: &EvalParams) -> f64 {
    let phi = (5f64.sqrt() - 1.0) / 2.0;
    let (mut lo, mut hi) = (0.1f64, 3.0f64);
    let mut a = hi - phi * (hi - lo);
    let mut b = lo + phi * (hi - lo);
    let mut ea = error(dataset, params, a);
    let mut eb = error(dataset, params, b);
    while hi - lo > 1e-4 {
        if ea < eb {
            hi = b;
            b = a;
            eb = ea;
            a = hi - phi * (hi - lo);
            ea = error(dataset, params, a);
        } else {
            lo = a;
            a = b;
            ea = eb;
            b = lo + phi * (hi - lo);
            eb = error(dataset, params, b);
        }
    }
    (lo + hi) / 2.0
}

/// Los 18 pesos como vector mutable, con nombre, para el descenso por
/// coordenadas. (El orden define el orden de visita en cada pasada.)
const PARAM_NAMES: [&str; 18] = [
    "pawn",
    "knight",
    "bishop",
    "rook",
    "queen",
    "mobility_mg",
    "mobility_eg",
    "doubled_mg",
    "doubled_eg",
    "isolated_mg",
    "isolated_eg",
    "passed_base",
    "passed_advancement",
    "passed_protected",
    "passed_connected",
    "passed_king_race",
    "king_open_file",
    "tempo",
];

fn to_vec(p: &EvalParams) -> [i32; 18] {
    [
        p.pawn,
        p.knight,
        p.bishop,
        p.rook,
        p.queen,
        p.mobility_mg,
        p.mobility_eg,
        p.doubled_mg,
        p.doubled_eg,
        p.isolated_mg,
        p.isolated_eg,
        p.passed_base,
        p.passed_advancement,
        p.passed_protected,
        p.passed_connected,
        p.passed_king_race,
        p.king_open_file,
        p.tempo,
    ]
}

fn from_vec(v: &[i32; 18]) -> EvalParams {
    EvalParams {
        pawn: v[0],
        knight: v[1],
        bishop: v[2],
        rook: v[3],
        queen: v[4],
        mobility_mg: v[5],
        mobility_eg: v[6],
        doubled_mg: v[7],
        doubled_eg: v[8],
        isolated_mg: v[9],
        isolated_eg: v[10],
        passed_base: v[11],
        passed_advancement: v[12],
        passed_protected: v[13],
        passed_connected: v[14],
        passed_king_race: v[15],
        king_open_file: v[16],
        tempo: v[17],
    }
}

/// Descenso por coordenadas clásico del método Texel: para cada peso prueba
/// +paso y −paso y se queda con lo que reduzca el error; repite pasadas
/// hasta que ninguna mejora, y entonces reduce el paso. Determinista.
pub fn tune(dataset: &Dataset, start: &EvalParams, k: f64) -> (EvalParams, f64, f64) {
    let mut v = to_vec(start);
    let e0 = error(dataset, start, k);
    let mut best_e = e0;
    for &step in &[16, 8, 4, 2, 1] {
        loop {
            let mut improved = false;
            for i in 0..v.len() {
                for delta in [step, -step] {
                    let old = v[i];
                    let candidate = old + delta;
                    // los pesos de este conjunto no tienen sentido negativos
                    if candidate < 0 {
                        continue;
                    }
                    v[i] = candidate;
                    let e = error(dataset, &from_vec(&v), k);
                    if e < best_e {
                        best_e = e;
                        eprintln!(
                            "  paso {step}: {} {} -> {} (E {:.6})",
                            PARAM_NAMES[i], old, candidate, best_e
                        );
                        improved = true;
                        break; // el peso cambió: siguiente peso
                    }
                    v[i] = old;
                }
            }
            if !improved {
                break;
            }
        }
    }
    (from_vec(&v), e0, best_e)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_PGN: &str = r#"[Event "test"]
[Result "1-0"]

1. e4 e5 2. Nf3 {comentario} Nc6 3. Bb5 a6 1-0

[Event "test2"]
[Result "0-1"]

1. d4 d5 2. c4 e6 3. Nc3 Nf6 0-1
"#;

    #[test]
    fn parses_two_games_with_results() {
        let games = parse_pgn(MINI_PGN);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].result, GameResult::WhiteWin);
        assert_eq!(games[0].tokens, vec!["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]);
        assert_eq!(games[1].result, GameResult::BlackWin);
        assert_eq!(games[1].tokens.len(), 6);
    }

    #[test]
    fn strips_comments_variations_and_nags() {
        let pgn = "[Result \"1/2-1/2\"]\n\n1. e4 $1 {bueno} (1. d4 d5) 1... e5 2. Nf3 1/2-1/2\n";
        let games = parse_pgn(pgn);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].tokens, vec!["e4", "e5", "Nf3"]);
    }

    #[test]
    fn resolves_uci_tokens_too() {
        let board = Board::start_pos();
        let mv = resolve_token(&board, "e2e4").expect("e2e4 legal en la inicial");
        assert_eq!(mv.to_string(), "e2e4");
    }

    #[test]
    fn whole_games_replay_without_failures() {
        // Las 2 partidas del PGN de prueba deben reproducirse enteras: si
        // el parser SAN o el tablero fallaran en algún punto, games_failed
        // lo delataría.
        let games = parse_pgn(MINI_PGN);
        for game in &games {
            let mut board = Board::start_pos();
            for token in &game.tokens {
                let mv = resolve_token(&board, token)
                    .unwrap_or_else(|| panic!("token irresoluble: {token}"));
                board = board.make_move(mv);
            }
        }
    }
}
