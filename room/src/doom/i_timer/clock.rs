//! The game clock: the `BASETIME` latch and the tic/millisecond mappers
//! built on the host hook `DG_GetTicksMs`.

#![allow(non_snake_case)]

use std::ffi::c_int;

use super::host::DG_GetTicksMs;

/// Game tick rate in Hz. Doom updates state at 35 tics per second; all
/// gameplay code expresses durations in multiples of this constant.
pub const TICRATE: c_int = 35;

/// Wall-clock millisecond timestamp captured on the first call to either
/// `I_GetTime` or `I_GetTimeMS`. All subsequent results are deltas relative
/// to this baseline so the game clock starts at zero. Mirrors the C
/// `basetime` static in `i_timer.c`.
static mut BASETIME: u32 = 0;

/// Return the time since game start measured in 1/35-second game tics.
///
/// On the first call, the current millisecond counter is recorded in
/// `BASETIME` so all subsequent calls return `(now - BASETIME) * TICRATE /
/// 1000`. Used throughout the game loop to drive tic-based timing.
///
/// Note: the C original uses `basetime == 0` as a not-yet-initialised
/// sentinel, which is preserved here. If the host's first tick value
/// happens to be exactly 0 the baseline is captured on the next call;
/// chocolate-doom relies on the same behaviour.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs` extern-declares it and the freeze-zone callers import
/// the upstream name through the root shim.
#[doc(alias = "I_GetTime")]
#[export_name = "I_GetTime"]
pub extern "C" fn get_time_tics() -> c_int {
    unsafe {
        let ticks = DG_GetTicksMs();
        if BASETIME == 0 {
            BASETIME = ticks;
        }
        ((ticks - BASETIME) * TICRATE as u32 / 1000) as c_int
    }
}

/// Return the time since game start in milliseconds.
///
/// Same baseline logic as `I_GetTime` but without converting to tics. Used
/// by code paths that need sub-tic precision (e.g. mouse polling).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs` extern-declares it and `shells/web/src/lib.rs` calls the
/// upstream path through the root shim.
#[doc(alias = "I_GetTimeMS")]
#[export_name = "I_GetTimeMS"]
pub extern "C" fn get_time_ms() -> c_int {
    unsafe {
        let ticks = DG_GetTicksMs();
        if BASETIME == 0 {
            BASETIME = ticks;
        }
        (ticks - BASETIME) as c_int
    }
}

/// Convert a raw `DG_GetTicksMs`-domain millisecond value to engine-relative
/// milliseconds — the same mapping `I_GetTimeMS` applies to its own clock
/// read. Read-only: unlike `I_GetTime`/`I_GetTimeMS` this never latches
/// `BASETIME` (the engine's boot path always latches it first) and never
/// queries the clock itself, so the frame/pump split can derive the render
/// fraction from the shell-supplied `now_ms` without a second time source.
/// Returns 0 while `BASETIME` is unset.
pub fn elapsed_ms_from(now_ms: u32) -> u32 {
    unsafe {
        let base = std::ptr::addr_of!(BASETIME).read();
        if base == 0 { 0 } else { now_ms.wrapping_sub(base) }
    }
}

#[cfg(test)]
mod tests {
    use super::BASETIME;
    use super::elapsed_ms_from;
    use std::ptr::addr_of;

    /// `elapsed_ms_from` is the read stage of the BASETIME contract: it maps
    /// its own baseline back to 0 on every call, returns the same result for
    /// the same input, and never mutates `BASETIME` (no latch, no clock
    /// read). Deterministic regardless of whether another test in this
    /// binary has already latched the clock.
    #[test]
    fn elapsed_ms_from_is_read_only() {
        let base_before = unsafe { addr_of!(BASETIME).read() };
        let a = elapsed_ms_from(base_before);
        let b = elapsed_ms_from(base_before);
        let base_after = unsafe { addr_of!(BASETIME).read() };
        assert_eq!(a, 0);
        assert_eq!(a, b);
        assert_eq!(base_after, base_before);
    }
}
