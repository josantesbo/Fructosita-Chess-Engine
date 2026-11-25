//! Chess960 (Fischer Random): modo UCI y posiciones iniciales.
//!
//! Las reglas viven en `board.rs` y `movegen.rs` y son las mismas en los dos
//! modos: el tablero guarda la casilla de la torre de cada derecho de enroque,
//! así que el ajedrez estándar es el caso particular rey en e1, torres en a1 y
//! h1. Este módulo solo guarda el modo de notación pedido por la interfaz
//! (`UCI_Chess960`) y numera las 960 posiciones iniciales.

use std::sync::atomic::{AtomicBool, Ordering};

static UCI_CHESS960: AtomicBool = AtomicBool::new(false);

/// ¿Pidió la interfaz notación Chess960 (`setoption name UCI_Chess960 value true`)?
#[inline]
pub fn enabled() -> bool {
    UCI_CHESS960.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    UCI_CHESS960.store(on, Ordering::Relaxed);
}

/// Fila de piezas blancas (columnas a..h) de la posición inicial número `n`
/// (0..960) en la numeración de Scharnagl; la 518 es la del ajedrez estándar.
///
/// El número se descompone en cuatro cifras: alfil de casillas claras
/// (n mod 4 → b, d, f, h), alfil de casillas oscuras (→ a, c, e, g), dama
/// (0..6: la casilla libre que le toca, contando desde la columna a) y un
/// código 0..10 que reparte los dos caballos entre las cinco casillas que
/// quedan; las tres últimas reciben torre, rey y torre en ese orden, lo que
/// deja siempre al rey entre las torres.
pub fn start_rank(n: u16) -> Option<[char; 8]> {
    if n >= 960 {
        return None;
    }
    let mut rank = [' '; 8];
    let n = n as usize;
    rank[1 + 2 * (n % 4)] = 'B';
    let n = n / 4;
    rank[2 * (n % 4)] = 'B';
    let n = n / 4;
    place_nth_free(&mut rank, n % 6, 'Q');
    // Los dos caballos: pares (i, j), i < j, en orden lexicográfico.
    let mut code = n / 6;
    let mut first = 0;
    let mut span = 4;
    while code >= span {
        code -= span;
        first += 1;
        span -= 1;
    }
    let second = first + 1 + code;
    // Se coloca primero el de más a la derecha para no desplazar el índice del otro.
    place_nth_free(&mut rank, second, 'N');
    place_nth_free(&mut rank, first, 'N');
    for piece in ['R', 'K', 'R'] {
        place_nth_free(&mut rank, 0, piece);
    }
    Some(rank)
}

fn place_nth_free(rank: &mut [char; 8], nth: usize, piece: char) {
    let mut seen = 0;
    for slot in rank.iter_mut() {
        if *slot == ' ' {
            if seen == nth {
                *slot = piece;
                return;
            }
            seen += 1;
        }
    }
}

/// FEN de la posición inicial `n`, con los derechos de enroque escritos con
/// la columna de cada torre (Shredder-FEN), que nunca es ambiguo.
pub fn start_fen(n: u16) -> Option<String> {
    let rank = start_rank(n)?;
    let white: String = rank.iter().collect();
    let black = white.to_ascii_lowercase();
    let mut rooks = rank
        .iter()
        .enumerate()
        .filter(|(_, &c)| c == 'R')
        .map(|(f, _)| (b'A' + f as u8) as char);
    let (q, k) = (rooks.next()?, rooks.next()?);
    Some(format!(
        "{black}/pppppppp/8/8/8/8/PPPPPPPP/{white} w {k}{q}{}{} - 0 1",
        k.to_ascii_lowercase(),
        q.to_ascii_lowercase()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank_str(n: u16) -> String {
        start_rank(n).unwrap().iter().collect()
    }

    #[test]
    fn known_positions() {
        // Referencias públicas de la numeración de Scharnagl.
        assert_eq!(rank_str(0), "BBQNNRKR");
        assert_eq!(rank_str(518), "RNBQKBNR");
        assert_eq!(rank_str(959), "RKRNNQBB");
        assert!(start_rank(960).is_none());
    }

    #[test]
    fn all_960_are_distinct_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for n in 0..960 {
            let r = start_rank(n).unwrap();
            let s: String = r.iter().collect();
            assert!(seen.insert(s.clone()), "duplicada: {n} {s}");
            let count = |c| r.iter().filter(|&&x| x == c).count();
            assert_eq!((count('K'), count('Q'), count('R'), count('B'), count('N')), (1, 1, 2, 2, 2), "{s}");
            let files = |c| r.iter().enumerate().filter(|(_, &x)| x == c).map(|(f, _)| f).collect::<Vec<_>>();
            let b = files('B');
            assert_ne!(b[0] % 2, b[1] % 2, "alfiles del mismo color: {s}");
            let (rk, k) = (files('R'), files('K')[0]);
            assert!(rk[0] < k && k < rk[1], "rey fuera de las torres: {s}");
        }
        assert_eq!(seen.len(), 960);
    }

    /// Las 960 posiciones contra una lista generada por una implementación
    /// independiente (python-chess, `Board.from_chess960_pos`); ver
    /// `testdata/chess960_startpos.txt`.
    #[test]
    fn matches_independent_reference() {
        let text = include_str!("../testdata/chess960_startpos.txt");
        let mut n = 0u16;
        for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let mut it = line.split_whitespace();
            let idx: u16 = it.next().unwrap().parse().unwrap();
            let fen: Vec<&str> = it.collect();
            assert_eq!(idx, n);
            let ours = start_fen(n).unwrap();
            assert_eq!(ours.split('/').next().unwrap(), fen[0].split('/').next().unwrap(), "posición {n}");
            // El FEN propio se relee y conserva piezas y los cuatro derechos.
            let b = crate::board::Board::from_fen(&ours).unwrap();
            let c = b.castling;
            assert!(c.white_kingside && c.white_queenside && c.black_kingside && c.black_queenside, "{n}");
            // Y el de referencia (X-FEN, "KQkq") describe la misma posición.
            let r = crate::board::Board::from_fen(&fen.join(" ")).unwrap();
            assert_eq!(b.hash, r.hash, "posición {n}");
            assert_eq!(b.castle_rooks, r.castle_rooks, "posición {n}");
            n += 1;
        }
        assert_eq!(n, 960);
    }
}
