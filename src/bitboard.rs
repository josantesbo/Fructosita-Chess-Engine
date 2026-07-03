//! Representación de bitboards y tablas de ataque precalculadas.
//!
//! Las piezas deslizantes (alfil/torre/dama) usan el método clásico de "rayos
//! con primer bloqueador": se precalcula el rayo completo en cada una de las
//! 8 direcciones para cada casilla, y en tiempo de ejecución se recorta el
//! rayo en el primer bloqueador encontrado. Es más simple y fácil de
//! verificar que magic bitboards; la optimización a magic bitboards queda
//! planeada para una fase posterior una vez que la velocidad sea el cuello
//! de botella (autojuego / búsqueda profunda).

use crate::types::{file_of, make_square, rank_of, Color, Square};
use std::sync::OnceLock;

pub type Bitboard = u64;

pub const EMPTY: Bitboard = 0;

#[inline(always)]
pub fn set_bit(bb: Bitboard, sq: Square) -> Bitboard {
    bb | (1u64 << sq)
}

#[inline(always)]
pub fn get_bit(bb: Bitboard, sq: Square) -> bool {
    (bb >> sq) & 1 != 0
}

#[inline(always)]
pub fn clear_bit(bb: Bitboard, sq: Square) -> Bitboard {
    bb & !(1u64 << sq)
}

/// Extrae y elimina el bit menos significativo (la casilla de menor índice).
#[inline(always)]
pub fn pop_lsb(bb: &mut Bitboard) -> Square {
    let sq = bb.trailing_zeros() as Square;
    *bb &= *bb - 1;
    sq
}

#[inline(always)]
pub fn lsb(bb: Bitboard) -> Square {
    bb.trailing_zeros() as Square
}

// count_bits y print_bitboard se usarán en la Fase 2 (evaluación: conteo de
// material/movilidad) y para depuración manual; se dejan documentados y
// probados desde ya para no tener que redescubrirlos después.
#[allow(dead_code)]
#[inline(always)]
pub fn count_bits(bb: Bitboard) -> u32 {
    bb.count_ones()
}

