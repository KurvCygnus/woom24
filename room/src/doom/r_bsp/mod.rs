//! BSP traversal and line-seg clipping for the Doom renderer.
//!
//! Corresponds to `vendor/doomgeneric/r_bsp.c`. Walks the BSP tree
//! front-to-back from the player's viewpoint, renders each subsector leaf,
//! and maintains the solid-column occlusion list (`solidsegs`) so that
//! already-covered screen columns are never redrawn.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the `r_defs.h` mirror vocabulary (`vertex_t`,
//!   `sector_t`, `side_t`, `slopetype_t`, `line_t`, `subsector_t`,
//!   `seg_t`, `node_t`, `drawseg_t`, the opaque `mobj_t`/`thinker_t`
//!   stubs), `ZERO_DRAWSEG`, the compile-time `layout_checks`, and the
//!   dead `visplane_t` mirror (carried verbatim, see mapping table)
//! - `state.rs` -- the three constants, the diagnostic probe counter +
//!   `seg_index` helper, `cliprange_t`, the 7 `#[no_mangle]` statics, and
//!   the private `solidsegs` / `newend` / `CHECKCOORD`
//! - `clipper.rs` -- `clear_clip_segs`, `clip_solid_wall_segment`,
//!   `clip_pass_wall_segment`, `add_line` (private), `check_bbox`
//! - `traverse.rs` -- `clear_draw_segs`, `subsector`, `render_bsp_node`
//! - `anchor.rs` -- the module link anchor (existed pre-split; carried)
//!
//! The module root is documentation + wiring + the type re-exports. The
//! relay contracts keep their upstream paths: `r_main` imports
//! `node_t`/`seg_t`/`subsector_t` and drives
//! `R_ClearClipSegs` -> `R_ClearDrawSegs` -> `R_RenderBSPNode` inside
//! `R_RenderPlayerView`; `r_segs` consumes
//! `curline`/`sidedef`/`linedef`/`frontsector`/`backsector`/`drawsegs`/
//! `ds_p` (r_bsp writes `rw_angle1` and calls `R_StoreWallRange` -- the
//! module-pair contract); `r_things` imports `sector_t` and scans
//! `drawsegs`/`ds_p` back-to-front during sprite clipping; `r_interp`
//! asserts `sector_t` size/offsets by path (a hard layout contract the
//! `types.rs` move must not break).
//!
//! # Rust-vs-C differences
//!
//! - `CHECKCOORD` is 12 rows vs C's 11-entry array (row 11 is zero
//!   padding; rows 3/7/5 unreachable) -- the full reachability note sits
//!   on the table in `state.rs`.
//! - The `BBox` TOP/BOTTOM/LEFT/RIGHT ordering bug history note (a port
//!   bug post-mortem, load-bearing for `check_bbox`) sits in
//!   `clipper.rs` and is kept verbatim.
//! - The diagnostic trace probes (`PROBE_FRAME`, `seg_index`, the
//!   `RUST_LOG=room::doom::r_bsp=trace` gating) carry unchanged.
//! - The private `visplane_t` mirror (plus its `SCREENWIDTH_RP` cap) has
//!   ZERO uses -- no r_bsp function ever dereferences a visplane. The
//!   investigation report PROPOSED deleting it as behavior-neutral dead
//!   code; the graduation CARRIES it verbatim instead (deletion deferred
//!   to freeze-zone retirement; "graduation moves, does not improve").
//! - No `#[no_mangle]` exists on `R_AddLine` today (private in C and
//!   Rust) -- renamed with a doc alias only, no pin.
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
//! | `R_ClearDrawSegs` | `traverse::clear_draw_segs` | glue | per-frame rewind of `ds_p`; shim + pin |
//! | `R_ClearClipSegs` | `clipper::clear_clip_segs` | glue | sentinel init; keeps the `PROBE_FRAME` trace logging verbatim; shim + pin |
//! | `R_ClipSolidWallSegment` | `clipper::clip_solid_wall_segment` | glue | solidsegs insert + the C `goto crunch` compaction ladder; frame-golden; too static-coupled to extract; shim + pin |
//! | `R_ClipPassWallSegment` | `clipper::clip_pass_wall_segment` | glue | shim + pin |
//! | `R_AddLine` (C static) | `clipper::add_line` | glue | backface cull + frustum clip + `viewangletox` projection + solid/pass classification (interp-sampled closed-door/window heights); private before and after (doc alias only, no pin -- no_mangle absent today) |
//! | `R_CheckBBox` | `clipper::check_bbox` | glue | `CHECKCOORD` corner selection + angle-span clip + solidsegs coverage test; shim + pin |
//! | `R_Subsector` | `traverse::subsector` | glue | visplane registration via r_interp samples; shim + pin |
//! | `R_RenderBSPNode` | `traverse::render_bsp_node` | glue | recursive front-to-back walk; `NF_SUBSECTOR` wrapping-safe `u32` arithmetic carried (never "idiomatically cleaned" into panicking forms); shim + pin |
//! | `R_Bsp_Link_Anchor` (house) | `anchor::R_Bsp_Link_Anchor` | house | anchor EXISTS -> name + `#[no_mangle]` carried, body re-pointed at the renamed fns through the root shims |
//! | 7 `#[no_mangle]` statics (`curline`..`ds_p`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`r_segs` pair contract, `r_things` sprite-clip scan, `r_main` frame sequence) |
//! | private `solidsegs`/`newend`/`CHECKCOORD`/`PROBE_FRAME`/`seg_index`/`cliprange_t` + 3 constants | `state` | data | `pub(super)` beside their only writers (`clipper`/`traverse`) |
//! | `vertex_t`..`node_t`, `drawseg_t` + `ZERO_DRAWSEG` + `layout_checks` | `types` | data | carried verbatim; never unify with the `c_ffi`/`p_lights`/`r_things` copies -- layout identity pinned three ways (`layout_checks`, `r_interp.rs`, `c_tests/struct_layouts.rs`) |
//! | dead private `visplane_t` mirror + `SCREENWIDTH_RP` | `types` | data | zero uses (report finding); deletion PROPOSED by the report and DEFERRED by graduation -- carried verbatim, retires with the freeze zone |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: frames are not the demo sync surface. This module
//! shapes the *presented frame* -- the occlusion clipper decides which
//! wall columns render at all, the traversal order decides what is drawn
//! first, and the visplane registration feeds the floor/ceiling pass --
//! pinned by the F9 frame goldens (`video_anchor.rs`, scenario
//! `frame_hash`), while `harness_hash`'s state ledger never reads
//! renderer state. The r_interp sampling call points (closed-door /
//! window classification, subsector plane registration) are the
//! uncapped-FPS interpolation contract and keep their positions exactly;
//! the frame goldens at sampling-disabled and sampled fractions pin them.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod anchor;
pub mod clipper;
pub mod state;
pub mod traverse;
pub mod types;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_bsp::R_*`: r_main's frame sequence
//* drives ClearClipSegs/ClearDrawSegs/RenderBSPNode). The C symbol each
//* shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. There are no C
//* referencers in the default build, so the pins are wasm-surface
//* conservatism. Shims die with the freeze zone.
pub use clipper::{
    check_bbox as R_CheckBBox, clear_clip_segs as R_ClearClipSegs,
    clip_pass_wall_segment as R_ClipPassWallSegment,
    clip_solid_wall_segment as R_ClipSolidWallSegment,
};
pub use traverse::{
    clear_draw_segs as R_ClearDrawSegs, render_bsp_node as R_RenderBSPNode,
    subsector as R_Subsector,
};

//* path-stability re-export: the 7 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* r_segs reads/writes the seg relay (curline/sidedef/linedef/
//* frontsector/backsector + drawsegs/ds_p), r_things scans drawsegs/ds_p
//* during sprite clipping.
pub use state::{backsector, curline, drawsegs, ds_p, frontsector, linedef, sidedef};

//* path-stability re-export: the type vocabulary keeps its module-root
//* paths (`r_main` imports node_t/seg_t/subsector_t, `r_things` sector_t
//* and subsector_t, `r_segs` drawseg_t, and `r_interp` asserts
//* `sector_t` size/offsets by full path -- a hard layout contract).
pub use types::{drawseg_t, node_t, sector_t, seg_t, subsector_t};
