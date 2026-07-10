//! Material y tablas posicionales (PST) con "tapered eval": interpola entre
//! valores de medio juego y de final según cuántas piezas quedan.
//!
//! Las PST no están copiadas de ningún motor existente: se generan por
//! fórmula (distancia al centro, avance de fila, etc.) en `build_pst`, lo
//! cual además las hace fáciles de razonar y ajustar más adelante con Texel
//! tuning.

use crate::bitboard::{pop_lsb, EMPTY};
use crate::board::Board;
use crate::eval::score::Score;
use crate::types::*;
use std::sync::OnceLock;

pub fn piece_value(p: PieceType) -> i32 {
    match p {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 0,
    }
}

const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0]; // pawn,knight,bishop,rook,queen,king
pub const MAX_PHASE: i32 = 24; // 2 bandos * (2N+2B+2R+1Q) = 2*(2+2+4+4) = 24

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

static PST: OnceLock<Pst> = OnceLock::new();
fn pst() -> &'static Pst {
    PST.get_or_init(build_pst)
}

pub fn game_phase(board: &Board) -> i32 {
    let mut phase = 0;
    for color in [Color::White, Color::Black] {
        for pt in [
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
        ] {
            let count = crate::bitboard::count_bits(board.pieces[color.index()][pt.index()]) as i32;
            phase += count * PHASE_WEIGHT[pt.index()];
        }
    }
    phase.min(MAX_PHASE)
}

pub fn material_and_pst(board: &Board, color: Color) -> Score {
    let p = pst();
    let mut mg = 0;
    let mut eg = 0;
    for pt in ALL_PIECE_TYPES {
        let mut bb = board.pieces[color.index()][pt.index()];
        let value = piece_value(pt);
        while bb != EMPTY {
            let sq = pop_lsb(&mut bb);
            let idx = if color == Color::White {
                sq
            } else {
                mirror(sq)
            };
            mg += value + p.mg[pt.index()][idx as usize];
            eg += value + p.eg[pt.index()][idx as usize];
        }
    }
    Score::new(mg, eg)
}
