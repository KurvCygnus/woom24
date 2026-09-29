//! Screen wipes: the mission-begin transition effects.
//!
//! Rust port of `vendor/doomgeneric/f_wipe.c`. Two algorithms bridge a
//! captured start frame to a captured end frame -- a palette-index
//! colour cross-fade (`crossfade`) and the classic column melt
//! (`melt`) -- each as an init/step/exit triple stored in the `WIPES`
//! dispatch table, indexed as `wipeno * 3 + {0, 1, 2}`. The frame
//! conductor (the graduated `d_main/display.rs`, which carries the
//! golden-adjacent wipe loop verbatim) calls
//! `screenwipe::start_screen` before the scene draw, `screenwipe::end_screen`
//! after it, and pumps `screenwipe::screen_wipe` once per tick from its
//! blocking wipe loop until the algorithm reports completion.
//!
//! Notable Rust-vs-C differences:
//! - Global statics use `static mut` with `unsafe` accessors rather than bare
//!   C globals.
//! - `wipe_shittyColMajorXform` is renamed to `melt::col_major_xform`
//!   to follow Rust naming conventions while matching the comment in the C
//!   source.
//!
//! ## Submodule Responsibility
//!
//! - `crossfade.rs` -- the colour cross-fade triple (`init`/`do`/`exit`)
//! - `melt.rs` -- the melt triple plus the column-major transpose helper
//! - `screenwipe.rs` -- the three public entry points: capture the start
//!   screen, capture the end screen, drive the wipe one frame forward
//! - `dtmc.rs` -- the extracted melt column-seed surface (the
//!   first-column start rule and the per-column recurrence) with its
//!   baseline vectors
//!
//! The module root holds the shared FFI surface (the `extern "C"` block
//! carried verbatim from the pre-split file), the five wipe statics, the
//! `WipeFn` vocabulary, and the `WIPES` table. The upstream extern
//! declarers of the `Z_`/`V_`/`I_` symbols live in the graduated
//! `z_zone`/`v_video`/`i_video` pins; this block links by symbol and
//! must stay verbatim.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the upstream-name shim re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module (the differential
//! oracle `c2rust-intermediate/src/d_main.rs:100-112` declares the
//! three `wipe_*` names by symbol). The six private callbacks were
//! never exported -- doc aliases only, no pins. Functions only:
//! statics/consts/tables keep their upstream names.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `wipe_shittyColMajorXform` | `melt::col_major_xform` | glue | private row/column transpose through a `Z_Malloc` transient (zone-sizing policy note, `docs/vanilla-workarounds.md:1191-1195`); never exported, doc alias only |
//! | `wipe_initColorXForm` | `crossfade::init_color_xform` | glue | private; copies the start screen into the working buffer |
//! | `wipe_doColorXForm` | `crossfade::do_color_xform` | glue | private; nudges each palette index toward the end frame |
//! | `wipe_exitColorXForm` | `crossfade::exit_color_xform` | glue | private no-op that always returns 0 |
//! | `wipe_initMelt` | `melt::init_melt` | dtmc | RNG ledger: draws `M_Random` 1 + (width-1) times into the column seeds; the call sites stay whole and in order, the pure math is extracted to `dtmc::melt_column_start`/`dtmc::melt_column_seed` (baseline vectors f8c7e47, pre-move) |
//! | `wipe_doMelt` | `melt::do_melt` | glue | pure integer buffer work; the visible content of every state transition, `while ticks > 0` step count verbatim |
//! | `wipe_exitMelt` | `melt::exit_melt` | glue | frees the Y array and both snapshots |
//! | `wipe_StartScreen` | `screenwipe::start_screen` | glue | capture + `I_ReadScreen`; shim + pin |
//! | `wipe_EndScreen` | `screenwipe::end_screen` | glue | capture + restore of the start frame; shim + pin |
//! | `wipe_ScreenWipe` | `screenwipe::screen_wipe` | glue | the `GO` latch + dispatch; driven from the blocking wipe loop; shim + pin |
//! | statics `GO`/`WIPE_SCR_START`/`WIPE_SCR_END`/`WIPE_SCR`/`Y` | module root | data | upstream names retained beside the shared FFI surface |
//! | `WipeFn`/`WIPES` | module root | data | upstream dispatch vocabulary |
//!
//! ## Deterministic Aspects
//!
//! The dtmc surface is the melt seeding: `melt::init_melt` draws
//! `M_Random` 1 + (width-1) times per wipe start (first column
//! `-(draw % 16)`, then the recurrence with its `> 0 -> 0` clamp and
//! `== -16 -> -15` re-bias). Those draws advance `rndindex`
//! (state-hash word 2), so the draw COUNT and ORDER are load-bearing:
//! `dtmc.rs` extracts only the pure per-column math and the baseline
//! vectors (written pre-move, commit f8c7e47) pin it; any future
//! reordering of the draws relative to the `Z_Malloc`/transpose
//! preamble is a behavior change. `melt::do_melt` is frame-golden
//! glue -- the melt is visible inside every state transition's
//! `frame_hash` anchors -- and runs inside `d_main/display.rs`'s
//! blocking wipe loop, whose timing order this module must never
//! disturb. Everything else is capture/dispatch glue pinned by the F9
//! frame goldens.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

