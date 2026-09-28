//! Evaluación clásica hecha a mano (sin red neuronal).
//!
//! Devuelve una puntuación en centipeones **relativa a quien tiene el
//! turno** (convención estándar para negamax: positivo = bueno para quien
//! mueve). Combina:
//!   - Material + tablas posicionales (PST) con "tapered eval" (interpola
//!     entre valores de medio juego y de final según cuántas piezas quedan)
//!   - Movilidad (nº de casillas atacadas)
//!   - Estructura de peones (doblados, aislados, pasados)
//!   - Seguridad del rey (columnas abiertas cerca del rey)
//!
//! Las PST no están copiadas de ningún motor existente: se generan por
//! fórmula (distancia al centro, avance de fila, etc.) en `build_pst`, lo
//! cual además las hace fáciles de razonar y ajustar más adelante con Texel
//! tuning (Fase 2).

use crate::bitboard::{count_bits, pop_lsb, tables, EMPTY};
use crate::board::Board;
use crate::types::*;
use std::sync::OnceLock;

const FILE_A: u64 = 0x0101010101010101;

/// Pesos escalares afinables de la evaluación (fase P1C, método Texel).
///
/// `Default` reproduce EXACTAMENTE los valores históricos del motor: si
/// nadie inyecta otros pesos, el juego es bit-idéntico al de siempre (lo
/// garantiza la firma de bench y la herramienta de equivalencia). El
/// afinador (`texel-tune`) construye variantes de esta struct y evalúa el
/// dataset con `evaluate_with` para buscar pesos con menor error.
///
/// Deliberadamente NO incluye (todavía) los coeficientes de las fórmulas
/// PST: esos son flotantes dentro de `build_pst` y se afinarán en una
/// pasada posterior, con más cuidado, porque interactúan con el material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalParams {
    pub pawn: i32,
    pub knight: i32,
    pub bishop: i32,
    pub rook: i32,
    pub queen: i32,
    pub mobility_mg: i32,
    pub mobility_eg: i32,
    pub doubled_mg: i32,
    pub doubled_eg: i32,
    pub isolated_mg: i32,
    pub isolated_eg: i32,
    pub passed_base: i32,
    pub passed_advancement: i32,
    pub passed_protected: i32,
    pub passed_connected: i32,
    pub passed_king_race: i32,
    pub king_open_file: i32,
    pub tempo: i32,
}

impl Default for EvalParams {
    /// Pesos afinados por Texel (descenso por coordenadas sobre 373.893
    /// posiciones de partidas reales de motores 3000-3600; K=0.8572;
    /// error 0.140148 -> 0.134527). CANDIDATO: pendiente de validación por
    /// match — si no gana de forma clara, se revierte a los históricos
    /// (pawn 100, knight 320, bishop 330, rook 500, queen 900, mobility 4/3,
    /// doubled 10/20, isolated 12/16, passed 3/4/10/8/4, king 15, tempo 10).
    fn default() -> Self {
        EvalParams {
            pawn: 70,
            knight: 324,
            bishop: 356,
            rook: 505,
            queen: 1051,
            mobility_mg: 5,
            mobility_eg: 15,
            doubled_mg: 15,
            doubled_eg: 26,
            isolated_mg: 0,
            isolated_eg: 19,
            passed_base: 3,
            passed_advancement: 23,
            passed_protected: 0,
            passed_connected: 0,
            passed_king_race: 39,
            king_open_file: 60,
            tempo: 6,
        }
    }
}

fn default_params() -> &'static EvalParams {
    static DEFAULTS: OnceLock<EvalParams> = OnceLock::new();
    DEFAULTS.get_or_init(EvalParams::default)
}

/// Accesor público con los pesos por defecto. Hoy `see.rs` mantiene su
/// propia tabla (con el rey a valor alto, ver allí), así que nadie lo llama
/// aún — se conserva como la puerta estable para cuando SEE derive sus
/// valores de aquí, como propone la hoja de ruta.
#[allow(dead_code)]
pub fn piece_value(p: PieceType) -> i32 {
    piece_value_with(default_params(), p)
}

fn piece_value_with(params: &EvalParams, p: PieceType) -> i32 {
    match p {
        PieceType::Pawn => params.pawn,
        PieceType::Knight => params.knight,
        PieceType::Bishop => params.bishop,
        PieceType::Rook => params.rook,
        PieceType::Queen => params.queen,
        PieceType::King => 0,
    }
}

