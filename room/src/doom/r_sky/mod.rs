//! Sky-mapping globals: the sky flat number, the current sky texture, and
//! the sky column midpoint, plus their one init function. The sky is drawn
//! as a wall-like texture that wraps around the full 360-degree view -- a
//! 1024-column sky map equals one full revolution; the default 256-column
//! sky repeats four times on a 320-pixel-wide screen.
//!
//! ## Submodule Responsibility
//!
//! - `sky_map.rs` -- `init_sky_map`, the view-size-reset entry point, and
//!   its unit tests
//!
//! The three `#[no_mangle]` statics are module-root state so every
//! freeze-zone importer keeps its `crate::doom::r_sky::` path: `g_game`
//! writes `skyflatnum`/`skytexture` at level start, `p_map`/`p_mobj` read
//! `skyflatnum` (sky-hack hitscan suppression, sky-wall movement), and the
//! renderer reads all three.
//!
//! # Rust-vs-C differences
//!
//! - `ANGLETOSKYSHIFT` lives in `r_sky.h` upstream but is defined in
//!   `r_plane` in this port -- left there; moving it would churn `r_plane`
//!   for no gain.
//! - The `skyflatnum = R_FlatNumForName(SKYFLATNAME)` line that appears
//!   commented-out in the C source stays commented-out: vanilla performs
//!   the assignment in `g_game` at level start. Vanilla-faithful, not a
//!   bug emulation -- no `docs/vanilla-workarounds.md` row.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_InitSkyMap` | `sky_map::init_sky_map` | glue | single store `skytexturemid = 100 * FRACUNIT`; shifts every sky column when the view size changes -- frame-golden surface, nothing qualifies for dtmc extraction; shim + pin; upstream `r_sky.c:47-51` |
//! | `skyflatnum` / `skytexture` / `skytexturemid` | module root | data | `#[no_mangle]` statics kept verbatim; `skyflatnum` is the one sim-observable item (reads in `p_map`/`p_mobj`, writes in `g_game`) |
//!
//! No module link anchor exists today -- absence preserved.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: nothing here resolves map-load or simulation content.
//! `skyflatnum` is *read* by simulation code, but its *write* (the
//! `SKYFLATNAME` resolution) lives in `g_game` over `r_data`'s lookup --
//! the sim-content side is theirs. This module's own observable behavior
//! is the presented frame (`skytexturemid` shifts every sky column on view
//! resize), pinned by the F9 frame goldens (`video_anchor.rs`, scenario
//! `frame_hash`), while `harness_hash`'s state ledger never reads
//! renderer state.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;

pub mod sky_map;

//* upstream-name shim: `r_main.rs:954` calls `R_InitSkyMap()` by full path
//* from `R_ExecuteSetViewSize`. The C symbol is re-pinned at the
//* definition with `#[export_name]`, so the wasm/extern symbol name set
//* stays byte-identical to the pre-split module. There are no C
//* referencers, so the pin is wasm-surface conservatism. Shims die with
//* the freeze zone.
pub use sky_map::init_sky_map as R_InitSkyMap;

/// WAD lump index of the sky flat (`F_SKY1`), filled in by `g_game.c`
/// via `R_FlatNumForName(SKYFLATNAME)`.
///
/// Sectors whose ceiling flat equals this value are treated as open sky by
/// the BSP, seg, and plane renderers (`r_bsp.c`, `r_segs.c`, `r_plane.c`,
/// `p_map.c`, `p_mobj.c`).
#[no_mangle]
pub static mut skyflatnum: c_int = 0;

/// Texture number of the current sky texture (SKY1 / SKY2 / SKY3 / SKY4),
/// assigned by `g_game.c` at level start.
///
/// `r_plane.c` uses this index when calling `R_GetColumn` to fetch sky
/// columns, and `r_data.c` marks it as always-present during precaching.
#[no_mangle]
pub static mut skytexture: c_int = 0;

/// Vertical midpoint for sky column drawing, in 16.16 fixed-point units.
///
/// Reset to `100 * FRACUNIT` every time the view size changes (see
/// `R_InitSkyMap`).  Used by `r_plane.c` as `dc_texturemid` when rendering
/// sky spans.
#[no_mangle]
pub static mut skytexturemid: c_int = 0;
