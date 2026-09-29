//! Intermission / victory screen: kill/item/secret percentages, par
//! time, animated episode map backgrounds, and level-entry/exit
//! animations. Supports single-player, cooperative netgame, and
//! deathmatch scoring modes for up to 4 players.
//!
//! Rust port of `vendor/doomgeneric/wi_stuff.c`. The g_game conductor
//! hands the module a `wbstartstruct_t` (ABI-identical to the C struct)
//! through [`lifecycle::start`]; per state-hash tic [`lifecycle::ticker`]
//! advances the count-up state machines and [`lifecycle::drawer`]
//! renders whichever stats page or map view is active.
//!
//! Notable Rust-vs-C differences:
//! - The C `anim_t` struct is split here into an immutable
//!   `anim_config_t` (compile-time data) and a mutable `anim_state_t`
//!   (runtime state), stored in separate `static` arrays. This avoids
//!   the need for `static mut` on the config data and makes borrowing
//!   semantics clearer.
//! - `AnimStateTable` wraps `UnsafeCell` to give interior mutability to
//!   the state arrays, which must be `Sync` for use as module-level
//!   statics.
//! - `DEH_String` and `SHORT` are identity shims (Dehacked and
//!   endianness conversion are not yet active in this port).
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the ABI structs (`wbplayerstruct_t`/`wbstartstruct_t`),
//!   the state enums, the animation config/state types, the geometry
//!   constants, and the runtime-raster-aware `SP_TIMEY()`
//! - `tables.rs` -- the level-node and per-episode animation tables
//!   (`LNODES`, `EPSD*_CONFIG`, `EPSD*_STATE`, `NUMANIMS`) plus the
//!   `anim_config`/`anim_state_ptr` accessors
//! - `state.rs` -- the ~45 run-time statics (upstream names), reached
//!   via `super::state::` from the subfiles
//! - `drawutil.rs` -- background slam, level-finished/entering overlays,
//!   `draw_on_lnode`, and the number/percent/time primitives
//! - `anim.rs` -- the animated background init/update/draw trio (the
//!   RNG-ledger surface; see Deterministic Aspects)
//! - `stats_sp.rs`/`stats_ng.rs`/`stats_dm.rs` -- the three count-up
//!   stats pages
//! - `lifecycle.rs` -- responder/end/ticker/drawer/start, the stats
//!   dispatch, the accelerate poll, and the WAD load/unload walker
//! - `dtmc.rs` -- the extracted anim-delay computation with its
//!   baseline vectors
//!
//! The module has NO extern block: every import is a Rust path
//! (pre-split file `:30-51`). `c_ffi` re-exports the `EPSD*_NANIM`
//! constants through this root; `g_game/state.rs` imports the two ABI
//! structs through it.
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
//! oracle `c2rust-intermediate/src/d_main.rs` declares `WI_Drawer` by
//! symbol; the conductors `d_main/display.rs:37,103`,
//! `g_game/ticker.rs:32,209,222`, and `g_game/actions.rs:37,338` reach
//! the renamed functions through these shims). The private callbacks
//! were never exported -- doc aliases only, no pins. Functions only:
//! statics/consts/tables keep their upstream names (the `EPSD*_NANIM`
//! constants keep their `pub(crate)` visibility for `c_ffi`).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `WI_slamBackground` | `drawutil::slam_background` | glue | base blit for every draw page; shim + pin |
//! | `WI_Responder` | `lifecycle::responder` | glue | constant 0 (all input goes through `check_for_accelerate`); shim + pin |
//! | `WI_drawLF` | `drawutil::draw_level_finished` | glue | deliberate `V_DrawPatch` bounds error for `last > NUMCMAPS` carried VERBATIM -- cataloged vanilla workaround (`docs/vanilla-workarounds.md`, MAP33/NUMCMAPS row); private, doc alias only |
//! | `WI_drawEL` | `drawutil::draw_entering_level` | glue | "Entering" overlay; private |
//! | `WI_drawOnLnode` | `drawutil::draw_on_lnode` | glue | fits-check + `c_printf1` diagnostic; private |
//! | `WI_initAnimatedBack` | `anim::init_animated_back` | dtmc | RNG ledger: one `M_Random` draw per ANIM_ALWAYS/ANIM_RANDOM anim; the delay routes through `dtmc::anim_delay`, the draws stay whole and in order (baseline vectors commit feb6318, pre-move); private |
//! | `WI_updateAnimatedBack` | `anim::update_animated_back` | dtmc | RNG ledger: ANIM_RANDOM cycle retrigger draw; same routing; the ANIM_LEVEL "gawd-awful hack" (`state == StatCount && i == 7`) moves verbatim; private |
//! | `WI_drawAnimatedBack` | `anim::draw_animated_back` | glue | frame blit of started anims; private |
//! | `WI_drawNum` | `drawutil::draw_num` | glue | right-justified digits, 1994 sentinel; private |
//! | `WI_drawPercent` | `drawutil::draw_percent` | glue | private |
//! | `WI_drawTime` | `drawutil::draw_time` | glue | MM:SS walk with the WISUCKS overflow; private |
//! | `WI_End` | `lifecycle::end` | glue | unload on dismissal; shim + pin |
//! | `WI_initNoState` | `lifecycle::init_no_state` | glue | private |
//! | `WI_updateNoState` | `lifecycle::update_no_state` | glue | `G_WorldDone` handoff; private |
//! | `WI_initShowNextLoc` | `lifecycle::init_show_next_loc` | glue | private |
//! | `WI_updateShowNextLoc` | `lifecycle::update_show_next_loc` | glue | pointer blink; private |
//! | `WI_drawShowNextLoc` | `lifecycle::draw_show_next_loc` | glue | splat/yah map marks; private |
//! | `WI_drawNoState` | `lifecycle::draw_no_state` | glue | private |
//! | `WI_fragSum` | `stats_dm::frag_sum` | glue | shared by the dm and ng pages; private |
//! | `WI_initDeathmatchStats` | `stats_dm::init_deathmatch_stats` | glue | private |
//! | `WI_updateDeathmatchStats` | `stats_dm::update_deathmatch_stats` | glue | odd=pause/even=tick state machine; private |
//! | `WI_drawDeathmatchStats` | `stats_dm::draw_deathmatch_stats` | glue | frag matrix; private |
//! | `WI_initNetgameStats` | `stats_ng::init_netgame_stats` | glue | private |
//! | `WI_updateNetgameStats` | `stats_ng::update_netgame_stats` | glue | private |
//! | `WI_drawNetgameStats` | `stats_ng::draw_netgame_stats` | glue | private |
//! | `WI_initStats` | `stats_sp::init_stats` | glue | private |
//! | `WI_updateStats` | `stats_sp::update_stats` | glue | private |
//! | `WI_drawStats` | `stats_sp::draw_stats` | glue | par row gated on `epsd < 3`; private |
//! | `WI_checkForAccelerate` | `lifecycle::check_for_accelerate` | glue | mutates `players[i].attackdown`/`usedown` through the graduated g_game path -- player-struct writes, recorded, not extracted; private |
//! | `WI_Ticker` | `lifecycle::ticker` | glue | `bcnt` + music start + stats dispatch; shim + pin |
//! | `WI_loadUnloadData` | `lifecycle::load_unload_data` | glue | port-extracted walker; the episode-1 anim-8 aliasing hack (`(*st).p[i]` shares episode 1 slot-4 patches) moves verbatim; private |
//! | `WI_loadCallback` | `lifecycle::load_callback` | glue | `LoadCallback` fn-pointer member; private |
//! | `WI_loadData` | `lifecycle::load_data` | glue | allocates `lnames`; shim + pin |
//! | `WI_unloadCallback` | `lifecycle::unload_callback` | glue | fn-pointer member; private |
//! | `WI_unloadData` | `lifecycle::unload_data` | glue | shim + pin |
//! | `WI_Drawer` | `lifecycle::drawer` | glue | per-frame dispatch; shim + pin (oracle-declared symbol) |
//! | `WI_initVariables` | `lifecycle::init_variables` | glue | clamps maxkills/maxitems/maxsecret to >= 1, `epsd -= 3` non-retail fix; private |
//! | `WI_Start` | `lifecycle::start` | glue | `&raw mut wminfo` ABI handoff from `g_game/state.rs:245`; shim + pin |
//! | types `wbplayerstruct_t`/`wbstartstruct_t` | `types` | data | `#[repr(C)]` ABI structs; names unchanged (crossed by pointer into `g_game`); the `statdump` trimmed mirror skew must NOT be unified |
//! | enums/types `stateenum_t`/`point_t`/`animenum_t`/`anim_config_t`/`anim_state_t`/`AnimStateTable` | `types` | data | upstream names kept |
//! | house-native `SP_TIMEY()` | `types` | data | F1 M2 runtime-raster computation; keeps its name |
//! | consts `EPSD0/1/2_NANIM` | `tables` | data | `pub(crate)`, re-exported through this root for `c_ffi.rs:780-784` |
//!
//! ## Deterministic Aspects
//!
//! The dtmc surface is the animated-background RNG ledger: `anim.rs`
//! draws `M_Random` once per ANIM_ALWAYS/ANIM_RANDOM anim at
//! intermission init (`init_animated_back`) and once per ANIM_RANDOM
//! cycle retrigger (`update_animated_back`) -- the only draws in the
//! module (report §0.6), each advancing `rndindex` (state-hash word 2).
//! `dtmc::anim_delay` extracts ONLY the pure `draw % modulus` delay;
//! the draws stay whole, in config order, at their exact call shapes
//! (`bcnt + 1 + delay` at init, `bcnt + data2 + delay` at retrigger) --
//! the baseline vectors (commit feb6318, pre-move transcription) plus
//! the live draw-ledger test in `anim.rs` pin count and order; any
//! shift breaks every frozen state digest. Everything else is
//! frame-golden integer state machines over statics pinned by the F9
//! demo goldens (the intermission sits inside the
//! `demo_playthrough`/`frame_split` windows). There are NO
//! pointer-compares in the module (report §0.2 item 3, verified).

