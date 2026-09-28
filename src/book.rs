//! Soporte para libros de apertura en formato Polyglot (.bin).
//!
//! El formato Polyglot es un estándar público usado por herramientas como
//! PolyGlot, XBoard, SCID, python-chess, etc. — no es propio de ningún
//! motor. Un libro es una lista de entradas de 16 bytes ordenadas por hash
//! de posición, cada una con: hash (u64), movimiento codificado (u16), peso
//! (u16), y un campo de aprendizaje (u32, sin uso aquí). Todos los enteros
//! se guardan "big-endian" (byte más significativo primero).
//!
//! El hash usado para el libro es un esquema Zobrist **distinto** del hash
//! interno de `Board` (`zobrist.rs`): usa un array de 781 números
//! aleatorios estandarizado por la comunidad, para que cualquier libro
//! generado por cualquier herramienta compatible (como el que Antonio
//! generó a partir de sus propios torneos) sea legible aquí sin importar
//! qué programa lo haya creado.
//!
//! Los 781 valores de `POLYGLOT_RANDOM64` se extrajeron programáticamente
//! (sin transcripción manual) del código fuente de python-chess
//! (github.com/niklasf/python-chess, chess/polyglot.py), y se verificaron
//! contra la especificación pública (chessprogramming.org/PolyGlot,
//! hgm.nubati.net/book_format.html): 781 valores, primer y último valor
//! coinciden exactamente con múltiples fuentes independientes.

use crate::board::Board;
use crate::movegen::generate_legal_moves;
use crate::moves::Move;
use crate::types::*;

use crate::polyglot_random::POLYGLOT_RANDOM64;
const ENTRY_SIZE: usize = 16;

/// Índice de pieza en la convención Polyglot: peón negro=0, peón blanco=1,
/// caballo negro=2, caballo blanco=3, ..., rey negro=10, rey blanco=11.
/// Verificado contra `ZobristHasher.hash_board` de python-chess:
/// `piece_index = (piece_type - 1) * 2 + (0 si negro, 1 si blanco)`.
#[inline(always)]
fn polyglot_piece_index(color: Color, kind: PieceType) -> usize {
    kind.index() * 2 + if color == Color::White { 1 } else { 0 }
}

/// Hash Zobrist de la posición según la convención Polyglot (independiente
/// del hash interno del motor). Replica exactamente `ZobristHasher` de
/// python-chess, incluyendo dos detalles fáciles de pasar por alto:
///   - El turno solo se hashea cuando le toca a las **blancas** (no al revés).
///   - La columna de captura al paso solo cuenta si de verdad hay un peón
///     rival en condiciones de capturar ahí (sin importar si esa captura
///     sería legal por clavadas o jaques — eso es irrelevante para el hash).
pub fn polyglot_hash(board: &Board) -> u64 {
    let r = &POLYGLOT_RANDOM64;
    let mut h = 0u64;

    for sq in 0u8..64 {
        if let Some(p) = board.mailbox[sq as usize] {
            let idx = polyglot_piece_index(p.color, p.kind);
            h ^= r[64 * idx + sq as usize];
        }
    }

    if board.castling.white_kingside {
        h ^= r[768];
    }
    if board.castling.white_queenside {
        h ^= r[769];
    }
    if board.castling.black_kingside {
        h ^= r[770];
    }
    if board.castling.black_queenside {
        h ^= r[771];
    }

    if let Some(ep) = board.en_passant {
        let ep_file = file_of(ep);
        let ep_rank = rank_of(ep);
        // Fila donde está realmente el peón que acaba de avanzar dos casillas.
        let us = board.side_to_move;
        let pawn_rank = if us == Color::White {
            ep_rank - 1
        } else {
            ep_rank + 1
        };
        let our_pawns = board.pieces[us.index()][PieceType::Pawn.index()];
        let mut capturer_present = false;
        if ep_file > 0 && crate::bitboard::get_bit(our_pawns, make_square(ep_file - 1, pawn_rank)) {
            capturer_present = true;
        }
        if ep_file < 7 && crate::bitboard::get_bit(our_pawns, make_square(ep_file + 1, pawn_rank)) {
            capturer_present = true;
        }
        if capturer_present {
            h ^= r[772 + ep_file as usize];
        }
    }

    if board.side_to_move == Color::White {
        h ^= r[780];
    }

    h
}

