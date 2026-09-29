//! Status bar logic: health, ammo, armor, keys, face widget, palette effects,
//! and cheat-code handling.
//!
//! Rust port of `vendor/doomgeneric/st_stuff.c`. The 32-pixel-tall bar at the
//! bottom of the screen is composed of widget primitives from the graduated
//! `st_lib`. Palette cycling (damage flash, berserk, radiation suit) is driven
//! here by `I_SetPalette`.
//!
//! Notable Rust-vs-C differences:
//! - Cheat-sequence tables are built at compile time with `const fn` helpers
//!   instead of being initialised by `ST_Start`.
//! - `DEH_String` is an identity shim; Dehacked string replacement is not yet
//!   wired up.
//! - `logical_gamemission` is duplicated here (it also lives in `st_stuff.c`
//!   as a macro) to keep the file self-contained.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the ST_* geometry/face constants, the STSTR_* message
//!   strings, the AM_MSG* automap-event values, and the palette indices
//! - `cheats.rs` -- the ten `#[no_mangle]` cheat-sequence statics and
//!   `responder` (the cheat feeder; automap-state events also land here)
//! - `face.rs` -- `calc_pain_offset` (dtmc formula) and `update_face_widget`
//! - `ticker.rs` -- `ticker` (dtmc whole-body, the RNG ledger) and
//!   `update_widgets`
//! - `palette.rs` -- `apply_palette`
//! - `drawer.rs` -- `refresh_background`, `draw_widgets`, `full_redraw`,
//!   `diff_redraw`, `drawer`, and the house-native `force_full_redraw`
//! - `assets.rs` -- the load/unload walker and its callback pair plus the
//!   six `ST_load*`/`ST_unload*` entry points
//! - `lifecycle.rs` -- `init_data`, `create_widgets`, `start`, `stop`, `init`
//! - `dtmc.rs` -- the extracted pain-offset formula and the baseline
//!   vectors + the ST_Ticker ledger vector
//!
//! The module root holds the shared state every subfile reaches via
//! `super::`: the player pointer, the widget/table patch statics, the
//! draw-state flags, and the ten `w_*` widget statics. This mirrors the
//! landed shared-vocabulary amendment (module-root state beside the doc
//! templates), and the upstream names are retained so the extern-declarer
//! web (`hu_stuff`'s extern block links `showMessages` into `m_menu`, not
//! here; `video_cfg.rs:312` reads `force_full_redraw` by root path) keeps
//! resolving.
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
//! oracle declares `ST_Drawer`/`ST_Init` by symbol; the conductors
//! `g_game/responder.rs`, `g_game/ticker.rs`, `p_mobj/mapthings.rs`,
//! `d_main/boot.rs`, `d_main/display.rs`, and `am_map/lifecycle.rs`
//! reach the renamed functions through these shims). The three private
//! walker callbacks were never exported -- doc aliases only, no pins.
//! Functions only: the statics keep their upstream names AND their
//! `#[no_mangle]` attributes (`st_backing_screen` + the ten cheat
//! sequences). `force_full_redraw` is house-native and keeps its name.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `ST_refreshBackground` | `drawer::refresh_background` | glue | backing-buffer blit; shim + pin |
//! | `ST_Responder` | `cheats::responder` | glue | cheat feeder + AM_MSG automap-state events; sim mutations ARE the behavior (matcher input is platform keys, never the demo stream); shim + pin |
//! | `ST_calcPainOffset` | `face::calc_pain_offset` | dtmc | the face-row formula extracted to `dtmc::pain_offset` (baseline f8c7e47); the `lastcalc`/`oldhealth` cache stays at the call site; shim + pin |
//! | `ST_updateFaceWidget` | `face::update_face_widget` | glue | priority cascade, verbatim; shim + pin |
//! | `ST_updateWidgets` | `ticker::update_widgets` | glue | widget pointer refresh + 1994 sentinel; shim + pin |
//! | `ST_Ticker` | `ticker::ticker` | dtmc | whole-body dtmc: exactly one `M_Random` draw per tic between the `st_clock` increment and `update_widgets` (report §0.6; ledger vector in `dtmc.rs`); shim + pin |
//! | `ST_doPaletteStuff` | `palette::apply_palette` | glue | palette priority cascade incl. the Chex red→RADIATIONPAL substitution (content behavior, not a workaround row); shim + pin |
//! | `ST_drawWidgets` | `drawer::draw_widgets` | glue | the st_lib update fan-out; shim + pin |
//! | `ST_doRefresh` | `drawer::full_redraw` | glue | shim + pin |
//! | `ST_diffDraw` | `drawer::diff_redraw` | glue | shim + pin |
//! | `ST_Drawer` | `drawer::drawer` | glue | per-frame entry; shim + pin (oracle-declared symbol) |
//! | `ST_loadUnloadGraphics` | `assets::load_unload_graphics` | glue | private walker; doc alias only |
//! | `ST_loadCallback` | `assets::load_callback` | glue | private; doc alias only |
//! | `ST_loadGraphics` | `assets::load_graphics` | glue | shim + pin |
//! | `ST_loadData` | `assets::load_data` | glue | shim + pin |
//! | `ST_unloadCallback` | `assets::unload_callback` | glue | private; doc alias only |
//! | `ST_unloadGraphics` | `assets::unload_graphics` | glue | shim + pin |
//! | `ST_unloadData` | `assets::unload_data` | glue | shim + pin |
//! | `ST_initData` | `lifecycle::init_data` | glue | shim + pin |
//! | `ST_createWidgets` | `lifecycle::create_widgets` | glue | pointer wiring into `plyr` arrays; shim + pin |
//! | `ST_Start` | `lifecycle::start` | glue | shim + pin (`p_mobj/mapthings.rs`) |
//! | `ST_Stop` | `lifecycle::stop` | glue | shim + pin |
//! | `ST_Init` | `lifecycle::init` | glue | shim + pin (oracle-declared symbol) |
//! | (house) `force_full_redraw` | `drawer::force_full_redraw` | glue | house-native (video_cfg re-arm latch); root re-export holds the path |
//! | static `st_backing_screen` | module root | data | `#[no_mangle]` retained; `st_lib`'s erase blits read it |
//! | statics `cheat_mus`..`cheat_mypos` | `cheats` | data | ten `#[no_mangle]` cheat sequences retained |
//! | the shared state below | module root | data | upstream names retained (`plyr`, `st_*` flags, patch tables, `w_*` widgets) |
//!
//! ## Deterministic Aspects
//!
//! The dtmc surface is the RNG ledger plus the face formula. `ticker::ticker`
//! draws exactly one `M_Random()` per tic (the `st_randomnumber` sample for
//! the idle straight-face pick) between the `st_clock` increment and
//! `update_widgets` -- the draw's position between state-hash-visible writes
//! is the observable, pinned by the live ledger vector in `dtmc.rs`
//! (report §0.6: these are the only M_Random-drawing functions in the wave,
//! and the draw count/order must never move). `dtmc::pain_offset` is the pure
//! face-row formula `ST_FACESTRIDE * (((100 - health) * ST_NUMPAINFACES) /
//! 101)`, extracted from `calc_pain_offset` with baseline vectors written
//! pre-move (commit f8c7e47); the `lastcalc`/`oldhealth` cache stays at the
//! call site, and the `> 100 -> 100` clamp remains there as the cache-key
//! decision (negative health flows through, upstream-faithful).
//! `cheats::responder` mutates `plyr` (god/idkfa/idfa/noclip/idclev) and
//! calls `G_DeferedInitNew`, but its input comes from platform keys, never
//! the demo stream (`m_cheat` adjudication) -- glue, recorded, not
//! extracted. Everything else is frame-golden drawing, palette, and WAD
//! caching pinned by the F9 frame goldens (the status bar sits inside the
//! `frame_hash` anchors of `e1m1_combat`/`title_attract` and scenario flow 5).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;
use std::ptr;

