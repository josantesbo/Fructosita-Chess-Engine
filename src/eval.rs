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

/// Pesos escalares afinables de la evaluación (método Texel).
///
/// EXP-0015 (EVAL-D): la lista se define con una macro para que struct,
/// `Default`, nombres y conversión a vector (la usa el afinador) salgan de la
/// MISMA lista y no puedan desincronizarse. Los valores por defecto son los
/// afinados en EXP-0015 (ver EXPERIMENT_LEDGER); los términos que ya existían
/// parten de los históricos.
macro_rules! eval_params {
    ($($name:ident = $val:expr),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct EvalParams {
            $(pub $name: i32,)*
            /// EVAL-F: corrección de PST por columna y fila:
            /// [fase (mg, eg)][pieza][0..8 columna, 8..16 fila relativa].
            pub pst_fr: [[[i32; 16]; 6]; 2],
        }
        impl Default for EvalParams {
            fn default() -> Self {
                EvalParams { $($name: $val,)* pst_fr: PST_FR_DEFAULT }
            }
        }
        pub const PARAM_NAMES: &[&str] = &[$(stringify!($name),)*];
        impl EvalParams {
            /// Número de escalares (los que no pueden ser negativos en el afinado).
            pub fn scalar_count() -> usize {
                PARAM_NAMES.len()
            }
            pub fn param_name(i: usize) -> String {
                if i < PARAM_NAMES.len() {
                    PARAM_NAMES[i].to_string()
                } else {
                    let j = i - PARAM_NAMES.len();
                    let (ph, pc, k) = (j / 96, (j / 16) % 6, j % 16);
                    format!("pst_fr[{}][{}][{}{}]", if ph == 0 { "mg" } else { "eg" }, pc,
                        if k < 8 { "file" } else { "rank" }, k % 8)
                }
            }
            pub fn to_vec(&self) -> Vec<i32> {
                let mut v = vec![$(self.$name,)*];
                for ph in &self.pst_fr { for pc in ph { v.extend_from_slice(pc); } }
                v
            }
            pub fn from_vec(v: &[i32]) -> Self {
                let mut it = v.iter();
                let mut e = EvalParams { $($name: *it.next().expect("vector de pesos corto"),)* pst_fr: [[[0; 16]; 6]; 2] };
                for ph in e.pst_fr.iter_mut() { for pc in ph.iter_mut() { for x in pc.iter_mut() {
                    *x = *it.next().expect("vector de pesos corto");
                } } }
                e
            }
        }
    };
}

/// EVAL-F: valores afinados de la corrección de PST por columna/fila.
pub const PST_FR_DEFAULT: [[[i32; 16]; 6]; 2] = [
    [
        [0, 6, 16, 17, 22, 30, 21, -8, 0, 9, 4, -4, -10, 8, -3, 0],
        [-16, -5, 3, 12, 20, 18, 17, -2, -1, 3, 13, 18, 20, -7, -10, -62],
        [-3, 6, 11, 7, 11, 10, 20, 2, -11, 9, 6, 6, -1, -14, -52, -56],
        [-11, -8, 1, 4, 12, 12, -37, -16, -6, -30, -22, -22, -25, -52, -34, -79],
        [-2, -2, -5, 1, 8, -11, 2, -3, -1, 10, 6, 3, -9, 1, -24, -46],
        [-27, 18, 1, -36, -2, -27, 16, 2, 26, 4, 20, -21, -24, 116, -19, -28],
    ],
    [
        [9, 1, 0, -8, -2, -4, -9, 1, 0, 6, -4, -8, -12, 6, 71, 0],
        [-23, -8, -3, -1, 0, -8, -16, -22, -49, -12, 2, 1, -3, 1, -17, -9],
        [-13, -2, -1, 1, -1, -3, -8, -9, -22, -17, -1, -1, 8, -2, 2, -33],
        [17, 18, 15, 10, 2, 4, 21, 7, 8, 7, -3, 5, 13, 12, 11, 28],
        [-41, -26, -10, -17, -12, 21, 5, 6, -5, -22, 9, 24, 14, -20, -8, 2],
        [-20, -11, 0, 6, 2, 12, 0, -16, -8, 8, 8, 4, 2, -22, -31, -38],
    ],
];

