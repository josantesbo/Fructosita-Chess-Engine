//! `Score`: par (mediojuego, final) que cada término de evaluación
//! devuelve, con aritmética propia (suma, resta, negación) para que el
//! orquestador (`evaluate()` en `mod.rs`) pueda combinar términos sin tener
//! que hilar manualmente dos variables sueltas (`_mg`, `_eg`) por cada uno.
//!
//! Es el mismo idioma que usa prácticamente todo motor HCE fuerte conocido
//! (Stockfish lo llama igual; Ethereal/Weiss lo empaquetan en un solo `i32`
//! por velocidad) — no está copiado de ninguno, es la forma obvia de
//! resolver "cada término aporta un valor de mediojuego y uno de final" sin
//! que la lista de términos se vuelva un muro de tuplas al crecer.

use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub mg: i32,
    pub eg: i32,
}

impl Score {
    pub const ZERO: Score = Score { mg: 0, eg: 0 };

    pub const fn new(mg: i32, eg: i32) -> Score {
        Score { mg, eg }
    }

    /// Colapsa el par (mg, eg) a un único valor de centipeones,
    /// interpolando según la fase de la partida: `phase == max_phase` es
    /// mediojuego puro, `phase == 0` es final puro.
    pub fn interpolate(self, phase: i32, max_phase: i32) -> i32 {
        (self.mg * phase + self.eg * (max_phase - phase)) / max_phase
    }
}

impl Add for Score {
    type Output = Score;
    fn add(self, rhs: Score) -> Score {
        Score::new(self.mg + rhs.mg, self.eg + rhs.eg)
    }
}

impl AddAssign for Score {
    fn add_assign(&mut self, rhs: Score) {
        self.mg += rhs.mg;
        self.eg += rhs.eg;
    }
}

impl Sub for Score {
    type Output = Score;
    fn sub(self, rhs: Score) -> Score {
        Score::new(self.mg - rhs.mg, self.eg - rhs.eg)
    }
}

impl SubAssign for Score {
    fn sub_assign(&mut self, rhs: Score) {
        self.mg -= rhs.mg;
        self.eg -= rhs.eg;
    }
}

impl Neg for Score {
    type Output = Score;
    fn neg(self) -> Score {
        Score::new(-self.mg, -self.eg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_sub_are_componentwise() {
        let a = Score::new(10, -5);
        let b = Score::new(3, 7);
        assert_eq!(a + b, Score::new(13, 2));
        assert_eq!(a - b, Score::new(7, -12));
    }

    #[test]
    fn add_assign_matches_add() {
        let mut a = Score::new(1, 2);
        a += Score::new(3, 4);
        assert_eq!(a, Score::new(4, 6));
    }

    #[test]
    fn neg_flips_both_components() {
        assert_eq!(-Score::new(5, -8), Score::new(-5, 8));
    }

    #[test]
    fn interpolate_at_the_extremes_picks_one_side() {
        let s = Score::new(100, 20);
        assert_eq!(s.interpolate(24, 24), 100); // fase máxima: mediojuego puro
        assert_eq!(s.interpolate(0, 24), 20); // fase 0: final puro
    }

    #[test]
    fn interpolate_matches_manual_formula() {
        let s = Score::new(100, 20);
        let phase = 10;
        let max_phase = 24;
        let expected = (100 * phase + 20 * (max_phase - phase)) / max_phase;
        assert_eq!(s.interpolate(phase, max_phase), expected);
    }
}
