//! The player vocabulary home: the FFI mirrors of `player_t`, `ticcmd_t`,
//! and `pspdef_t` (collected from `d_player.h`, `d_ticcmd.h`, and
//! `p_pspr.h` -- a tight cluster that exists so pointer fields and the
//! F9 harnesses have concrete types), the size/cheat constants, the
//! extern view of the `players` / `consoleplayer` globals, and the
//! HUD-message setter inherited from `m_menu_shim.c`. This is a
//! header-mirror module, not a port of a C translation unit:
//! `d_player.h` has no `.c`.
//!
//! ## Submodule Responsibility
//!
//! - `player.rs` -- `PlayerT`, the opaque `mobj_t` / `state_t` handles,
//!   the `NUM*` / `MAXPLAYERS` size constants, the `CF_*` cheat flags,
//!   and the compile-time layout guards
//! - `ticcmd.rs` -- `TiccmdT` (`d_ticcmd.h` provenance)
//! - `pspr.rs` -- `PspdefT` (`p_pspr.h` provenance)
//! - `message.rs` -- `set_player_message` (renamed
//!   `M_Menu_SetPlayerMessage`) and its export pin
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the verbatim extern block below, the root re-exports,
//! and the upstream-name shim; no content lives here.
//!
//! # Extern-by-symbol contract (carried VERBATIM)
//!
//! The `extern "C"` block below is carried verbatim from pre-split
//! `d_player.rs:220-232`. These are extern DECLARATIONS, not
//! definitions: they link BY SYMBOL to the freeze-zone definitions in
//! `g_game.rs:376` (`#[no_mangle] pub static mut players: [PlayerT;
//! MAXPLAYERS]`) and `g_game.rs:386` (`#[no_mangle] pub static mut
//! consoleplayer: c_int = 0`). A third declarer exists:
//! `hu_stuff.rs:404` (private extern `consoleplayer`). Never re-point
//! this block to Rust paths (`crate::doom::g_game::players`) while the
//! freeze zone exists -- that would silently breach the
//! extern-by-symbol contract and lose the declarer record. The F9
//! harness paths `room::doom::d_player::{consoleplayer, players,
//! MAXPLAYERS}` (`frame_split_common`, `demo_playthrough`,
//! `sprite_interp_probe`, `shells/web/src/lib.rs:265/:296/:302`) resolve
//! through the root re-exports below.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! functions only: exactly ONE rename in this module (the single
//! function); every type, constant, and static keeps its upstream name
//! (data-tier renaming comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_Menu_SetPlayerMessage` (m_menu_shim.c) | `message::set_player_message` | glue | shim + pin; writes `players[consoleplayer].message` at menu-action time; kept `pub unsafe extern "C"` (parity); upstream `m_menu_shim.c` (removed; `build.rs:100` comment) |
//! | (data) `player_t` | `player::PlayerT` | data | name kept; 33 engine files import it here; 328-byte layout = savegame record = F9 `FinalState` capture; layout guards in `player.rs` |
//! | (data) `ticcmd_t` | `ticcmd::TiccmdT` | data | name kept; field order IS the save format (`p_saveg/records.rs` `read_ticcmd`/`write_ticcmd`); explicit `_pad` carried |
//! | (data) `pspdef_t` | `pspr::PspdefT` | data | name kept; 10 files |
//! | (data) opaque `mobj_t` / `state_t` | `player.rs` | data | pointer-target types only; `c_ffi.rs:26/:183` defines a SEPARATE mirror pair and `p_telept/types.rs:110/:124` a third -- mirrors are NOT unified during a graduation; the F9 harnesses cast `p.mo` through `c_ffi::mobj_t`, so both mirrors stay alive |
//! | (data) `NUMPOWERS`/`NUMCARDS`/`NUMWEAPONS`/`NUMAMMO`/`NUMPSPRITES`/`MAXPLAYERS`, `CF_NOCLIP`/`CF_GODMODE`/`CF_NOMOMENTUM` | `player.rs` | data | names kept, root re-exported; local same-named copies in `m_menu.rs:63` / `c_ffi.rs:620` / `d_items/table.rs` / `p_inter/consts.rs:16` are NOT consumers -- never unify |
//! | (data) extern statics `players` / `consoleplayer` | this root (extern block) | data | carried VERBATIM; definitions are freeze-zone `g_game.rs:376/:386`; see the extern-by-symbol contract above |
//! | `doomgeneric.rs` anchor list | -- | wiring | has no d_player entry (pre-move or now); per the wave ruling no anchors were added -- the `M_Menu_SetPlayerMessage` pin plus the untouched g_game `#[no_mangle]` statics keep the symbol set byte-identical |
//!
//! ## Deterministic Aspects
//!
//! No dtmc submodule by adjudication (F10 wave C1; m_argv data-tier
//! precedent): the module contains no per-tic logic -- the one function
//! is menu-action glue -- but its DATA is the demo synchronization
//! surface, so nothing may change layout or field order:
//!
//! - `players[]` / `consoleplayer` are simulation state hashed by the F9
//!   harnesses (`frame_split_common/mod.rs` reads five `PlayerT` fields
//!   into `FinalState`).
//! - `PlayerT`'s 328-byte layout IS the savegame record layout
//!   (`p_saveg/records.rs` `read_player_record`/`write_player_record`
//!   mirror the field order).
//! - `consoleplayer` is part of the LMP demo header (`g_game.rs` writes
//!   it at record time and validates it at playback).
//! - `set_player_message` is glue: `player_t.message` is
//!   render/HUD-observable but never read by the simulation and never
//!   carried on the demo stream.
//!
//! The layout guards in `player.rs` (size 328, `message` offset 232) are
//! the §2.3 baseline; they moved with the type unchanged and rerun after
//! the move, same vectors. `TiccmdT` is field-order-exact serialized by
//! `p_saveg/records.rs`; the save/load round-trip and the demo goldens
//! are the whole-body oracle.

