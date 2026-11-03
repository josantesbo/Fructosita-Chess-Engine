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
    RFP_MARGIN = "RfpMargin", 75, 30, 160;
    RFP_IMPROVING_BONUS = "RfpImproving", 17, 0, 80;
    FUTILITY_BASE = "FutBase", 98, 20, 250;
    FUTILITY_STEP = "FutStep", 107, 30, 250;
    FUTILITY_IMPROVING_BONUS = "FutImproving", 52, 0, 150;
    LMP_BASE = "LmpBase", 3, 1, 8;
    LMR_BASE_X100 = "LmrBase", 53, 0, 150;
    LMR_DIV_X100 = "LmrDiv", 244, 150, 500;
    NULL_DIV = "NullDiv", 192, 80, 400;
    RAZOR_MARGIN = "RazorMargin", 222, 100, 500;
    PROBCUT_MARGIN = "ProbcutMargin", 165, 60, 300;
    ASP_DELTA = "AspDelta", 21, 8, 60;
    SEE_PRUNE_MARGIN = "SeePruneMargin", 86, 30, 250;
    DELTA_MARGIN = "DeltaMargin", 240, 50, 400;
    TM_PANIC_DROP = "PanicDrop", 29, 10, 80;
    SE_MIN_DEPTH = "SeMinDepth", 8, 4, 12;
    SE_MARGIN = "SeMargin", 2, 1, 6;
    // S4-002: constantes estructurales que nunca se habían afinado (valor por defecto = el de 3.2.0).
    RFP_MAX_DEPTH = "RfpMaxDepth", 6, 3, 12;
    FUT_MAX_DEPTH = "FutMaxDepth", 5, 2, 10;
    LMP_MAX_DEPTH = "LmpMaxDepth", 5, 2, 10;
    SEE_PRUNE_MAX_DEPTH = "SeePruneMaxDepth", 4, 2, 10;
    IIR_MIN_DEPTH = "IirMinDepth", 4, 2, 10;
    NULL_DEPTH_DIV = "NullDepthDiv", 4, 2, 8;
    LMR_HIST_GOOD = "LmrHistGood", 1119, 100, 20000;
    LMR_HIST_BAD = "LmrHistBad", 77, -5000, 1000;
    TM_MOVES_TO_GO = "TmMovesToGo", 31, 12, 60;
    TM_START_PCT = "TmStartPct", 61, 25, 100;
    TM_HARD_X10 = "TmHardX10", 32, 12, 80;
    TM_INSTAB_12 = "TmInstab12", 17, 12, 36;
    TM_PANIC_12 = "TmPanic12", 19, 12, 36;
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
