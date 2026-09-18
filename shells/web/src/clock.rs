//! performance.now clock (D2): milliseconds since the shell's start point.
//!
//! Feeds only DG_GetTicksMs (the engine heartbeat), never simulation state
//! (D6 determinism guard).

use std::cell::Cell;

thread_local! {
    static START_MS: Cell<f64> = const { Cell::new(0.0) };
}

/// Call once, before doomgeneric_Create.
pub fn init_start_time() {
    START_MS.with(|t| t.set(performance_now()));
}

/// Engine heartbeat clock: milliseconds since start (u32 truncation = same
/// behavior as the native Instant).
pub fn now_ms() -> u32 {
    START_MS.with(|t| (performance_now() - t.get()) as u32)
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