use crate::doom::d_player::{PlayerT, NUMAMMO, NUMCARDS, NUMWEAPONS};
use crate::doom::st_lib::{st_binicon_t, st_multicon_t, st_number_t, st_percent_t};
use crate::doom::v_video::patch_t;

pub mod assets;
pub mod cheats;
pub mod consts;
pub mod dtmc;
pub mod drawer;
pub mod face;
pub mod lifecycle;
pub mod palette;
pub mod ticker;

/// Pass-through shim for Dehacked string replacement.
///
/// In the C codebase, `DEH_String` may substitute a string that was patched by
/// a `.deh` file. This port does not yet support Dehacked, so the function
/// returns its argument unchanged.
///
/// # Safety
///
/// Caller must ensure `s` is either null or a valid C-string pointer; this
/// implementation does not dereference it and simply returns the pointer
/// unchanged.
#[inline(always)]
pub(super) unsafe fn DEH_String(s: *mut std::ffi::c_char) -> *mut std::ffi::c_char {
    s
}

/// Return the canonical game mission, collapsing Chex Quest and HacX aliases.
///
/// Mirrors the `logical_gamemission` macro from `doomstat.h`:
/// - `pack_chex` maps to `doom`.
/// - `pack_hacx` maps to `doom2`.
/// - All other values are returned as-is.
///
/// # Safety
///
/// Reads the global `gamemission` static. Caller must ensure `D_DoomMain` has
/// initialised the game-mode globals before calling.
pub(super) unsafe fn logical_gamemission() -> c_int {
    if crate::doom::doomstat::gamemission == crate::doom::d_mode::pack_chex {
        crate::doom::d_mode::doom
    } else if crate::doom::doomstat::gamemission == crate::doom::d_mode::pack_hacx {
        crate::doom::d_mode::doom2
    } else {
        crate::doom::doomstat::gamemission
    }
}

