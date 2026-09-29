//! The host-hook surface: the `DG_GetTicksMs` / `DG_SleepMs` extern block
//! (reverse contract, carried verbatim) and the sleep / vbl / init glue.

use std::ffi::c_int;

extern "C" {
    /// Host-provided clock: returns wall-clock milliseconds. Implemented per
    /// platform in the doomgeneric backend (e.g. SDL, raw POSIX).
    ///
    //* Reverse contract: implemented by both shells
    //* (`shells/web/src/dg.rs`, `shells/native/src/platform.rs`); never
    //* re-pointed at a Rust path.
    pub(super) fn DG_GetTicksMs() -> u32;
    /// Host-provided sleep: blocks for approximately `ms` milliseconds.
    pub(super) fn DG_SleepMs(ms: u32);
}

/// Return the host's raw millisecond tick counter (not relative to game
/// start). Thin wrapper around `DG_GetTicksMs`.
///
/// Called from a few places that need wall-clock deltas independent of the
/// game tic clock.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_GetTicks")]
#[export_name = "I_GetTicks"]
pub extern "C" fn host_ticks() -> c_int {
    unsafe { DG_GetTicksMs() as c_int }
}

/// Sleep for approximately `ms` milliseconds by delegating to the host
/// `DG_SleepMs` hook. The actual resolution depends on the backend.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs` extern-declares it.
#[doc(alias = "I_Sleep")]
#[export_name = "I_Sleep"]
pub extern "C" fn host_sleep_ms(ms: c_int) {
    unsafe { DG_SleepMs(ms as u32) }
}

/// No-op port of the original vertical-blank wait. The chocolate-doom and
/// doomgeneric C versions are both empty (the original would have called
/// `I_Sleep((count * 1000) / 70)`); the Rust port preserves the no-op so
/// timing matches.
///
/// Dead-but-exported (zero callers in the tree beyond `m_menu`'s root-shim
/// path call): kept for symbol-set byte-identity, retires with the freeze
/// zone. The pre-move export symbol is kept with `#[export_name]` below.
#[doc(alias = "I_WaitVBL")]
#[export_name = "I_WaitVBL"]
pub extern "C" fn wait_vbl(_count: c_int) {}

/// Initialise the timer subsystem. The C version originally called
/// `SDL_Init(SDL_INIT_TIMER)`; doomgeneric drops that and the Rust port
/// follows suit, leaving the function as a no-op kept for ABI parity.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` calls the upstream path through the root shim.
#[doc(alias = "I_InitTimer")]
#[export_name = "I_InitTimer"]
pub extern "C" fn init_timer() {}
