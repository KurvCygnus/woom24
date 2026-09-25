//! performance.now clock (D2): milliseconds since the shell's start point.
//!
//! Feeds only DG_GetTicksMs (the engine heartbeat), never simulation state
//! (D6 determinism guard).

use std::cell::Cell;

thread_local!
{
    static START_MS: Cell<f64> = const { Cell::new(0.0) };
    //? DEBUG(probe): div-probe harness clock. Non-browser hosts have no
    //? window(), so performance_now() falls back to js_sys::Date::now().
    //? When the probe override is armed (Node tick harness), now_ms() instead
    //? advances a fake clock: PROBE_BASE plus real elapsed time scaled by
    //? CLOCK_SPEEDUP (so init gaps advance like the browser's slow boot) plus
    //? CLOCK_STEP_MS per call (so the engine's wait-loop spins - which poll
    //? I_GetTime in a hot loop - advance even when wall time stands still).
    static PROBE_ARMED: Cell<bool> = const { Cell::new(false) };
    static PROBE_BASE: Cell<f64> = const { Cell::new(0.0) };
    static PROBE_ARM_WALL: Cell<f64> = const { Cell::new(0.0) };
    static PROBE_CALLS: Cell<u32> = const { Cell::new(0) };
}

/// Real milliseconds mapped onto the fake clock per real millisecond while
/// armed: 20x lets a few real minutes of harness run cover many engine
/// minutes without distorting ordering.
const CLOCK_SPEEDUP: f64 = 20.0;

/// Fake-clock advance per now_ms() call while the probe override is armed.
/// 0.5 ms keeps the TryRunTics spin bounded (~a few thousand iterations per
/// tic) while letting a few thousand tick calls cover minutes of engine time.
const CLOCK_STEP_MS: f64 = 0.5;

/// Call once, before doomgeneric_Create.
pub fn init_start_time() { START_MS.set(performance_now()); }

/// Convert a raw `performance.now()` value (as handed in by the rAF loader)
/// into the engine clock domain — the same mapping `now_ms()` applies to its
/// own reads, so `woom24_frame` and `DG_GetTicksMs` share one clock (F1 M1
/// frame/pump split; no second time source). Under the Node probe harness
/// there is no meaningful rAF timestamp, so the fake clock wins.
pub fn engine_ms(perf_now: f64) -> u32
{
    if PROBE_ARMED.with(Cell::get)
    {
        now_ms()
    }
    else
    {
        START_MS.with(|t| (perf_now - t.get()) as u32)
    }
}

/// Engine heartbeat clock: milliseconds since start (u32 truncation = same
/// behavior as the native Instant).
pub fn now_ms() -> u32
{
    if PROBE_ARMED.with(Cell::get)
    {
        PROBE_CALLS.with(|c| c.set(c.get().wrapping_add(1)));
        let calls = PROBE_CALLS.with(|c| c.get()) as f64;
        let base = PROBE_BASE.with(Cell::get);
        let arm_wall = PROBE_ARM_WALL.with(Cell::get);
        (base + (js_sys::Date::now() - arm_wall) * CLOCK_SPEEDUP + calls * CLOCK_STEP_MS) as u32
    }
    else
    {
        START_MS.with(|t| (performance_now() - t.get()) as u32)
    }
}

fn performance_now() -> f64
{
    web_sys::window().
        and_then(|w| w.performance()).
        map(|p| p.now()).
        //? Headless hosts (Node harness) have no window/performance; the
        //? epoch clock keeps the engine's real-time paths exercisable there.
        unwrap_or_else(js_sys::Date::now)
}

//? DEBUG(probe): arms the fake clock (div-probe hunt); strips with the other
//? div-probe hooks once the browser re-test passes. `initial_ms` seeds the
//? clock so the boot's first TryRunTics sees real elapsed time (the browser
//? boot takes 1-2 s, guaranteeing tics run before the first D_Display; the
//? harness needs the same head start or the display runs with a NULL
//? pagename before the demo sequencer ever ticked).
pub fn probe_set_armed(on: bool, initial_ms: f64)
{
    PROBE_ARMED.with(|p| p.set(on));
    PROBE_BASE.with(|m| m.set(if on { initial_ms } else { 0.0 }));
    PROBE_ARM_WALL.with(|w| w.set(js_sys::Date::now()));
    PROBE_CALLS.with(|c| c.set(0));
}