// ---------------------------------------------------------------------------
// Shared state (module root): every subfile reaches these via `super::`.
// ---------------------------------------------------------------------------

/// Pointer to the local player's `PlayerT` struct; set by `lifecycle::init_data`.
pub(super) static mut plyr: *mut PlayerT = ptr::null_mut();

/// Non-zero when the status bar needs a full redraw on the next `drawer` call.
pub(super) static mut st_firsttime: c_int = 0;

/// WAD lump number of `PLAYPAL`, cached by `assets::load_data` for palette lookups.
pub(super) static mut lu_palette: c_int = 0;

/// Monotonically increasing tic counter incremented by `ticker::ticker`.
pub(super) static mut st_clock: u32 = 0;

/// Countdown used to temporarily suppress chat-message override of the HUD message.
pub(super) static mut st_msgcounter: c_int = 0;

/// Current chat state: 0 = `StartChatState`, used to track HUD message display mode.
pub(super) static mut st_chatstate: c_int = 0; // StartChatState = 0

/// Current view state: 0 = automap active, 1 = first-person view.
pub(super) static mut st_gamestate: c_int = 0; // AutomapState = 0

/// Non-zero when the status bar should be rendered (i.e. not in full-screen mode).
pub(super) static mut st_statusbaron: c_int = 0;

/// Non-zero while a chat message is being composed.
pub(super) static mut st_chat: c_int = 0;

/// Previous value of `st_chat`; restored after `st_msgcounter` expires.
pub(super) static mut st_oldchat: c_int = 0;

/// Blink flag for the chat cursor; toggled by `st_msgcounter`.
pub(super) static mut st_cursoron: c_int = 0;

/// Non-zero in cooperative (non-deathmatch) games; gates the weapons-owned display.
pub(super) static mut st_notdeathmatch: c_int = 0;

/// Non-zero when the weapons-owned display should be shown.
pub(super) static mut st_armson: c_int = 0;

/// Non-zero when the frag counter should be shown (deathmatch only).
pub(super) static mut st_fragson: c_int = 0;

/// Pointer to the `STBAR` background patch.
pub(super) static mut sbar: *mut patch_t = ptr::null_mut();

/// Tall digit patches 0-9 (`STTNUM0`-`STTNUM9`), used for health, armor, and ammo.
pub(super) static mut tallnum: [*mut patch_t; 10] = [ptr::null_mut(); 10];

/// Tall percent-sign patch (`STTPRCNT`).
pub(super) static mut tallpercent: *mut patch_t = ptr::null_mut();

/// Short digit patches 0-9 (`STYSNUM0`-`STYSNUM9`), used in the per-ammo sidebar.
pub(super) static mut shortnum: [*mut patch_t; 10] = [ptr::null_mut(); 10];

/// Key icon patches, one per `NUMCARDS` card type.
pub(super) static mut keys: [*mut patch_t; NUMCARDS] = [ptr::null_mut(); NUMCARDS];

/// All face patches in a flat array; indexed by `st_faceindex`.
pub(super) static mut faces: [*mut patch_t; consts::ST_NUMFACES as usize] =
    [ptr::null_mut(); consts::ST_NUMFACES as usize];

/// Network-game face-background patch (`STFB#` where `#` is the console player index).
pub(super) static mut faceback: *mut patch_t = ptr::null_mut();

/// Arms-grid background patch (`STARMS`).
pub(super) static mut armsbg: *mut patch_t = ptr::null_mut();

/// Arms patches: `arms[weapon][0]` = dim `STGNUM#`, `arms[weapon][1]` = bright short digit.
pub(super) static mut arms: [[*mut patch_t; 2]; 6] = [[ptr::null_mut(); 2]; 6];

