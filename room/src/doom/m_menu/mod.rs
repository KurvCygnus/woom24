//! Main menu system: episode selection, skill level, options,
//! load/save game, and "Read This" help screens. Also handles keyboard,
//! mouse, and joystick navigation and all related F-key shortcuts
//! (quicksave, quickload, gamma, screenshot, etc.).
//!
//! Rust port of `vendor/doomgeneric/m_menu.c`. The menu sits BEFORE
//! `G_Responder` in the event chain (`d_main/events.rs:30`) -- its
//! consume decision changes which events the simulation sees; scenario
//! flows 2 (restart_flow: ESC/ENTER menu-driven game start) and 5
//! (save_load_roundtrip: save-name typing) are this module's gate.
//!
//! Notable Rust-vs-C differences:
//! - Menu item arrays (`MainMenu`, `EpisodeMenu`, etc.) are `static mut`
//!   values rather than global arrays; the `menuitems` pointers inside each
//!   `menu_t` are wired up at runtime in `lifecycle::init` because raw-pointer
//!   field initializers cannot reference other statics in Rust's const
//!   evaluator.
//! - `make_gamma_msg` and `mi` are `const fn` helpers replacing C compound
//!   literals, keeping the lookup tables in static storage.
//! - `logical_gamemission` has a safe signature (`fn`, not `unsafe fn`); the
//!   C-linked global read is wrapped in an internal `unsafe { ... }` block.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- `menuitem_t`/`menu_t` (model-aware size guards),
//!   `patch_stub`, and the `mi`/`make_gamma_msg` const constructors
//! - `consts.rs` -- the key/event constants and the menu item indices
//! - `tables.rs` -- the nine item arrays + nine page descriptors,
//!   `gammamsg` (workaround entry 9's bytes -- path AND content pinned
//!   by `g_game/dtmc.rs`), `skullName`, and the quit-sound tables
//! - `state.rs` -- the remaining statics (upstream names), including the
//!   `#[no_mangle]` config/video surface (`mouseSensitivity`,
//!   `showMessages`, `detailLevel`, `screenblocks`) and the modal
//!   message state
//! - `saveload.rs` -- the save/load pages and the quicksave/quickload
//!   flows
//! - `pages.rs` -- the 33 item callbacks and the page draw helpers
//! - `text.rs` -- the HUD-font text measurement/rendering family
//! - `message.rs` -- the modal overlay message state machine
//! - `responder.rs` -- `responder` (the input surface) and
//!   `start_control_panel`
//! - `lifecycle.rs` -- `drawer`, `clear_menus`, `setup_next_menu`,
//!   `ticker`, `init`
//!
//! The module root holds the shared vocabulary (`logical_gamemission`)
//! and the libc extern block carried VERBATIM from the pre-split file
//! (`fopen`/`fread`/`fclose`/`toupper`/`strlen`/`strcmp`), reached via
//! `super::` from the subfiles. The statics keep their upstream names
//! so every consumer keeps resolving: `video_cfg.rs:299-300` and
//! `r_main/frame.rs` read `screenblocks`/`detailLevel` by root path;
//! `d_main/bind.rs:20,69-74` binds all four config statics by path
//! while `m_config.rs:184` binds `"screenblocks"` by C-STRING NAME;
//! `hu_stuff` extern-links `showMessages` BY SYMBOL;
//! `g_game/dtmc.rs:25` consumes `gammamsg` (workaround entry 9); the
//! room test crate restores `screenblocks`/`detailLevel`
//! (`video_raster.rs:291-292`).
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the upstream-name shim re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module -- with ONE
//! brief-mandated addition: `M_QuitResponse` carries
//! `#[export_name = "M_QuitResponse"]` because the
//! `messageRoutine == M_QuitResponse` function-address compare
//! (`responder.rs`, carried VERBATIM from the pre-split m_menu.rs:1539-1543) needs
//! the pinned symbol to be the compare's stable anchor (the C symbol
//! is exported too; the compare rides the root shim so set-site and
//! compare-site reference the same function item). The conductors
//! (`d_main/events.rs:10,30`, `d_main/display.rs:27,98,128,139,141,
//! 156,200`, `d_main/boot.rs:42,486`, `g_game/responder.rs:22,113`,
//! `p_tick/ticker.rs:13,41`) reach the renamed functions through the
//! shims, and `M_Ticker`'s pin is MANDATORY: `d_loop/mod.rs:331` and
//! `d_net/mod.rs:174` extern-declare it by symbol and
//! `d_net/loop_table.rs:113` binds its ADDRESS into the loop table
//! (`RunMenu: Some(M_Ticker)`). The 33 non-mangled `extern "C"` item
//! callbacks (menu-table members) were never exported -- doc aliases
//! only, no pins (M_QuitResponse excepted per the brief ruling).
//! Functions only: the statics keep their upstream names AND their
//! `#[no_mangle]` attributes.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_ReadSaveStrings` | `saveload::read_save_strings` | glue | private; libc stdio through the root extern block |
//! | `M_DrawLoad` | `saveload::draw_load` | glue | shim + pin not needed (never exported); doc alias only |
//! | `M_DrawSaveLoadBorder` | `saveload::draw_save_load_border` | glue | private |
//! | `M_LoadSelect` | `saveload::load_select` | glue | `G_LoadGame` simulation TRANSITION from UI -- the call IS the behavior |
//! | `M_LoadGame` | `saveload::load_game` | glue | |
//! | `M_DrawSave` | `saveload::draw_save` | glue | blinking cursor when editing |
//! | `M_DoSave` | `saveload::do_save` | glue | private; `G_SaveGame` transition + quicksave-slot latch |
//! | `M_SaveSelect` | `saveload::save_select` | glue | string-edit mode entry |
//! | `M_SaveGame` | `saveload::save_game` | glue | |
//! | `M_QuickSave` | `saveload::quick_save` | glue | private |
//! | `M_QuickSaveResponse` | `saveload::quick_save_response` | glue | |
//! | `M_QuickLoad` | `saveload::quick_load` | glue | private |
//! | `M_QuickLoadResponse` | `saveload::quick_load_response` | glue | |
//! | `M_DrawReadThis1` | `pages::draw_read_this1` | glue | per-gameversion lump/skull selection |
//! | `M_DrawReadThis2` | `pages::draw_read_this2` | glue | |
//! | `M_DrawSound` | `pages::draw_sound` | glue | |
//! | `M_Sound` | `pages::sound_item` | glue | no-op item callback |
//! | `M_SfxVol` | `pages::sfx_vol` | glue | |
//! | `M_MusicVol` | `pages::music_vol` | glue | |
//! | `M_DrawMainMenu` | `pages::draw_main_menu` | glue | |
//! | `M_DrawNewGame` | `pages::draw_new_game` | glue | |
//! | `M_NewGame` | `pages::new_game` | glue | |
//! | `M_DrawEpisode` | `pages::draw_episode` | glue | |
//! | `M_VerifyNightmare` | `pages::verify_nightmare` | glue | `G_DeferedInitNew` transition |
//! | `M_ChooseSkill` | `pages::choose_skill` | glue | `G_DeferedInitNew` transition |
//! | `M_Episode` | `pages::episode` | glue | shareware/registered restrictions |
//! | `M_DrawOptions` | `pages::draw_options` | glue | |
//! | `M_Options` | `pages::options_item` | glue | no-op item callback |
//! | `M_ChangeMessages` | `pages::change_messages` | glue | |
//! | `M_EndGameResponse` | `pages::end_game_response` | glue | |
//! | `M_EndGame` | `pages::end_game` | glue | |
//! | `M_ReadThis` | `pages::read_this` | glue | no-op item callback |
//! | `M_ReadThis2` | `pages::read_this2` | glue | |
//! | `M_FinishReadThis` | `pages::finish_read_this` | glue | no-op item callback |
//! | `M_QuitResponse` | `pages::quit_response` | glue | quit-sound/quip pick is gametic-keyed (`(gametic >> 2) & 7`), NOT RNG (matches the dstrings corrected record); PINNED per the brief (function-address compare anchor) |
//! | `M_SelectEndMessage` | `pages::select_end_message` | glue | private; gametic-keyed quip pick |
//! | `M_QuitDOOM` | `pages::quit_doom` | glue | sets the compare's other half via `Some(quit_response)` |
//! | `M_ChangeSensitivity` | `pages::change_sensitivity` | glue | |
//! | `M_ChangeDetail` | `pages::change_detail` | glue | |
//! | `M_SizeDisplay` | `pages::size_display` | glue | screenblocks slider |
//! | `M_DrawThermo` | `pages::draw_thermo` | glue | private thermometer slider |
//! | `M_DrawEmptyCell` | `pages::draw_empty_cell` | glue | dead no-op, kept (report §8.1) |
//! | `M_DrawSelCell` | `pages::draw_sel_cell` | glue | dead no-op, kept |
//! | `M_StartMessage` | `message::start_message` | glue | private |
//! | `M_StopMessage` | `message::stop_message` | glue | private |
//! | `M_StringWidth` | `text::string_width` | glue | private; static-reading glue (no dtmc extraction, report §8.4) |
//! | `M_StringHeight` | `text::string_height` | glue | private |
//! | `M_WriteText` | `text::write_text` | glue | private |
//! | `IsNullKey` | `text::is_null_key` | glue | private Rust addition |
//! | `M_Responder` | `responder::responder` | glue | input surface: sits BEFORE G_Responder (d_main/events.rs:30); input keys are platform events, never demo ticcmds; shim + pin |
//! | `M_StartControlPanel` | `responder::start_control_panel` | glue | shim + pin |
//! | `M_Drawer` | `lifecycle::drawer` | glue | shim + pin (oracle-declared symbol) |
//! | `M_ClearMenus` | `lifecycle::clear_menus` | glue | private |
//! | `M_SetupNextMenu` | `lifecycle::setup_next_menu` | glue | private |
//! | `M_Ticker` | `lifecycle::ticker` | glue | skull animation; shim + pin MANDATORY (extern declarers + loop-table address binding) |
//! | `M_Init` | `lifecycle::init` | glue | runtime `menuitems`/`prevMenu` wiring + commercial surgery verbatim; shim + pin (oracle-declared symbol) |
//! | statics `mouseSensitivity`/`showMessages`/`detailLevel`/`screenblocks`/`gammamsg`/`inhelpscreens`/`menuactive`/`currentMenu` | `state`/`tables` | data | `#[no_mangle]` retained; the config/video/raster/gammamsg consumer web binds by name |
//! | the remaining statics | `state`/`responder` | data | upstream names retained (`RESP_*` single-consumer, kept in `responder`) |
//! | tables `MainMenu`..`SaveDef`/`gammamsg`/`skullName`/`quitsounds*` | `tables` | data | pointer cross-links wired at runtime by `lifecycle::init` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module draws NO `M_Random` (report §8.4); the
//! quit-sound/quip picks are gametic-keyed (`(gametic >> 2) & 7` /
//! `gametic & 7`), keyed on the demo-visible clock, not RNG (matches
//! the dstrings corrected record). `responder` sits before
//! `G_Responder` in the event chain -- its consume decision changes
//! which events the simulation sees -- but its input comes from
//! platform events, never the demo ticcmd stream, so the matcher/sim
//! boundary holds (recorded, not extracted); scenario flows 2/5
//! exercise it every gate run. `load_select`/`do_save`/
//! `choose_skill`/`verify_nightmare`/`quick_*` call
//! `G_LoadGame`/`G_SaveGame`/`G_DeferedInitNew` -- simulation
//! TRANSITIONS triggered from UI; the calls are the behavior.
//! `gammamsg[0]` is workaround entry 9's model (`GAMMALVL0` bytes):
//! the static's name, root path, AND byte content are pinned by
//! `g_game/dtmc.rs` + its test guard -- keep all three. Everything
//! else is frame-golden drawing pinned by the F9 goldens (the menu
//! overlay is inside the flow-2/5 frame anchors).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int, c_void};

