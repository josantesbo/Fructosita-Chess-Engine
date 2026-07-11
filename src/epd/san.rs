//! Empareja un token en notación algebraica estándar (SAN, p. ej. "Nxe5",
//! "e8=Q", "O-O") con el `Move` legal correspondiente.
//!
//! Empareja, no genera: como ya tenemos la lista de movimientos legales de
//! la posición (`legal`), resolver la desambiguación ("¿cuál de los dos
//! caballos va a d7?") es mucho más simple que generar SAN completo con
//! desambiguación desde cero, que es lo que necesitaría un motor para
//! *imprimir* SAN en vez de solo *leerlo*. Aquí solo hace falta leerlo.

use crate::moves::{Move, MoveKind};
use crate::types::{file_of, rank_of, str_to_square, PieceType};

fn piece_from_char(c: char) -> Option<PieceType> {
    match c {
        'N' => Some(PieceType::Knight),
        'B' => Some(PieceType::Bishop),
        'R' => Some(PieceType::Rook),
        'Q' => Some(PieceType::Queen),
        'K' => Some(PieceType::King),
        _ => None,
    }
}

fn promo_from_char(c: char) -> Option<PieceType> {
    piece_from_char(c.to_ascii_uppercase())
}

/// Busca, entre `legal`, el movimiento que corresponde al token SAN dado.
/// `board` se usa únicamente para saber qué tipo de pieza ocupaba la
/// casilla de origen de cada candidato (SAN codifica el tipo de pieza, no
/// la casilla de origen exacta cuando no hace falta desambiguar).
pub fn match_san(board: &crate::board::Board, legal: &[Move], token: &str) -> Option<Move> {
    let cleaned = token.trim_end_matches(['+', '#', '!', '?']);

    if cleaned == "O-O" || cleaned == "0-0" {
        return legal
            .iter()
            .copied()
            .find(|m| m.kind == MoveKind::CastleKingside);
    }
    if cleaned == "O-O-O" || cleaned == "0-0-0" {
        return legal
            .iter()
            .copied()
            .find(|m| m.kind == MoveKind::CastleQueenside);
    }

    let mut chars = cleaned.chars();
    let first = chars.next()?;

    if let Some(piece) = piece_from_char(first) {
        let rest: String = chars.filter(|&c| c != 'x').collect();
        if rest.len() < 2 {
            return None;
        }
        let dest = str_to_square(&rest[rest.len() - 2..])?;
        let disambig = &rest[..rest.len() - 2];
        let disambig_file = disambig.chars().find(char::is_ascii_lowercase);
        let disambig_rank = disambig.chars().find(char::is_ascii_digit);

        let mut candidates = legal.iter().copied().filter(|m| {
            m.to == dest
                && board.mailbox[m.from as usize].map(|p| p.kind) == Some(piece)
                && disambig_file.is_none_or(|f| file_of(m.from) == f as u8 - b'a')
                && disambig_rank
                    .is_none_or(|r| rank_of(m.from) == r.to_digit(10).unwrap() as u8 - 1)
        });
        let found = candidates.next()?;
        return if candidates.next().is_none() {
            Some(found)
        } else {
            None // token ambiguo entre >1 movimiento legal: EPD mal formado.
        };
    }

    // Movimiento de peón: "<col_origen?>x?<destino><=Promoción?>".
    let eq_pos = cleaned.find('=');
    let promo = eq_pos
        .and_then(|i| cleaned[i + 1..].chars().next())
        .and_then(promo_from_char);
    let without_promo = match eq_pos {
        Some(i) => &cleaned[..i],
        None => cleaned,
    };
    let no_x: String = without_promo.chars().filter(|&c| c != 'x').collect();
    if no_x.len() < 2 {
        return None;
    }
    let dest = str_to_square(&no_x[no_x.len() - 2..])?;
    let origin_file = if no_x.len() > 2 {
        Some(no_x.chars().next().unwrap() as u8 - b'a')
    } else {
        None
    };

    legal.iter().copied().find(|m| {
        m.to == dest
            && board.mailbox[m.from as usize].map(|p| p.kind) == Some(PieceType::Pawn)
            && origin_file.is_none_or(|f| file_of(m.from) == f)
            && (origin_file.is_some() || file_of(m.from) == file_of(dest))
            && m.promotion() == promo
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::movegen::generate_legal_moves;

    fn find(fen: &str, token: &str) -> Option<String> {
        let board = Board::from_fen(fen).unwrap();
        let legal = generate_legal_moves(&board);
        match_san(&board, &legal, token).map(|m| m.to_string())
    }

    #[test]
    fn simple_pawn_push() {
        assert_eq!(
            find(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "e4"
            ),
            Some("e2e4".to_string())
        );
    }

    #[test]
    fn simple_knight_move() {
        assert_eq!(
            find(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "Nf3"
            ),
            Some("g1f3".to_string())
        );
    }

    #[test]
    fn pawn_capture_with_file_disambiguation() {
        // Blancas en e4 pueden capturar en d5; "exd5" debe indicar la
        // columna de origen (e), no confundirse con otra pieza en e-algo.
        assert_eq!(
            find("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "exd5"),
            Some("e4d5".to_string())
        );
    }

    #[test]
    fn promotion_with_capture() {
        assert_eq!(
            find("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "axb8=Q"),
            Some("a7b8q".to_string())
        );
    }

    #[test]
    fn castling_kingside() {
        assert_eq!(
            find("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "O-O"),
            Some("e1g1".to_string())
        );
    }

    #[test]
    fn rook_disambiguation_by_file() {
        // Dos torres blancas en la primera fila (a1 y h1): solo "Rad1"
        // debe emparejar con la de a1, no con la de h1.
        assert_eq!(
            find("4k3/8/8/8/8/8/8/R2RK3 w - - 0 1", "Rad1"),
            None // ya hay una torre en d1: "Rad1" no tiene sentido aquí,
                 // se prueba con una posición real más abajo.
        );
        // Rey fuera de la fila para que ambas torres puedan llegar a d1
        // legalmente (con el rey en e1, la torre de h1 lo tendría en medio
        // del camino y "Rhd1" sería, con razón, un movimiento ilegal).
        assert_eq!(
            find("4k3/8/8/8/8/6K1/8/R6R w - - 0 1", "Rad1"),
            Some("a1d1".to_string())
        );
        assert_eq!(
            find("4k3/8/8/8/8/6K1/8/R6R w - - 0 1", "Rhd1"),
            Some("h1d1".to_string())
        );
    }

    #[test]
    fn checkmate_annotation_is_ignored() {
        assert_eq!(
            find("6k1/5ppp/8/8/8/8/8/4R1K1 w - - 0 1", "Re8#"),
            Some("e1e8".to_string())
        );
    }

    #[test]
    fn unmatched_token_returns_none() {
        assert_eq!(
            find(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "Qh5"
            ),
            None
        );
    }
}
