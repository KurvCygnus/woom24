//! Heads-up display: map title, player messages, and chat input.
//!
//! Rust port of `vendor/doomgeneric/hu_stuff.c`. This module owns the three
//! HUD widgets that are visible during normal play:
//! * `w_message` (`hu_stext_t`) - scrolling player messages (ammo pickups,
//!   key cards, etc.).
//! * `w_title` (`hu_textline_t`) - current map name shown on the automap.
//! * `w_chat` (`hu_itext_t`) - live chat input line in multiplayer games.
//!
//! Exported globals (`chat_macros`, `player_names`, `hu_font`, `mapnames`,
//! `mapnames_commercial`, `chat_on`, `message_dontfuckwithme`, `chat_char`)
//! match the C `extern` declarations in `hu_stuff.h` and are accessed by the
//! C side of the engine.
//!
//! Notable Rust-vs-C differences:
//! * The `DEH_String` macro (Dehacked string substitution) is omitted; all
//!   string literals are used directly.
//! * C `boolean` maps to `c_int`; the `always_off` local uses `c_int` rather
//!   than `bool` to keep the ABI identical.
//! * The `SHORT` macro (little-endian swap) is an identity function on x86_64.
//!
//! ## Submodule Responsibility
//!
//! - `tables.rs` -- the four string tables (`chat_macros`, `player_names`,
//!   `mapnames`, `mapnames_commercial`), the nine HU_* public constants,
//!   and `QUEUESIZE`
//! - `state.rs` -- `hu_font` + the remaining statics: the four
//!   `#[no_mangle]` HUD globals and the private widget/message state
//! - `hud.rs` -- `load_font`/`stop`/`start`/`drawer`/`erase`/`ticker`
//! - `chat.rs` -- `queue_chat_char`/`dequeue_chat_char`/`responder`
//!
//! The module root holds the FFI surface carried VERBATIM from the
//! pre-split file (`W_CacheLumpName`/`S_StartSound` + the eight static
//! externs spanning am_map/g_game/m_menu -- `showMessages` links to an
//! m_menu-owned static; statics keep names, so the link resolves
//! regardless) and the `short_swap`/`logical_gamemission` shared
//! vocabulary.
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
//! oracle `c2rust-intermediate/src/d_main.rs:198,2062` declares
//! `HU_Init`/`HU_Drawer`/`HU_Erase` by symbol; `c_tests/hu_stuff_c.rs`
//! reads the tables/consts through the root paths). The quit-message
//! gametic-keyed pick rides the graduated `dstrings` surface (NOT an
//! M_Random consumer; corrected record `dstrings/mod.rs:35-45`).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `HU_Init` | `hud::load_font` | glue | caches the STCFNxxx font lumps; shim + pin (oracle-declared) |
//! | `HU_Stop` | `hud::stop` | glue | headsupactive latch clear; shim + pin |
//! | `HU_Start` | `hud::start` | glue | widget construction + map-title pick via `logical_gamemission`/`mapnames*`; shim + pin (`p_mobj/mapthings.rs`) |
//! | `HU_Drawer` | `hud::drawer` | glue | message/chat/title draw; shim + pin (oracle-declared) |
//! | `HU_Erase` | `hud::erase` | glue | dead-but-exported (zero callers in the tree; vendor d_main.c:208 calls it, our display never did): kept for symbol-set byte-identity, retires with the freeze zone; shim + pin (oracle-declared) |
//! | `HU_Ticker` | `hud::ticker` | glue | message lifecycle + netgame chat pump; player-struct mutations ordered by the graduated G_Ticker; shim + pin (`g_game/ticker.rs`) |
//! | `HU_queueChatChar` | `chat::queue_chat_char` | glue | outgoing ring push; shim + pin |
//! | `HU_dequeueChatChar` | `chat::dequeue_chat_char` | glue | outgoing ring pop (net layer unported; extern-surface conservatism per report §4.2); shim + pin |
//! | `HU_Responder` | `chat::responder` | glue | the chat/message input surface (consume decision inside G_Responder); shim + pin (`g_game/responder.rs`) |
//! | `chat_macros`/`player_names`/`mapnames`/`mapnames_commercial` | `tables` | data | `#[no_mangle]` retained; `mapnames[45]`/`mapnames_commercial[96]` layouts are c_test-pinned -- moved verbatim |
//! | `HU_FONTSTART`..`HU_MSGTIMEOUT` | `tables` | data | nine public consts, root-re-exported (`c_tests/hu_stuff_c.rs`, `f_finale/{cast,textstage}.rs`) |
//! | `hu_font`/`chat_char`/`chat_on`/`message_dontfuckwithme` | `state` | data | `#[no_mangle]` retained; `m_menu/{pages, responder,text}.rs`/`f_finale/{cast,textstage}.rs` read by root path |
//! | private widget/message state | `state` | data | `pub(super)` beside their writers/readers in `hud`/`chat` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface, by adjudication (report §4.4): all glue. `hud::ticker`
//! nulls `(*plr).message` and reads `cmd.chatchar` -- player-struct
//! mutations ordered by the graduated `g_game` ticker; the single-player
//! demo path skips the netgame chat loop (`netgame != 0` gate). No RNG
//! anywhere in the module (the dstrings correction already removed the
//! stale M_Random claim). Message lifecycle is frame-golden (HUD text in
//! the `e1m1_combat` anchors) and the `message_dontfuckwithme` handoff
//! from `p_inter`/`m_menu` is preserved by statics keeping names.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int};