const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0]; // pawn,knight,bishop,rook,queen,king
const MAX_PHASE: i32 = 24; // 2 bandos * (2N+2B+2R+1Q) = 2*(2+2+4+4) = 24

#[inline(always)]
fn mirror(sq: Square) -> Square {
    sq ^ 56
}

struct Pst {
    mg: [[i32; 64]; 6],
    eg: [[i32; 64]; 6],
}

fn build_pst() -> Pst {
    let mut mg = [[0i32; 64]; 6];
    let mut eg = [[0i32; 64]; 6];

    for sq in 0u8..64 {
        let file = file_of(sq) as f32;
        let rank = rank_of(sq) as f32;
        let cdist = ((file - 3.5).powi(2) + (rank - 3.5).powi(2)).sqrt();
        let i = sq as usize;

        // Peón: favorece columnas centrales y avance; bonus extra por
        // ocupar el centro clásico (d4/e4/d5/e5).
        let file_center = 3.5 - (file - 3.5).abs();
        let mut p_mg = (file_center * 4.0 + rank * 5.0) as i32;
        if (file == 3.0 || file == 4.0) && (rank == 3.0 || rank == 4.0) {
            p_mg += 15;
        }
        mg[PieceType::Pawn.index()][i] = p_mg;
        eg[PieceType::Pawn.index()][i] = (rank * rank * 2.0) as i32;

        // Caballo: "en la banda, se pasma" — penaliza fuerte la distancia al centro.
        mg[PieceType::Knight.index()][i] = (24.0 - cdist * 7.0) as i32;
        eg[PieceType::Knight.index()][i] = (18.0 - cdist * 5.0) as i32;

        // Alfil: centralización más suave que el caballo.
        mg[PieceType::Bishop.index()][i] = (12.0 - cdist * 3.0) as i32;
        eg[PieceType::Bishop.index()][i] = (10.0 - cdist * 3.0) as i32;

        // Torre: preferencia central leve; en el final, bonus por 7ª fila.
        mg[PieceType::Rook.index()][i] = (6.0 - (file - 3.5).abs() * 1.5) as i32;
        eg[PieceType::Rook.index()][i] =
            (4.0 - (file - 3.5).abs()) as i32 + if rank as u8 == 6 { 16 } else { 0 };

        // Dama: centralización suave.
        mg[PieceType::Queen.index()][i] = (6.0 - cdist * 1.5) as i32;
        eg[PieceType::Queen.index()][i] = (10.0 - cdist * 2.5) as i32;

        // Rey: en medio juego prefiere el fondo/esquina (seguridad); en el
        // final, se centraliza (pieza activa).
        mg[PieceType::King.index()][i] = (cdist * 9.0) as i32 + ((7.0 - rank) * 3.0) as i32;
        eg[PieceType::King.index()][i] = (24.0 - cdist * 8.0) as i32;
    }

    Pst { mg, eg }
}

// ---------------------------------------------------------------------------
// BASELINE v4 — corrección PST por fila afinada contra la EVALUACIÓN DE BÚSQUEDA
// PROFUNDA del propio motor (objetivo λ = 1, profundidad 10).
//
// ACEPTADA por SPRT: +27,62 ± 11,55 Elo, nElo 34,53, LOS 100%, 2.244 partidas,
// H1 [0,5] aceptado (10+0.1, 1t, 64MB, UHO_4060_v4.epd). Primera aceptación del
// programa en el subsistema de EVALUACIÓN.
//
// El MISMO vector afinado contra el resultado de la partida dio −203,57 Elo
// (investigación 12). La diferencia —231 Elo— es atribuible al objetivo de
// afinado. Ver research/13 y analysis/margin-study-05-search-target.md.
//
// `build_pst()` NO se toca: la corrección se suma una sola vez al construir el
// `OnceLock`. Coste en tiempo de ejecución CERO; con la tabla a 0 el motor es
// bit a bit el baseline v3 (firma 4a32f4a9db48eebe).
//
// 92 enteros (6 piezas × 8 filas × 2 fases, menos las filas 1 y 8 del peón).
// La SUMA de cada fila es exactamente 0: restricción de media cero por
// (pieza, fase), que impide reabsorber valor material y mantiene congelados
// los 18 escalares de Texel v1.
//
// Procedencia: analysis/margin-study-05-search-target.md · candidato λ=1.
// ---------------------------------------------------------------------------

