//! Gestión determinista de tiempo para comandos UCI `go`.
//!
//! Este módulo calcula presupuestos relativos en milisegundos. No consulta el
//! reloj de pared ni crea `Instant`; esa conversión queda en el punto de
//! integración UCI para que la lógica sea pura y testeable.

use crate::types::Color;

const DEFAULT_MOVES_TO_GO: u64 = 30;
const CLOCK_SAFETY_MS: u64 = 50;
const MIN_CLOCK_ALLOCATION_MS: u64 = 1;
const MOVETIME_MARGIN_MS: u64 = 5;
const HARD_MULTIPLIER: u64 = 3;
const INCREMENT_NUMERATOR: u64 = 4;
const INCREMENT_DENOMINATOR: u64 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeControlInput {
    pub side_to_move: Color,
    pub wtime_ms: Option<u64>,
    pub btime_ms: Option<u64>,
    pub winc_ms: Option<u64>,
    pub binc_ms: Option<u64>,
    pub movestogo: Option<u64>,
    pub movetime_ms: Option<u64>,
    pub depth: Option<i32>,
    pub infinite: bool,
    pub ponder: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeAllocationReason {
    MoveTime,
    Clock,
    DepthOnly,
    InfiniteOrPonder,
    NoClock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeAllocation {
    pub soft_ms: Option<u64>,
    pub hard_ms: Option<u64>,
    pub reason: TimeAllocationReason,
}

pub fn allocate_time(input: TimeControlInput) -> TimeAllocation {
    if input.infinite || input.ponder {
        return no_deadline(TimeAllocationReason::InfiniteOrPonder);
    }

    if let Some(movetime_ms) = input.movetime_ms {
        let soft_ms = movetime_ms
            .saturating_sub(MOVETIME_MARGIN_MS)
            .max(1)
            .min(movetime_ms);
        return TimeAllocation {
            soft_ms: Some(soft_ms),
            hard_ms: Some(movetime_ms),
            reason: TimeAllocationReason::MoveTime,
        };
    }

    let (time_left_ms, increment_ms) = match input.side_to_move {
        Color::White => (input.wtime_ms, input.winc_ms.unwrap_or(0)),
        Color::Black => (input.btime_ms, input.binc_ms.unwrap_or(0)),
    };

    if let Some(time_left_ms) = time_left_ms {
        return allocate_clock_time(time_left_ms, increment_ms, input.movestogo);
    }

    if input.depth.is_some() {
        return no_deadline(TimeAllocationReason::DepthOnly);
    }

    no_deadline(TimeAllocationReason::NoClock)
}

fn allocate_clock_time(
    time_left_ms: u64,
    increment_ms: u64,
    movestogo: Option<u64>,
) -> TimeAllocation {
    let reserve_ms = CLOCK_SAFETY_MS.min(time_left_ms.saturating_sub(MIN_CLOCK_ALLOCATION_MS));
    let usable_ms = time_left_ms.saturating_sub(reserve_ms);
    let moves_left = movestogo.unwrap_or(DEFAULT_MOVES_TO_GO).max(1);
    let base_ms = usable_ms / moves_left;
    let increment_budget_ms = increment_ms
        .saturating_mul(INCREMENT_NUMERATOR)
        .checked_div(INCREMENT_DENOMINATOR)
        .unwrap_or(0);
    let soft_ms = if usable_ms == 0 {
        0
    } else {
        base_ms
            .saturating_add(increment_budget_ms)
            .clamp(MIN_CLOCK_ALLOCATION_MS, usable_ms)
    };
    let hard_ms = soft_ms
        .saturating_mul(HARD_MULTIPLIER)
        .min(usable_ms)
        .max(soft_ms);

    TimeAllocation {
        soft_ms: Some(soft_ms),
        hard_ms: Some(hard_ms),
        reason: TimeAllocationReason::Clock,
    }
}

fn no_deadline(reason: TimeAllocationReason) -> TimeAllocation {
    TimeAllocation {
        soft_ms: None,
        hard_ms: None,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> TimeControlInput {
        TimeControlInput {
            side_to_move: Color::White,
            wtime_ms: None,
            btime_ms: None,
            winc_ms: None,
            binc_ms: None,
            movestogo: None,
            movetime_ms: None,
            depth: None,
            infinite: false,
            ponder: false,
        }
    }

    fn assert_ordered(allocation: TimeAllocation) {
        if let (Some(soft), Some(hard)) = (allocation.soft_ms, allocation.hard_ms) {
            assert!(hard >= soft);
        }
    }

    #[test]
    fn movetime_1000_keeps_hard_within_command_time() {
        let allocation = allocate_time(TimeControlInput {
            movetime_ms: Some(1000),
            ..base_input()
        });
        assert_eq!(allocation.reason, TimeAllocationReason::MoveTime);
        assert!(allocation.hard_ms.unwrap() <= 1000);
        assert!(allocation.soft_ms.unwrap() <= allocation.hard_ms.unwrap());
        assert!(allocation.soft_ms.unwrap() > 0);
    }

    #[test]
    fn tiny_movetime_does_not_underflow_or_exceed_command_time() {
        for movetime_ms in [1, 5] {
            let allocation = allocate_time(TimeControlInput {
                movetime_ms: Some(movetime_ms),
                ..base_input()
            });
            assert_eq!(allocation.soft_ms, Some(movetime_ms.min(1)));
            assert!(allocation.hard_ms.unwrap() <= movetime_ms);
            assert_ordered(allocation);
        }
    }

    #[test]
    fn clock_with_movestogo_does_not_consume_whole_clock() {
        let allocation = allocate_time(TimeControlInput {
            wtime_ms: Some(60_000),
            winc_ms: Some(0),
            movestogo: Some(30),
            ..base_input()
        });
        assert_eq!(allocation.reason, TimeAllocationReason::Clock);
        assert!(allocation.soft_ms.unwrap() > 0);
        assert!(allocation.hard_ms.unwrap() < 60_000);
        assert_ordered(allocation);
    }

    #[test]
    fn clock_without_movestogo_uses_conservative_estimate_and_increment() {
        let allocation = allocate_time(TimeControlInput {
            wtime_ms: Some(60_000),
            winc_ms: Some(1_000),
            ..base_input()
        });
        assert_eq!(allocation.soft_ms, Some(2_798));
        assert!(allocation.hard_ms.unwrap() < 60_000);
        assert_ordered(allocation);
    }

    #[test]
    fn low_clock_leaves_safety_margin_when_possible() {
        let allocation = allocate_time(TimeControlInput {
            wtime_ms: Some(200),
            winc_ms: Some(0),
            ..base_input()
        });
        assert!(allocation.hard_ms.unwrap() < 200);
        assert!(allocation.soft_ms.unwrap() > 0);
        assert_ordered(allocation);
    }

    #[test]
    fn zero_clock_does_not_panic_or_overflow() {
        let allocation = allocate_time(TimeControlInput {
            wtime_ms: Some(0),
            winc_ms: Some(0),
            ..base_input()
        });
        assert_eq!(allocation.reason, TimeAllocationReason::Clock);
        assert_eq!(allocation.soft_ms, Some(0));
        assert_eq!(allocation.hard_ms, Some(0));
        assert_ordered(allocation);
    }

    #[test]
    fn black_uses_black_clock_and_increment() {
        let allocation = allocate_time(TimeControlInput {
            side_to_move: Color::Black,
            wtime_ms: Some(60_000),
            btime_ms: Some(9_000),
            winc_ms: Some(5_000),
            binc_ms: Some(100),
            movestogo: Some(9),
            ..base_input()
        });
        assert_eq!(allocation.soft_ms, Some(1_074));
        assert!(allocation.hard_ms.unwrap() < 9_000);
        assert_ordered(allocation);
    }

    #[test]
    fn depth_only_has_no_artificial_deadline() {
        let allocation = allocate_time(TimeControlInput {
            depth: Some(5),
            ..base_input()
        });
        assert_eq!(allocation, no_deadline(TimeAllocationReason::DepthOnly));
    }

    #[test]
    fn no_clock_has_stable_no_deadline_result() {
        let allocation = allocate_time(base_input());
        assert_eq!(allocation, no_deadline(TimeAllocationReason::NoClock));
    }

    #[test]
    fn infinite_and_ponder_have_no_artificial_deadline() {
        for input in [
            TimeControlInput {
                infinite: true,
                ..base_input()
            },
            TimeControlInput {
                ponder: true,
                ..base_input()
            },
        ] {
            assert_eq!(
                allocate_time(input),
                no_deadline(TimeAllocationReason::InfiniteOrPonder)
            );
        }
    }

    #[test]
    fn same_input_produces_same_allocation() {
        let input = TimeControlInput {
            wtime_ms: Some(60_000),
            winc_ms: Some(1_000),
            movestogo: Some(30),
            ..base_input()
        };
        assert_eq!(allocate_time(input), allocate_time(input));
    }
}
