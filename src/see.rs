//! Static Exchange Evaluation (SEE).
//!
//! Dado un movimiento de captura, simula la secuencia completa de capturas y
//! recapturas en esa casilla (usando siempre la pieza atacante de menor
//! valor disponible en cada turno, técnica estándar "swap-off list" de la
//! Chess Programming Wiki) y devuelve el resultado material neto asumiendo
//! que ambos bandos juegan óptimamente — incluyendo la posibilidad de que
//! el bando que recaptura decida *no* seguir capturando si le conviene más
//! detenerse.
//!
//! Esto es mucho más preciso que MVV-LVA (que solo mira "víctima menos
//! atacante" sin considerar qué pasa después): permite podar en quiescence
//! las capturas que claramente pierden material sin necesidad de buscarlas
//! a profundidad completa, y ordenar las capturas rentables antes que las
//! que no lo son.

use crate::bitboard::{clear_bit, lsb, tables, Bitboard, EMPTY};
use crate::board::Board;
use crate::moves::{Move, MoveKind};
use crate::types::*;

/// Valores usados específicamente para SEE. Coinciden con `eval::piece_value`
/// para peón..dama (misma escala de centipeones que el resto del motor),
/// pero el rey recibe un valor artificialmente alto en vez de 0: aquí lo que
/// nos interesa es "qué tan cara es esta pieza si la pierdo en el
/// intercambio", así que el rey debe ser siempre la última pieza elegida
/// como atacante, nunca la primera (al contrario que en `eval`, donde el
/// rey vale 0 porque ambos bandos siempre tienen uno y se cancela).
fn see_piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 20_000,
    }
}

/// Todas las piezas (de ambos colores) que atacan `sq` dada una ocupación
/// hipotética `occ`. Se recalcula en cada paso del intercambio porque
/// quitar una pieza puede "destapar" un ataque de rayos-x (p. ej. una torre
/// detrás de otra torre en la misma columna).
fn attackers_to(board: &Board, sq: Square, occ: Bitboard) -> Bitboard {
    let t = tables();
    let mut attackers = EMPTY;

    // Para hallar peones blancos que atacan `sq`, usamos el patrón de
    // ataque de un peón NEGRO imaginario parado en `sq` (apunta "hacia
    // atrás", justo a las casillas de origen de peones blancos atacantes),
    // y viceversa para peones negros.
    attackers |= t.pawn_attacks(Color::Black, sq)
        & board.pieces[Color::White.index()][PieceType::Pawn.index()];
    attackers |= t.pawn_attacks(Color::White, sq)
        & board.pieces[Color::Black.index()][PieceType::Pawn.index()];

    let knights = board.pieces[Color::White.index()][PieceType::Knight.index()]
        | board.pieces[Color::Black.index()][PieceType::Knight.index()];
    attackers |= t.knight_attacks(sq) & knights;

    let kings = board.pieces[Color::White.index()][PieceType::King.index()]
        | board.pieces[Color::Black.index()][PieceType::King.index()];
    attackers |= t.king_attacks(sq) & kings;

    let bishops_queens = board.pieces[Color::White.index()][PieceType::Bishop.index()]
        | board.pieces[Color::Black.index()][PieceType::Bishop.index()]
        | board.pieces[Color::White.index()][PieceType::Queen.index()]
        | board.pieces[Color::Black.index()][PieceType::Queen.index()];
    attackers |= t.bishop_attacks(sq, occ) & bishops_queens;

    let rooks_queens = board.pieces[Color::White.index()][PieceType::Rook.index()]
        | board.pieces[Color::Black.index()][PieceType::Rook.index()]
        | board.pieces[Color::White.index()][PieceType::Queen.index()]
        | board.pieces[Color::Black.index()][PieceType::Queen.index()];
    attackers |= t.rook_attacks(sq, occ) & rooks_queens;

    attackers & occ
}

fn least_valuable_attacker(
    board: &Board,
    attackers: Bitboard,
    side: Color,
) -> Option<(Square, PieceType)> {
    for &pt in &[
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ] {
        let candidates = attackers & board.pieces[side.index()][pt.index()];
        if candidates != EMPTY {
            return Some((lsb(candidates), pt));
        }
    }
    None
}