pub const PST_CORR_MG: [[i32; 8]; 6] = [
    [0, -16, -16, -16, -10, 8, 50, 0],       // Peón   (filas 1 y 8 inexistentes)
    [6, 6, -4, -4, -8, 42, 12, -50],         // Caballo
    [-2, 2, -2, -10, 0, 24, 6, -18],         // Alfil
    [-20, -20, -22, -28, -16, 24, 32, 50],   // Torre
    [16, 8, -4, -14, -18, 2, -12, 22],       // Dama
    [18, 30, -22, -38, -44, -34, 40, 50],    // Rey
];

pub const PST_CORR_EG: [[i32; 8]; 6] = [
    [0, 8, 6, 4, 4, 2, -24, 0],              // Peón
    [12, -6, -14, -2, 12, -10, 8, 0],        // Caballo
    [10, -4, -8, -8, -10, -8, 0, 28],        // Alfil
    [-14, -6, 2, 10, 10, 8, -4, -6],         // Torre
    [-50, -32, -28, -14, 16, 26, 48, 34],    // Dama
    [-26, -16, -12, 0, 10, 14, 26, 4],       // Rey
];

// ---------------------------------------------------------------------------
// CANDIDATO PST-E3o-mg — corrección de tabla completa restringida al MEDIO JUEGO
// y ortogonalizada contra el eje de fila.
//
// «Medio juego» = la mitad `mg` del par de tablas del taper. `PST_E3OMG_EG` es
// idénticamente 0: la tabla `eg` queda congelada en los valores de v4. Definición
// ESTRUCTURAL, no una selección de posiciones ni de casillas.
// (Ojo: por el taper, una corrección en `mg` sigue actuando en finales con peso
// fase/24. Lo congelado es la TABLA, no el régimen de posiciones.)
//
// Eje de fila eliminado POR CONSTRUCCIÓN, en dos capas: base ortogonalizada por
// Gram-Schmidt antes de afinar, y redondeo entero fila a fila. Cada
// (pieza, fase, fila) suma exactamente 0, lo que implica media cero por grupo.
//
// 368 valores libres − 46 restricciones de fila = 322 parámetros EFECTIVOS.
//
// Preregistrado en analysis/pst-e3o-mg-preregistro.md (sha256 0ae26e9b7798...),
// congelado ANTES de medir. Objetivo: búsqueda d10 de v4 · K = 0,9369 congelada ·
// tope ±50 (máx alcanzado 27).
//
// Con esta tabla a 0 el motor es bit a bit el baseline v4 (b5cd444789b83db1).
// NADA de esto es evidencia de Elo. Sin screening y sin SPRT.
// ---------------------------------------------------------------------------

pub const PST_E3OMG_MG: [[i32; 64]; 6] = [
    [ // Peón
           0,    0,    0,    0,    0,    0,    0,    0,
          -2,   -4,  -12,   -2,   -5,   -4,   22,    7,
          -5,   -2,  -10,  -14,   -8,   13,   17,    9,
          -3,  -10,    3,   -9,  -11,    4,   22,    4,
          -9,   -9,   -8,  -11,  -11,   15,   26,    7,
         -12,   -4,  -11,    1,   15,    4,   13,   -6,
           6,    0,   -3,    5,   -1,   -1,   -7,    1,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Caballo
           7,   -1,   -3,    2,    3,   -2,  -12,    6,
          -3,    1,   -6,    2,    0,    0,    8,   -2,
         -13,    5,  -11,    7,   11,   -1,    1,    1,
          -1,    7,    9,  -14,  -12,   -4,    9,    6,
           1,   -3,   -7,   -5,  -27,   17,   18,    6,
         -13,   -4,  -15,    4,    3,    3,    8,   14,
          -3,    2,   -2,   -5,   -2,    5,    2,    3,
         -16,    3,    2,    1,    2,    2,    4,    2,
    ],
    [ // Alfil
          -3,   15,    0,  -10,    1,   -4,    1,    0,
           5,    4,    5,   -7,    2,   -6,    2,   -5,
           3,    5,    3,   -1,   -7,    5,  -12,    4,
          15,   -6,   -6,    6,    1,  -12,   -1,    3,
          -2,    3,   -6,    0,    3,    1,  -10,   11,
           0,   -6,   -2,    5,   -7,    6,   12,   -8,
          -5,   -2,    4,   -2,    3,    0,    3,   -1,
           1,    0,   -1,   -3,   -3,    2,    2,    2,
    ],
    [ // Torre
          -9,   -2,    3,   -1,    3,   -5,   14,   -3,
          -7,  -11,   -2,    9,   -5,    3,    4,    9,
          -6,   -4,   -2,   -6,   -3,    3,    7,   11,
           2,   -2,   -1,   -4,   -3,    5,   -1,    4,
           2,   -4,   -6,    1,  -15,   12,    2,    8,
           2,    0,  -10,   -8,    6,   -5,   11,    4,
           2,   -7,   -4,  -12,    6,    5,    4,    6,
          -1,   -2,   -7,   -1,    2,    2,    2,    5,
    ],
    [ // Dama
          -4,    9,    6,    8,   -4,   -9,   -9,    3,
           1,    0,    3,    7,    5,   -3,   -5,   -8,
          -3,    2,   -1,    0,    4,    2,   -1,   -3,
           6,   -5,   -4,   -3,  -18,    8,   -1,   17,
          -1,   -7,   -5,  -17,   -1,   -5,   17,   19,
         -10,   -9,  -10,   -9,   -4,   15,   14,   13,
          -2,  -10,   -1,   -2,    2,   -3,    3,   13,
          -1,   -5,    0,   -4,    0,    4,    5,    1,
    ],
    [ // Rey
          -4,    9,   20,  -21,    1,   -5,    3,   -3,
           0,    9,   -7,    4,    0,  -11,    3,    2,
           5,    2,    5,   -1,  -11,    0,    2,   -2,
           2,    2,    3,   -1,   -3,    1,    0,   -4,
          -1,    2,   -1,    1,   -1,   -1,    1,    0,
          -2,    1,    0,   -2,   -1,    0,    2,    2,
          -3,    1,    0,   -1,    1,    1,    1,    0,
          -2,   -1,    0,    0,    0,    1,    1,    1,
    ],
];