pub mod crossfade;
pub mod dtmc;
pub mod melt;
pub mod screenwipe;

extern "C" {
    /// Allocate `size` bytes from zone memory with the given cache tag.
    fn Z_Malloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void;
    /// Free a zone-allocated block back to the heap.
    fn Z_Free(ptr: *mut c_void);
    /// Capture the current video buffer into `scr` (`SCREENWIDTH * SCREENHEIGHT` bytes).
    fn I_ReadScreen(scr: *mut u8);
    /// Blit a `width x height` block from `src` to the video buffer at `(x, y)`.
    fn V_DrawBlock(x: c_int, y: c_int, width: c_int, height: c_int, src: *mut u8);
    /// Mark a rectangle of the video buffer as dirty so it gets blitted to the display.
    fn V_MarkRect(x: c_int, y: c_int, width: c_int, height: c_int);
    /// Pointer to the primary video framebuffer; `SCREENWIDTH * SCREENHEIGHT` bytes.
    static mut I_VideoBuffer: *mut u8;
}

/// Whether the current wipe is active (`1`) or idle (`0`).
///
/// Set to 1 by [`screenwipe::screen_wipe`] on the first call and cleared when the
/// wipe algorithm signals completion.  C origin: `static int go` in f_wipe.c.
pub(super) static mut GO: c_int = 0;

/// Snapshot of the screen taken at wipe-start (the "old" frame).
///
/// Allocated in [`screenwipe::start_screen`] and freed by `melt::exit_melt`.
/// C origin: `wipe_scr_start` in f_wipe.c.
pub(super) static mut WIPE_SCR_START: *mut u8 = std::ptr::null_mut();

/// Snapshot of the screen taken at wipe-end (the "new" frame).
///
/// Allocated in [`screenwipe::end_screen`] and freed by `melt::exit_melt`.
/// C origin: `wipe_scr_end` in f_wipe.c.
pub(super) static mut WIPE_SCR_END: *mut u8 = std::ptr::null_mut();

/// Working buffer used during the wipe; points into the live video buffer.
///
/// Set to [`I_VideoBuffer`] at the start of each wipe pass.
/// C origin: `wipe_scr` in f_wipe.c.
pub(super) static mut WIPE_SCR: *mut u8 = std::ptr::null_mut();

/// Per-column Y position array used by the melt algorithm.
///
/// One `c_int` per screen column; negative values mean the column has not yet
/// started falling.  Allocated by `melt::init_melt` and freed by
/// `melt::exit_melt`.  C origin: `y` (static) in f_wipe.c.
pub(super) static mut Y: *mut c_int = std::ptr::null_mut();

/// Function-pointer type shared by all wipe init/step/exit callbacks.
///
/// Arguments are `(width, height, ticks)`.  Return value: 0 = still running,
/// 1 = complete.  C origin: the implicit function-pointer type used in the
/// `wipes` table in f_wipe.c.
type WipeFn = unsafe extern "C" fn(c_int, c_int, c_int) -> c_int;

/// Dispatch table for wipe algorithms.
///
/// Indexed as `wipeno * 3 + phase`, where `phase` is 0 (init), 1 (step), or
/// 2 (exit).  `wipeno` 0 selects the colour cross-fade; `wipeno` 1 selects the
/// melt.  C origin: `wipes[2][3]` in f_wipe.c.
const WIPES: [WipeFn; 6] = [
    crossfade::init_color_xform,
    crossfade::do_color_xform,
    crossfade::exit_color_xform,
    melt::init_melt,
    melt::do_melt,
    melt::exit_melt,
];

//* upstream-name shim: the three public entry points keep their
//* freeze-zone caller paths (`crate::doom::f_wipe::wipe_*`; sole in-tree
//* consumer `d_main/display.rs:17,88,166,190`) and their C symbols stay
//* re-pinned at the definitions with `#[export_name = "OriginalName"]`
//* for the differential oracle, which declares them by symbol. Shims
//* die with the freeze zone.
pub use screenwipe::{end_screen as wipe_EndScreen, screen_wipe as wipe_ScreenWipe, start_screen as wipe_StartScreen};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_fns_are_callable() {
        // Sanity: verify all six function pointers are distinct and
        // non-null at compile-time.
        let fns: [*const (); 6] = WIPES.map(|f| f as *const ());
        for i in 0..6 {
            for j in (i + 1)..6 {
                assert_ne!(fns[i], fns[j], "WIPES[{i}] == WIPES[{j}]");
            }
        }
    }
}