/// Resultado neto en centipeones (desde la perspectiva de quien juega `mv`)
/// de la secuencia completa de capturas en la casilla destino, asumiendo
/// que cada bando recaptura con su pieza de menor valor disponible y se
/// detiene en cuanto seguir capturando dejaría de convenirle. Devuelve 0
/// para movimientos que no son captura.
pub fn see(board: &Board, mv: &Move) -> i32 {
    if !mv.is_capture() {
        return 0;
    }

    let to = mv.to;
    let mut occ = board.occupancy();

    // Valor de lo que captura `mv` en sí (paso 0 del intercambio).
    let step0_victim_value = if mv.kind == MoveKind::EnPassantCapture {
        let captured_sq = if board.side_to_move == Color::White {
            mv.to - 8
        } else {
            mv.to + 8
        };
        occ = clear_bit(occ, captured_sq); // el peón capturado al paso no está en `to`
        see_piece_value(PieceType::Pawn)
    } else {
        let victim_kind = board.mailbox[to as usize]
            .map(|p| p.kind)
            .unwrap_or(PieceType::Pawn);
        see_piece_value(victim_kind)
    };

    // gains[i] = valor de la pieza que se captura en el paso i del intercambio.
    const MAX_STEPS: usize = 32; // más que suficiente: nunca hay más de 16 piezas por bando
    let mut gains = [0i32; MAX_STEPS];
    gains[0] = step0_victim_value;

    // La pieza que ejecuta `mv` queda "de pie" sobre `to`, vulnerable a la
    // siguiente recaptura. Si `mv` es una promoción con captura, lo que
    // realmente queda ahí es la pieza promovida, no el peón.
    let mut standing_value = if let Some(promo) = mv.promotion() {
        see_piece_value(promo)
    } else {
        let original_kind = board.mailbox[mv.from as usize]
            .map(|p| p.kind)
            .unwrap_or(PieceType::Pawn);
        see_piece_value(original_kind)
    };
    occ = clear_bit(occ, mv.from);

    let mut side = board.side_to_move.opposite();
    let mut n = 1usize;
    let promo_rank_0 = 0u8;
    let promo_rank_7 = 7u8;
    let to_rank = rank_of(to);

    while n < MAX_STEPS {
        let attackers = attackers_to(board, to, occ) & occ;
        let (sq, pt) = match least_valuable_attacker(board, attackers, side) {
            Some(v) => v,
            None => break,
        };

        gains[n] = standing_value; // lo que había quedado parado se captura ahora

        // Si un peón recaptura y llega a la última fila, asumimos que
        // promociona a dama (igual que hace nuestro propio movegen): su
        // valor "de pie" tras esta recaptura es el de una dama, no un peón.
        standing_value =
            if pt == PieceType::Pawn && (to_rank == promo_rank_0 || to_rank == promo_rank_7) {
                see_piece_value(PieceType::Queen)
            } else {
                see_piece_value(pt)
            };

        occ = clear_bit(occ, sq);
        side = side.opposite();
        n += 1;
    }

    // Reducción hacia atrás (minimax): cada bando, en su turno dentro del
    // intercambio, elige entre seguir capturando (ganando gains[i], menos
    // lo que el rival logre a su vez) o detenerse (ganando 0). Esto es lo
    // que le permite a SEE reconocer, por ejemplo, que no conviene
    // recapturar con una torre si detrás viene una dama enemiga.
    for i in (0..n.saturating_sub(1)).rev() {
        gains[i] -= gains[i + 1].max(0);
    }

    gains[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen::find_move;

    fn see_of(fen: &str, uci_move: &str) -> i32 {
        let board = Board::from_fen(fen).unwrap();
        let mv = find_move(&board, uci_move)
            .unwrap_or_else(|| panic!("movimiento inesperadamente ilegal: {uci_move} en {fen}"));
        see(&board, &mv)
    }

    #[test]
    fn non_capture_returns_zero() {
        assert_eq!(see_of("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", "a1a4"), 0);
    }

    #[test]
    fn free_capture_no_recapture() {
        // Torre captura un caballo totalmente indefenso: gana su valor completo.
        let value = see_of("4k3/8/8/4n3/8/8/8/4R2K w - - 0 1", "e1e5");
        assert_eq!(value, 320);
    }

    #[test]
    fn losing_trade_queen_takes_defended_pawn() {
        // Dama captura un peón defendido por el rey enemigo: pierde dama por peón.
        let value = see_of("4k3/3p4/8/8/8/8/8/K2Q4 w - - 0 1", "d1d7");
        assert_eq!(value, 100 - 900);
    }

    #[test]
    fn winning_trade_with_recapture() {
        // Peón captura caballo, un peón enemigo recaptura: gana caballo menos peón.
        let value = see_of("4k3/8/2p5/3n4/4P3/8/8/K7 w - - 0 1", "e4d5");
        assert_eq!(value, 320 - 100);
    }

    #[test]
    fn defender_should_not_over_recapture() {
        // Peón captura peón (cambio parejo). La torre negra PODRÍA recapturar,
        // pero si lo hace, la dama blanca recapturaría a su vez y las negras
        // saldrían perdiendo. SEE debe reconocer que a las negras les
        // conviene no recapturar, dejando el resultado en solo +100 (el
        // cambio de peones), no en el desastre que sería seguir la cadena.
        let value = see_of("k2r4/8/8/3p4/4P3/8/8/K2Q4 w - - 0 1", "e4d5");
        assert_eq!(value, 100);
    }

    #[test]
    fn xray_attack_through_battery() {
        // Dos torres blancas apiladas en la columna e (e1 detrás de e2).
        // La torre de e2 captura un caballo en e5, defendido por un alfil en
        // d6. Tras el primer intercambio, la torre de e1 "destapa" su
        // ataque a través de la columna e ahora vacía y puede recapturar el
        // alfil. Esto solo da el resultado correcto (+150) si el código
        // recalcula los atacantes (incluyendo rayos-x) en cada paso.
        let value = see_of("k7/8/3b4/4n3/8/8/4R3/K3R3 w - - 0 1", "e2e5");
        assert_eq!(value, 150);
    }

    #[test]
    fn en_passant_capture_see() {
        // La pieza capturada al paso está en d5, no en la casilla destino
        // d6: si el código buscara mal la víctima, esto fallaría o daría 0.
        let value = see_of("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "e5d6");
        assert_eq!(value, 100);
    }

    #[test]
    fn promotion_capture_see_uses_promoted_value() {
        // Peón captura una torre coronando a dama; el rey enemigo recaptura
        // la casilla. Lo que se pierde en la recaptura es la DAMA recién
        // coronada (900), no un peón (100) — por eso el resultado neto es
        // negativo pese a haber ganado una torre en el primer paso.
        let value = see_of("4rk2/3P4/8/8/8/8/8/K7 w - - 0 1", "d7e8q");
        assert_eq!(value, 500 - 900);
    }
}
