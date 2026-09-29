//! Renderer main loop, view setup, and BSP/geometry/trigonometry utilities.
//!
//! Rust port of `vendor/doomgeneric/r_main.c`. Hosts all renderer global state
//! (viewport dimensions, view position/angle, light tables, column/span function
//! pointers) and the per-frame entry point [`crate::doom::r_main::frame::render_player_view`].
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the light-table constants + `lighttable_t` vocabulary and
//!   all 35 `#[no_mangle]` statics (view globals, projection, light tables,
//!   fn-pointer table, resize request triple) with their verbatim docs
//! - `geometry.rs` -- `add_point_to_box`, `point_on_side`, `point_on_seg_side`,
//!   `point_to_angle`, `point_to_angle2`, `point_to_dist`, `init_point_to_angle`,
//!   `init_tables`, `scale_from_global_angle`, `point_in_subsector`, plus the
//!   angle/dist/side unit tests
//! - `viewsize.rs` -- `set_view_size`, `execute_set_view_size`,
//!   `init_texture_mapping`, `init_light_tables`
//! - `frame.rs` -- `init`, `setup_frame`, `render_player_view`
//! - `dtmc.rs` -- the wave's only dtmc extraction: `angle_from_delta` (the
//!   pure octant classifier) + `DBITS` + the known-vector baseline
//! - `anchor.rs` -- the module link anchor (the ONE live renderer anchor:
//!   `doomgeneric.rs:47` imports it and `:261` calls it)
//!
//! The module root is documentation + wiring only. Consumers keep their
//! upstream identifiers through this root: the SIM tier reads
//! `R_PointToAngle2`/`R_PointInSubsector`/`validcount` (`p_enemy`,
//! `p_pspr`, `p_mobj`, `p_map`, `p_maputl`, `p_inter`, `p_user`, `s_sound`,
//! `st_stuff`, `g_game`, `p_sight`, `p_enemy/noise`, `d_net`'s
//! `viewangleoffset` extern), the render tier (`r_bsp`, `r_segs`,
//! `r_plane`, `r_things`, `r_interp`) reads the view globals and fn-pointer
//! table, `m_menu`/`video_cfg` drive `R_SetViewSize`, `d_main` drives the
//! boot/resize/frame sequence, `hu_lib` externs read the r_draw view-window
//! statics (not this module's), and `tests/video_raster.rs` drives
//! `R_SetViewSize`/`R_ExecuteSetViewSize` directly.
//!
//! # Rust-vs-C differences
//!
//! - `DBITS = FRACBITS - SLOPEBITS = 5` (`dtmc.rs`), with the regression
//!   history in the test doc; `R_PointToDist` indexes `tantoangle` through
//!   it. `viewangletox` is sized `FINEANGLES/2 + 1` for `r_bsp`'s defensive
//!   bounds (size is a cross-module contract), `xtoviewangle` is sized
//!   `MAX_SCREENWIDTH + 1` (boom `r_main.c:81` shape).
//! - `R_PointToAngle2` is de-aliased from the C global trick: it computes
//!   purely from the delta and never touches `viewx`/`viewy` (the
//!   preserve-globals and translation-invariance tests pin it).
//! - **`validcount` render-vs-sim dual-bump contract**: `validcount` is
//!   bumped once per frame by `setup_frame` (vanilla-faithful) AND by SIM
//!   code (`p_maputl/intercepts`). Sim code only compares equality and
//!   self-bumps, so render rate never influences sim decisions -- any
//!   temptation to make the counter render-owned or remove the render bump
//!   is a behavior change. Moved verbatim; this paragraph is the contract.
//! - `execute_set_view_size` writes `r_plane::yslope`/`distscale` and
//!   `r_things::pspritescale`/`pspriteiscale`/`screenheightarray`
//!   cross-module; the write order inside the body is golden-bearing and is
//!   carried verbatim.
//! - `init`'s progress-dot interleaving with `c_printf` is
//!   output-observable and kept verbatim; `render_player_view`'s
//!   `NetUpdate` interleave order is demo-machinery glue and kept exact.
//! - `framecount`/`sscount` are profiling counters (`sscount` consumed by
//!   `r_bsp`); `linecount`/`loopcount` are dead-but-exported (zero
//!   consumers in the tree): kept for symbol-set byte-identity, retire
//!   with the freeze zone.
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
//! comes with freeze-zone retirement). Unsafe-signature parity preserved:
//! the seventeen `pub unsafe extern "C"` functions stay unsafe-bodied, the
//! two no-op ABI stubs stay safe `extern "C"`.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_AddPointToBox` | `geometry::add_point_to_box` | glue | bbox growth; automap/`maputil` consumers; shim + pin |
//! | `R_PointOnSide` | `geometry::point_on_side` | dtmc | sign-bit shortcut + cross product is the exact-answer core under `point_in_subsector`'s SIM walk; whole body moved verbatim (nothing extractable beyond it); shim + pin |
//! | `R_PointOnSegSide` | `geometry::point_on_seg_side` | glue | only consumer `r_things::draw_sprite`; shim + pin |
//! | `R_PointToAngle` | `geometry::point_to_angle` | dtmc | view-relative wrapper over the extracted core; SIM consumers: p_enemy, p_pspr, p_user, s_sound, st_stuff; shim + pin |
//! | `R_PointToAngle2` | `geometry::point_to_angle2` | dtmc | heaviest SIM consumer set (p_enemy/p_pspr/p_mobj/p_map/p_inter/p_user/s_sound/st_stuff); de-aliased global trick documented; shim + pin |
//! | `R_PointToDist` | `geometry::point_to_dist` | dtmc | render-only consumer today (`r_segs`) but shares the exact-angle `tantoangle`/`DBITS` contract with the core; `DBITS` lives in `dtmc` beside the vectors; shim + pin |
//! | `R_InitPointToAngle` | `geometry::init_point_to_angle` | glue | no-op ABI stub; shim + pin |
//! | `R_ScaleFromGlobalAngle` | `geometry::scale_from_global_angle` | glue | wrapping ops load-bearing (wraparound test); shim + pin |
//! | `R_InitTables` | `geometry::init_tables` | glue | no-op ABI stub; shim + pin |
//! | `R_InitTextureMapping` | `viewsize::init_texture_mapping` | glue | frame-golden LUT build (viewangletox fencepost cleanup); shim + pin |
//! | `R_InitLightTables` | `viewsize::init_light_tables` | glue | `zlight` precompute; shim + pin |
//! | `R_SetViewSize` | `viewsize::set_view_size` | glue | deferred resize request; `m_menu` + `video_cfg` driven, `tests/video_raster.rs` asserted; shim + pin |
//! | `R_ExecuteSetViewSize` | `viewsize::execute_set_view_size` | glue | frame-golden LUT rebuild; cross-module writes carried in order; shim + pin |
//! | `R_Init` | `frame::init` | glue | boot order is hash-bearing (progress dots); shim + pin |
//! | `R_PointInSubsector` | `geometry::point_in_subsector` | dtmc | SIM spawn/movement placement (`g_game`, `p_mobj`, `p_map`, `p_maputl`); shim + pin |
//! | `R_SetupFrame` | `frame::setup_frame` | glue | render-only; bumps `validcount` (contract above); reads `r_interp::sample_camera` (F1 M1 call point kept exactly); shim + pin |
//! | `R_RenderPlayerView` | `frame::render_player_view` | glue | `NetUpdate` interleave order load-bearing; shim + pin |
//! | (private) `point_to_angle_from_delta` | `dtmc::angle_from_delta` | dtmc | THE wave's only dtmc extraction: pure octant classifier + baseline vectors (c7ab33f written pre-move); no pin (private, no C symbol) |
//! | `R_Main_Link_Anchor` (house) | `anchor::R_Main_Link_Anchor` | house | name + `#[no_mangle]` carried; `doomgeneric.rs:47/:261` wired -- the ONE live renderer anchor |
//! | 35 `#[no_mangle]` statics (`viewangleoffset`..`setdetail`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (SIM `validcount`/`viewangleoffset` consumers, render-tier view globals, `hu_lib`'s neighbors, `c_ffi` re-export block, `tests/video_raster.rs`'s `R_SetViewSize` machinery) |
//! | `FIELDOFVIEW` | `viewsize` | data | beside `init_texture_mapping`, its only reader |
//! | `LIGHTLEVELS`/`LIGHTSEGSHIFT`/`MAXLIGHTSCALE`/`LIGHTSCALESHIFT`/`MAXLIGHTZ`/`LIGHTZSHIFT`/`NUMCOLORMAPS`/`DISTMAP`/`lighttable_t` | `state` | data | `pub(super)` beside the statics they size and the readers in `viewsize`/`frame` |
//! | `DBITS` | `dtmc` | data | beside the extracted core + its 45-degree-index vector; `point_to_dist` reaches it via `super::dtmc` |
//! | `NF_SUBSECTOR` | `geometry` | data | beside `point_in_subsector` |
//!
//! ## Deterministic Aspects
//!
//! The dtmc surface is the exact-angle core: `dtmc::angle_from_delta`
//! classifies a delta vector into octants and reads `tantoangle`; its
//! answers steer SIM turning (`p_enemy` chase/attacks/revenant), autoaim
//! (`p_pspr/weapons`), spawn/movement placement (`R_PointInSubsector` --
//! `g_game/spawn`, `p_mobj/lifecycle`, `p_map/move`, `p_maputl/position`),
//! sound propagation (`s_sound`), and status tracking (`st_stuff`) --
//! demo-visible behavior. `point_on_side`/`point_to_dist` ride the same
//! exact-answer contract. The baseline (octant vectors + cardinals +
//! translation invariance + DBITS + 45-degree index, c7ab33f) pins them.
//! Everything else is render-loop or boot glue: frame-golden-relevant,
//! pinned by the F9 frame goldens (`video_anchor.rs`, scenario
//! `frame_hash`), while `harness_hash`'s state ledger never reads renderer
//! state. The `validcount` dual-bump contract above is the one place the
//! render side touches a SIM-shared counter -- render rate never influences
//! sim decisions because sim code only compares equality.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod anchor;
pub mod dtmc;
pub mod frame;
pub mod geometry;
pub mod state;
pub mod viewsize;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_main::R_*`: the SIM angle/BSP consumers,
//* the render-tier view-global readers, m_menu/video_cfg resize drivers,
//* d_main boot/display, the video_raster sweep). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. No r_main function
//* is extern-declared by freeze-zone files (`d_net` externs only the
//* `viewangleoffset` STATIC), so the pins keep the wasm/cdylib export
//* surface byte-identical (d_main precedent). Shims die with the
//* freeze zone.
pub use anchor::R_Main_Link_Anchor;
pub use frame::{init as R_Init, render_player_view as R_RenderPlayerView, setup_frame as R_SetupFrame};
pub use geometry::{
    add_point_to_box as R_AddPointToBox, init_point_to_angle as R_InitPointToAngle,
    init_tables as R_InitTables, point_in_subsector as R_PointInSubsector,
    point_on_seg_side as R_PointOnSegSide, point_on_side as R_PointOnSide,
    point_to_angle as R_PointToAngle, point_to_angle2 as R_PointToAngle2,
    point_to_dist as R_PointToDist, scale_from_global_angle as R_ScaleFromGlobalAngle,
};
pub use viewsize::{
    execute_set_view_size as R_ExecuteSetViewSize, init_light_tables as R_InitLightTables,
    init_texture_mapping as R_InitTextureMapping, set_view_size as R_SetViewSize,
};

//* path-stability re-export: the 35 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* the SIM tier (`validcount`, `viewangleoffset`), the render tier
//* (`r_bsp` clipangle/sscount/viewangle/viewangletox/viewx/viewy/viewz,
//* `r_segs` centeryfrac/colfunc/extralight/fixedcolormap/scalelight/
//* viewangle/viewz/xtoviewangle, `r_plane` centerxfrac/colfunc/
//* detailshift/extralight/fixedcolormap/spanfunc/viewangle/viewx/viewy/
//* viewz/xtoviewangle/zlight, `r_things` 18 statics, `r_interp`
//* viewplayer), `d_main` boot/display, `g_game/savegentry`, the `c_ffi`
//* re-export block, and the unit tests in `geometry`/`dtmc`/`viewsize`.
pub use state::{
    basecolfunc, centerx, centerxfrac, centery, centeryfrac, clipangle, colfunc, detailshift,
    extralight, fixedcolormap, framecount, fuzzcolfunc, linecount, loopcount, projection,
    scalelight, scalelightfixed, setblocks, setdetail, setsizeneeded, spanfunc, sscount,
    transcolfunc, validcount, viewangle, viewangleoffset, viewangletox, viewcos, viewplayer,
    viewsin, viewx, viewy, viewz, xtoviewangle, zlight,
};