pub const PST_E3OMG_EG: [[i32; 64]; 6] = [
    [ // Peón
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Caballo
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Alfil
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Torre
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Dama
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
    [ // Rey
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
           0,    0,    0,    0,    0,    0,    0,    0,
    ],
];

fn apply_pst_correction(p: &mut Pst) {
    for pc in 0..6 {
        for sq in 0..64usize {
            let rank = sq / 8;
            p.mg[pc][sq] += PST_CORR_MG[pc][rank];
            p.eg[pc][sq] += PST_CORR_EG[pc][rank];
        }
    }
}

/// Candidato PST-E3o-mg: corrección por casilla, sólo tabla `mg`, sin componente
/// de fila, encima de v4.
fn apply_pst_e3o_mg(p: &mut Pst) {
    for pc in 0..6 {
        for sq in 0..64usize {
            p.mg[pc][sq] += PST_E3OMG_MG[pc][sq];
            p.eg[pc][sq] += PST_E3OMG_EG[pc][sq];
        }
    }
}

static PST: OnceLock<Pst> = OnceLock::new();
fn pst() -> &'static Pst {
    PST.get_or_init(|| {
        let mut p = build_pst();
        apply_pst_correction(&mut p);
        apply_pst_e3o_mg(&mut p);
        p
    })
}

fn game_phase(board: &Board) -> i32 {
    let mut phase = 0;
    for color in [Color::White, Color::Black] {
        for pt in [
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
        ] {
            let count = count_bits(board.pieces[color.index()][pt.index()]) as i32;
            phase += count * PHASE_WEIGHT[pt.index()];
        }
    }
    phase.min(MAX_PHASE)
}

#[allow(dead_code)]
fn material_and_pst(board: &Board, color: Color, params: &EvalParams) -> (i32, i32) {
    let (material, pst) = material_and_pst_components(board, color, params);
    (material.0 + pst.0, material.1 + pst.1)
}

fn material_and_pst_components(
    board: &Board,
    color: Color,
    params: &EvalParams,
) -> ((i32, i32), (i32, i32)) {
    let p = pst();
    let mut material_mg = 0;
    let mut material_eg = 0;
    let mut pst_mg = 0;
    let mut pst_eg = 0;
    for pt in ALL_PIECE_TYPES {
        let mut bb = board.pieces[color.index()][pt.index()];
        let value = piece_value_with(params, pt);
        while bb != EMPTY {
            let sq = pop_lsb(&mut bb);
            let idx = if color == Color::White {
                sq
            } else {
                mirror(sq)
            };
            material_mg += value;
            material_eg += value;
            pst_mg += p.mg[pt.index()][idx as usize];
            pst_eg += p.eg[pt.index()][idx as usize];
        }
    }
    ((material_mg, material_eg), (pst_mg, pst_eg))
}