// The extern block below declares lowercase statics (`players`,
// `consoleplayer`); names are verbatim upstream symbols, so the lint
// is silenced file-wide.
#![allow(non_upper_case_globals)]

use std::os::raw::c_int;

pub mod message;
pub mod player;
pub mod pspr;
pub mod ticcmd;

// ---------------------------------------------------------------------------
// External C declarations -- carried VERBATIM from pre-split
// d_player.rs:220-232 (see the extern-by-symbol contract above).
// ---------------------------------------------------------------------------

extern "C" {
    /// Global array of player state structs; defined in C (`g_game.c`).
    ///
    /// Index 0 is the local player when `consoleplayer == 0`.  Slots
    /// `[1..MAXPLAYERS-1]` are used in network games.
    pub static mut players: [PlayerT; MAXPLAYERS];

    /// Index into `players` for the player whose view is shown on the local screen.
    ///
    /// Defined in C (`g_game.c`).  Always 0 for single-player games; may differ
    /// in split-screen or network configurations.
    pub static mut consoleplayer: c_int;
}

//* path-stability re-export: the full player vocabulary keeps its
//* module-root paths. 33 engine files import `PlayerT` here, plus the
//* F9 harnesses and the web shell
//* (`room::doom::d_player::{consoleplayer, players, MAXPLAYERS}`), the
//* TiccmdT/PspdefT consumers, and the fully-qualified
//* `crate::doom::d_player::mobj_t` / `::state_t` reference families.
//* Nothing here may move off the root.
pub use player::{
    mobj_t, state_t, CF_GODMODE, CF_NOCLIP, CF_NOMOMENTUM, MAXPLAYERS, NUMAMMO, NUMCARDS,
    NUMPSPRITES, NUMPOWERS, NUMWEAPONS, PlayerT,
};
pub use pspr::PspdefT;
pub use ticcmd::TiccmdT;

//* upstream-name shim: the freeze-zone caller (`m_menu.rs:26`, five call
//* sites) keeps the upstream name. Plain `pub use` of ONE function item;
//* the C symbol is re-pinned at the definition with
//* `#[export_name = "M_Menu_SetPlayerMessage"]`, so the wasm/extern
//* symbol name set stays byte-identical to the pre-split module.
pub use message::set_player_message as M_Menu_SetPlayerMessage;
