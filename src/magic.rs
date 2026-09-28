//! Magic bitboards para piezas deslizantes (torre, alfil).
//!
//! Reemplaza el método de rayos usado en la Fase 1 (recortar un rayo
//! precalculado en el primer bloqueador) con una tabla de consulta O(1) por
//! casilla: se aplica una máscara a la ocupación real del tablero, se
//! multiplica por un "número mágico" específico de esa casilla, y los bits
//! altos del resultado dan directamente el índice en una tabla
//! precalculada con el patrón de ataque correcto. Es la técnica estándar
//! (Chess Programming Wiki) usada por prácticamente todo motor moderno
//! rápido en CPU.
//!
//! Los 128 números mágicos de abajo (64 para torre, 64 para alfil) no
//! fueron copiados de ningún motor: se generaron con una búsqueda propia,
//! incluida en este mismo repositorio en `src/bin/find_magics.rs`
//! (`cargo run --release --bin find_magics`), que cualquiera puede correr
//! de forma independiente para reproducirlos. Cada número fue verificado
//! sin colisiones contra un oráculo de rayos autocontenido dentro de esa
//! misma herramienta. Además, la suma total de bits relevantes por casilla
//! calculada aquí (672 para torre, 364 para alfil) coincide exactamente con
//! las tablas de referencia públicas y bien conocidas de la comunidad, lo
//! cual valida la máscara completa de las 64 casillas, no solo un puñado
//! verificado a mano.

use crate::bitboard::{gen_ray, Bitboard, EMPTY};
use crate::types::Square;
use std::sync::OnceLock;

use crate::magic_constants::{BISHOP_MAGICS, ROOK_MAGICS};

fn rook_mask(sq: Square) -> Bitboard {
    trim_edge(gen_ray(sq, 0, 1), true) // N
        | trim_edge(gen_ray(sq, 0, -1), false) // S
        | trim_edge(gen_ray(sq, 1, 0), true) // E
        | trim_edge(gen_ray(sq, -1, 0), false) // W
}

fn bishop_mask(sq: Square) -> Bitboard {
    trim_edge(gen_ray(sq, 1, 1), true) // NE (índice crece)
        | trim_edge(gen_ray(sq, -1, 1), true) // NW (índice crece)
        | trim_edge(gen_ray(sq, 1, -1), false) // SE (índice decrece)
        | trim_edge(gen_ray(sq, -1, -1), false) // SW (índice decrece)
}

/// Quita el bit más alto (si `increasing`) o más bajo (si no) de un rayo:
/// esa casilla es siempre el borde real del tablero en esa dirección y, por
/// construcción, nunca cambia el patrón de ataque resultante (el rayo llega
/// hasta ahí exista o no una pieza bloqueando exactamente en ese límite),
/// así que se excluye de la máscara de casillas relevantes como bloqueador.
fn trim_edge(ray: Bitboard, increasing: bool) -> Bitboard {
    if ray == 0 {
        return 0;
    }
    if increasing {
        let highest = 63 - ray.leading_zeros() as u8;
        ray & !(1u64 << highest)
    } else {
        let lowest = ray.trailing_zeros() as u8;
        ray & !(1u64 << lowest)
    }
}

/// Oráculo de rayos independiente (idéntico en técnica al de
/// `bitboard.rs`, reimplementado aquí para no depender de `Tables` durante
/// la construcción de las tablas mágicas — evita cualquier dependencia
/// circular y mantiene este módulo fácil de auditar por separado).
fn slide(sq: Square, df: i32, dr: i32, occ: Bitboard) -> Bitboard {
    use crate::types::{file_of, make_square, rank_of};
    let mut bb = EMPTY;
    let mut f = file_of(sq) as i32 + df;
    let mut r = rank_of(sq) as i32 + dr;
    while (0..8).contains(&f) && (0..8).contains(&r) {
        let s = make_square(f as u8, r as u8);
        bb |= 1u64 << s;
        if occ & (1u64 << s) != 0 {
            break;
        }
        f += df;
        r += dr;
    }
    bb
}

fn rook_attacks_oracle(sq: Square, occ: Bitboard) -> Bitboard {
    slide(sq, 0, 1, occ) | slide(sq, 0, -1, occ) | slide(sq, 1, 0, occ) | slide(sq, -1, 0, occ)
}

