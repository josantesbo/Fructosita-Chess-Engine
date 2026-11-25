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
    /// S4-001: casillas ocupadas por cada bando (la unión de sus seis
    /// bitboards), mantenida en `make_move`.
    pub occ: [Bitboard; 2],
    /// S4-001: material + PST con los pesos por defecto, blancas − negras
    /// (medio juego, final), mantenido en `make_move` (ver `eval::psq_table`).
    pub psq: (i32, i32),
    /// Chess960: casilla de la torre de cada derecho de enroque, en el orden
    /// de las claves Zobrist (blancas corto, blancas largo, negras corto,
    /// negras largo). Solo tiene significado mientras el derecho existe; en
    /// ajedrez estándar vale siempre h1, a1, h8, a8.
    pub castle_rooks: [Square; 4],
}

/// S3-002: ver `Board::check_info`.
pub struct CheckInfo {
    /// Por tipo de pieza (peón, caballo, alfil, torre, dama, rey).
    squares: [Bitboard; 6],
    blockers: Bitboard,
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

        // Derechos de enroque. Se aceptan las dos escrituras en uso: "KQkq"
        // (FEN clásico y X-FEN: la torre más exterior de ese lado del rey) y
        // la columna de la torre, "HAha" (Shredder-FEN). Un derecho sin rey en
        // su primera fila o sin torre propia en esa casilla no puede ejercerse
        // nunca y se descarta.
        let mut castling = CastlingRights::default();
        let mut castle_rooks = [H1, A1, H8, A8];
        if parts[2] != "-" {
            for c in parts[2].chars() {
                let lower = c.to_ascii_lowercase();
                if !matches!(lower, 'k' | 'q' | 'a'..='h') {
                    return Err(format!("Derecho de enroque FEN inválido: {c}"));
                }
                let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                let ci = color.index();
                let base: Square = if color == Color::White { 0 } else { 56 };
                let king_bb = pieces[ci][PieceType::King.index()];
                if king_bb == EMPTY {
                    continue;
                }
                let ksq = king_bb.trailing_zeros() as Square;
                if ksq < base || ksq > base + 7 {
                    continue;
                }
                let kf = file_of(ksq);
                let rook_files = ((pieces[ci][PieceType::Rook.index()] >> base) & 0xFF) as u8;
                let rook_file = match lower {
                    'k' => (kf + 1..8).rev().find(|&f| rook_files >> f & 1 == 1),
                    'q' => (0..kf).find(|&f| rook_files >> f & 1 == 1),
                    letter => Some(letter as u8 - b'a').filter(|&f| f != kf && rook_files >> f & 1 == 1),
                };
                let Some(rf) = rook_file else {
                    continue;
                };
                let idx = ci * 2 + usize::from(rf < kf);
                castle_rooks[idx] = base + rf;
                match idx {
                    0 => castling.white_kingside = true,
                    1 => castling.white_queenside = true,
                    2 => castling.black_kingside = true,
                    _ => castling.black_queenside = true,
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

        let occ = [
            pieces[0].iter().fold(EMPTY, |acc, &bb| acc | bb),
            pieces[1].iter().fold(EMPTY, |acc, &bb| acc | bb),
        ];
        let psq_table = crate::eval::psq_table();
        let mut psq = (0, 0);
        for sq in 0..64usize {
            if let Some(p) = mailbox[sq] {
                let v = psq_table[p.color.index()][p.kind.index()][sq];
                psq.0 += v.0;
                psq.1 += v.1;
            }
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
            occ,
            psq,
            castle_rooks,
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
            // Con notación Chess960 se escribe la columna de cada torre
            // (Shredder-FEN); si no, el "KQkq" de siempre.
            let frc = self.frc_notation();
            let rights = [c.white_kingside, c.white_queenside, c.black_kingside, c.black_queenside];
            for (i, classic) in ['K', 'Q', 'k', 'q'].into_iter().enumerate() {
                if !rights[i] {
                    continue;
                }
                if frc {
                    let letter = (b'a' + file_of(self.castle_rooks[i])) as char;
                    s.push(if i < 2 { letter.to_ascii_uppercase() } else { letter });
                } else {
                    s.push(classic);
                }
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

    /// ¿Hay algún derecho de enroque con el rey fuera de la columna e o la
    /// torre fuera de las columnas a/h? Entonces la notación clásica del
    /// enroque (e1g1) no sirve: puede coincidir con un paso normal del rey.
    pub fn frc_geometry(&self) -> bool {
        let c = &self.castling;
        let r = &self.castle_rooks;
        let white = c.white_kingside || c.white_queenside;
        let black = c.black_kingside || c.black_queenside;
        (white && self.king_square(Color::White) != E1)
            || (black && self.king_square(Color::Black) != E8)
            || (c.white_kingside && r[0] != H1)
            || (c.white_queenside && r[1] != A1)
            || (c.black_kingside && r[2] != H8)
            || (c.black_queenside && r[3] != A8)
    }

    /// ¿Se escriben los enroques y los FEN con la convención Chess960? Sí si
    /// la interfaz lo pidió (`UCI_Chess960`) o si la posición lo exige.
    pub fn frc_notation(&self) -> bool {
        crate::chess960::enabled() || self.frc_geometry()
    }

    /// Jugada en notación UCI. El enroque en Chess960 se escribe "el rey
    /// captura su torre" (convención de `UCI_Chess960`): e1h1, b1a1…; en
    /// ajedrez estándar, con la casilla de llegada del rey: e1g1.
    pub fn move_uci(&self, mv: Move) -> String {
        if mv.is_castle() && self.frc_notation() {
            let idx = self.side_to_move.index() * 2 + usize::from(mv.kind == MoveKind::CastleQueenside);
            format!("{}{}", square_to_str(mv.from), square_to_str(self.castle_rooks[idx]))
        } else {
            mv.to_string()
        }
    }

    #[inline]
    pub fn color_occupancy(&self, color: Color) -> Bitboard {
        self.occ[color.index()]
    }

    #[inline]
    pub fn occupancy(&self) -> Bitboard {
        self.occ[0] | self.occ[1]
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

    /// S2-009: ¿la jugada (legal) deja al rival en jaque? Equivale exactamente a
    /// `self.make_move(mv).in_check(rival)` sin copiar el tablero: jaque directo
    /// de la pieza que llega (o de la pieza promovida) y jaque descubierto de un
    /// deslizante propio con la ocupación actualizada. Enroque y captura al paso
    /// (dos casillas se vacían o se mueve una torre) usan el camino exacto.
    pub fn gives_check(&self, mv: Move) -> bool {
        let us = self.side_to_move;
        let them = us.opposite();
        if matches!(mv.kind, MoveKind::CastleKingside | MoveKind::CastleQueenside | MoveKind::EnPassantCapture) {
            return self.make_move(mv).in_check(them);
        }
        let t = tables();
        let ksq = self.king_square(them);
        let kbb = 1u64 << ksq;
        let from_bb = 1u64 << mv.from;
        let occ = (self.occupancy() & !from_bb) | (1u64 << mv.to);
        let piece = match mv.promotion() {
            Some(p) => p,
            None => match self.mailbox[mv.from as usize] {
                Some(p) => p.kind,
                None => return self.make_move(mv).in_check(them),
            },
        };
        let direct = match piece {
            PieceType::Pawn => t.pawn_attacks(us, mv.to),
            PieceType::Knight => t.knight_attacks(mv.to),
            PieceType::Bishop => t.bishop_attacks(mv.to, occ),
            PieceType::Rook => t.rook_attacks(mv.to, occ),
            PieceType::Queen => t.queen_attacks(mv.to, occ),
            PieceType::King => EMPTY,
        };
        if direct & kbb != EMPTY {
            return true;
        }
        let own = &self.pieces[us.index()];
        let queens = own[PieceType::Queen.index()];
        let bq = (own[PieceType::Bishop.index()] | queens) & !from_bb;
        let rq = (own[PieceType::Rook.index()] | queens) & !from_bb;
        (t.bishop_attacks(ksq, occ) & bq) | (t.rook_attacks(ksq, occ) & rq) != EMPTY
    }

    /// S3-002: datos de jaque del nodo para el bando al turno, calculados una vez:
    /// casillas desde las que cada tipo de pieza daría jaque directo al rey rival
    /// (con la ocupación actual) y piezas que son el único obstáculo entre un
    /// deslizante propio y ese rey (candidatas a jaque descubierto).
    pub fn check_info(&self) -> CheckInfo {
        let t = tables();
        let us = self.side_to_move;
        let them = us.opposite();
        let ksq = self.king_square(them);
        let occ = self.occupancy();
        let b = t.bishop_attacks(ksq, occ);
        let r = t.rook_attacks(ksq, occ);
        let own = &self.pieces[us.index()];
        let queens = own[PieceType::Queen.index()];
        let mut snipers = (t.bishop_attacks(ksq, EMPTY) & (own[PieceType::Bishop.index()] | queens))
            | (t.rook_attacks(ksq, EMPTY) & (own[PieceType::Rook.index()] | queens));
        let mut blockers = EMPTY;
        if snipers != EMPTY {
            let between = &crate::movegen::between_table()[ksq as usize];
            while snipers != EMPTY {
                let s = snipers.trailing_zeros() as usize;
                snipers &= snipers - 1;
                let btw = between[s] & occ;
                if btw != EMPTY && btw & (btw - 1) == EMPTY {
                    blockers |= btw;
                }
            }
        }
        CheckInfo {
            squares: [t.pawn_attacks(them, ksq), t.knight_attacks(ksq), b, r, b | r, EMPTY],
            blockers,
        }
    }

    /// S3-002: igual que `gives_check` (mismo resultado para toda jugada legal),
    /// usando los datos del nodo. Jaque directo: la casilla de destino está entre
    /// las de jaque de la pieza (para un deslizante la ocupación actual basta: si
    /// el origen fuera el único obstáculo entre destino y rey, la pieza ya estaría
    /// dando jaque, imposible con el turno propio). Jaque descubierto: solo si la
    /// pieza que sale es un obstáculo único; ese caso, las promociones, el enroque
    /// y la captura al paso se resuelven con el camino exacto.
    #[inline]
    pub fn gives_check_with(&self, mv: Move, ci: &CheckInfo) -> bool {
        if !matches!(mv.kind, MoveKind::Quiet | MoveKind::DoublePawnPush | MoveKind::Capture) {
            return self.gives_check(mv);
        }
        let piece = match self.mailbox[mv.from as usize] {
            Some(p) => p.kind,
            None => return self.gives_check(mv),
        };
        if ci.squares[piece.index()] & (1u64 << mv.to) != EMPTY {
            return true;
        }
        if ci.blockers & (1u64 << mv.from) != EMPTY {
            return self.gives_check(mv);
        }
        false
    }

    /// Aplica `mv` y devuelve el tablero resultante. Asume que `mv` es al
    /// menos pseudo-legal en `self` (no valida legalidad por jaque; eso lo
    /// hace el generador de movimientos llamando a `in_check` después).
    pub fn make_move(&self, mv: Move) -> Board {
        if mv.is_castle() {
            return self.make_castle(mv);
        }
        let mut b = *self;
        let us = self.side_to_move;
        let them = us.opposite();
        let keys = zobrist::keys();
        let psq = crate::eval::psq_table();
        let (ui, ti) = (us.index(), them.index());
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
        b.occ[ui] ^= 1u64 << mv.from;
        let v = psq[ui][moving_piece.kind.index()][mv.from as usize];
        b.psq.0 -= v.0;
        b.psq.1 -= v.1;

        match mv.kind {
            MoveKind::Capture | MoveKind::PromotionCapture(_) => {
                let captured = self.mailbox[mv.to as usize].unwrap_or_else(|| {
                    panic!("captura sin pieza objetivo en {}", square_to_str(mv.to))
                });
                b.pieces[them.index()][captured.kind.index()] =
                    clear_bit(b.pieces[them.index()][captured.kind.index()], mv.to);
                b.hash ^= keys.piece(them, captured.kind, mv.to);
                b.occ[ti] ^= 1u64 << mv.to;
                let v = psq[ti][captured.kind.index()][mv.to as usize];
                b.psq.0 -= v.0;
                b.psq.1 -= v.1;
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
                b.occ[ti] ^= 1u64 << captured_sq;
                let v = psq[ti][PieceType::Pawn.index()][captured_sq as usize];
                b.psq.0 -= v.0;
                b.psq.1 -= v.1;
            }
            _ => {}
        }

        let placed_kind = mv.promotion().unwrap_or(moving_piece.kind);
        b.pieces[us.index()][placed_kind.index()] =
            set_bit(b.pieces[us.index()][placed_kind.index()], mv.to);
        b.mailbox[mv.to as usize] = Some(Piece::new(us, placed_kind));
        b.hash ^= keys.piece(us, placed_kind, mv.to);
        b.occ[ui] |= 1u64 << mv.to;
        let v = psq[ui][placed_kind.index()][mv.to as usize];
        b.psq.0 += v.0;
        b.psq.1 += v.1;

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
        let [wk_rook, wq_rook, bk_rook, bq_rook] = self.castle_rooks;
        if mv.from == wq_rook || mv.to == wq_rook {
            b.castling.white_queenside = false;
        }
        if mv.from == wk_rook || mv.to == wk_rook {
            b.castling.white_kingside = false;
        }
        if mv.from == bq_rook || mv.to == bq_rook {
            b.castling.black_queenside = false;
        }
        if mv.from == bk_rook || mv.to == bk_rook {
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

    /// Enroque (estándar y Chess960). `mv.from` es la casilla del rey y
    /// `mv.to` su casilla de llegada (columna g o c); la torre sale de
    /// `castle_rooks` y llega a la columna f o d. En Chess960 cualquiera de
    /// las dos piezas puede no moverse, y la casilla de llegada de una puede
    /// ser la de salida de la otra: por eso primero se retiran las dos y
    /// después se colocan las dos.
    fn make_castle(&self, mv: Move) -> Board {
        let mut b = *self;
        let us = self.side_to_move;
        let ui = us.index();
        let keys = zobrist::keys();
        let psq = crate::eval::psq_table();
        let (king, rook) = (PieceType::King.index(), PieceType::Rook.index());
        let queenside = mv.kind == MoveKind::CastleQueenside;
        let base: Square = if us == Color::White { 0 } else { 56 };
        let (king_from, king_to) = (mv.from, mv.to);
        let rook_from = self.castle_rooks[ui * 2 + usize::from(queenside)];
        let rook_to = base + if queenside { 3 } else { 5 };

        if let Some(old_ep) = self.en_passant {
            b.hash ^= keys.en_passant_file[file_of(old_ep) as usize];
        }
        b.en_passant = None;
        b.halfmove_clock += 1;

        b.pieces[ui][king] = set_bit(clear_bit(b.pieces[ui][king], king_from), king_to);
        b.pieces[ui][rook] = set_bit(clear_bit(b.pieces[ui][rook], rook_from), rook_to);
        b.mailbox[king_from as usize] = None;
        b.mailbox[rook_from as usize] = None;
        b.mailbox[king_to as usize] = Some(Piece::new(us, PieceType::King));
        b.mailbox[rook_to as usize] = Some(Piece::new(us, PieceType::Rook));
        b.occ[ui] = (b.occ[ui] & !((1u64 << king_from) | (1u64 << rook_from)))
            | (1u64 << king_to)
            | (1u64 << rook_to);
        b.hash ^= keys.piece(us, PieceType::King, king_from)
            ^ keys.piece(us, PieceType::King, king_to)
            ^ keys.piece(us, PieceType::Rook, rook_from)
            ^ keys.piece(us, PieceType::Rook, rook_to);
        for (kind, from, to) in [(king, king_from, king_to), (rook, rook_from, rook_to)] {
            let (a, z) = (psq[ui][kind][from as usize], psq[ui][kind][to as usize]);
            b.psq.0 += z.0 - a.0;
            b.psq.1 += z.1 - a.1;
        }

        if us == Color::White {
            if self.castling.white_kingside {
                b.hash ^= keys.castling[0];
            }
            if self.castling.white_queenside {
                b.hash ^= keys.castling[1];
            }
            b.castling.white_kingside = false;
            b.castling.white_queenside = false;
        } else {
            if self.castling.black_kingside {
                b.hash ^= keys.castling[2];
            }
            if self.castling.black_queenside {
                b.hash ^= keys.castling[3];
            }
            b.castling.black_kingside = false;
            b.castling.black_queenside = false;
            b.fullmove_number += 1;
        }
        b.side_to_move = us.opposite();
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
mod gives_check_tests {
    use super::*;
    use crate::movegen::generate_legal_moves;

    fn walk(b: &Board, depth: u32, n: &mut u64) {
        let ci = b.check_info();
        for mv in generate_legal_moves(b) {
            let next = b.make_move(mv);
            assert_eq!(b.gives_check(mv), next.in_check(next.side_to_move), "{mv} en {}", b);
            assert_eq!(b.gives_check_with(mv, &ci), next.in_check(next.side_to_move), "rápido {mv} en {}", b);
            *n += 1;
            if depth > 1 {
                walk(&next, depth - 1, n);
            }
        }
    }

    #[test]
    fn gives_check_matches_make_move_everywhere() {
        // S2-009: equivalencia exacta sobre todas las jugadas legales de las
        // posiciones de perft (enroques, al paso, promociones, clavadas).
        let fens = [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 4),
            ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3),
            ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),
            ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 3),
            ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3),
        ];
        let mut n = 0;
        for (fen, d) in fens {
            walk(&Board::from_fen(fen).unwrap(), d, &mut n);
        }
        assert!(n > 100_000);
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