/// Imprime un bitboard en formato 8x8 (rank 8 arriba), útil para depurar.
#[allow(dead_code)]
pub fn print_bitboard(bb: Bitboard) -> String {
    let mut s = String::new();
    for rank in (0..8).rev() {
        for file in 0..8 {
            let sq = make_square(file, rank);
            s.push(if get_bit(bb, sq) { '1' } else { '.' });
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// Genera el rayo completo (sin incluir la casilla de origen) desde `sq` en
/// la dirección (df, dr), deteniéndose en el borde del tablero.
pub(crate) fn gen_ray(sq: Square, df: i32, dr: i32) -> Bitboard {
    let mut bb = EMPTY;
    let mut f = file_of(sq) as i32 + df;
    let mut r = rank_of(sq) as i32 + dr;
    while (0..8).contains(&f) && (0..8).contains(&r) {
        bb = set_bit(bb, make_square(f as u8, r as u8));
        f += df;
        r += dr;
    }
    bb
}

fn gen_stepping(sq: Square, offsets: &[(i32, i32)]) -> Bitboard {
    let mut bb = EMPTY;
    let f0 = file_of(sq) as i32;
    let r0 = rank_of(sq) as i32;
    for (df, dr) in offsets {
        let f = f0 + df;
        let r = r0 + dr;
        if (0..8).contains(&f) && (0..8).contains(&r) {
            bb = set_bit(bb, make_square(f as u8, r as u8));
        }
    }
    bb
}

pub struct Tables {
    pub knight: [Bitboard; 64],
    pub king: [Bitboard; 64],
    pub pawn: [[Bitboard; 64]; 2],
    ray_n: [Bitboard; 64],
    ray_s: [Bitboard; 64],
    ray_e: [Bitboard; 64],
    ray_w: [Bitboard; 64],
    ray_ne: [Bitboard; 64],
    ray_nw: [Bitboard; 64],
    ray_se: [Bitboard; 64],
    ray_sw: [Bitboard; 64],
}

impl Tables {
    fn new() -> Tables {
        let mut knight = [EMPTY; 64];
        let mut king = [EMPTY; 64];
        let mut pawn = [[EMPTY; 64]; 2];
        let mut ray_n = [EMPTY; 64];
        let mut ray_s = [EMPTY; 64];
        let mut ray_e = [EMPTY; 64];
        let mut ray_w = [EMPTY; 64];
        let mut ray_ne = [EMPTY; 64];
        let mut ray_nw = [EMPTY; 64];
        let mut ray_se = [EMPTY; 64];
        let mut ray_sw = [EMPTY; 64];

        const KNIGHT_OFFSETS: [(i32, i32); 8] = [
            (1, 2), (2, 1), (2, -1), (1, -2),
            (-1, -2), (-2, -1), (-2, 1), (-1, 2),
        ];
        const KING_OFFSETS: [(i32, i32); 8] = [
            (1, 0), (1, 1), (0, 1), (-1, 1),
            (-1, 0), (-1, -1), (0, -1), (1, -1),
        ];

        for sq in 0u8..64 {
            knight[sq as usize] = gen_stepping(sq, &KNIGHT_OFFSETS);
            king[sq as usize] = gen_stepping(sq, &KING_OFFSETS);
            pawn[Color::White.index()][sq as usize] = gen_stepping(sq, &[(-1, 1), (1, 1)]);
            pawn[Color::Black.index()][sq as usize] = gen_stepping(sq, &[(-1, -1), (1, -1)]);

            ray_n[sq as usize] = gen_ray(sq, 0, 1);
            ray_s[sq as usize] = gen_ray(sq, 0, -1);
            ray_e[sq as usize] = gen_ray(sq, 1, 0);
            ray_w[sq as usize] = gen_ray(sq, -1, 0);
            ray_ne[sq as usize] = gen_ray(sq, 1, 1);
            ray_nw[sq as usize] = gen_ray(sq, -1, 1);
            ray_se[sq as usize] = gen_ray(sq, 1, -1);
            ray_sw[sq as usize] = gen_ray(sq, -1, -1);
        }

        Tables {
            knight, king, pawn,
            ray_n, ray_s, ray_e, ray_w,
            ray_ne, ray_nw, ray_se, ray_sw,
        }
    }

    #[inline(always)]
    pub fn knight_attacks(&self, sq: Square) -> Bitboard {
        self.knight[sq as usize]
    }

    #[inline(always)]
    pub fn king_attacks(&self, sq: Square) -> Bitboard {
        self.king[sq as usize]
    }

    #[inline(always)]
    pub fn pawn_attacks(&self, color: Color, sq: Square) -> Bitboard {
        self.pawn[color.index()][sq as usize]
    }

    /// Recorta un rayo precalculado en el primer bloqueador. Método de
    /// referencia ("oráculo"): ya no se usa en la ruta rápida del motor
    /// (ver `bishop_attacks`/`rook_attacks`, que usan magic bitboards desde
    /// la Fase 1 continuada), pero se mantiene disponible para comparar
    /// contra él en tests — es la implementación original, validada con
    /// perft exacto, y sirve como red de seguridad independiente.
    /// `positive` indica si la dirección incrementa el índice de casilla
    /// (N, E, NE, NW) o lo decrementa (S, W, SE, SW).
    #[allow(dead_code)]
    #[inline(always)]
    fn slide(table: &[Bitboard; 64], sq: Square, occupancy: Bitboard, positive: bool) -> Bitboard {
        let attacks = table[sq as usize];
        let blockers = attacks & occupancy;
        if blockers == EMPTY {
            return attacks;
        }
        let blocker_sq = if positive {
            lsb(blockers)
        } else {
            63 - blockers.leading_zeros() as Square
        };
        attacks & !table[blocker_sq as usize]
    }

    /// Implementación de referencia por rayos (ver comentario en `slide`).
    #[allow(dead_code)]
    pub fn bishop_attacks_ray(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        Self::slide(&self.ray_ne, sq, occupancy, true)
            | Self::slide(&self.ray_nw, sq, occupancy, true)
            | Self::slide(&self.ray_se, sq, occupancy, false)
            | Self::slide(&self.ray_sw, sq, occupancy, false)
    }

    /// Implementación de referencia por rayos (ver comentario en `slide`).
    #[allow(dead_code)]
    pub fn rook_attacks_ray(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        Self::slide(&self.ray_n, sq, occupancy, true)
            | Self::slide(&self.ray_s, sq, occupancy, false)
            | Self::slide(&self.ray_e, sq, occupancy, true)
            | Self::slide(&self.ray_w, sq, occupancy, false)
    }

    /// Ataques de alfil: magic bitboards (O(1) por consulta), validados
    /// exhaustivamente contra `bishop_attacks_ray` — ver `src/magic.rs`.
    #[inline(always)]
    pub fn bishop_attacks(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        crate::magic::magic_tables().bishop_attacks(sq, occupancy)
    }

    /// Ataques de torre: magic bitboards (O(1) por consulta), validados
    /// exhaustivamente contra `rook_attacks_ray` — ver `src/magic.rs`.
    #[inline(always)]
    pub fn rook_attacks(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        crate::magic::magic_tables().rook_attacks(sq, occupancy)
    }

    #[inline(always)]
    pub fn queen_attacks(&self, sq: Square, occupancy: Bitboard) -> Bitboard {
        self.bishop_attacks(sq, occupancy) | self.rook_attacks(sq, occupancy)
    }
}

static TABLES: OnceLock<Tables> = OnceLock::new();

pub fn tables() -> &'static Tables {
    TABLES.get_or_init(Tables::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn knight_corner() {
        // Un caballo en a1 solo tiene 2 movimientos posibles.
        let attacks = tables().knight_attacks(A1);
        assert_eq!(count_bits(attacks), 2);
        assert!(get_bit(attacks, str_to_square("b3").unwrap()));
        assert!(get_bit(attacks, str_to_square("c2").unwrap()));
    }

    #[test]
    fn king_center() {
        let e4 = str_to_square("e4").unwrap();
        assert_eq!(count_bits(tables().king_attacks(e4)), 8);
    }

    #[test]
    fn rook_blocked() {
        // Torre en a1, pieza propia/enemiga en a4: debe parar ahí (puede capturarla)
        // pero no seguir más allá.
        let a1 = str_to_square("a1").unwrap();
        let a4 = str_to_square("a4").unwrap();
        let occ = set_bit(EMPTY, a4);
        let attacks = tables().rook_attacks(a1, occ);
        assert!(get_bit(attacks, a4));
        assert!(!get_bit(attacks, str_to_square("a5").unwrap()));
        assert!(get_bit(attacks, str_to_square("a3").unwrap()));
        assert!(get_bit(attacks, str_to_square("h1").unwrap()));
    }

    #[test]
    fn bishop_open_center() {
        let d4 = str_to_square("d4").unwrap();
        // Diagonales completas desde d4 en tablero vacío: 13 casillas.
        assert_eq!(count_bits(tables().bishop_attacks(d4, EMPTY)), 13);
    }

    #[test]
    fn magic_matches_ray_reference_on_sample_positions() {
        // Comparación directa, en este mismo archivo, de la ruta activa
        // (magic bitboards) contra la implementación de referencia por
        // rayos, sobre varias ocupaciones representativas de una partida
        // real (no aleatorias): posición inicial, kiwipete, y una posición
        // con piezas dispersas. Complementa (no sustituye) la validación
        // exhaustiva de src/magic.rs.
        use crate::board::Board;
        let positions = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ];
        for fen in positions {
            let board = Board::from_fen(fen).unwrap();
            let occ = board.occupancy();
            for sq in 0u8..64 {
                assert_eq!(
                    tables().rook_attacks(sq, occ),
                    tables().rook_attacks_ray(sq, occ),
                    "torre: magic vs rayos difieren en {sq} para {fen}"
                );
                assert_eq!(
                    tables().bishop_attacks(sq, occ),
                    tables().bishop_attacks_ray(sq, occ),
                    "alfil: magic vs rayos difieren en {sq} para {fen}"
                );
            }
        }
    }
}