eval_params! {
    pawn = 89,
    knight = 382,
    bishop = 401,
    rook = 622,
    queen = 1244,
    // EXP-0015: movilidad SEGURA por tipo de pieza (casillas no ocupadas por
    // piezas propias ni atacadas por peones rivales), en cp por casilla.
    mob_knight_mg = 13,
    mob_knight_eg = 1,
    mob_bishop_mg = 9,
    mob_bishop_eg = 2,
    mob_rook_mg = 5,
    mob_rook_eg = 5,
    mob_queen_mg = 5,
    mob_queen_eg = 5,
    doubled_mg = 15,
    doubled_eg = 19,
    isolated_mg = 16,
    isolated_eg = 15,
    passed_base = 3,
    passed_advancement = 9,
    passed_protected = 0,
    passed_connected = 0,
    passed_king_race = 23,
    king_open_file = 21,
    tempo = 31,
    // EXP-0015: amenazas y piezas colgadas, outposts de caballo.
    threat_pawn_mg = 73,
    threat_pawn_eg = 15,
    threat_minor_mg = 54,
    threat_minor_eg = 7,
    hanging_mg = 23,
    hanging_eg = 36,
    outpost_mg = 41,
    outpost_eg = 12,
    bishop_pair_mg = 45,
    bishop_pair_eg = 52,
    rook_open_mg = 52,
    rook_open_eg = 0,
    rook_semi_mg = 14,
    rook_semi_eg = 15,
    // EVAL-E: escudo de peones delante del rey (1 y 2 filas), tormenta de
    // peones rivales que se acercan, y peón pasado bloqueado / con paso libre.
    shield_near_mg = 12,
    shield_far_mg = 4,
    storm_mg = 0,
    passed_blocked_mg = 0,
    passed_blocked_eg = 38,
    passed_free_eg = 13,
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
            let (f, r) = ((idx & 7) as usize, 8 + (idx >> 3) as usize);
            let fr = &params.pst_fr;
            pst_mg += p.mg[pt.index()][idx as usize] + fr[0][pt.index()][f] + fr[0][pt.index()][r];
            pst_eg += p.eg[pt.index()][idx as usize] + fr[1][pt.index()][f] + fr[1][pt.index()][r];
        }
    }
    ((material_mg, material_eg), (pst_mg, pst_eg))
}


// ---------------------------------------------------------------------------
// EVAL-A — ataque al rey, pareja de alfiles y torres en columnas (diseño propio).
//
// ATAQUE AL REY. La "seguridad del rey" histórica solo mira columnas sin peones
// propios junto al rey; no ve las piezas enemigas que apuntan a él. Aquí, para
// el bando `color` que ATACA, se cuentan las casillas de la ZONA del rey rival
// (rey + casillas adyacentes + la fila siguiente hacia el atacante) que ataca
// cada pieza. Cada casilla vale unas "unidades" según la pieza, proporcionales
// a su capacidad de rematar (caballo/alfil 2, torre 3, dama 5 ≈ valor relativo
// de las piezas redondeado a enteros pequeños). Con UNA sola pieza no hay
// ataque real (se defiende fácil): solo cuenta con >= 2 atacantes. La
// penalización crece de forma CUADRÁTICA con las unidades porque el peligro de
// un ataque coordinado es superlineal (cada atacante extra multiplica las
// líneas de mate). Sin dama atacante se divide por 2 (sin la dama casi ningún
// ataque de medio juego prospera). Tope de 400 cp (≈ 5,7 peones a la escala
// pawn=70 del motor) para que ningún término domine al material. Solo medio
// juego (el taper lo desvanece hacia el final, donde el rey debe salir).
// Coeficientes elegidos por razonamiento, NO afinados ni copiados: 3 atacantes
// sobre 2 casillas cada uno (N+R+Q) = 20 unidades -> 20²/5 = 80 cp.
// ---------------------------------------------------------------------------
const KA_UNITS: [i32; 6] = [0, 2, 2, 3, 5, 0]; // pawn, knight, bishop, rook, queen, king
const KA_DIVISOR: i32 = 5;
const KA_CAP: i32 = 400;

