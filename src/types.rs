//! Tipos fundamentales compartidos por todo el motor.

/// Color de una pieza o del turno actual.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    #[inline(always)]
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    #[inline(always)]
    pub fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }
}

/// Tipo de pieza, independiente de color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// Usado para iterar los 6 tipos de pieza al calcular material/PST en `eval`.
pub const ALL_PIECE_TYPES: [PieceType; 6] = [
    PieceType::Pawn,
    PieceType::Knight,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::Queen,
    PieceType::King,
];

impl PieceType {
    #[inline(always)]
    pub fn index(self) -> usize {
        match self {
            PieceType::Pawn => 0,
            PieceType::Knight => 1,
            PieceType::Bishop => 2,
            PieceType::Rook => 3,
            PieceType::Queen => 4,
            PieceType::King => 5,
        }
    }

    /// Letra minúscula estándar (usada en promociones UCI: e7e8q).
    pub fn to_char(self) -> char {
        match self {
            PieceType::Pawn => 'p',
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook => 'r',
            PieceType::Queen => 'q',
            PieceType::King => 'k',
        }
    }

    pub fn from_char(c: char) -> Option<PieceType> {
        match c.to_ascii_lowercase() {
            'p' => Some(PieceType::Pawn),
            'n' => Some(PieceType::Knight),
            'b' => Some(PieceType::Bishop),
            'r' => Some(PieceType::Rook),
            'q' => Some(PieceType::Queen),
            'k' => Some(PieceType::King),
            _ => None,
        }
    }
}

/// Una pieza concreta: color + tipo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceType,
}

impl Piece {
    pub fn new(color: Color, kind: PieceType) -> Self {
        Piece { color, kind }
    }

    /// Letra FEN: mayúscula para blancas, minúscula para negras.
    pub fn to_fen_char(self) -> char {
        let c = self.kind.to_char();
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }
}

/// Casilla representada como índice 0..64, mapeo LERF (Little-Endian Rank-File):
/// a1 = 0, b1 = 1, ..., h1 = 7, a2 = 8, ..., h8 = 63.
pub type Square = u8;

#[inline(always)]
pub const fn make_square(file: u8, rank: u8) -> Square {
    rank * 8 + file
}

#[inline(always)]
pub const fn file_of(sq: Square) -> u8 {
    sq % 8
}

#[inline(always)]
pub const fn rank_of(sq: Square) -> u8 {
    sq / 8
}

/// Convierte una casilla a notación algebraica ("e4").
pub fn square_to_str(sq: Square) -> String {
    let file = (b'a' + file_of(sq)) as char;
    let rank = (b'1' + rank_of(sq)) as char;
    format!("{file}{rank}")
}

/// Convierte notación algebraica ("e4") a casilla.
pub fn str_to_square(s: &str) -> Option<Square> {
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let file = bytes[0];
    let rank = bytes[1];
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return None;
    }
    Some(make_square(file - b'a', rank - b'1'))
}

// Nombres de casillas usados frecuentemente (esquinas de torres/rey, etc.)
pub const A1: Square = 0;
pub const B1: Square = 1;
pub const C1: Square = 2;
pub const D1: Square = 3;
pub const E1: Square = 4;
pub const F1: Square = 5;
pub const G1: Square = 6;
pub const H1: Square = 7;
pub const A8: Square = 56;
pub const B8: Square = 57;
pub const C8: Square = 58;
pub const D8: Square = 59;
pub const E8: Square = 60;
pub const F8: Square = 61;
pub const G8: Square = 62;
pub const H8: Square = 63;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algebraic_roundtrip() {
        for sq in 0..64u8 {
            let s = square_to_str(sq);
            assert_eq!(str_to_square(&s), Some(sq));
        }
    }

    #[test]
    fn known_squares() {
        assert_eq!(str_to_square("a1"), Some(A1));
        assert_eq!(str_to_square("h8"), Some(H8));
        assert_eq!(str_to_square("e4"), Some(make_square(4, 3)));
    }
}
