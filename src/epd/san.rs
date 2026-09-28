use crate::board::Board;
use crate::movegen::generate_legal_moves;
use crate::moves::{Move, MoveKind};
use crate::types::{file_of, str_to_square, PieceType};

pub fn resolve_san_token(board: &Board, token: &str) -> Option<Move> {
    let token = clean_token(token);
    if token.is_empty() {
        return None;
    }

    let legal = generate_legal_moves(board);
    let matches: Vec<Move> = legal
        .into_iter()
        .filter(|mv| san_matches(board, *mv, &token))
        .collect();

    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn clean_token(token: &str) -> String {
    token
        .trim()
        .trim_end_matches(['+', '#', '!', '?'])
        .to_string()
}

fn san_matches(board: &Board, mv: Move, token: &str) -> bool {
    if matches!(mv.kind, MoveKind::CastleKingside) {
        return token == "O-O" || token == "0-0";
    }
    if matches!(mv.kind, MoveKind::CastleQueenside) {
        return token == "O-O-O" || token == "0-0-0";
    }

    let Some(piece) = board.mailbox[mv.from as usize] else {
        return false;
    };

    let mut rest = token;
    let wanted_piece = match rest.chars().next() {
        Some('N') => {
            rest = &rest[1..];
            PieceType::Knight
        }
        Some('B') => {
            rest = &rest[1..];
            PieceType::Bishop
        }
        Some('R') => {
            rest = &rest[1..];
            PieceType::Rook
        }
        Some('Q') => {
            rest = &rest[1..];
            PieceType::Queen
        }
        Some('K') => {
            rest = &rest[1..];
            PieceType::King
        }
        _ => PieceType::Pawn,
    };
    if piece.kind != wanted_piece {
        return false;
    }

    let promotion = if let Some(idx) = rest.find('=') {
        let promoted = rest[idx + 1..]
            .chars()
            .next()
            .and_then(PieceType::from_char);
        rest = &rest[..idx];
        promoted
    } else {
        None
    };
    if mv.promotion() != promotion {
        return false;
    }

    let Some(dest_text) = rest.get(rest.len().saturating_sub(2)..) else {
        return false;
    };
    if str_to_square(dest_text) != Some(mv.to) {
        return false;
    }

    let prefix = &rest[..rest.len().saturating_sub(2)];
    let capture = prefix.contains('x');
    if capture != mv.is_capture() {
        return false;
    }
    let disambiguation = prefix.replace('x', "");

    if wanted_piece == PieceType::Pawn {
        if capture {
            let Some(file) = disambiguation.as_bytes().first() else {
                return false;
            };
            return (b'a'..=b'h').contains(file) && file_of(mv.from) == file - b'a';
        }
        return disambiguation.is_empty();
    }

    disambiguation.chars().all(|c| match c {
        'a'..='h' => file_of(mv.from) == c as u8 - b'a',
        '1'..='8' => crate::types::rank_of(mv.from) == c as u8 - b'1',
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(fen: &str, san: &str) -> String {
        let board = Board::from_fen(fen).unwrap();
        resolve_san_token(&board, san).unwrap().to_string()
    }

    #[test]
    fn simple_pawn_push() {
        assert_eq!(mv("8/8/8/8/8/8/4P3/4K2k w - - 0 1", "e4"), "e2e4");
    }

    #[test]
    fn simple_knight_move() {
        assert_eq!(mv("7k/8/8/8/8/8/8/4K1N1 w - - 0 1", "Nf3"), "g1f3");
    }

    #[test]
    fn pawn_capture_with_file_disambiguation() {
        assert_eq!(mv("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "exd5"), "e4d5");
    }

    #[test]
    fn promotion() {
        assert_eq!(mv("k7/4P3/8/8/8/8/8/4K3 w - - 0 1", "e8=Q"), "e7e8q");
    }

    #[test]
    fn piece_capture() {
        assert_eq!(mv("4k3/3n4/8/8/8/8/8/3QK3 w - - 0 1", "Qxd7"), "d1d7");
    }

    #[test]
    fn castling_token_if_legal_position_available() {
        assert_eq!(mv("4k3/8/8/8/8/8/8/4K2R w K - 0 1", "O-O"), "e1g1");
    }

    #[test]
    fn ambiguous_token_returns_none_when_applicable() {
        let board = Board::from_fen("7k/8/8/8/8/8/8/2N1K1N1 w - - 0 1").unwrap();
        assert!(resolve_san_token(&board, "Ne2").is_none());
    }
}