/// Ready-ammo number widget state.
pub(super) static mut w_ready: st_number_t = unsafe { std::mem::zeroed() };
/// Frag-count number widget state (deathmatch only).
pub(super) static mut w_frags: st_number_t = unsafe { std::mem::zeroed() };
/// Health percent widget state.
pub(super) static mut w_health: st_percent_t = unsafe { std::mem::zeroed() };
/// Arms-background binary icon widget state.
pub(super) static mut w_armsbg: st_binicon_t = unsafe { std::mem::zeroed() };
/// Per-weapon multi-icon widget states (weapons 2-7).
pub(super) static mut w_arms: [st_multicon_t; 6] = unsafe { std::mem::zeroed() };
/// Face multi-icon widget state.
pub(super) static mut w_faces: st_multicon_t = unsafe { std::mem::zeroed() };
/// Key-slot multi-icon widget states (three slots: blue, yellow, red).
pub(super) static mut w_keyboxes: [st_multicon_t; 3] = unsafe { std::mem::zeroed() };
/// Armor percent widget state.
pub(super) static mut w_armor: st_percent_t = unsafe { std::mem::zeroed() };
/// Per-ammo-type current-ammo number widget states.
pub(super) static mut w_ammo: [st_number_t; NUMAMMO] = unsafe { std::mem::zeroed() };
/// Per-ammo-type max-ammo number widget states.
pub(super) static mut w_maxammo: [st_number_t; NUMAMMO] = unsafe { std::mem::zeroed() };

/// Running frag total for the local player, updated each tic.
pub(super) static mut st_fragscount: c_int = 0;

/// Player health from the previous tic; used to detect large drops for the ouch face.
pub(super) static mut st_oldhealth: c_int = -1;

/// Snapshot of which weapons the player owned at the previous tic; detects new pickups.
pub(super) static mut oldweaponsowned: [c_int; NUMWEAPONS] = [0; NUMWEAPONS];

/// Countdown controlling how many tics the current face expression persists.
pub(super) static mut st_facecount: c_int = 0;

/// Current index into `faces[]` that the face widget displays.
pub(super) static mut st_faceindex: c_int = 0;

/// Key-slot values: the card/skull index to display, or -1 for empty.
pub(super) static mut keyboxes: [c_int; 3] = [0; 3];

/// Random number sampled from `M_Random` each tic to vary the idle straight-face frame.
pub(super) static mut st_randomnumber: c_int = 0;

/// Last palette index passed to `I_SetPalette`; avoids redundant calls.
pub(super) static mut st_palette: c_int = 0;

/// Non-zero when the status bar subsystem has been stopped via `lifecycle::stop`.
pub(super) static mut st_stopped: c_int = 1;

/// Backing pixel buffer for the status bar, allocated in `lifecycle::init`.
///
/// Exported as `st_backing_screen` for C callers. The buffer is `ST_WIDTH *
/// ST_HEIGHT` bytes and is used by `drawer::refresh_background` to save and
/// restore the pixels beneath the bar.
#[no_mangle]
pub static mut st_backing_screen: *mut u8 = ptr::null_mut();

//* upstream-name shim: the twenty-one exported entry points keep their
//* freeze-zone caller paths (`crate::doom::st_stuff::ST_*`: the
//* conductors g_game/responder+ticker, p_mobj/mapthings, d_main/boot+
//* display, am_map's AM_MSG forwarder, video_cfg's force_full_redraw)
//* and their C symbols stay re-pinned at the definitions with
//* `#[export_name = "OriginalName"]` for the differential oracle.
//* Shims die with the freeze zone.
pub use assets::{
    load_data as ST_loadData, load_graphics as ST_loadGraphics, unload_data as ST_unloadData,
    unload_graphics as ST_unloadGraphics,
};
pub use cheats::responder as ST_Responder;
pub use drawer::{
    diff_redraw as ST_diffDraw, draw_widgets as ST_drawWidgets, drawer as ST_Drawer,
    full_redraw as ST_doRefresh, refresh_background as ST_refreshBackground, force_full_redraw,
};
pub use face::{calc_pain_offset as ST_calcPainOffset, update_face_widget as ST_updateFaceWidget};
pub use lifecycle::{
    create_widgets as ST_createWidgets, init as ST_Init, init_data as ST_initData,
    start as ST_Start, stop as ST_Stop,
};
pub use palette::apply_palette as ST_doPaletteStuff;
pub use ticker::{ticker as ST_Ticker, update_widgets as ST_updateWidgets};
