//! Wall-segment rasterizer.
//!
//! Rust port of `vendor/doomgeneric/r_segs.c`.
//!
//! # Overview
//!
//! Each BSP leaf that the traversal visits submits one wall segment to
//! [`R_StoreWallRange`], which:
//!
//! 1. Computes the perpendicular distance from the viewpoint to the seg and
//!    derives scale values at both screen endpoints.
//! 2. Decides which wall textures (upper, middle, lower) are visible.
//! 3. Calls `R_CheckPlane` to extend the current floor/ceiling visplanes.
//! 4. Runs `render_seg_loop` to draw textured columns and update the
//!    per-column clip arrays for later sprite rendering.
//! 5. Saves sprite-clipping info into the `openings` scratch buffer.
//!
//! Transparent mid-textures (fences, windows) on two-sided lines are deferred
//! and drawn by [`R_RenderMaskedSegRange`] after all opaque geometry is done.
//!
//! # Coordinate system
//!
//! `fixed_t = i32`, 16.16 fixed point.  `angle_t = u32`, full circle =
//! `0x1_0000_0000`.  "Scale" in this module means the reciprocal distance from
//! the view plane to a wall column (larger = closer).
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the eleven private constants and all 33 `#[no_mangle]`
//!   statics, one data home
//! - `masked.rs` -- `render_masked_seg_range`, the deferred mid-texture pass
//!   called from `r_things`
//! - `wall.rs` -- `store_wall_range` (the `r_bsp`-invoked entry point) and
//!   `render_seg_loop` (the per-column raster loop)
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below. Every consumer keeps its upstream identifier
//! through this root: `r_bsp` writes `rw_angle1` and calls
//! `R_StoreWallRange` (six call sites), `r_main` reads `rw_distance` /
//! `rw_normalangle` / `walllights` and its unit tests write the latter two
//! directly, `r_things` calls [`R_RenderMaskedSegRange`], and `c_ffi`'s
//! re-export block feeds `c_tests/r_segs_c.rs`'s zero-init and `c_int`
//! width pins.
//!
//! # Rust-vs-C differences
//!
//! - `store_wall_range` computes `rw_scalestep` only when `stop > start`,
//!   leaving the previous seg's value in place for single-column segs --
//!   vanilla-faithful (FIXME carried at the definition).
//! - Fake contrast: the seg light level is nudged by +-1 on perfectly
//!   horizontal/vertical segs (both in `store_wall_range` and
//!   [`R_RenderMaskedSegRange`]) -- vanilla `R_StoreWallRange` behaviour,
//!   frame-visible and sim-invisible. Not a bug emulation; no
//!   `docs/vanilla-workarounds.md` row.
//! - A wall range is silently ignored when the `drawsegs[MAXDRAWSEGS]`
//!   pool is full -- a vanilla LIMIT kept, not limit-removed.
//! - No module link anchor exists today -- absence preserved.
//!
//! Cross-module contracts documented once here:
//! - **r_bsp relay**: `r_bsp`'s clip functions set `rw_angle1` and call
//!   `R_StoreWallRange`; `store_wall_range` consumes `curline` /
//!   `frontsector` / `backsector` / `drawsegs` / `ds_p` straight from
//!   `r_bsp`'s statics (the module-pair contract).
//! - **r_things bidirectional contract**: `render_masked_seg_range` writes
//!   `r_things`' `spryscale` / `mfloorclip` / `mceilingclip` /
//!   `sprtopscreen` directly -- do not route through accessors; the
//!   statics keep names and the writes keep their call points.
//! - **openings pointer arithmetic**: `maskedtexturecol` /
//!   `lastopening` slices into `r_plane`'s `openings` buffer move
//!   verbatim; `r_plane` graduates in its own commit with the same rule.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the `upstream-name shim` re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. Functions only:
//! statics/consts/tables keep their upstream names (data-tier renaming
//! comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_StoreWallRange` | `wall::store_wall_range` | glue | THE seg entry point called by the `r_bsp` clip fns; distance/scale ladder, texture-pegging ladders, and the stale-`rw_scalestep` single-column quirk move verbatim; shim + pin; upstream `r_segs.c:371` |
//! | `R_RenderSegLoop` (C static) | `wall::render_seg_loop` | glue | the per-column raster loop; ceiling/floor mark recording + clip updates; private before and after (doc alias only) |
//! | `R_RenderMaskedSegRange` | `masked::render_masked_seg_range` | glue | deferred mid-texture pass called from `r_things`; writes `r_things`' `spryscale`/clip statics directly (bidirectional contract); shim + pin; upstream `r_segs.c:96` |
//! | 33 `#[no_mangle]` statics (`segtextured`..`maskedtexturecol`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`c_ffi`'s re-export block, `r_bsp`, `r_main` and its tests, `c_tests/r_segs_c.rs`) |
//! | 11 private constants (`HEIGHTBITS`..`MAXDRAWSEGS`) | `state` | data | `pub(super)` beside their only readers (`masked`/`wall`) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: nothing here resolves map-load or simulation content.
//! Every function in this module shapes the *presented frame* -- the wall
//! columns, the per-column clip arrays, the sprite-clip openings, and the
//! fake-contrast light nudges -- pinned by the F9 frame goldens
//! (`video_anchor.rs`, scenario `frame_hash`), while `harness_hash`'s
//! state ledger never reads renderer state. The `drawsegs`-pool-full
//! silent ignore is a vanilla limit kept (failure mode discussion belongs
//! to limit-removal intake, not this module).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod masked;
pub mod state;
pub mod wall;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_segs::R_*`; `r_bsp` calls
//* `R_StoreWallRange` at six sites, `r_things` calls
//* `R_RenderMaskedSegRange`). The C symbol each shim forwards to is
//* re-pinned at the definition with `#[export_name = "OriginalName"]`,
//* so the wasm/extern symbol name set stays byte-identical to the
//* pre-split module. There are no C referencers in the default build,
//* so the pins are wasm-surface conservatism. Shims die with the
//* freeze zone.
pub use masked::render_masked_seg_range as R_RenderMaskedSegRange;
pub use wall::store_wall_range as R_StoreWallRange;

//* path-stability re-export: the 33 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* `c_ffi`'s re-export block (34-name list feeding `c_tests/r_segs_c.rs`),
//* `r_bsp` (`rw_angle1` write + `R_StoreWallRange` relay state), `r_main`
//* (`rw_distance`/`rw_normalangle`/`walllights`, including direct writes
//* from its unit tests).
pub use state::{
    bottomfrac, bottomstep, bottomtexture, markceiling, markfloor, maskedtexture,
    maskedtexturecol, midtexture, pixhigh, pixhighstep, pixlow, pixlowstep, rw_angle1,
    rw_bottomtexturemid, rw_centerangle, rw_distance, rw_midtexturemid, rw_normalangle, rw_offset,
    rw_scale, rw_scalestep, rw_stopx, rw_toptexturemid, rw_x, segtextured, topfrac, topstep,
    toptexture, walllights, worldbottom, worldhigh, worldlow, worldtop,
};
