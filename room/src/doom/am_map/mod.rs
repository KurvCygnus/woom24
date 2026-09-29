//! The fullscreen automap: player arrow, wall lines, grid, zoom/pan,
//! mark points, and crosshair.
//!
//! Rust port of `vendor/doomgeneric/am_map.c`. The automap renders a
//! top-down view of the level geometry directly into the video
//! framebuffer. It maintains its own coordinate system: "map" units
//! (fixed-point, same scale as the game world) and "frame" units
//! (screen pixels within the automap window). Two conversion scale
//! factors, `scale_mtof` (map-to-frame) and `scale_ftom`
//! (frame-to-map), govern the zoom level and are updated by
//! `view::change_window_scale`.
//!
//! Notable Rust-vs-C differences:
//! - All globals use `static mut` with `unsafe` accessors instead of
//!   bare C globals.
//! - Coordinate-conversion macros (`FTOM`, `MTOF`, `CXMTOF`, `CYMTOF`)
//!   are `unsafe` inline functions rather than C preprocessor macros.
//! - The Cohen-Sutherland clip loop in `raster::clip_mline` uses Rust
//!   integer arithmetic; the `OC_*` outcode constants are typed `c_int`.
//! - `AM_Map_Link_Anchor` is a Rust addition: it forces the linker to
//!   retain all exported symbols that would otherwise be dead-stripped.
//! - The house-native reseat trio (`AM_reseatVideoState`,
//!   `am_framebuffer`, `am_window_dims`) has no C origin: it re-seats
//!   the latched video state after a `video_cfg` reconfiguration and
//!   keeps its maintainer hand-pass names (root re-exports hold the
//!   `room/tests/video_raster.rs:80-88` paths).
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- palette/colour constants, `AM_MSG*` values, the
//!   shape tables (`PLAYER_ARROW`/`CHEAT_ARROW`/`TRIANGLE_GUY`/
//!   `THINTRIANGLE_GUY`), the Cohen-Sutherland outcodes, and the
//!   point/line types
//! - `state.rs` -- the ~50 run-time statics (upstream names) including
//!   the two `#[no_mangle]` exports `automapactive` and `cheat_amap`
//! - `view.rs` -- the scale/window family plus the `FTOM`/`MTOF`/
//!   `CXMTOF`/`CYMTOF` conversion helpers
//! - `lifecycle.rs` -- `add_mark`, `init_variables`, `load_pics`,
//!   `unload_pics`, `clear_marks`, `stop`, `start`, `ticker`
//! - `responder.rs` -- `responder` (input surface) and the
//!   `make_cheat_seq` const constructor
//! - `raster.rs` -- `clear_fb`..`drawer` plus the dead
//!   `update_light_lev`
//! - `reseat.rs` -- the house-native video re-seat trio
//! - `anchor.rs` -- the dead-but-exported linker anchor
//!
//! The module has NO extern block: every import is a Rust path
//! (pre-split file `:30-55`). The `automapactive` static is
//! extern-declared by `hu_lib`/`hu_stuff` (link by symbol) and read by
//! Rust path by the freeze zone, `f_finale`, `st_stuff`, and
//! `d_main/display.rs` -- the `#[no_mangle]` + root re-export keep
//! every link resolving.
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
//! oracle `c2rust-intermediate/src/p_inter.rs:24` declares `AM_Stop`
//! and `c2rust-intermediate/src/am_map.rs:23` declares `ST_Responder`'s
//! sibling `AM_Responder` by symbol; the conductors
//! `g_game/responder.rs:16,123`, `g_game/ticker.rs:16,219`,
//! `d_main/display.rs:13,96`, `g_game/actions.rs:21,222`, and
//! `p_inter/damage.rs:13,98-100` reach the renamed functions through
//! these shims). The house-native reseat trio has NO export (plain
//! `pub unsafe fn` / `pub fn`) -- no pins, root re-exports only.
//! Functions only: the statics keep their upstream names AND their
//! `#[no_mangle]` attributes.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `AM_getIslope` | `view::get_islope` | glue | private slope helper of the unused slope-clipping path; doc alias only |
//! | `AM_activateNewScale` | `view::activate_new_scale` | glue | private; also reaches `reseat` |
//! | `AM_saveScaleAndLoc` | `view::save_scale_and_loc` | glue | private; reaches `responder` (maxzoom toggle) |
//! | `AM_restoreScaleAndLoc` | `view::restore_scale_and_loc` | glue | private; reaches `responder` |
//! | `AM_addMark` | `lifecycle::add_mark` | glue | private |
//! | `AM_findMinMaxBoundaries` | `view::find_min_max_boundaries` | glue | private; reaches `level_init` and `reseat` |
//! | `AM_changeWindowLoc` | `view::change_window_loc` | glue | private |
//! | `AM_initVariables` | `lifecycle::init_variables` | glue | sends AM_MSGENTERED; private |
//! | `AM_loadPics` | `lifecycle::load_pics` | glue | private |
//! | `AM_unloadPics` | `lifecycle::unload_pics` | glue | private |
//! | `AM_clearMarks` | `lifecycle::clear_marks` | glue | private |
//! | `AM_LevelInit` | `view::level_init` | glue | private |
//! | `AM_Stop` | `lifecycle::stop` | glue | the misordered AM_MSGEXITED event moves VERBATIM (cataloged vanilla fidelity; see Deterministic Aspects); shim + pin (oracle-declared symbol) |
//! | `AM_Start` | `lifecycle::start` | glue | function-local `lastlevel`/`lastepisode` statics verbatim; shim + pin |
//! | `AM_minOutWindowScale` | `view::min_out_window_scale` | glue | private |
//! | `AM_maxOutWindowScale` | `view::max_out_window_scale` | glue | private |
//! | `AM_Responder` | `responder::responder` | glue | input surface: iddt only cycles the render-only `cheating` counter and forces `rc = 0` (no RNG anywhere in the module, report §5.4); shim + pin |
//! | `AM_changeWindowScale` | `view::change_window_scale` | glue | private |
//! | `AM_doFollowPlayer` | `view::do_follow_player` | glue | private |
//! | `AM_updateLightLev` | `raster::update_light_lev` | glue | dead-but-kept (call site commented `:1575` upstream); `#[allow(dead_code)]` |
//! | `AM_Ticker` | `lifecycle::ticker` | glue | shim + pin |
//! | `AM_clearFB` | `raster::clear_fb` | glue | private |
//! | `AM_clipMline` | `raster::clip_mline` | glue | Cohen-Sutherland clip, buffer-writing (no dtmc extraction -- report §5.4); private |
//! | `AM_drawFline` | `raster::draw_fline` | glue | Bresenham; the `fuck` debug counter static keeps its upstream name; private |
//! | `AM_drawMline` | `raster::draw_mline` | glue | private |
//! | `AM_drawGrid` | `raster::draw_grid` | glue | private |
//! | `AM_drawWalls` | `raster::draw_walls` | glue | private |
//! | `AM_rotate` | `raster::rotate` | glue | private |
//! | `AM_drawLineCharacter` | `raster::draw_line_character` | glue | private |
//! | `AM_drawPlayers` | `raster::draw_players` | glue | private |
//! | `AM_drawThings` | `raster::draw_things` | glue | private |
//! | `AM_drawMarks` | `raster::draw_marks` | glue | private |
//! | `AM_drawCrosshair` | `raster::draw_crosshair` | glue | private |
//! | `AM_Drawer` | `raster::drawer` | glue | reseat detect-by-diff block verbatim; shim + pin |
//! | (Rust addition) `AM_Map_Link_Anchor` | `anchor::amap_link_anchor` | glue | dead-but-exported (zero callers in the tree; the `doomgeneric.rs` anchor list has no am_map entry): kept for symbol-set byte-identity, retires with the freeze zone; shim + pin |
//! | (house) `AM_reseatVideoState`/`am_framebuffer`/`am_window_dims` | `reseat` | glue | house-native (fix round 1, Critical 2); KEEP NAMES, no pins; root re-exports hold the `video_raster.rs` test paths |
//! | macros-as-fns `FTOM`/`MTOF`/`CXMTOF`/`CYMTOF` | `view::ftom`/`mtof`/`cxmtof`/`cymtof` | glue | read `scale_*`/window statics |
//! | statics `automapactive`/`cheat_amap` | `state` | data | `#[no_mangle]` retained; symbol links (hu_lib/hu_stuff extern blocks) and Rust paths both resolve |
//! | the remaining statics | `state` | data | upstream names retained |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the automap draws NO `M_Random` anywhere (report
//! §5.4, verified) -- it never perturbs the RNG ledger; the zoom/pan
//! math is deterministic by construction and not on any hash surface
//! except the frame. The `iddt` cheat only cycles the render-only
//! `cheating` counter and never mutates simulation state. The
//! load-bearing vanilla fidelity here is the AM_Stop/st_stuff event
//! pairing: the misordered AM_MSGEXITED event (`type_ = 0` with
//! `ev_keyup` in `data1`, vendor am_map.c:543) NEVER satisfies
//! `st_stuff`'s `ev.type_ == 1` check and falls into the keydown/cheat
//! path, while `init_variables`' well-formed AM_MSGENTERED
//! (`type_ = 1`) DOES match -- BOTH halves move VERBATIM; any
//! "cleanup" of either side changes the st_gamestate transition
//! behavior. `AM_Stop` is called mid-simulation from
//! `p_inter/damage.rs:100` (god-mode telefrag-class path), so its
//! symbol path must never move.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_char;

