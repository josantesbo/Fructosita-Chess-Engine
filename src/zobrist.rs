//! Claves Zobrist para hashing incremental de posiciones.
//!
//! El hash de una posición es el XOR de una clave aleatoria por cada
//! característica presente: pieza en casilla, turno, derechos de enroque,
//! columna de captura al paso disponible. Esto permite actualizar el hash de
//! forma incremental en `Board::make_move` (XOR de lo que cambia) en vez de
//! recalcularlo desde cero en cada posición.

use crate::types::{Color, PieceType, Square};
use std::sync::OnceLock;

pub struct ZobristKeys {
    piece_square: [[[u64; 64]; 6]; 2], // [color][piece_type][square]
    pub side_to_move: u64,
    pub castling: [u64; 4], // 0=WK, 1=WQ, 2=BK, 3=BQ
    pub en_passant_file: [u64; 8],
}

/// xorshift64* determinista con semilla fija: las claves son siempre las
/// mismas entre ejecuciones, lo cual hace que el hash de una posición dada
/// sea reproducible (útil para depurar y para tests).
struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = XorShift64(0x9E3779B97F4A7C15);
        let mut piece_square = [[[0u64; 64]; 6]; 2];
        for c in piece_square.iter_mut() {
            for p in c.iter_mut() {
                for s in p.iter_mut() {
                    *s = rng.next();
                }
            }
        }
        let side_to_move = rng.next();
        let mut castling = [0u64; 4];
        for k in castling.iter_mut() {
            *k = rng.next();
        }
        let mut en_passant_file = [0u64; 8];
        for k in en_passant_file.iter_mut() {
            *k = rng.next();
        }
        ZobristKeys {
            piece_square,
            side_to_move,
            castling,
            en_passant_file,
        }
    }

    #[inline(always)]
    pub fn piece(&self, color: Color, piece: PieceType, sq: Square) -> u64 {
        self.piece_square[color.index()][piece.index()][sq as usize]
    }
}

static KEYS: OnceLock<ZobristKeys> = OnceLock::new();

pub fn keys() -> &'static ZobristKeys {
    KEYS.get_or_init(ZobristKeys::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_distinct() {
        let k = keys();
        assert_ne!(
            k.piece(Color::White, PieceType::Pawn, 0),
            k.piece(Color::White, PieceType::Pawn, 1)
        );
        assert_ne!(
            k.piece(Color::White, PieceType::Pawn, 0),
            k.piece(Color::Black, PieceType::Pawn, 0)
        );
        assert_ne!(k.side_to_move, k.castling[0]);
    }
}