fn bishop_attacks_oracle(sq: Square, occ: Bitboard) -> Bitboard {
    slide(sq, 1, 1, occ) | slide(sq, -1, 1, occ) | slide(sq, 1, -1, occ) | slide(sq, -1, -1, occ)
}

fn subset_from_index(index: usize, mask: Bitboard) -> Bitboard {
    let mut result = 0u64;
    let mut m = mask;
    let mut idx = index;
    while m != 0 {
        let bit = m & m.wrapping_neg();
        m ^= bit;
        if idx & 1 != 0 {
            result |= bit;
        }
        idx >>= 1;
    }
    result
}

struct MagicEntry {
    mask: Bitboard,
    magic: u64,
    shift: u32,
    table: Vec<Bitboard>,
}

impl MagicEntry {
    #[inline(always)]
    fn index(&self, occ: Bitboard) -> usize {
        (((occ & self.mask).wrapping_mul(self.magic)) >> self.shift) as usize
    }

    #[inline(always)]
    fn attacks(&self, occ: Bitboard) -> Bitboard {
        self.table[self.index(occ)]
    }
}

fn build_table(sq: Square, magic: u64, is_rook: bool) -> MagicEntry {
    let mask = if is_rook {
        rook_mask(sq)
    } else {
        bishop_mask(sq)
    };
    let bits = mask.count_ones();
    let shift = 64 - bits;
    let size = 1usize << bits;
    let mut table = vec![EMPTY; size];
    for i in 0..size {
        let occ = subset_from_index(i, mask);
        let attacks = if is_rook {
            rook_attacks_oracle(sq, occ)
        } else {
            bishop_attacks_oracle(sq, occ)
        };
        let index = ((occ.wrapping_mul(magic)) >> shift) as usize;
        // No debería haber colisiones destructivas: estos números mágicos
        // ya fueron verificados exhaustivamente al buscarlos (ver
        // find_magics.rs). Si esto llegara a fallar, sería una señal de que
        // el número mágico usado no corresponde a esta casilla/pieza.
        debug_assert!(
            table[index] == EMPTY || table[index] == attacks,
            "colisión mágica destructiva en casilla {sq}, índice {index}"
        );
        table[index] = attacks;
    }
    MagicEntry {
        mask,
        magic,
        shift,
        table,
    }
}

pub struct MagicTables {
    rook: Vec<MagicEntry>,
    bishop: Vec<MagicEntry>,
}

impl MagicTables {
    fn new() -> Self {
        let rook = (0u8..64)
            .map(|sq| build_table(sq, ROOK_MAGICS[sq as usize], true))
            .collect();
        let bishop = (0u8..64)
            .map(|sq| build_table(sq, BISHOP_MAGICS[sq as usize], false))
            .collect();
        MagicTables { rook, bishop }
    }

    #[inline(always)]
    pub fn rook_attacks(&self, sq: Square, occ: Bitboard) -> Bitboard {
        self.rook[sq as usize].attacks(occ)
    }

    #[inline(always)]
    pub fn bishop_attacks(&self, sq: Square, occ: Bitboard) -> Bitboard {
        self.bishop[sq as usize].attacks(occ)
    }
}

static MAGIC_TABLES: OnceLock<MagicTables> = OnceLock::new();

