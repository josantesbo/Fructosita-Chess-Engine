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

pub const ROOK_MAGICS: [u64; 64] = [
    0x4080004004611180, // a1 (shift 52)
    0x414000A00040100A, // b1 (shift 53)
    0x4080100080200008, // c1 (shift 53)
    0x0100082100100004, // d1 (shift 53)
    0x0100021100040800, // e1 (shift 53)
    0x8A00080110048200, // f1 (shift 53)
    0x4100008200010004, // g1 (shift 53)
    0x0100048148220100, // h1 (shift 52)
    0x0020800040003080, // a2 (shift 53)
    0x0322404010002000, // b2 (shift 54)
    0x09A0808020001000, // c2 (shift 54)
    0x5002801000800800, // d2 (shift 54)
    0x2004808004008800, // e2 (shift 54)
    0x4223000401000802, // f2 (shift 54)
    0x9091000401008200, // g2 (shift 54)
    0x0942002082004411, // h2 (shift 53)
    0x4140008020408000, // a3 (shift 53)
    0x5440048020008050, // b3 (shift 54)
    0x0084110020004105, // c3 (shift 54)
    0x00C00B001000A100, // d3 (shift 54)
    0x000E1D0008005100, // e3 (shift 54)
    0x0105010004000208, // f3 (shift 54)
    0x0088440010410822, // g3 (shift 54)
    0x0842220000409114, // h3 (shift 53)
    0x0000800080204000, // a4 (shift 53)
    0x8120100040002040, // b4 (shift 54)
    0x0000448200221200, // c4 (shift 54)
    0x0201002100100008, // d4 (shift 54)
    0x0206080080040082, // e4 (shift 54)
    0x0102020080800400, // f4 (shift 54)
    0x1000040101000200, // g4 (shift 54)
    0x40C0008200240041, // h4 (shift 53)
    0x020281C0048000E4, // a5 (shift 53)
    0x0060005002400020, // b5 (shift 54)
    0x0010002800200400, // c5 (shift 54)
    0x000040120200200A, // d5 (shift 54)
    0x1000040801001100, // e5 (shift 54)
    0x0862810200800400, // f5 (shift 54)
    0x8204101234000128, // g5 (shift 54)
    0x004A009C42000405, // h5 (shift 53)
    0x0100904002218000, // a6 (shift 53)
    0x0020003000404000, // b6 (shift 54)
    0x9020001000208080, // c6 (shift 54)
    0x0405049000090020, // d6 (shift 54)
    0x0400080100050010, // e6 (shift 54)
    0x1020020004008080, // f6 (shift 54)
    0x6428481082040001, // g6 (shift 54)
    0x4832004099060014, // h6 (shift 53)
    0x0008A20045028200, // a7 (shift 53)
    0x2002010040288200, // b7 (shift 54)
    0x0000100020008080, // c7 (shift 54)
    0x0104082100100100, // d7 (shift 54)
    0x0080800400080080, // e7 (shift 54)
    0x0102040002008080, // f7 (shift 54)
    0x0070F00201082400, // g7 (shift 54)
    0x0002800100006080, // h7 (shift 53)
    0x0002108009022043, // a8 (shift 52)
    0x104040001081002D, // b8 (shift 53)
    0xE086008141100822, // c8 (shift 53)
    0x00020D1000090061, // d8 (shift 53)
    0x0202001008042002, // e8 (shift 53)
    0x2201000400020801, // f8 (shift 53)
    0x0000010802500084, // g8 (shift 53)
    0x0000811400408426, // h8 (shift 52)
];

