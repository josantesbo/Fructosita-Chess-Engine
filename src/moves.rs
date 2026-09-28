//! Representación de un movimiento de ajedrez.

use crate::types::{square_to_str, PieceType, Square};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveKind {
    Quiet,
    DoublePawnPush,
    Capture,
    EnPassantCapture,
    CastleKingside,
    CastleQueenside,
    Promotion(PieceType),
    PromotionCapture(PieceType),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub kind: MoveKind,
}

impl Move {
    pub fn new(from: Square, to: Square, kind: MoveKind) -> Self {
        Move { from, to, kind }
    }

    #[inline]
    pub fn is_capture(&self) -> bool {
        matches!(
            self.kind,
            MoveKind::Capture | MoveKind::EnPassantCapture | MoveKind::PromotionCapture(_)
        )
    }

    // Se usará en la Fase de búsqueda para ordenamiento de movimientos
    // (las jugadas de enroque suelen priorizarse). Probado desde ya.
    #[allow(dead_code)]
    #[inline]
    pub fn is_castle(&self) -> bool {
        matches!(
            self.kind,
            MoveKind::CastleKingside | MoveKind::CastleQueenside
        )
    }

    #[inline]
    pub fn promotion(&self) -> Option<PieceType> {
        match self.kind {
            MoveKind::Promotion(p) | MoveKind::PromotionCapture(p) => Some(p),
            _ => None,
        }
    }
}

/// Notación algebraica larga usada por UCI: "e2e4", "e7e8q".
impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}{}", square_to_str(self.from), square_to_str(self.to))?;
        if let Some(p) = self.promotion() {
            write!(f, "{}", p.to_char())?;
        }
        Ok(())
    }
}