fn king_zone(king_sq: Square, king_color: Color) -> u64 {
    let t = tables();
    let near = t.king_attacks(king_sq) | (1u64 << king_sq);
    // Una fila más hacia el bando atacante (delante del rey defensor).
    let ahead = match king_color {
        Color::White => near << 8,
        Color::Black => near >> 8,
    };
    near | ahead
}

const FILE_H: u64 = FILE_A << 7;

/// Casillas atacadas por los peones de `color`.
#[inline]
fn pawn_attacks_bb(pawns: u64, color: Color) -> u64 {
    match color {
        Color::White => ((pawns << 7) & !FILE_H) | ((pawns << 9) & !FILE_A),
        Color::Black => ((pawns >> 9) & !FILE_H) | ((pawns >> 7) & !FILE_A),
    }
}

/// Resultado del recorrido de piezas de un bando.
struct PieceScan {
    mob_mg: i32,
    mob_eg: i32,
    king_attack: i32,
    /// Todas las casillas atacadas por el bando (piezas, peones y rey).
    attacks: u64,
    /// Casillas atacadas por sus caballos y alfiles.
    minor_attacks: u64,
}

/// Movilidad y ataque al rey rival de `color` en un solo recorrido.
/// EXP-0015: la movilidad es por tipo de pieza y SEGURA (sin contar casillas
/// propias ni atacadas por peones rivales: ahí la pieza no puede instalarse).
/// El ataque al rey (EVAL-A) no cambia.
fn scan_pieces(board: &Board, color: Color, params: &EvalParams) -> PieceScan {
    let t = tables();
    let occ = board.occupancy();
    let own = board.color_occupancy(color);
    let enemy = color.opposite();
    let zone = king_zone(board.king_square(enemy), enemy);
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawn_att = pawn_attacks_bb(board.pieces[enemy.index()][PieceType::Pawn.index()], enemy);
    let safe = !own & !enemy_pawn_att;
    let (mut mob_mg, mut mob_eg) = (0i32, 0i32);
    let mut units = 0i32;
    let mut attackers = 0i32;
    let mut attacks = pawn_attacks_bb(own_pawns, color) | t.king_attacks(board.king_square(color));
    let mut minor_attacks = 0u64;

    for pt in [PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
        let (wmg, weg) = match pt {
            PieceType::Knight => (params.mob_knight_mg, params.mob_knight_eg),
            PieceType::Bishop => (params.mob_bishop_mg, params.mob_bishop_eg),
            PieceType::Rook => (params.mob_rook_mg, params.mob_rook_eg),
            _ => (params.mob_queen_mg, params.mob_queen_eg),
        };
        let mut bb = board.pieces[color.index()][pt.index()];
        while bb != EMPTY {
            let sq = pop_lsb(&mut bb);
            let att = match pt {
                PieceType::Knight => t.knight_attacks(sq),
                PieceType::Bishop => t.bishop_attacks(sq, occ),
                PieceType::Rook => t.rook_attacks(sq, occ),
                _ => t.queen_attacks(sq, occ),
            };
            attacks |= att;
            if matches!(pt, PieceType::Knight | PieceType::Bishop) {
                minor_attacks |= att;
            }
            let n = count_bits(att & safe) as i32;
            mob_mg += n * wmg;
            mob_eg += n * weg;
            let hits = count_bits(att & zone) as i32;
            if hits > 0 {
                attackers += 1;
                units += KA_UNITS[pt.index()] * hits;
            }
        }
    }

    let mut king_attack = 0;
    if attackers >= 2 {
        king_attack = (units * units / KA_DIVISOR).min(KA_CAP);
        if board.pieces[color.index()][PieceType::Queen.index()] == EMPTY {
            king_attack /= 2;
        }
    }
    PieceScan { mob_mg, mob_eg, king_attack, attacks, minor_attacks }
}