use crate::doom::d_mode;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomstat::gamemission;
use std::ffi::c_void;

pub mod chat;
pub mod hud;
pub mod state;
pub mod tables;

extern "C" {
    /// Load a WAD lump by name and return a pointer to its data (w_wad.c).
    fn W_CacheLumpName(name: *const c_char, tag: c_int) -> *mut c_void;
    /// Start a sound effect, optionally at a map-object origin (s_sound.c).
    fn S_StartSound(origin_p: *mut c_void, sfx_id: c_int);
    /// Non-zero while the automap overlay is active (am_map.c).
    static mut automapactive: c_int;
    /// Non-zero for network games (d_net.c).
    static mut netgame: c_int;
    /// Per-player presence flags; `playeringame[i]` is non-zero if player `i`
    /// is active (doomstat.c / g_game.c).
    static mut playeringame: [c_int; MAXPLAYERS];
    /// Index of the local player (0-3) (doomstat.c).
    static mut consoleplayer: c_int;
    /// Non-zero if the "show messages" option is enabled (doomstat.c).
    static mut showMessages: c_int;
    /// Current game mode (retail, shareware, commercial, etc.) (doomstat.c).
    static mut gamemode: c_int;
    /// Current episode number (1-4) for Doom 1 (doomstat.c).
    static mut gameepisode: c_int;
    /// Current map number within the episode (doomstat.c).
    static mut gamemap: c_int;
}

/// Byte-swap for little-endian (SHORT macro from `i_swap.h`).
/// On x86_64 this is an identity cast; the value is returned unchanged.
#[inline(always)]
pub(super) fn short_swap(v: i16) -> i16 {
    v
}

/// Return the logical game mission, collapsing Chex Quest and HacX variants.
///
/// Replicates the C `logical_gamemission` macro from `doomstat.h`:
/// * `pack_chex` is treated as `doom`.
/// * `pack_hacx` is treated as `doom2`.
/// * All other missions are returned as-is.
pub(super) unsafe fn logical_gamemission() -> c_int {
    if gamemission == d_mode::pack_chex {
        d_mode::doom
    } else if gamemission == d_mode::pack_hacx {
        d_mode::doom2
    } else {
        gamemission
    }
}

//* upstream-name shim: the nine entry points keep their freeze-zone
//* caller paths (`crate::doom::hu_stuff::HU_*`: d_main/boot+display,
//* p_mobj/mapthings, g_game/responder+ticker, c_tests/hu_stuff_c.rs)
//* and their C symbols stay re-pinned at the definitions with
//* `#[export_name = "OriginalName"]` for the oracle/cdylib surface.
//* Shims die with the freeze zone.
pub use chat::{
    dequeue_chat_char as HU_dequeueChatChar, queue_chat_char as HU_queueChatChar,
    responder as HU_Responder,
};
pub use hud::{
    drawer as HU_Drawer, erase as HU_Erase, load_font as HU_Init, start as HU_Start,
    stop as HU_Stop, ticker as HU_Ticker,
};
//* path-stability re-export: the tables and HUD globals keep their
//* module-root paths (`m_menu/pages.rs`, `f_finale/cast.rs`,
//* `g_game/ticker.rs:23`, `c_tests/hu_stuff_c.rs`).
pub use state::{chat_char, chat_on, hu_font, message_dontfuckwithme};
pub use tables::{
    chat_macros, mapnames, mapnames_commercial, player_names, HU_BROADCAST, HU_FONTEND,
    HU_FONTSIZE, HU_FONTSTART, HU_MSGHEIGHT, HU_MSGTIMEOUT, HU_MSGWIDTH, HU_MSGX, HU_MSGY,
};
