//! Herramienta de desarrollo, NO forma parte del motor en tiempo de
//! ejecución: busca números mágicos propios para las piezas deslizantes
//! (torre y alfil) en las 64 casillas, y los imprime en formato Rust listo
//! para copiar a `src/magic.rs`.
//!
//! Se incluye en el repositorio para que cualquiera pueda reproducir la
//! búsqueda de forma independiente y confirmar que estos números no fueron
//! copiados de ningún otro motor — son autocontenidos a propósito (no
//! dependen de los módulos del motor) para que este archivo se pueda leer y
//! auditar sin tener que revisar el resto del código base.
//!
//! Uso: cargo run --release --bin find_magics

type Bitboard = u64;
type Square = u8;

const EMPTY: Bitboard = 0;

fn file_of(sq: Square) -> u8 {
    sq % 8
}
fn rank_of(sq: Square) -> u8 {
    sq / 8
}
fn make_square(file: u8, rank: u8) -> Square {
    rank * 8 + file
}

/// Genera el rayo completo (sin la casilla de origen) desde `sq` en la
/// dirección (df, dr), hasta el borde del tablero. Idéntica en espíritu a
/// la función homónima ya usada y validada en `src/bitboard.rs` (perft
/// exacto contra valores de referencia), reimplementada aquí para que esta
/// herramienta sea 100% autocontenida.
fn gen_ray(sq: Square, df: i32, dr: i32) -> Bitboard {
    let mut bb = EMPTY;
    let mut f = file_of(sq) as i32 + df;
    let mut r = rank_of(sq) as i32 + dr;
    while (0..8).contains(&f) && (0..8).contains(&r) {
        bb |= 1u64 << make_square(f as u8, r as u8);
        f += df;
        r += dr;
    }
    bb
}

/// Oráculo de rayos: recorta un rayo completo en el primer bloqueador según
/// `occ`. Es la misma técnica ya usada en `src/bitboard.rs`; sirve aquí como
/// "verdad de referencia" contra la que se valida cada número mágico candidato.
fn slide(sq: Square, df: i32, dr: i32, occ: Bitboard) -> Bitboard {
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

/// Quita el bit más alto (si `increasing`) o más bajo (si no) de un rayo:
/// ese bit es siempre la casilla justo en el borde del tablero para esa
/// dirección, y por construcción nunca cambia el resultado del ataque
/// (el rayo llega hasta ahí exista o no una pieza bloqueando exactamente
/// en esa casilla límite), así que no aporta información relevante como
/// bloqueador y se excluye de la máscara.
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
    trim_edge(gen_ray(sq, 0, 1), true)   // N
        | trim_edge(gen_ray(sq, 0, -1), false) // S
        | trim_edge(gen_ray(sq, 1, 0), true)   // E
        | trim_edge(gen_ray(sq, -1, 0), false) // W
}

fn bishop_mask(sq: Square) -> Bitboard {
    trim_edge(gen_ray(sq, 1, 1), true)    // NE (delta +9)
        | trim_edge(gen_ray(sq, -1, 1), true)  // NW (delta +7)
        | trim_edge(gen_ray(sq, 1, -1), false) // SE (delta -7)
        | trim_edge(gen_ray(sq, -1, -1), false) // SW (delta -9)
}

/// Extrae el subconjunto número `index` (0..2^popcount(mask)) de `mask`,
/// técnica estándar "enumerar subconjuntos de una máscara de bits".
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
    /// Número disperso (pocos bits en 1): empíricamente encuentra buenos
    /// números mágicos mucho más rápido que un u64 uniformemente aleatorio.
    fn sparse(&mut self) -> u64 {
        self.next() & self.next() & self.next()
    }
}

fn find_magic(sq: Square, is_rook: bool, rng: &mut XorShift64) -> (u64, u32) {
    let mask = if is_rook { rook_mask(sq) } else { bishop_mask(sq) };
    let bits = mask.count_ones();
    let shift = 64 - bits;
    let size = 1usize << bits;

    let mut occupancies = Vec::with_capacity(size);
    let mut true_attacks = Vec::with_capacity(size);
    for i in 0..size {
        let occ = subset_from_index(i, mask);
        occupancies.push(occ);
        true_attacks.push(if is_rook {
            rook_attacks_oracle(sq, occ)
        } else {
            bishop_attacks_oracle(sq, occ)
        });
    }

    loop {
        let candidate = rng.sparse();
        // Filtro barato: un buen mágico debe distribuir bien los bits altos
        // al multiplicar por la máscara completa.
        if ((mask.wrapping_mul(candidate)) >> 56).count_ones() < 6 {
            continue;
        }

        let mut table: Vec<Option<Bitboard>> = vec![None; size];
        let mut ok = true;
        for i in 0..size {
            let idx = ((occupancies[i].wrapping_mul(candidate)) >> shift) as usize;
            match table[idx] {
                None => table[idx] = Some(true_attacks[i]),
                Some(existing) if existing == true_attacks[i] => {} // colisión inofensiva
                Some(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return (candidate, shift);
        }
    }
}

fn square_name(sq: Square) -> String {
    let file = (b'a' + file_of(sq)) as char;
    let rank = (b'1' + rank_of(sq)) as char;
    format!("{file}{rank}")
}

fn main() {
    let mut rng = XorShift64(0xC0FFEE_D15EA5E_u64 ^ 0x9E3779B97F4A7C15);

    println!("// Generado por `cargo run --release --bin find_magics`.");
    println!("// Cada número fue verificado sin colisiones contra un oráculo de rayos");
    println!("// independiente (implementado en este mismo archivo) para las 2^n");
    println!("// ocupaciones relevantes de esa casilla, n = bits de la máscara.");
    println!();
    println!("pub const ROOK_MAGICS: [u64; 64] = [");
    for sq in 0u8..64 {
        let (magic, shift) = find_magic(sq, true, &mut rng);
        println!("    0x{magic:016X}, // {} (shift {shift})", square_name(sq));
    }
    println!("];");
    println!();
    println!("pub const BISHOP_MAGICS: [u64; 64] = [");
    for sq in 0u8..64 {
        let (magic, shift) = find_magic(sq, false, &mut rng);
        println!("    0x{magic:016X}, // {} (shift {shift})", square_name(sq));
    }
    println!("];");
}