fn mobility(board: &Board, color: Color, params: &EvalParams) -> (i32, i32) {
    let t = tables();
    let occ = board.occupancy();
    let own = board.color_occupancy(color);
    let mut count = 0i32;

    let mut bb = board.pieces[color.index()][PieceType::Knight.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.knight_attacks(sq) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Bishop.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.bishop_attacks(sq, occ) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Rook.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.rook_attacks(sq, occ) & !own) as i32;
    }
    let mut bb = board.pieces[color.index()][PieceType::Queen.index()];
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        count += count_bits(t.queen_attacks(sq, occ) & !own) as i32;
    }

    (count * params.mobility_mg, count * params.mobility_eg)
}

#[allow(dead_code)]
fn pawn_structure(board: &Board, color: Color, params: &EvalParams) -> (i32, i32) {
    let (structure, passed) = pawn_structure_components(board, color, params);
    (structure.0 + passed.0, structure.1 + passed.1)
}

// ---------------------------------------------------------------------------
// Mascaras precalculadas de peon pasado (investigacion 35).
//
// `PASSED_MASK`, `CONNECT_MASK` y `PROTECT_MASK` contienen, para cada casilla
// (y color cuando procede), EXACTAMENTE el mismo conjunto de casillas que
// recorrian los bucles anteriores. Los tres predicados devuelven por tanto los
// mismos valores que antes: es un cambio de COMO se calcula, no de QUE se
// calcula. Las tablas son estaticas, de solo lectura y compartidas por todos
// los hilos (2,5 KB en total); no hay estado, ni cache, ni invalidacion.
// Ver `kb/analysis/mask-03-preregistro-fase2.md`.
// ---------------------------------------------------------------------------
const fn build_passed_mask() -> [[u64; 64]; 2] {
    let mut t = [[0u64; 64]; 2];
    let mut c = 0usize;
    while c < 2 {
        let mut sq = 0usize;
        while sq < 64 {
            let file = (sq % 8) as i32;
            let rank = (sq / 8) as i32;
            let lo = if file > 0 { file - 1 } else { 0 };
            let hi = if file < 7 { file + 1 } else { 7 };
            let mut m = 0u64;
            if c == 0 {
                let mut r = rank + 1;
                while r < 8 {
                    let mut f = lo;
                    while f <= hi {
                        m |= 1u64 << (r * 8 + f);
                        f += 1;
                    }
                    r += 1;
                }
            } else {
                let mut r = 0i32;
                while r < rank {
                    let mut f = lo;
                    while f <= hi {
                        m |= 1u64 << (r * 8 + f);
                        f += 1;
                    }
                    r += 1;
                }
            }
            t[c][sq] = m;
            sq += 1;
        }
        c += 1;
    }
    t
}

const fn build_connect_mask() -> [u64; 64] {
    let mut t = [0u64; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let file = (sq % 8) as i32;
        let rank = (sq / 8) as i32;
        let lo = if rank > 0 { rank - 1 } else { 0 };
        let hi = if rank < 7 { rank + 1 } else { 7 };
        let mut m = 0u64;
        if file > 0 {
            let mut r = lo;
            while r <= hi {
                m |= 1u64 << (r * 8 + file - 1);
                r += 1;
            }
        }
        if file < 7 {
            let mut r = lo;
            while r <= hi {
                m |= 1u64 << (r * 8 + file + 1);
                r += 1;
            }
        }
        t[sq] = m;
        sq += 1;
    }
    t
}

const fn build_protect_mask() -> [[u64; 64]; 2] {
    let mut t = [[0u64; 64]; 2];
    let mut c = 0usize;
    while c < 2 {
        let mut sq = 0usize;
        while sq < 64 {
            let file = (sq % 8) as i32;
            let rank = (sq / 8) as i32;
            let mut m = 0u64;
            let pr = if c == 0 {
                if rank > 0 { rank - 1 } else { -1 }
            } else if rank < 7 {
                rank + 1
            } else {
                -1
            };
            if pr >= 0 {
                if file > 0 {
                    m |= 1u64 << (pr * 8 + file - 1);
                }
                if file < 7 {
                    m |= 1u64 << (pr * 8 + file + 1);
                }
            }
            t[c][sq] = m;
            sq += 1;
        }
        c += 1;
    }
    t
}

pub static PASSED_MASK: [[u64; 64]; 2] = build_passed_mask();
pub static CONNECT_MASK: [u64; 64] = build_connect_mask();
pub static PROTECT_MASK: [[u64; 64]; 2] = build_protect_mask();

fn is_passed_pawn(enemy_pawns: u64, color: Color, sq: Square) -> bool {
    enemy_pawns & PASSED_MASK[color.index()][sq as usize] == EMPTY
}

