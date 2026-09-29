//! Status bar widget library: number, percent, multi-icon, and binary-icon widgets.
//!
//! Rust port of `vendor/doomgeneric/st_lib.c`. Four widget types are provided,
//! mirroring `st_lib.h`:
//! * [`widgets::st_number_t`] - right-justified integer rendered digit-by-digit with a
//!   patch font; supports negative values and a magic "no-draw" sentinel (1994).
//! * [`widgets::st_percent_t`] - wraps an [`widgets::st_number_t`] and appends a `%` patch.
//! * [`widgets::st_multicon_t`] - displays one patch from an indexed array, e.g. for
//!   key-card icons or face sprites.
//! * [`widgets::st_binicon_t`] - shows a patch when a boolean flag is non-zero, hides
//!   it otherwise.
//!
//! All widgets use a "dirty bit" pattern: the previous value is cached and the
//! widget is only redrawn when the value changes or `refresh` is requested.
//! Erasing is done by blitting from `st_backing_screen` (a saved copy of the
//! status bar background, owned by the status bar module).
//!
//! Notable Rust-vs-C differences:
//! * C `boolean*` is `*mut c_int` throughout.
//! * The `SHORT` macro (little-endian swap) is an identity function on x86_64.
//!
//! ## Submodule Responsibility
//!
//! - `widgets.rs` -- the four `repr(C)` widget types (root-re-exported) and
//!   the five `STlib_init*` constructors
//! - `draw.rs` -- the five draw/update entry points
//!
//! The module root holds the `sttminus` static (its `#[no_mangle]` home),
//! the `ST_HEIGHT`/`ST_Y` geometry vocabulary, and the `short_swap`
//! endian helper.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the upstream-name shim re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. The sole consumer
//! is `st_stuff` (imports all ten functions and the four types);
//! there are no extern declarers anywhere.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `STlib_init` | `widgets::init_widget_library` | glue | caches the `STTMINUS` lump; shim + pin |
//! | `STlib_initNum` | `widgets::init_number_widget` | glue | shim + pin |
//! | `STlib_drawNum` | `draw::draw_number_widget` | glue | unconditional draw; 1994 no-draw sentinel; `i_error!` above-bar guards; shim + pin |
//! | `STlib_updateNum` | `draw::update_number_widget` | glue | visibility-gated wrapper; shim + pin |
//! | `STlib_initPercent` | `widgets::init_percent_widget` | glue | shim + pin |
//! | `STlib_updatePercent` | `draw::update_percent_widget` | glue | shim + pin |
//! | `STlib_initMultIcon` | `widgets::init_multicon_widget` | glue | shim + pin |
//! | `STlib_updateMultIcon` | `draw::update_multicon_widget` | glue | erase-via-backing-screen then redraw; shim + pin |
//! | `STlib_initBinIcon` | `widgets::init_binicon_widget` | glue | shim + pin |
//! | `STlib_updateBinIcon` | `draw::update_binicon_widget` | glue | draw/erase on boolean transitions; shim + pin |
//! | `st_number_t`/`st_percent_t`/`st_multicon_t`/`st_binicon_t` | `widgets` | data | `repr(C)` ABI mirrors; root-re-exported for `st_stuff` |
//! | static `sttminus` | module root | data | `#[no_mangle]` retained (loaded by `init_widget_library`, read by `draw_number_widget`) |
//! | `ST_HEIGHT`/`ST_Y` | module root | data | duplicated with `st_stuff` by upstream design -- do not unify during graduation (report risk §1.6) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface, by adjudication (report §1.4): every function here
//! is glue -- pure framebuffer widget drawing from caller-supplied
//! state. No RNG, no simulation reads (the widgets read `plyr` fields
//! through `st_stuff`-supplied pointers). The draws ARE
//! frame-golden-relevant (the status bar sits inside the
//! `frame_hash` anchors), but their ordering is owned by the graduated
//! `d_main/display.rs` frame conductor, not by this module; there is
//! deliberately no `dtmc` submodule.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::os::raw::c_int;

pub mod draw;
pub mod widgets;

/// Height of the status bar in pixels.
/// Mirrors `ST_HEIGHT` from `st_stuff.h`.
pub(super) const ST_HEIGHT: c_int = 32;

/// Y coordinate of the top of the status bar (screen height minus bar height).
/// All widget Y coordinates are expected to be at or below this value.
/// Mirrors `ST_Y` from `st_stuff.h`.
pub(super) const ST_Y: c_int = 200 - ST_HEIGHT; // 168

/// Byte-swap for little-endian (SHORT macro from i_swap.h).
/// On x86_64 this is an identity cast.
#[inline(always)]
pub(super) fn short_swap(v: i16) -> i16 {
    v
}

/// The minus-sign patch (`STTMINUS`) used when rendering negative numbers.
///
/// Loaded by [`widgets::init_widget_library`] from the WAD and drawn to the left of the
/// most-significant digit when a number widget displays a negative value.
/// Corresponds to `sttminus` in `st_lib.c`; exported so `st_stuff.c` can
/// reference it directly.
#[no_mangle]
pub static mut sttminus: *mut crate::doom::v_video::patch_t = std::ptr::null_mut();

//* upstream-name shim: the ten widget entry points keep their
//* freeze-zone caller paths (`crate::doom::st_lib::STlib_*`; sole
//* consumer `st_stuff`, which imports the whole surface) and their C
//* symbols stay re-pinned at the definitions with
//* `#[export_name = "OriginalName"]`. Shims die with the freeze zone.
pub use draw::{
    draw_number_widget as STlib_drawNum, update_binicon_widget as STlib_updateBinIcon,
    update_multicon_widget as STlib_updateMultIcon, update_number_widget as STlib_updateNum,
    update_percent_widget as STlib_updatePercent,
};
pub use widgets::{
    init_binicon_widget as STlib_initBinIcon, init_multicon_widget as STlib_initMultIcon,
    init_number_widget as STlib_initNum, init_percent_widget as STlib_initPercent,
    init_widget_library as STlib_init, st_binicon_t, st_multicon_t, st_number_t, st_percent_t,
};