#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    static_mut_refs,
    clippy::missing_safety_doc,
    clippy::not_unsafe_ptr_arg_deref,
    clippy::needless_range_loop,
    clippy::ptr_offset_with_cast,
    clippy::manual_clamp,
    clippy::manual_c_str_literals
)]

use std::ffi::c_char;

pub mod anim;
pub mod dtmc;
pub mod drawutil;
pub mod lifecycle;
pub mod state;
pub mod stats_dm;
pub mod stats_ng;
pub mod stats_sp;
pub mod tables;
pub mod types;

pub(crate) use tables::{EPSD0_NANIM, EPSD1_NANIM, EPSD2_NANIM};
pub use types::{wbplayerstruct_t, wbstartstruct_t};

/// Pass-through shim for Dehacked string replacement (not yet active in this port).
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

/// Identity endian-swap shim; this port runs on little-endian hosts matching the WAD format.
#[inline(always)]
pub(super) fn SHORT(x: i16) -> i16 {
    x
}

//* upstream-name shim: the eight exported entry points keep their
//* freeze-zone caller paths (`crate::doom::wi_stuff::WI_*`; the
//* conductors d_main/display.rs, g_game/ticker.rs, g_game/actions.rs
//* and the c2rust differential oracle) and their C symbols stay
//* re-pinned at the definitions with `#[export_name = "OriginalName"]`.
//* Shims die with the freeze zone.
pub use drawutil::slam_background as WI_slamBackground;
pub use lifecycle::{
    drawer as WI_Drawer, end as WI_End, load_data as WI_loadData,
    responder as WI_Responder, start as WI_Start, ticker as WI_Ticker,
    unload_data as WI_unloadData,
};