fn is_protected_passed_pawn(own_pawns: u64, color: Color, sq: Square) -> bool {
    own_pawns & PROTECT_MASK[color.index()][sq as usize] != EMPTY
}

fn is_connected_passed_pawn(own_pawns: u64, sq: Square) -> bool {
    own_pawns & CONNECT_MASK[sq as usize] != EMPTY
}

fn promotion_rank_distance(color: Color, sq: Square) -> i32 {
    match color {
        Color::White => 7 - rank_of(sq) as i32,
        Color::Black => rank_of(sq) as i32,
    }
}

fn promotion_square(color: Color, sq: Square) -> Square {
    let rank = match color {
        Color::White => 7,
        Color::Black => 0,
    };
    make_square(file_of(sq), rank)
}

fn king_distance_to_promotion_square(
    board: &Board,
    king_color: Color,
    pawn_color: Color,
    sq: Square,
) -> i32 {
    let king = board.king_square(king_color);
    let promotion = promotion_square(pawn_color, sq);
    let file_distance = (file_of(king) as i32 - file_of(promotion) as i32).abs();
    let rank_distance = (rank_of(king) as i32 - rank_of(promotion) as i32).abs();
    file_distance.max(rank_distance)
}

fn passed_pawn_bonus(
    board: &Board,
    own_pawns: u64,
    color: Color,
    sq: Square,
    params: &EvalParams,
) -> (i32, i32) {
    let advance = match color {
        Color::White => rank_of(sq) as i32,
        Color::Black => 7 - rank_of(sq) as i32,
    };
    let distance = promotion_rank_distance(color, sq);
    let base = advance * advance * params.passed_base;
    let advancement = (6 - distance).max(0) * params.passed_advancement;
    let protected = if is_protected_passed_pawn(own_pawns, color, sq) {
        params.passed_protected + advancement / 2
    } else {
        0
    };
    let connected = if is_connected_passed_pawn(own_pawns, sq) {
        params.passed_connected + advancement / 2
    } else {
        0
    };
    let own_king_distance = king_distance_to_promotion_square(board, color, color, sq);
    let enemy_king_distance = king_distance_to_promotion_square(board, color.opposite(), color, sq);
    let king_race =
        (enemy_king_distance - own_king_distance).clamp(-3, 3) * params.passed_king_race;

    let mg = base / 2 + advancement / 2 + protected / 2 + connected / 2 + king_race / 2;
    let eg = base + advancement + protected + connected + king_race;
    (mg.max(0), eg.max(0))
}

fn pawn_structure_components(
    board: &Board,
    color: Color,
    params: &EvalParams,
) -> ((i32, i32), (i32, i32)) {
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let mut mg = 0;
    let mut eg = 0;
    let mut passed_mg = 0;
    let mut passed_eg = 0;

    for file in 0u8..8 {
        let file_mask: u64 = FILE_A << file;
        let count_on_file = count_bits(own_pawns & file_mask) as i32;
        if count_on_file > 1 {
            mg -= params.doubled_mg * (count_on_file - 1);
            eg -= params.doubled_eg * (count_on_file - 1);
        }
        if count_on_file > 0 {
            let mut adjacent: u64 = 0;
            if file > 0 {
                adjacent |= FILE_A << (file - 1);
            }
            if file < 7 {
                adjacent |= FILE_A << (file + 1);
            }
            if own_pawns & adjacent == EMPTY {
                mg -= params.isolated_mg;
                eg -= params.isolated_eg;
            }
        }
    }

    // Peones pasados: bonus creciente según lo avanzados que estén.
    let mut bb = own_pawns;
    while bb != EMPTY {
        let sq = pop_lsb(&mut bb);
        if is_passed_pawn(enemy_pawns, color, sq) {
            let (bonus_mg, bonus_eg) = passed_pawn_bonus(board, own_pawns, color, sq, params);
            passed_mg += bonus_mg;
            passed_eg += bonus_eg;
        }
    }

    ((mg, eg), (passed_mg, passed_eg))
}

/// Heurística ligera: penaliza columnas abiertas/semi-abiertas junto al rey
/// cuando este todavía está cerca de su casa (aprox. "sigue enrocado o sin
/// desarrollar"). No pretende ser un modelo completo de seguridad del rey;
/// eso se refina en Fase 2 con datos reales.
fn king_safety(board: &Board, color: Color, params: &EvalParams) -> i32 {
    let king_sq = board.king_square(color);
    let file = file_of(king_sq);
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];

    let near_home = match color {
        Color::White => rank_of(king_sq) <= 1,
        Color::Black => rank_of(king_sq) >= 6,
    };
    if !near_home {
        return 0;
    }

    let mut score = 0;
    let lo = file.saturating_sub(1);
    let hi = (file + 1).min(7);
    for f in lo..=hi {
        let file_mask: u64 = FILE_A << f;
        if own_pawns & file_mask == EMPTY {
            score -= params.king_open_file;
        }
    }
    score
}

