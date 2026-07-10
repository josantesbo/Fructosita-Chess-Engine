//! Representación del estado del tablero y aplicación de movimientos.
//!
//! Se usa el enfoque "copy-make": `make_move` no modifica el tablero actual,
//! sino que devuelve una copia nueva con el movimiento aplicado. Esto es más
//! simple y menos propenso a errores que make/unmake con deshacer manual; el
//! costo en rendimiento es aceptable para esta fase (optimizable después).
//!
//! El tablero mantiene un hash Zobrist (`hash`) actualizado de forma
//! incremental en cada `make_move`: en vez de recalcularlo desde cero, se
//! hace XOR únicamente de lo que cambió (pieza que se mueve, captura,
//! derechos de enroque que se pierden, columna de captura al paso). Este
//! hash es la base de la tabla de transposición y de la detección de
//! repeticiones en la búsqueda.

use crate::bitboard::{clear_bit, set_bit, tables, Bitboard, EMPTY};
use crate::moves::{Move, MoveKind};
use crate::types::*;
use crate::zobrist;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Board {
    /// pieces[color][piece_type] -> bitboard de esa pieza/color.
    pub pieces: [[Bitboard; 6]; 2],
    /// Búsqueda O(1) de qué pieza ocupa cada casilla (se mantiene sincronizada con `pieces`).
    pub mailbox: [Option<Piece>; 64],
    pub side_to_move: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
    /// Hash Zobrist de la posición actual, mantenido incrementalmente.
    pub hash: u64,
}