use crate::doom::d_mode;
use crate::doom::doomstat::gamemission;

pub mod consts;
pub mod lifecycle;
pub mod message;
pub mod pages;
pub mod responder;
pub mod saveload;
pub mod state;
pub mod tables;
pub mod text;
pub mod types;

pub use state::{
    currentMenu, detailLevel, inhelpscreens, menuactive, mouseSensitivity, screenblocks,
    showMessages,
};
pub use tables::gammamsg;

/// Return the canonical game mission, collapsing Chex Quest and HacX aliases.
///
/// Mirrors the `logical_gamemission` macro from `doomstat.h`:
/// `pack_chex` maps to `doom`, `pack_hacx` maps to `doom2`,
/// all other values are returned as-is.
pub(super) fn logical_gamemission() -> c_int {
    unsafe {
        if gamemission == d_mode::pack_chex {
            d_mode::doom
        } else if gamemission == d_mode::pack_hacx {
            d_mode::doom2
        } else {
            gamemission
        }
    }
}

extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
    fn fclose(stream: *mut c_void) -> c_int;
    fn toupper(c: c_int) -> c_int;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
}

//* upstream-name shim: the five exported entry points keep their
//* freeze-zone caller paths (`crate::doom::m_menu::M_*`; the conductors
//* d_main/events+boot+display, g_game/responder, p_tick/ticker, the
//* d_loop/d_net extern declarers + loop-table address binding, and the
//* c2rust oracle) and their C symbols stay re-pinned at the definitions
//* with `#[export_name = "OriginalName"]`. M_QuitResponse carries the
//* brief-mandated extra pin (function-address compare anchor; the one
//* deliberate +1 on the symbol surface). Shims die with the freeze
//* zone.
pub use lifecycle::{drawer as M_Drawer, init as M_Init, ticker as M_Ticker};
pub use pages::quit_response as M_QuitResponse;
pub use responder::{responder as M_Responder, start_control_panel as M_StartControlPanel};
