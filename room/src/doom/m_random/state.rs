//! The two demo-visible random cursors: the shared generator state
//! advanced by the `random.rs` wrappers and read directly by the net
//! consistency cookie, the simulation state digest, and the golden
//! snapshots.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

/// `extern int rndindex` -- the game-thinker random cursor
/// (upstream `m_random.c:46`, declared `extern` in `doomstat.h:276`).
/// Read into the net-play consistency cookie and the simulation state
/// digest; kept as the module's shared cursor state (`state.rs`)
/// with its C symbol unchanged.
#[no_mangle]
pub static mut rndindex: c_int = 0;

/// `int prndindex` -- the play-simulation random cursor
/// (upstream `m_random.c:47`). Exported with C linkage so the
/// demo-playthrough and frame-split test crates can read it via
/// `extern "C"` for golden snapshots.
#[no_mangle]
pub static mut prndindex: c_int = 0;
