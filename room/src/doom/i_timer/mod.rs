//! Rust port of vendor/doomgeneric/i_timer.c.
//!
//! Game timer functions built on top of the doomgeneric host hooks
//! `DG_GetTicksMs` and `DG_SleepMs`. Doom counts time in 35 Hz tics; these
//! helpers convert between wall-clock milliseconds and tics, and provide a
//! sleep primitive used by the main loop. `BASETIME` is captured on the
//! first time query so that `I_GetTime` / `I_GetTimeMS` return values
//! relative to game start rather than the host's epoch.
//!
//! ## Submodule Responsibility
//!
//! - `clock.rs` -- the game clock: `TICRATE`, the `BASETIME` static, the
//!   latching `get_time_tics` / `get_time_ms`, and the read-only
//!   `elapsed_ms_from`
//! - `host.rs` -- the host hooks: the `DG_GetTicksMs` / `DG_SleepMs`
//!   extern block (reverse contract, carried verbatim), `host_ticks`,
//!   `host_sleep_ms`, `wait_vbl`, and `init_timer`
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `I_GetTicks` | `host::host_ticks` | glue | raw host millisecond counter, thin `DG_GetTicksMs` wrapper; C symbol pinned via `#[export_name]` (dead-but-exported: zero in-tree extern declarers, kept for wasm symbol-set parity); upstream `vendor/doomgeneric/i_timer.c:38` |
//! | `I_GetTime` | `clock::get_time_tics` | glue | latches `BASETIME` on first call, returns `(now - BASETIME) * TICRATE / 1000`; C symbol pinned via `#[export_name]` (`d_loop/mod.rs` extern-declares it); upstream `i_timer.c:43` |
//! | `I_GetTimeMS` | `clock::get_time_ms` | glue | same latch, no tic conversion; C symbol pinned via `#[export_name]` (`d_loop/mod.rs` extern-declares it; `shells/web/src/lib.rs` calls the upstream path through the root shim); upstream `i_timer.c:62` |
//! | `elapsed_ms_from` (Rust-only, F1 M1) | `clock::elapsed_ms_from` | glue | name kept (already house style); read-only, never latches -- see Deterministic Aspects |
//! | `I_Sleep` | `host::host_sleep_ms` | glue | delegates to `DG_SleepMs`; C symbol pinned via `#[export_name]` (`d_loop/mod.rs` extern-declares it); upstream `i_timer.c:76` |
//! | `I_WaitVBL` | `host::wait_vbl` | glue | no-op preserved from upstream (`i_timer.c:84`); C symbol pinned via `#[export_name]` (wasm symbol-set parity); upstream `i_timer.c:84` |
//! | `I_InitTimer` | `host::init_timer` | glue | no-op kept for ABI parity (C's `SDL_Init` was dropped by doomgeneric); C symbol pinned via `#[export_name]` (`d_main/boot.rs` calls the upstream path through the root shim); upstream `i_timer.c:90` |
//! | `TICRATE` (const) | `clock::TICRATE` | data | constant keeps its upstream name (`m_fixed` precedent) |
//! | `BASETIME` (static) | `clock::BASETIME` | data | static keeps its name; stays private to the module subtree |
//! | `basetime` (C static) | -- | -- | the C name; the Rust port had already spelled it `BASETIME` pre-graduation and keeps that spelling |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: the game clock feeds `d_loop`'s
//! net-tic scheduling, never demo bytes, and the pure fraction math that IS
//! render-exact lives outside this module in `r_interp`. The load-bearing
//! contract is the two-stage `BASETIME` clock, preserved verbatim:
//!
//! 1. **Latch stage.** `get_time_tics` / `get_time_ms` latch `BASETIME` on
//!    first call; `BASETIME == 0` is the C not-yet-initialised sentinel
//!    (if the host's first tick value is exactly 0 the baseline is captured
//!    on the next call -- chocolate-doom relies on the same behaviour). The
//!    engine boot path always latches first.
//! 2. **Read stage (read-only).** `elapsed_ms_from(now_ms)` maps a
//!    shell-supplied `now_ms` into the same `BASETIME` domain **without
//!    latching and without reading the clock**; it reads `BASETIME` through
//!    `addr_of!` (no `&STATIC_MUT` reference, `static_mut_refs`-clean).
//!    Consumed solely by `r_interp::begin_frame`, which requires `now_ms`
//!    to come from the same clock the shell feeds to `DG_GetTicksMs` -- no
//!    new time source.
//!
//! The `clock` test pins the read-only guarantee (same input, same result,
//! `BASETIME` unchanged) so the latch-once semantics cannot regress.

pub mod clock;
pub mod host;

//* path-stability re-export: the tick-rate constant keeps its module-root
//* path (`d_loop`, `d_main`, `hu_stuff`, `m_menu`, `st_stuff`, `wi_stuff`,
//* `p_doors`, `p_plats`, `p_mobj`, `p_spec`, `c_ffi`, `c_tests`, ...).
pub use clock::TICRATE;

//* path-stability re-export: the Rust-only read-only mapper keeps its
//* module-root path (`r_interp`).
pub use clock::elapsed_ms_from;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use clock::{get_time_ms as I_GetTimeMS, get_time_tics as I_GetTime};
pub use host::{
    host_sleep_ms as I_Sleep, host_ticks as I_GetTicks, init_timer as I_InitTimer,
    wait_vbl as I_WaitVBL,
};