pub fn magic_tables() -> &'static MagicTables {
    MAGIC_TABLES.get_or_init(MagicTables::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitboard::{clear_bit, set_bit};

    /// PRNG simple y determinista solo para generar ocupaciones de prueba.
    struct XorShift64(u64);
    impl XorShift64 {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    #[test]
    fn matches_ray_oracle_on_mask_subsets_exhaustively() {
        // Para cada casilla, probamos las 2^n ocupaciones posibles dentro de
        // su propia máscara (no una muestra: TODAS), comparando la consulta
        // mágica contra el oráculo de rayos. Esto cubre cada combinación de
        // bloqueadores relevante que la tabla puede llegar a ver.
        let mt = magic_tables();
        for sq in 0u8..64 {
            let rmask = rook_mask(sq);
            for i in 0..(1usize << rmask.count_ones()) {
                let occ = subset_from_index(i, rmask);
                assert_eq!(
                    mt.rook_attacks(sq, occ),
                    rook_attacks_oracle(sq, occ),
                    "torre: discrepancia en casilla {sq}, ocupación {occ:#x}"
                );
            }
            let bmask = bishop_mask(sq);
            for i in 0..(1usize << bmask.count_ones()) {
                let occ = subset_from_index(i, bmask);
                assert_eq!(
                    mt.bishop_attacks(sq, occ),
                    bishop_attacks_oracle(sq, occ),
                    "alfil: discrepancia en casilla {sq}, ocupación {occ:#x}"
                );
            }
        }
    }

    #[test]
    fn matches_ray_oracle_on_full_random_occupancies() {
        // Complemento crítico del test anterior: aquí NO nos limitamos a
        // subconjuntos de la máscara que asumimos relevante, sino que
        // probamos ocupaciones de tablero completas y aleatorias. Si la
        // máscara hubiera omitido por error alguna casilla realmente
        // relevante, el test anterior (que solo varía casillas dentro de la
        // máscara asumida) no lo detectaría — pero este sí, porque aquí
        // cualquier casilla del tablero puede estar ocupada.
        let mut rng = XorShift64(0xD1CE_D1CE_D1CE_D1CE);
        for sq in 0u8..64 {
            for _ in 0..20_000 {
                let occ = rng.next() & rng.next(); // sesgado a ocupaciones dispersas, más realista
                assert_eq!(
                    magic_tables().rook_attacks(sq, occ),
                    rook_attacks_oracle(sq, occ),
                    "torre: discrepancia (ocupación completa aleatoria) en casilla {sq}"
                );
                assert_eq!(
                    magic_tables().bishop_attacks(sq, occ),
                    bishop_attacks_oracle(sq, occ),
                    "alfil: discrepancia (ocupación completa aleatoria) en casilla {sq}"
                );
            }
        }
    }

    #[test]
    fn total_relevant_bits_match_known_reference_tables() {
        // La suma de bits relevantes en las 64 casillas es un invariante
        // públicamente conocido y bien documentado: 672 para torre, 364
        // para alfil. Si la fórmula de la máscara cambiara por error, esta
        // suma cambiaría también, así que sirve como verificación rápida
        // independiente de los valores casilla por casilla.
        let rook_total: u32 = (0u8..64).map(|sq| rook_mask(sq).count_ones()).sum();
        let bishop_total: u32 = (0u8..64).map(|sq| bishop_mask(sq).count_ones()).sum();
        assert_eq!(rook_total, 672);
        assert_eq!(bishop_total, 364);
    }

    #[test]
    fn corner_and_center_bit_counts_match_known_values() {
        // Esquinas: 12 bits para torre, 6 para alfil.
        for &sq in &[
            crate::types::A1,
            crate::types::H1,
            crate::types::A8,
            crate::types::H8,
        ] {
            assert_eq!(rook_mask(sq).count_ones(), 12);
            assert_eq!(bishop_mask(sq).count_ones(), 6);
        }
        // Centro (d4/e4/d5/e5): 10 bits para torre, 9 para alfil.
        for sq_name in ["d4", "e4", "d5", "e5"] {
            let sq = crate::types::str_to_square(sq_name).unwrap();
            assert_eq!(rook_mask(sq).count_ones(), 10, "torre en {sq_name}");
            assert_eq!(bishop_mask(sq).count_ones(), 9, "alfil en {sq_name}");
        }
    }

    #[test]
    fn no_destructive_collisions_were_silently_accepted() {
        // Reconstruir todas las tablas no debe entrar en pánico por el
        // debug_assert de build_table (que detectaría una colisión
        // destructiva). Si los números mágicos fueran inválidos para su
        // casilla, este test lo revelaría.
        let _ = MagicTables::new();
    }

    #[allow(dead_code)]
    fn unused_helpers_reference(occ: Bitboard, sq: Square) -> Bitboard {
        // Referencia para que `clear_bit`/`set_bit` importados no generen
        // warning si algún día se usan directamente en más tests.
        set_bit(clear_bit(occ, sq), sq)
    }
}