impl Board {
    pub fn start_pos() -> Board {
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("FEN de posición inicial debe ser válido")
    }

    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(format!(
                "FEN incompleto (se esperaban al menos 4 campos): {fen}"
            ));
        }

        let mut pieces = [[EMPTY; 6]; 2];
        let mut mailbox: [Option<Piece>; 64] = [None; 64];

        let ranks: Vec<&str> = parts[0].split('/').collect();
        if ranks.len() != 8 {
            return Err(format!(
                "La posición FEN debe tener 8 filas, tiene {}",
                ranks.len()
            ));
        }
        for (i, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file: u8 = 0;
            for c in rank_str.chars() {
                if let Some(d) = c.to_digit(10) {
                    file += d as u8;
                } else {
                    if file >= 8 {
                        return Err(format!("Fila FEN excede 8 columnas: {rank_str}"));
                    }
                    let color = if c.is_uppercase() {
                        Color::White
                    } else {
                        Color::Black
                    };
                    let kind = PieceType::from_char(c)
                        .ok_or_else(|| format!("Carácter de pieza FEN inválido: {c}"))?;
                    let sq = make_square(file, rank);
                    pieces[color.index()][kind.index()] =
                        set_bit(pieces[color.index()][kind.index()], sq);
                    mailbox[sq as usize] = Some(Piece::new(color, kind));
                    file += 1;
                }
            }
        }

        let side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(format!("Color de turno FEN inválido: {other}")),
        };

        let mut castling = CastlingRights::default();
        if parts[2] != "-" {
            for c in parts[2].chars() {
                match c {
                    'K' => castling.white_kingside = true,
                    'Q' => castling.white_queenside = true,
                    'k' => castling.black_kingside = true,
                    'q' => castling.black_queenside = true,
                    _ => return Err(format!("Derecho de enroque FEN inválido: {c}")),
                }
            }
        }

        let en_passant = if parts[3] == "-" {
            None
        } else {
            str_to_square(parts[3])
        };
        let halfmove_clock = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
        let fullmove_number = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(1);

        // Hash Zobrist calculado desde cero (solo ocurre aquí y en tests; en
        // juego normal, `make_move` lo actualiza incrementalmente).
        let keys = zobrist::keys();
        let mut hash = 0u64;
        for sq in 0u8..64 {
            if let Some(p) = mailbox[sq as usize] {
                hash ^= keys.piece(p.color, p.kind, sq);
            }
        }
        if side_to_move == Color::Black {
            hash ^= keys.side_to_move;
        }
        if castling.white_kingside {
            hash ^= keys.castling[0];
        }
        if castling.white_queenside {
            hash ^= keys.castling[1];
        }
        if castling.black_kingside {
            hash ^= keys.castling[2];
        }
        if castling.black_queenside {
            hash ^= keys.castling[3];
        }
        if let Some(ep) = en_passant {
            hash ^= keys.en_passant_file[file_of(ep) as usize];
        }

        Ok(Board {
            pieces,
            mailbox,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
            hash,
        })
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_fen(&self) -> String {
        let mut s = String::new();
        for rank in (0..8).rev() {
            let mut empty_run = 0;
            for file in 0..8 {
                let sq = make_square(file, rank);
                match self.mailbox[sq as usize] {
                    None => empty_run += 1,
                    Some(p) => {
                        if empty_run > 0 {
                            s.push_str(&empty_run.to_string());
                            empty_run = 0;
                        }
                        s.push(p.to_fen_char());
                    }
                }
            }
            if empty_run > 0 {
                s.push_str(&empty_run.to_string());
            }
            if rank > 0 {
                s.push('/');
            }
        }
        s.push(' ');
        s.push(if self.side_to_move == Color::White {
            'w'
        } else {
            'b'
        });
        s.push(' ');
        let c = &self.castling;
        if !(c.white_kingside || c.white_queenside || c.black_kingside || c.black_queenside) {
            s.push('-');
        } else {
            if c.white_kingside {
                s.push('K');
            }
            if c.white_queenside {
                s.push('Q');
            }
            if c.black_kingside {
                s.push('k');
            }
            if c.black_queenside {
                s.push('q');
            }
        }
        s.push(' ');
        s.push_str(
            &self
                .en_passant
                .map(square_to_str)
                .unwrap_or_else(|| "-".to_string()),
        );
        s.push(' ');
        s.push_str(&self.halfmove_clock.to_string());
        s.push(' ');
        s.push_str(&self.fullmove_number.to_string());
        s
    }

    #[inline]
    pub fn color_occupancy(&self, color: Color) -> Bitboard {
        self.pieces[color.index()]
            .iter()
            .fold(EMPTY, |acc, &bb| acc | bb)
    }

    #[inline]
    pub fn occupancy(&self) -> Bitboard {
        self.color_occupancy(Color::White) | self.color_occupancy(Color::Black)
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Square {
        let bb = self.pieces[color.index()][PieceType::King.index()];
        debug_assert!(bb != EMPTY, "no hay rey {:?} en el tablero", color);
        bb.trailing_zeros() as Square
    }

    /// ¿Está `sq` atacada por alguna pieza de `by_color`?
    pub fn is_square_attacked(&self, sq: Square, by_color: Color) -> bool {
        let t = tables();
        let occ = self.occupancy();
        let idx = by_color.index();

        if t.pawn_attacks(by_color.opposite(), sq) & self.pieces[idx][PieceType::Pawn.index()]
            != EMPTY
        {
            return true;
        }
        if t.knight_attacks(sq) & self.pieces[idx][PieceType::Knight.index()] != EMPTY {
            return true;
        }
        if t.king_attacks(sq) & self.pieces[idx][PieceType::King.index()] != EMPTY {
            return true;
        }
        let bishops_queens = self.pieces[idx][PieceType::Bishop.index()]
            | self.pieces[idx][PieceType::Queen.index()];
        if t.bishop_attacks(sq, occ) & bishops_queens != EMPTY {
            return true;
        }
        let rooks_queens =
            self.pieces[idx][PieceType::Rook.index()] | self.pieces[idx][PieceType::Queen.index()];
        if t.rook_attacks(sq, occ) & rooks_queens != EMPTY {
            return true;
        }
        false
    }

    pub fn in_check(&self, color: Color) -> bool {
        self.is_square_attacked(self.king_square(color), color.opposite())
    }

    /// Aplica `mv` y devuelve el tablero resultante. Asume que `mv` es al
    /// menos pseudo-legal en `self` (no valida legalidad por jaque; eso lo
    /// hace el generador de movimientos llamando a `in_check` después).
    pub fn make_move(&self, mv: Move) -> Board {
        let mut b = *self;
        let us = self.side_to_move;
        let them = us.opposite();
        let keys = zobrist::keys();
        let moving_piece = self.mailbox[mv.from as usize]
            .unwrap_or_else(|| panic!("make_move: no hay pieza en {}", square_to_str(mv.from)));

        // Quitar del hash la columna de captura al paso anterior (si había).
        if let Some(old_ep) = self.en_passant {
            b.hash ^= keys.en_passant_file[file_of(old_ep) as usize];
        }
        b.en_passant = None;

        if moving_piece.kind == PieceType::Pawn || mv.is_capture() {
            b.halfmove_clock = 0;
        } else {
            b.halfmove_clock += 1;
        }

        // Quitar la pieza que se mueve de su casilla de origen.
        b.pieces[us.index()][moving_piece.kind.index()] =
            clear_bit(b.pieces[us.index()][moving_piece.kind.index()], mv.from);
        b.mailbox[mv.from as usize] = None;
        b.hash ^= keys.piece(us, moving_piece.kind, mv.from);

        match mv.kind {
            MoveKind::Capture | MoveKind::PromotionCapture(_) => {
                let captured = self.mailbox[mv.to as usize].unwrap_or_else(|| {
                    panic!("captura sin pieza objetivo en {}", square_to_str(mv.to))
                });
                b.pieces[them.index()][captured.kind.index()] =
                    clear_bit(b.pieces[them.index()][captured.kind.index()], mv.to);
                b.hash ^= keys.piece(them, captured.kind, mv.to);
            }
            MoveKind::EnPassantCapture => {
                let captured_sq = if us == Color::White {
                    mv.to - 8
                } else {
                    mv.to + 8
                };
                b.pieces[them.index()][PieceType::Pawn.index()] =
                    clear_bit(b.pieces[them.index()][PieceType::Pawn.index()], captured_sq);
                b.mailbox[captured_sq as usize] = None;
                b.hash ^= keys.piece(them, PieceType::Pawn, captured_sq);
            }
            _ => {}
        }

        let placed_kind = mv.promotion().unwrap_or(moving_piece.kind);
        b.pieces[us.index()][placed_kind.index()] =
            set_bit(b.pieces[us.index()][placed_kind.index()], mv.to);
        b.mailbox[mv.to as usize] = Some(Piece::new(us, placed_kind));
        b.hash ^= keys.piece(us, placed_kind, mv.to);

        match mv.kind {
            MoveKind::DoublePawnPush => {
                let ep_sq = if us == Color::White {
                    mv.from + 8
                } else {
                    mv.from - 8
                };
                b.en_passant = Some(ep_sq);
                b.hash ^= keys.en_passant_file[file_of(ep_sq) as usize];
            }
            MoveKind::CastleKingside => {
                let (rook_from, rook_to) = if us == Color::White {
                    (H1, F1)
                } else {
                    (H8, F8)
                };
                b.pieces[us.index()][PieceType::Rook.index()] =
                    clear_bit(b.pieces[us.index()][PieceType::Rook.index()], rook_from);
                b.pieces[us.index()][PieceType::Rook.index()] =
                    set_bit(b.pieces[us.index()][PieceType::Rook.index()], rook_to);
                b.mailbox[rook_from as usize] = None;
                b.mailbox[rook_to as usize] = Some(Piece::new(us, PieceType::Rook));
                b.hash ^= keys.piece(us, PieceType::Rook, rook_from);
                b.hash ^= keys.piece(us, PieceType::Rook, rook_to);
            }
            MoveKind::CastleQueenside => {
                let (rook_from, rook_to) = if us == Color::White {
                    (A1, D1)
                } else {
                    (A8, D8)
                };
                b.pieces[us.index()][PieceType::Rook.index()] =
                    clear_bit(b.pieces[us.index()][PieceType::Rook.index()], rook_from);
                b.pieces[us.index()][PieceType::Rook.index()] =
                    set_bit(b.pieces[us.index()][PieceType::Rook.index()], rook_to);
                b.mailbox[rook_from as usize] = None;
                b.mailbox[rook_to as usize] = Some(Piece::new(us, PieceType::Rook));
                b.hash ^= keys.piece(us, PieceType::Rook, rook_from);
                b.hash ^= keys.piece(us, PieceType::Rook, rook_to);
            }
            _ => {}
        }

        match (us, moving_piece.kind) {
            (Color::White, PieceType::King) => {
                b.castling.white_kingside = false;
                b.castling.white_queenside = false;
            }
            (Color::Black, PieceType::King) => {
                b.castling.black_kingside = false;
                b.castling.black_queenside = false;
            }
            _ => {}
        }
        // Cubre tanto "la torre se movió desde su casilla original" como
        // "capturaron una torre en su casilla original" (en cuyo caso
        // `mv.to` coincide, sin importar de quién sea el turno).
        if mv.from == A1 || mv.to == A1 {
            b.castling.white_queenside = false;
        }
        if mv.from == H1 || mv.to == H1 {
            b.castling.white_kingside = false;
        }
        if mv.from == A8 || mv.to == A8 {
            b.castling.black_queenside = false;
        }
        if mv.from == H8 || mv.to == H8 {
            b.castling.black_kingside = false;
        }

        // Hash: aplicar el cambio en derechos de enroque (comparando antes/después).
        if self.castling.white_kingside != b.castling.white_kingside {
            b.hash ^= keys.castling[0];
        }
        if self.castling.white_queenside != b.castling.white_queenside {
            b.hash ^= keys.castling[1];
        }
        if self.castling.black_kingside != b.castling.black_kingside {
            b.hash ^= keys.castling[2];
        }
        if self.castling.black_queenside != b.castling.black_queenside {
            b.hash ^= keys.castling[3];
        }

        if us == Color::Black {
            b.fullmove_number += 1;
        }
        b.side_to_move = them;
        b.hash ^= keys.side_to_move;

        b
    }

    /// "Movimiento nulo": pasa el turno sin mover ninguna pieza. Se usa
    /// exclusivamente como técnica de poda en la búsqueda (null-move
    /// pruning), nunca representa un movimiento legal real.
    pub fn make_null_move(&self) -> Board {
        let mut b = *self;
        let keys = zobrist::keys();
        if let Some(ep) = self.en_passant {
            b.hash ^= keys.en_passant_file[file_of(ep) as usize];
        }
        b.en_passant = None;
        b.side_to_move = self.side_to_move.opposite();
        b.hash ^= keys.side_to_move;
        b.halfmove_clock += 1;
        b
    }
}

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f)?;
        for rank in (0..8).rev() {
            write!(f, "  {} ", rank + 1)?;
            for file in 0..8 {
                let sq = make_square(file, rank);
                let c = match self.mailbox[sq as usize] {
                    Some(p) => p.to_fen_char(),
                    None => '.',
                };
                write!(f, "{c} ")?;
            }
            writeln!(f)?;
        }
        writeln!(f, "    a b c d e f g h")?;
        write!(f, "  FEN: {}", self.to_fen())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fen_roundtrip_startpos() {
        let b = Board::start_pos();
        assert_eq!(
            b.to_fen(),
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    #[test]
    fn fen_roundtrip_kiwipete() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let b = Board::from_fen(fen).unwrap();
        assert_eq!(b.to_fen(), fen);
    }

    #[test]
    fn start_pos_not_in_check() {
        let b = Board::start_pos();
        assert!(!b.in_check(Color::White));
        assert!(!b.in_check(Color::Black));
    }

    #[test]
    fn incremental_hash_matches_from_scratch() {
        // El hash actualizado incrementalmente en make_move debe coincidir
        // siempre con el hash recalculado desde cero (from_fen) de la misma
        // posición resultante. Probamos con una secuencia que toca todos los
        // casos especiales: enroque, captura al paso, promoción, captura.
        let b0 = Board::start_pos();
        let moves = ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6", "e1g1"];
        let mut b = b0;
        for mv_str in moves {
            let mv = crate::movegen::find_move(&b, mv_str)
                .unwrap_or_else(|| panic!("movimiento inesperadamente ilegal: {mv_str}"));
            b = b.make_move(mv);
            let recomputed = Board::from_fen(&b.to_fen()).unwrap();
            assert_eq!(
                b.hash, recomputed.hash,
                "hash incremental desincronizado tras {mv_str}"
            );
        }

        // Captura al paso.
        let ep_setup = Board::from_fen("4k3/8/8/8/pP6/8/8/4K3 b - b3 0 1").unwrap();
        let mv = crate::movegen::find_move(&ep_setup, "a4b3").unwrap();
        let after = ep_setup.make_move(mv);
        let recomputed = Board::from_fen(&after.to_fen()).unwrap();
        assert_eq!(
            after.hash, recomputed.hash,
            "hash incremental desincronizado tras captura al paso"
        );

        // Promoción con captura.
        let promo_setup = Board::from_fen("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let mv = crate::movegen::find_move(&promo_setup, "a7b8q").unwrap();
        let after = promo_setup.make_move(mv);
        let recomputed = Board::from_fen(&after.to_fen()).unwrap();
        assert_eq!(
            after.hash, recomputed.hash,
            "hash incremental desincronizado tras promoción con captura"
        );
    }
}