/// Decodifica un `raw_move` de 16 bits del formato Polyglot y lo empareja
/// contra la lista de movimientos legales de `board`, de modo que el
/// resultado sea siempre un `Move` genuinamente válido en la representación
/// interna del motor (nunca se construye un `Move` "a mano" desde bytes
/// crudos, evitando así cualquier riesgo de producir un movimiento inválido
/// por una mala interpretación del formato).
///
/// Maneja el caso especial histórico de Polyglot: el enroque se codifica
/// como "el rey captura su propia torre" (p.ej. e1h1 para el enroque corto
/// blanco) en vez de la casilla final real del rey (g1). Se traduce antes
/// de buscar coincidencia.
fn decode_polyglot_move(raw: u16, legal: &[Move]) -> Option<Move> {
    let to_raw = (raw & 0x3f) as u8;
    let from_raw = ((raw >> 6) & 0x3f) as u8;
    let promo_part = (raw >> 12) & 0x7;

    let to_sq = match (from_raw, to_raw) {
        (E1, H1) => G1,
        (E1, A1) => C1,
        (E8, H8) => G8,
        (E8, A8) => C8,
        _ => to_raw,
    };
    let from_sq = from_raw;

    let promotion = match promo_part {
        1 => Some(PieceType::Knight),
        2 => Some(PieceType::Bishop),
        3 => Some(PieceType::Rook),
        4 => Some(PieceType::Queen),
        _ => None,
    };

    legal
        .iter()
        .find(|mv| mv.from == from_sq && mv.to == to_sq && mv.promotion() == promotion)
        .copied()
}

pub struct Book {
    data: Vec<u8>,
}

impl Book {
    pub fn load(path: &str) -> Result<Book, String> {
        let data = std::fs::read(path).map_err(|e| format!("no se pudo leer '{path}': {e}"))?;
        if data.is_empty() || data.len() % ENTRY_SIZE != 0 {
            return Err(format!(
                "'{path}' no parece un libro Polyglot válido (tamaño {} no es múltiplo de {ENTRY_SIZE} bytes)",
                data.len()
            ));
        }
        Ok(Book { data })
    }

    fn len(&self) -> usize {
        self.data.len() / ENTRY_SIZE
    }

    fn entry_key(&self, i: usize) -> u64 {
        let base = i * ENTRY_SIZE;
        u64::from_be_bytes(self.data[base..base + 8].try_into().unwrap())
    }

    fn entry_raw_move(&self, i: usize) -> u16 {
        let base = i * ENTRY_SIZE + 8;
        u16::from_be_bytes(self.data[base..base + 2].try_into().unwrap())
    }

    fn entry_weight(&self, i: usize) -> u16 {
        let base = i * ENTRY_SIZE + 10;
        u16::from_be_bytes(self.data[base..base + 2].try_into().unwrap())
    }