pub const BISHOP_MAGICS: [u64; 64] = [
    0x0202A00101010104, // a1 (shift 58)
    0x20C2300400808480, // b1 (shift 59)
    0x2004010A0E138002, // c1 (shift 59)
    0xB104051201100080, // d1 (shift 59)
    0x0104042100000000, // e1 (shift 59)
    0x0080901048010014, // f1 (shift 59)
    0x000A180108880090, // g1 (shift 59)
    0xA044410090012010, // h1 (shift 58)
    0x0010080890208600, // a2 (shift 59)
    0x0100085800840250, // b2 (shift 59)
    0x0006100092004020, // c2 (shift 59)
    0x6004840408808220, // d2 (shift 59)
    0x02A0020210009402, // e2 (shift 59)
    0xA038810120100080, // f2 (shift 59)
    0x4180240202422000, // g2 (shift 59)
    0x80101A0062080400, // h2 (shift 59)
    0x0410424024082880, // a3 (shift 59)
    0x0010A00404208400, // b3 (shift 59)
    0x8024841004220041, // c3 (shift 57)
    0x012020220208C000, // d3 (shift 57)
    0x0044040202110800, // e3 (shift 57)
    0x0902000822102200, // f3 (shift 57)
    0x0080401084104808, // g3 (shift 59)
    0x0241087040482404, // h3 (shift 59)
    0x0020608010120A20, // a4 (shift 59)
    0x0C02080010109080, // b4 (shift 59)
    0x1040480090062041, // c4 (shift 57)
    0x2008080090820002, // d4 (shift 55)
    0x8081010000104000, // e4 (shift 55)
    0x0211020201080100, // f4 (shift 57)
    0x0809162400421002, // g4 (shift 59)
    0xE000410000840104, // h4 (shift 59)
    0x1028205040090228, // a5 (shift 59)
    0x0002414400A03802, // b5 (shift 59)
    0xC082008200102024, // c5 (shift 57)
    0x8000040400380120, // d5 (shift 55)
    0x0000408020020200, // e5 (shift 55)
    0x2020211040120800, // f5 (shift 57)
    0xAA70310500320098, // g5 (shift 59)
    0x802801110A002088, // h5 (shift 59)
    0x0405180840022434, // a6 (shift 59)
    0x0000841002300808, // b6 (shift 59)
    0x0210510801001802, // c6 (shift 57)
    0x2080004200800802, // d6 (shift 57)
    0x0080AA00A2002400, // e6 (shift 57)
    0x2040500048800040, // f6 (shift 57)
    0x0050018101000400, // g6 (shift 59)
    0x0002423408210100, // h6 (shift 59)
    0x0007010821048A20, // a7 (shift 59)
    0x0000320824040108, // b7 (shift 59)
    0x4102064202410002, // c7 (shift 59)
    0x0008C00020884000, // d7 (shift 59)
    0x0000000505040030, // e7 (shift 59)
    0x0000400821810020, // f7 (shift 59)
    0x5204202812008000, // g7 (shift 59)
    0x040449180A058000, // h7 (shift 59)
    0x4200242504104004, // a8 (shift 58)
    0x0400010301100380, // b8 (shift 59)
    0x0004000B04150400, // c8 (shift 59)
    0x2081440028411080, // d8 (shift 59)
    0x1022028440082200, // e8 (shift 59)
    0x0400001020010440, // f8 (shift 59)
    0x0080048408084100, // g8 (shift 59)
    0x0008011116120200, // h8 (shift 58)
];

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
    let mask = if is_rook { rook_mask(sq) } else { bishop_mask(sq) };
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
    MagicEntry { mask, magic, shift, table }
}

pub struct MagicTables {
    rook: Vec<MagicEntry>,
    bishop: Vec<MagicEntry>,
}

impl MagicTables {
    fn new() -> Self {
        let rook = (0u8..64).map(|sq| build_table(sq, ROOK_MAGICS[sq as usize], true)).collect();
        let bishop = (0u8..64).map(|sq| build_table(sq, BISHOP_MAGICS[sq as usize], false)).collect();
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
        for &sq in &[crate::types::A1, crate::types::H1, crate::types::A8, crate::types::H8] {
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
