//! G19: parámetros de búsqueda ajustables. EXP-0066: los valores por defecto son
//! la media de θ de las últimas 60 iteraciones del SPSA (results/SPSA-G19B).
//! Parámetros en tiempo de ejecución (opciones UCI
//! `spin`), para afinarlos con SPSA sin recompilar. Con los valores por
//! defecto el motor es idéntico al compilado con constantes (misma firma de
//! bench). Lectura con `Ordering::Relaxed`: solo cambian entre búsquedas.

use std::sync::atomic::{AtomicI32, Ordering};

pub struct Param {
    pub name: &'static str,
    pub default: i32,
    pub min: i32,
    pub max: i32,
}

macro_rules! params {
    ($($idx:ident = $name:literal, $def:expr, $min:expr, $max:expr;)*) => {
        pub const PARAMS: &[Param] = &[$(Param { name: $name, default: $def, min: $min, max: $max },)*];
        static VALUES: [AtomicI32; PARAMS.len()] = [$(AtomicI32::new($def),)*];
        #[allow(non_camel_case_types, dead_code)]
        #[repr(usize)]
        enum Idx { $($idx,)* }
        $(pub const $idx: usize = Idx::$idx as usize;)*
    };
}

params! {
    RFP_MARGIN = "RfpMargin", 76, 30, 160;
    RFP_IMPROVING_BONUS = "RfpImproving", 24, 0, 80;
    FUTILITY_BASE = "FutBase", 96, 20, 250;
    FUTILITY_STEP = "FutStep", 103, 30, 250;
    FUTILITY_IMPROVING_BONUS = "FutImproving", 53, 0, 150;
    LMP_BASE = "LmpBase", 4, 1, 8;
    LMR_BASE_X100 = "LmrBase", 58, 0, 150;
    LMR_DIV_X100 = "LmrDiv", 253, 150, 500;
    NULL_DIV = "NullDiv", 202, 80, 400;
    RAZOR_MARGIN = "RazorMargin", 229, 100, 500;
    PROBCUT_MARGIN = "ProbcutMargin", 153, 60, 300;
    ASP_DELTA = "AspDelta", 22, 8, 60;
    SEE_PRUNE_MARGIN = "SeePruneMargin", 93, 30, 250;
    DELTA_MARGIN = "DeltaMargin", 224, 50, 400;
    TM_PANIC_DROP = "PanicDrop", 31, 10, 80;
    SE_MIN_DEPTH = "SeMinDepth", 8, 4, 12;
    SE_MARGIN = "SeMargin", 2, 1, 6;
}

#[inline(always)]
pub fn get(i: usize) -> i32 {
    VALUES[i].load(Ordering::Relaxed)
}

/// Asigna por nombre UCI (sin distinguir mayúsculas). Devuelve `true` si existe.
pub fn set(name: &str, value: &str) -> bool {
    for (i, p) in PARAMS.iter().enumerate() {
        if p.name.eq_ignore_ascii_case(name) {
            if let Ok(v) = value.trim().parse::<i32>() {
                VALUES[i].store(v.clamp(p.min, p.max), Ordering::Relaxed);
            }
            return true;
        }
    }
    false
}