pub mod anchor;
pub mod consts;
pub mod lifecycle;
pub mod raster;
pub mod reseat;
pub mod responder;
pub mod state;
pub mod view;

pub use consts::{AM_NUMMARKPOINTS, F_PANINC, INITSCALEMTOF, M_ZOOMIN, M_ZOOMOUT};
pub use state::{automapactive, cheat_amap};

/// Pass-through stub for DEH_String when dehacked patching is disabled.
///
/// In a dehacked build this would look up the string in a replacement table;
/// here it simply returns its argument unchanged.
/// C origin: `DEH_String` macro/function pattern used throughout Chocolate Doom.
///
/// # Safety
///
/// Caller must ensure `s` is either null or a valid C-string pointer; this
/// implementation does not dereference it and simply returns the pointer
/// unchanged.
#[inline(always)]
pub(super) unsafe fn DEH_String(s: *mut c_char) -> *mut c_char {
    s
}

//* upstream-name shim: the five exported entry points + the linker
//* anchor keep their freeze-zone caller paths (`crate::doom::am_map::AM_*`;
//* conductors g_game/responder+ticker+actions, d_main/display,
//* p_inter/damage, and the c2rust oracle) and their C symbols stay
//* re-pinned at the definitions with `#[export_name = "OriginalName"]`.
//* The house-native reseat trio keeps its plain names via plain
//* re-exports (video_raster.rs test paths). Shims die with the freeze
//* zone.
pub use anchor::amap_link_anchor as AM_Map_Link_Anchor;
pub use lifecycle::{start as AM_Start, stop as AM_Stop, ticker as AM_Ticker};
pub use raster::drawer as AM_Drawer;
pub use reseat::{am_framebuffer, am_window_dims, AM_reseatVideoState};
pub use responder::responder as AM_Responder;
