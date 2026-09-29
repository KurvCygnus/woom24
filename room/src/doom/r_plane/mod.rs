//! Visplane (floor and ceiling) renderer.
//!
//! Rust port of `vendor/doomgeneric/r_plane.c`.
//!
//! # Overview
//!
//! Doom renders floors and ceilings as horizontal pixel spans.  During BSP
//! traversal each wall segment calls [`R_CheckPlane`] / [`R_FindPlane`] to
//! record the per-column screen-y extents into a `visplane_t`.  After the
//! full BSP walk, [`R_DrawPlanes`] iterates over every accumulated visplane
//! and rasterizes it into horizontal spans via [`R_MakeSpans`] /
//! [`R_MapPlane`].  Sky columns are treated as a special case: they are drawn
//! with `colfunc` rather than `spanfunc`.
//!
//! # Coordinate system
//!
//! All heights use the fixed-point `fixed_t` type (`i32`, 16.16 format,
//! `FRACUNIT = 1 << 16`).  Angles are `angle_t = u32` where a full circle is
//! `0x1_0000_0000` (wrapping arithmetic).
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `visplane_t` pool type (`Default` + compile-time
//!   layout asserts adjacent, pad-sentinel scheme), `planefunction_t`,
//!   `lighttable_t`, the `finecosine` accessor, the nine private constants,
//!   and all 22 `#[no_mangle]` statics
//! - `visplane.rs` -- `init_planes`, `clear_planes`, `find_plane`,
//!   `check_plane` (pool management)
//! - `span.rs` -- `map_plane`, `make_spans`, `draw_planes` (the rasterizer)
//!
//! The module root is documentation + wiring only. Consumers keep their
//! upstream identifiers through this root: `r_main` calls
//! `R_ClearPlanes`/`R_DrawPlanes`/`R_InitPlanes` in the frame sequence and
//! writes `yslope`/`distscale` at view-size init, `r_bsp` writes
//! `floorplane`/`ceilingplane` via `R_FindPlane`, and `r_segs` writes the
//! clip arrays and allocates from `openings` via `lastopening`.
//!
//! # Rust-vs-C differences
//!
//! - `find_plane` returns **null** on visplane-pool exhaustion (128) where
//!   the C `I_Error`s; callers deref unconditionally, so the failure mode
//!   is a Rust panic, not silent corruption. NOT limit-removed -- a
//!   vanilla-limit preservation with a different failure mode (FIXME
//!   carried at the definition; limit-removal intake, do not change here).
//! - `check_plane` grows `lastvisplane` with no pool-limit check --
//!   vanilla-faithful (C has no check there either); the classic
//!   visplane-overflow trample risk. Same intake.
//! - `clear_planes` resets only `cachedheight[0..200]` (hardcoded 200 vs
//!   the `MAXH` array cap) -- carried verbatim from C. At view heights
//!   above 200, rows 200 and up keep stale caches across a view-size
//!   change; a latent HOM-class hazard on the custom-resolution path that
//!   the 640x400 frame goldens do not trip. **Report-only finding: do NOT
//!   fix during graduation** -- F2/limit-removal intake.
//! - `MAXOPENINGS` = `MAXW * 64` (4096*64) vs C `SCREENWIDTH*64` =
//!   320*64 -- already in the Boom `MAX_SCREENWIDTH` shape (effectively
//!   limit-removed for openings); C's opening-overflow `I_Error` was
//!   `#ifdef RANGECHECK`, so the port dropping it matches the default C
//!   build.
//! - `floorfunc` / `ceilingfunc` / `spanstop` are dead-but-exported (zero
//!   readers/writers in tree; C parity) -- kept for symbol-set
//!   byte-identity, they retire with the freeze zone.
//! - `ANGLETOSKYSHIFT` lives in `r_sky.h` upstream but is defined in this
//!   module's `state.rs` -- left here; moving it would churn `r_sky` for
//!   no gain.
//! - `visplane_t` has no `c_ffi` twin; the pad-byte sentinel scheme
//!   (`pad1..pad4` mimicking C `top[minx-1]`/`top[maxx+1]` writes) is
//!   layout-sensitive and pinned by the compile-time asserts that sit
//!   directly below the struct.
//! - No module link anchor exists today -- absence preserved.
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
//! comes with freeze-zone retirement). All seven were safe
//! `extern "C"` pre-split and stay safe-bodied (parity with today, not
//! with C).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_InitPlanes` | `visplane::init_planes` | glue | no-op ("Doh!", C :94-96); shim + pin |
//! | `R_ClearPlanes` | `visplane::clear_planes` | glue | per-frame reset; carries the hardcoded 200-row `cachedheight` reset (see Rust-vs-C differences); shim + pin |
//! | `R_FindPlane` | `visplane::find_plane` | glue | pool search/claim; null-on-exhaustion divergence carried (FIXME); shim + pin |
//! | `R_CheckPlane` | `visplane::check_plane` | glue | split-or-extend decision; no pool-limit check -- vanilla-faithful; shim + pin |
//! | `R_MapPlane` | `span::map_plane` | glue | span setup + per-row step cache ladder; shim + pin |
//! | `R_MakeSpans` | `span::make_spans` | glue | span close/open ladder; shim + pin |
//! | `R_DrawPlanes` | `span::draw_planes` | glue | sky special case (fullbright `colormaps[0]`, INVUL hack comment; `ANGLETOSKYSHIFT`) + pad-sentinel boundary handling; shim + pin |
//! | 22 `#[no_mangle]` statics (`floorfunc`..`cachedystep`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`r_main` writers, `r_bsp` plane pointers, `r_segs` clip/openings relay) |
//! | `visplane_t` (+`Default`, +layout asserts), `planefunction_t`, `lighttable_t`, `finecosine`, 9 constants | `state` | data | carried verbatim; zero external type consumers |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: nothing here resolves map-load or simulation content.
//! Every function shapes the *presented frame* (floor/ceiling spans and
//! sky columns) or runs at startup, pinned by the F9 frame goldens
//! (`video_anchor.rs`, scenario `frame_hash`), while `harness_hash`'s
//! state ledger never reads renderer state. The three limit/overflow
//! divergences above are report-only findings carried out of scope per
//! the no-behavioral-change rule; they file into limit-removal intake.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod span;
pub mod state;
pub mod visplane;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_plane::R_*`: r_main's frame sequence,
//* r_bsp's plane claim, r_segs' per-seg plane extension). The C symbol
//* each shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. There are no C
//* referencers in the default build, so the pins are wasm-surface
//* conservatism. Shims die with the freeze zone.
pub use span::{draw_planes as R_DrawPlanes, make_spans as R_MakeSpans, map_plane as R_MapPlane};
pub use visplane::{
    check_plane as R_CheckPlane, clear_planes as R_ClearPlanes, find_plane as R_FindPlane,
    init_planes as R_InitPlanes,
};

//* path-stability re-export: the 22 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* `r_main` writes `yslope`/`distscale` by full path at view-size init,
//* `r_bsp` stores `floorplane`/`ceilingplane`, `r_segs` writes
//* `floorclip`/`ceilingclip` per column and allocates `openings` slices
//* through `lastopening`.
pub use state::{
    basexscale, baseyscale, cacheddistance, cachedheight, cachedxstep, cachedystep,
    ceilingclip, ceilingplane, distscale, floorclip, floorplane, lastopening, lastvisplane,
    openings, planeheight, planezlight, spanstart, spanstop, visplanes, yslope, ceilingfunc,
    floorfunc,
};