/// EXP-0015 (EVAL-D): amenazas de `color` sobre el rival y outposts propios.
/// - Pieza (N, B, R, Q) rival atacada por un peón propio: casi siempre gana
///   material o fuerza una retirada con pérdida de tiempo.
/// - Torre o dama rival atacada por una menor propia: mismo motivo.
/// - Pieza rival colgada: atacada y sin ninguna defensa.
/// - Outpost: caballo propio en las filas 4–6 relativas, defendido por un peón
///   y fuera del alcance futuro de los peones rivales (columnas adyacentes).
fn threats_and_outposts(
    board: &Board,
    color: Color,
    params: &EvalParams,
    own: &PieceScan,
    their: &PieceScan,
) -> (i32, i32) {
    let (xmg, xeg) = king_shelter_and_passers(board, color, params, their);
    let (tmg, teg) = threats_core(board, color, params, own, their);
    (xmg + tmg, xeg + teg)
}

/// EVAL-E (diseño propio):
/// - ESCUDO: con el rey propio en sus dos primeras filas, cada peón propio en
///   las columnas del rey y adyacentes, una fila por delante (`near`) o dos
///   (`far`), suma: es la cobertura que impide abrir líneas contra el rey.
/// - TORMENTA: peones rivales en esas columnas que ya están a <= 3 filas del
///   rey propio: preparan la apertura de columnas (penaliza, solo mg).
/// - PASADOS: la casilla delante ocupada por una pieza rival (bloqueo) resta;
///   vacía y no atacada por el rival suma, creciendo con el avance (solo eg).
fn king_shelter_and_passers(board: &Board, color: Color, params: &EvalParams, their: &PieceScan) -> (i32, i32) {
    let ci = color.index();
    let ei = color.opposite().index();
    let own_pawns = board.pieces[ci][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[ei][PieceType::Pawn.index()];
    let mut mg = 0;
    let mut eg = 0;

    let ksq = board.king_square(color);
    let (kf, kr) = (file_of(ksq) as i32, rank_of(ksq) as i32);
    let rel = |r: i32| if color == Color::White { r } else { 7 - r };
    if rel(kr) <= 1 {
        for f in (kf - 1).max(0)..=(kf + 1).min(7) {
            let file_mask = FILE_A << f;
            let mut own_on = own_pawns & file_mask;
            while own_on != EMPTY {
                let sq = pop_lsb(&mut own_on);
                let d = rel(rank_of(sq) as i32) - rel(kr);
                if d == 1 {
                    mg += params.shield_near_mg;
                } else if d == 2 {
                    mg += params.shield_far_mg;
                }
            }
            let mut en_on = enemy_pawns & file_mask;
            while en_on != EMPTY {
                let sq = pop_lsb(&mut en_on);
                let d = rel(rank_of(sq) as i32) - rel(kr);
                if (1..=3).contains(&d) {
                    mg -= params.storm_mg;
                }
            }
        }
    }

    let occ_enemy = board.color_occupancy(color.opposite());
    let mut pawns = own_pawns;
    while pawns != EMPTY {
        let sq = pop_lsb(&mut pawns);
        if !is_passed_pawn(enemy_pawns, color, sq) {
            continue;
        }
        let ahead = if color == Color::White { sq + 8 } else { sq.wrapping_sub(8) };
        if ahead >= 64 {
            continue;
        }
        let ahead_bb = 1u64 << ahead;
        if occ_enemy & ahead_bb != EMPTY {
            mg -= params.passed_blocked_mg;
            eg -= params.passed_blocked_eg;
        } else if board.occupancy() & ahead_bb == EMPTY && their.attacks & ahead_bb == EMPTY {
            eg += params.passed_free_eg * rel(rank_of(sq) as i32);
        }
    }
    (mg, eg)
}

fn threats_core(
    board: &Board,
    color: Color,
    params: &EvalParams,
    own: &PieceScan,
    their: &PieceScan,
) -> (i32, i32) {
    let enemy = color.opposite();
    let ei = enemy.index();
    let ci = color.index();
    let pieces = board.pieces[ei][PieceType::Knight.index()]
        | board.pieces[ei][PieceType::Bishop.index()]
        | board.pieces[ei][PieceType::Rook.index()]
        | board.pieces[ei][PieceType::Queen.index()];
    let majors = board.pieces[ei][PieceType::Rook.index()] | board.pieces[ei][PieceType::Queen.index()];
    let own_pawn_att = pawn_attacks_bb(board.pieces[ci][PieceType::Pawn.index()], color);
    let by_pawn = count_bits(pieces & own_pawn_att) as i32;
    let by_minor = count_bits(majors & own.minor_attacks) as i32;
    let hanging = count_bits(pieces & own.attacks & !their.attacks) as i32;
    let mut mg = by_pawn * params.threat_pawn_mg + by_minor * params.threat_minor_mg + hanging * params.hanging_mg;
    let mut eg = by_pawn * params.threat_pawn_eg + by_minor * params.threat_minor_eg + hanging * params.hanging_eg;

    let enemy_pawns = board.pieces[ei][PieceType::Pawn.index()];
    let mut knights = board.pieces[ci][PieceType::Knight.index()] & own_pawn_att;
    while knights != EMPTY {
        let sq = pop_lsb(&mut knights);
        let rel_rank = if color == Color::White { rank_of(sq) } else { 7 - rank_of(sq) };
        if !(3..=5).contains(&rel_rank) {
            continue;
        }
        let file_mask = FILE_A << file_of(sq);
        if enemy_pawns & PASSED_MASK[ci][sq as usize] & !file_mask == EMPTY {
            mg += params.outpost_mg;
            eg += params.outpost_eg;
        }
    }
    (mg, eg)
}

/// Pareja de alfiles y torres en columnas abiertas/semiabiertas de `color`.
/// PAREJA: dos alfiles cubren ambos colores de casilla; su ventaja crece al
/// abrirse la posición, de ahí más peso en el final (20/40 ≈ 0,3/0,6 peones).
/// TORRES: una torre sin peones propios delante tiene la columna para entrar;
/// abierta del todo (sin peones) vale el doble que semiabierta. Más en medio
/// juego (presión sobre el enroque / 7ª) que en el final.
fn piece_extras(board: &Board, color: Color, params: &EvalParams) -> (i32, i32) {
    let mut mg = 0;
    let mut eg = 0;
    if count_bits(board.pieces[color.index()][PieceType::Bishop.index()]) >= 2 {
        mg += params.bishop_pair_mg;
        eg += params.bishop_pair_eg;
    }
    let own_pawns = board.pieces[color.index()][PieceType::Pawn.index()];
    let enemy_pawns = board.pieces[color.opposite().index()][PieceType::Pawn.index()];
    let mut rooks = board.pieces[color.index()][PieceType::Rook.index()];
    while rooks != EMPTY {
        let sq = pop_lsb(&mut rooks);
        let file_mask: u64 = FILE_A << file_of(sq);
        if own_pawns & file_mask == EMPTY {
            if enemy_pawns & file_mask == EMPTY {
                mg += params.rook_open_mg;
                eg += params.rook_open_eg;
            } else {
                mg += params.rook_semi_mg;
                eg += params.rook_semi_eg;
            }
        }
    }
    (mg, eg)
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

// ---------------------------------------------------------------------------
// EXP-0012 (EVAL-C) — conocimiento de finales (diseño propio).
//
// La eval sumaba material sin saber si ese material PUEDE ganar: KB contra K
// valía +3 peones y KR contra KB, +2. La búsqueda no lo corrige a STC (el mate o
// la tablas quedan a decenas de plies), así que el motor evitaba cambios que
// llevaban a tablas seguras, o los buscaba creyendo que ganaba. Reglas:
//   1. Material insuficiente sin peones (K, K+menor, K+N+N contra K o K+menor):
//      eval 0 exacto.
//   2. El bando que va ganando no tiene peones: si solo tiene una menor, no
//      puede ganar (score/16); si su ventaja en piezas es menor que una torre
//      (KRKB, KRKN, KRBKR, KQKRB…), el final es muy tablífero (score/4).
//   3. Alfiles de distinto color "puros" (cada bando: un alfil, sin más piezas
//      que peones y reyes): los peones de más valen mucho menos (score/2).
//   4. Mop-up: contra un rey desnudo, premio por empujarlo al borde y acercar
//      el rey propio, para que el motor convierta KQK/KRK/KBBK/KBNK sin depender
//      de ver el mate en la búsqueda.
// Las fracciones son propias, por razonamiento (umbral de "no puede ganar" =
// una torre de ventaja), no afinadas ni copiadas.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
struct SideMaterial {
    pawns: i32,
    knights: i32,
    bishops: i32,
    rooks: i32,
    queens: i32,
}

impl SideMaterial {
    fn of(board: &Board, color: Color) -> Self {
        let c = color.index();
        SideMaterial {
            pawns: count_bits(board.pieces[c][PieceType::Pawn.index()]) as i32,
            knights: count_bits(board.pieces[c][PieceType::Knight.index()]) as i32,
            bishops: count_bits(board.pieces[c][PieceType::Bishop.index()]) as i32,
            rooks: count_bits(board.pieces[c][PieceType::Rook.index()]) as i32,
            queens: count_bits(board.pieces[c][PieceType::Queen.index()]) as i32,
        }
    }
    fn minors(&self) -> i32 {
        self.knights + self.bishops
    }
    fn pieces(&self) -> i32 {
        self.minors() + self.rooks + self.queens
    }
    fn npm(&self, p: &EvalParams) -> i32 {
        self.knights * p.knight + self.bishops * p.bishop + self.rooks * p.rook + self.queens * p.queen
    }
}

const LIGHT_SQUARES: u64 = 0x55AA_55AA_55AA_55AA;

/// Ajuste de finales sobre el score blanco-relativo ya interpolado.
fn endgame_adjust(board: &Board, score: i32, params: &EvalParams) -> i32 {
    let w = SideMaterial::of(board, Color::White);
    let b = SideMaterial::of(board, Color::Black);

    // 1. Material insuficiente (sin peones en el tablero).
    if w.pawns == 0 && b.pawns == 0 {
        let weak_only = |m: &SideMaterial| m.rooks == 0 && m.queens == 0 && m.minors() <= 1;
        let nn_only = |m: &SideMaterial| m.rooks == 0 && m.queens == 0 && m.bishops == 0 && m.knights == 2;
        if (weak_only(&w) || nn_only(&w)) && (weak_only(&b) || nn_only(&b)) {
            return 0;
        }
    }

    let (strong, weak, strong_color) = if score > 0 {
        (w, b, Color::White)
    } else {
        (b, w, Color::Black)
    };
    let mut adjusted = score;

    // 2. El bando fuerte sin peones.
    if strong.pawns == 0 && score != 0 {
        if strong.rooks == 0 && strong.queens == 0 && strong.minors() <= 1 {
            adjusted /= 16;
        } else if strong.npm(params) - weak.npm(params) < params.rook {
            adjusted /= 4;
        }
    }

    // 3. Alfiles de distinto color puros.
    if w.bishops == 1
        && b.bishops == 1
        && w.pieces() == 1
        && b.pieces() == 1
    {
        let wb = board.pieces[Color::White.index()][PieceType::Bishop.index()];
        let bb = board.pieces[Color::Black.index()][PieceType::Bishop.index()];
        if ((wb & LIGHT_SQUARES) != 0) != ((bb & LIGHT_SQUARES) != 0) {
            adjusted /= 2;
        }
    }

    // 4. Mop-up contra rey desnudo, con material de mate.
    if weak.pawns == 0 && weak.pieces() == 0 && strong.pawns == 0 {
        let can_mate = strong.queens > 0
            || strong.rooks > 0
            || strong.bishops >= 2
            || (strong.bishops >= 1 && strong.knights >= 1);
        if can_mate {
            let wk = board.king_square(strong_color.opposite());
            let sk = board.king_square(strong_color);
            let (wf, wr) = (file_of(wk) as i32, rank_of(wk) as i32);
            let (sf, sr) = (file_of(sk) as i32, rank_of(sk) as i32);
            let edge = (3 - wf).max(wf - 4) + (3 - wr).max(wr - 4); // 0 (centro) .. 6 (esquina)
            let dist = (wf - sf).abs() + (wr - sr).abs(); // 1 .. 14
            let bonus = 20 * edge + 6 * (14 - dist);
            adjusted += if strong_color == Color::White { bonus } else { -bonus };
        }
    }
    adjusted
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
    /// EXP-0012: ajuste de finales (escalado, tablas por material, mop-up).
    pub endgame: i32,
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
    let ws = scan_pieces(board, Color::White, params);
    let bs = scan_pieces(board, Color::Black, params);
    let (wm_mg, wm_eg, w_attack) = (ws.mob_mg, ws.mob_eg, ws.king_attack);
    let (bm_mg, bm_eg, b_attack) = (bs.mob_mg, bs.mob_eg, bs.king_attack);
    let (wt_mg, wt_eg) = threats_and_outposts(board, Color::White, params, &ws, &bs);
    let (bt_mg, bt_eg) = threats_and_outposts(board, Color::Black, params, &bs, &ws);
    let (wx_mg, wx_eg) = piece_extras(board, Color::White, params);
    let (bx_mg, bx_eg) = piece_extras(board, Color::Black, params);
    let (w_pawns, w_passed) = pawn_structure_components(board, Color::White, params);
    let (b_pawns, b_passed) = pawn_structure_components(board, Color::Black, params);
    let (wp_mg, wp_eg) = (w_pawns.0 + w_passed.0, w_pawns.1 + w_passed.1);
    let (bp_mg, bp_eg) = (b_pawns.0 + b_passed.0, b_pawns.1 + b_passed.1);
    let wk = king_safety(board, Color::White, params);
    let bk = king_safety(board, Color::Black, params);

    // EVAL-A: el ataque de un bando cuenta como seguridad NEGATIVA del rival
    // (mismo signo que king_safety), y las piezas extra suman a su bando.
    let mg = (w_mg + wm_mg + wp_mg + wk + w_attack + wx_mg + wt_mg)
        - (b_mg + bm_mg + bp_mg + bk + b_attack + bx_mg + bt_mg);
    let eg = (w_eg + wm_eg + wp_eg + wx_eg + wt_eg) - (b_eg + bm_eg + bp_eg + bx_eg + bt_eg);

    let phase = game_phase(board);
    let tapered = taper(mg, eg, phase);
    let score = endgame_adjust(board, tapered, params);
    // EXP-0012: con material insuficiente la posición es tablas: sin tempo.
    let tempo = if score == 0 && tapered != 0 { 0 } else { params.tempo };

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
        mobility: relative_to_move(
            board,
            taper(
                wm_mg + wx_mg + wt_mg - bm_mg - bx_mg - bt_mg,
                wm_eg + wx_eg + wt_eg - bm_eg - bx_eg - bt_eg,
                phase,
            ),
        ),
        king_safety: relative_to_move(board, (wk + w_attack) - (bk + b_attack)),
        tempo,
        endgame: relative_to_move(board, score - tapered),
        total: relative + tempo,
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
         eval endgame {}\n\
         eval total {}\n",
        breakdown.material,
        breakdown.piece_square,
        breakdown.pawn_structure,
        breakdown.passed_pawns,
        breakdown.mobility,
        breakdown.king_safety,
        breakdown.tempo,
        breakdown.endgame,
        breakdown.total
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_material_is_a_draw() {
        for fen in [
            "8/8/4k3/8/8/3BK3/8/8 w - - 0 1",
            "8/8/4k3/8/8/3NK3/8/8 b - - 0 1",
            "8/8/4k3/8/8/2NNK3/8/8 w - - 0 1",
            "8/8/3bk3/8/8/3NK3/8/8 w - - 0 1",
        ] {
            assert_eq!(evaluate(&Board::from_fen(fen).unwrap()), 0, "{fen}");
        }
    }

    #[test]
    fn rook_vs_minor_without_pawns_is_scaled_down() {
        let krkb = evaluate(&Board::from_fen("8/8/4k3/3b4/8/3RK3/8/8 w - - 0 1").unwrap());
        assert!(krkb > 0 && krkb < 100, "KRKB debe ser ventaja pequeña: {krkb}");
        let krk = evaluate(&Board::from_fen("8/8/4k3/8/8/3RK3/8/8 w - - 0 1").unwrap());
        assert!(krk > 400, "KRK es ganado: {krk}");
    }

    #[test]
    fn mop_up_prefers_enemy_king_on_edge() {
        let center = evaluate(&Board::from_fen("8/8/8/3k4/8/8/8/R3K3 w - - 0 1").unwrap());
        let edge = evaluate(&Board::from_fen("k7/8/8/8/8/8/8/R3K3 w - - 0 1").unwrap());
        assert!(edge > center, "edge {edge} center {center}");
    }

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