/// Puntuación relativa a quien tiene el turno (positivo = bueno para el que mueve).
pub fn evaluate(board: &Board) -> i32 {
    breakdown(board).total
}

/// Igual que `evaluate`, pero con pesos arbitrarios: es la puerta que usa el
/// afinador Texel para probar candidatos sin tocar los defaults del motor.
pub fn evaluate_with(board: &Board, params: &EvalParams) -> i32 {
    breakdown_with(board, params).total
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EvalBreakdown {
    pub material: i32,
    pub piece_square: i32,
    pub pawn_structure: i32,
    pub passed_pawns: i32,
    pub mobility: i32,
    pub king_safety: i32,
    pub tempo: i32,
    pub total: i32,
}

fn taper(mg: i32, eg: i32, phase: i32) -> i32 {
    (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE
}

fn relative_to_move(board: &Board, white_minus_black: i32) -> i32 {
    if board.side_to_move == Color::White {
        white_minus_black
    } else {
        -white_minus_black
    }
}

/// Desglose de la evaluación relativa a quien tiene el turno.
pub fn breakdown(board: &Board) -> EvalBreakdown {
    breakdown_with(board, default_params())
}

/// Igual que `breakdown`, pero con pesos arbitrarios (ver `evaluate_with`).
pub fn breakdown_with(board: &Board, params: &EvalParams) -> EvalBreakdown {
    // L1 (inv. 29 fase 2): `material_and_pst(c)` y `pawn_structure(c)` son, POR
    // DEFINICION del propio codigo, la suma de los componentes que ya se
    // calculan aqui. v8 los ejecutaba CUATRO veces para obtener DOS resultados.
    // Se reutilizan. Aritmetica identica; ningun otro cambio.
    let (w_material, w_pst) = material_and_pst_components(board, Color::White, params);
    let (b_material, b_pst) = material_and_pst_components(board, Color::Black, params);
    let (w_mg, w_eg) = (w_material.0 + w_pst.0, w_material.1 + w_pst.1);
    let (b_mg, b_eg) = (b_material.0 + b_pst.0, b_material.1 + b_pst.1);
    let (wm_mg, wm_eg) = mobility(board, Color::White, params);
    let (bm_mg, bm_eg) = mobility(board, Color::Black, params);
    let (w_pawns, w_passed) = pawn_structure_components(board, Color::White, params);
    let (b_pawns, b_passed) = pawn_structure_components(board, Color::Black, params);
    let (wp_mg, wp_eg) = (w_pawns.0 + w_passed.0, w_pawns.1 + w_passed.1);
    let (bp_mg, bp_eg) = (b_pawns.0 + b_passed.0, b_pawns.1 + b_passed.1);
    let wk = king_safety(board, Color::White, params);
    let bk = king_safety(board, Color::Black, params);

    let mg = (w_mg + wm_mg + wp_mg + wk) - (b_mg + bm_mg + bp_mg + bk);
    let eg = (w_eg + wm_eg + wp_eg) - (b_eg + bm_eg + bp_eg);

    let phase = game_phase(board);
    let score = taper(mg, eg, phase);

    // Convertimos primero a la perspectiva de quien mueve...
    let relative = relative_to_move(board, score);

    // ...y SOLO DESPUÉS sumamos el bono de tempo: así queda garantizado que
    // beneficia a quien tiene el turno sin importar su color. Sumarlo antes
    // de la conversión (como se hacía originalmente) lo convertía en una
    // penalización para las negras en vez de un bono — bug real, detectado
    // por el test `evaluation_is_color_symmetric`.
    EvalBreakdown {
        material: relative_to_move(
            board,
            taper(
                w_material.0 - b_material.0,
                w_material.1 - b_material.1,
                phase,
            ),
        ),
        piece_square: relative_to_move(board, taper(w_pst.0 - b_pst.0, w_pst.1 - b_pst.1, phase)),
        pawn_structure: relative_to_move(
            board,
            taper(w_pawns.0 - b_pawns.0, w_pawns.1 - b_pawns.1, phase),
        ),
        passed_pawns: relative_to_move(
            board,
            taper(w_passed.0 - b_passed.0, w_passed.1 - b_passed.1, phase),
        ),
        mobility: relative_to_move(board, taper(wm_mg - bm_mg, wm_eg - bm_eg, phase)),
        king_safety: relative_to_move(board, wk - bk),
        tempo: params.tempo,
        total: relative + params.tempo,
    }
}

pub fn trace(board: &Board) -> String {
    let side_to_move = match board.side_to_move {
        Color::White => "White",
        Color::Black => "Black",
    };
    let breakdown = breakdown(board);
    format!(
        "eval side_to_move {side_to_move}\n\
         eval material {}\n\
         eval piece_square {}\n\
         eval pawn_structure {}\n\
         eval passed_pawns {}\n\
         eval mobility {}\n\
         eval king_safety {}\n\
         eval tempo {}\n\
         eval total {}\n",
        breakdown.material,
        breakdown.piece_square,
        breakdown.pawn_structure,
        breakdown.passed_pawns,
        breakdown.mobility,
        breakdown.king_safety,
        breakdown.tempo,
        breakdown.total
    )
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
    fn protected_passed_pawn_scores_higher_than_unprotected() {
        let unprotected = Board::from_fen("4k3/8/8/3P4/8/8/2P5/4K3 w - - 0 1").unwrap();
        let protected = Board::from_fen("4k3/8/8/3P4/2P5/8/8/4K3 w - - 0 1").unwrap();
        assert!(
            breakdown(&protected).passed_pawns > breakdown(&unprotected).passed_pawns,
            "protected passer should increase passed_pawns term"
        );
    }

    #[test]
    fn connected_passed_pawns_score_higher_than_single_passer() {
        let single = Board::from_fen("4k3/8/8/3P4/8/8/8/4K3 w - - 0 1").unwrap();
        let connected = Board::from_fen("4k3/8/8/3PP3/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(
            breakdown(&connected).passed_pawns > breakdown(&single).passed_pawns,
            "connected passers should increase passed_pawns term"
        );
    }

    #[test]
    fn king_supports_passed_pawn_more_than_enemy_king_blockades() {
        let supported = Board::from_fen("4k3/8/3P4/3K4/8/8/8/8 w - - 0 1").unwrap();
        let blockaded = Board::from_fen("3k4/8/3P4/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(
            breakdown(&supported).passed_pawns > breakdown(&blockaded).passed_pawns,
            "own king support should outscore enemy king blockade in passed_pawns"
        );
    }

    #[test]
    fn breakdown_total_matches_evaluate_after_passed_pawn_enrichment() {
        let board = Board::from_fen("4k3/8/3PP3/8/8/8/8/3K4 w - - 0 1").unwrap();
        assert_eq!(breakdown(&board).total, evaluate(&board));
    }

    #[test]
    fn trace_reports_enriched_passed_pawns() {
        let board = Board::from_fen("4k3/8/3PP3/8/8/8/8/3K4 w - - 0 1").unwrap();
        let passed = breakdown(&board).passed_pawns;
        let trace = trace(&board);
        assert!(passed > 0);
        assert!(
            trace.contains(&format!("eval passed_pawns {passed}")),
            "trace should report enriched passed_pawns term: {trace}"
        );
        assert!(
            trace.contains(&format!("eval total {}", breakdown(&board).total)),
            "trace should report total consistently: {trace}"
        );
    }

    #[test]
    fn breakdown_total_matches_evaluate_startpos() {
        let board = Board::start_pos();
        assert_eq!(breakdown(&board).total, evaluate(&board));
    }

    #[test]
    fn breakdown_total_matches_evaluate_tactical_position() {
        let board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
                .unwrap();
        assert_eq!(breakdown(&board).total, evaluate(&board));
    }

    #[test]
    fn trace_contains_total_and_terms() {
        let trace = trace(&Board::start_pos());
        for term in [
            "eval side_to_move",
            "eval material",
            "eval piece_square",
            "eval pawn_structure",
            "eval passed_pawns",
            "eval mobility",
            "eval king_safety",
            "eval tempo",
            "eval total",
        ] {
            assert!(trace.contains(term), "trace missing {term}: {trace}");
        }
    }

    #[test]
    fn evaluation_trace_is_deterministic() {
        let board =
            Board::from_fen("r1bqk2r/2pp1ppp/p1n2n2/1pb1p3/4P3/1B3N2/PPPP1PPP/RNBQ1RK1 w kq - 0 8")
                .unwrap();
        assert_eq!(trace(&board), trace(&board));
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