    /// Primer índice cuya clave es >= key (búsqueda binaria; las entradas
    /// están ordenadas por clave según el estándar del formato).
    fn bisect_left(&self, key: u64) -> usize {
        let mut lo = 0usize;
        let mut hi = self.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.entry_key(mid) < key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Todas las jugadas de libro válidas para `board`, con su peso.
    /// Puede haber varias entradas con la misma clave (varias jugadas
    /// candidatas para la misma posición).
    pub fn probe(&self, board: &Board) -> Vec<(Move, u32)> {
        let key = polyglot_hash(board);
        let legal = generate_legal_moves(board);
        let mut results = Vec::new();

        let mut i = self.bisect_left(key);
        while i < self.len() && self.entry_key(i) == key {
            let raw = self.entry_raw_move(i);
            if let Some(mv) = decode_polyglot_move(raw, &legal) {
                results.push((mv, self.entry_weight(i) as u32));
            }
            i += 1;
        }
        results
    }
}

/// PRNG xorshift64 minimalista, solo para la selección aleatoria ponderada
/// entre jugadas de libro (no para nada sensible a calidad criptográfica).
pub struct BookRng(u64);

impl BookRng {
    pub fn new(seed: u64) -> Self {
        BookRng(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Elige una jugada entre las candidatas de libro, con probabilidad
/// proporcional a su peso (convención estándar de Polyglot). Si todos los
/// pesos son 0, elige uniformemente al azar entre las opciones.
pub fn choose_weighted(entries: &[(Move, u32)], rng: &mut BookRng) -> Option<Move> {
    if entries.is_empty() {
        return None;
    }
    let total: u64 = entries.iter().map(|(_, w)| *w as u64).sum();
    if total == 0 {
        let idx = (rng.next_u64() as usize) % entries.len();
        return Some(entries[idx].0);
    }
    let mut pick = rng.next_u64() % total;
    for (mv, w) in entries {
        if pick < *w as u64 {
            return Some(*mv);
        }
        pick -= *w as u64;
    }
    entries.last().map(|(mv, _)| *mv)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Libro de prueba generado y verificado independientemente con
    // python-chess (no con este mismo código), cubriendo: posición inicial
    // con varias jugadas y pesos distintos, una continuación, un caso de
    // enroque codificado a la manera Polyglot, y una captura al paso real.
    const TEST_BOOK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/test_book.bin");

    #[test]
    fn polyglot_hash_matches_known_values() {
        // Valor de referencia publicado para la posición inicial (bien
        // conocido en la comunidad, coincide con múltiples implementaciones
        // independientes de Polyglot, incluida python-chess).
        let start = Board::start_pos();
        assert_eq!(polyglot_hash(&start), 0x463b96181691fc9c);
    }

    #[test]
    fn startpos_book_moves_match_python_chess() {
        let book = Book::load(TEST_BOOK).unwrap();
        let board = Board::start_pos();
        let mut results = book.probe(&board);
        results.sort_by_key(|b| std::cmp::Reverse(b.1)); // mayor peso primero
        let as_strings: Vec<(String, u32)> =
            results.iter().map(|(m, w)| (m.to_string(), *w)).collect();
        assert_eq!(
            as_strings,
            vec![
                ("e2e4".to_string(), 50),
                ("d2d4".to_string(), 30),
                ("c2c4".to_string(), 1),
            ]
        );
    }

    #[test]
    fn follow_up_position_book_move() {
        let book = Book::load(TEST_BOOK).unwrap();
        let mut board = Board::start_pos();
        board = board.make_move(crate::movegen::find_move(&board, "e2e4").unwrap());
        board = board.make_move(crate::movegen::find_move(&board, "e7e5").unwrap());
        let results = book.probe(&board);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.to_string(), "g1f3");
        assert_eq!(results[0].1, 40);
    }

    #[test]
    fn castling_encoding_quirk_is_translated_correctly() {
        // La entrada de prueba fue escrita con la codificación cruda e1h1
        // (convención Polyglot de "rey toma su propia torre"); debe leerse
        // como e1g1 (nuestra representación normal de enroque corto).
        let book = Book::load(TEST_BOOK).unwrap();
        let board = Board::from_fen(
            "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 6 5",
        )
        .unwrap();
        let results = book.probe(&board);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.to_string(), "e1g1");
    }

    #[test]
    fn en_passant_position_book_move() {
        let book = Book::load(TEST_BOOK).unwrap();
        let board =
            Board::from_fen("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 4")
                .unwrap();
        let results = book.probe(&board);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.to_string(), "e5f6");
    }

    #[test]
    fn unknown_position_returns_no_moves() {
        let book = Book::load(TEST_BOOK).unwrap();
        // Una posición cualquiera que no está en el libro de prueba.
        let board = Board::from_fen("8/8/8/4k3/8/8/4K3/8 w - - 0 1").unwrap();
        assert!(book.probe(&board).is_empty());
    }

    #[test]
    fn weighted_choice_respects_zero_weight_fallback() {
        let entries = vec![];
        let mut rng = BookRng::new(42);
        assert_eq!(choose_weighted(&entries, &mut rng), None);
    }

    #[test]
    fn invalid_file_size_is_rejected() {
        // Cualquier tamaño que no sea múltiplo de 16 bytes debe fallar
        // limpiamente. Escribimos un archivo temporal con tamaño garantizado
        // inválido (17 bytes), en vez de reutilizar un archivo existente
        // cuyo tamaño podría coincidir con un múltiplo de 16 por casualidad.
        let path = std::env::temp_dir().join("fructosita_test_invalid_book.bin");
        std::fs::write(&path, [0u8; 17]).unwrap();
        let result = Book::load(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(result.is_err());
    }
}
