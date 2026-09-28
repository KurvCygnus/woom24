//! Upstream-named private constants and the vanilla C type aliases for
//! `g_game` (the gameplay controller): game-state slots, the deferred
//! `ga_*` action codes, player-state and weapon indices, ticcmd button
//! bits, sizing caps and the Dehacked-default starting inventory.
//!
//! Data tier (statics/consts ruling): every name here keeps its
//! upstream spelling and is `pub(super)` -- reachable only through the
//! `g_game` module directory.

#![allow(non_upper_case_globals, non_camel_case_types)]

use std::ffi::c_int;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Vanilla Doom `boolean`, matching the C `int` truthy/falsy convention.
pub(super) type boolean = c_int;
/// Vanilla Doom `byte` (unsigned 8-bit).
pub(super) type byte = u8;
/// Vanilla Doom `skill_t` enum stored as `int` (sk_baby=0 .. sk_nightmare=4).
pub(super) type skill_t = c_int;
/// Vanilla Doom 16.16 fixed-point type (`int`, with 16 fractional bits).
pub(super) type fixed_t = c_int;
/// Platform `long`, used here for the savegame size limit.
pub(super) type c_long = libc::c_long;

// ---------------------------------------------------------------------------
// Game-state constants (from doomstat.h)
// ---------------------------------------------------------------------------

/// `GS_LEVEL` - actively playing a map; `P_Ticker` advances world simulation.
pub(super) const GS_LEVEL: c_int = 0;
/// `GS_INTERMISSION` - between-level statistics screen driven by `WI_Ticker`.
pub(super) const GS_INTERMISSION: c_int = 1;
/// `GS_FINALE` - end-of-episode text crawl / cast call, driven by `F_Ticker`.
pub(super) const GS_FINALE: c_int = 2;
/// `GS_DEMOSCREEN` - title/credits/demo cycle, driven by `D_PageTicker`.
pub(super) const GS_DEMOSCREEN: c_int = 3;

// ---------------------------------------------------------------------------
// Game-action constants (from doomstat.h)
// ---------------------------------------------------------------------------

/// No deferred action pending; `G_Ticker` falls through to normal state.
pub(super) const ga_nothing: c_int = 0;
/// Defer a level (re)load via `G_DoLoadLevel`.
pub(super) const ga_loadlevel: c_int = 1;
/// Defer a new game start via `G_DoNewGame`.
pub(super) const ga_newgame: c_int = 2;
/// Defer a savegame load via `G_DoLoadGame`.
pub(super) const ga_loadgame: c_int = 3;
/// Defer a savegame write via `G_DoSaveGame`.
pub(super) const ga_savegame: c_int = 4;
/// Defer demo playback via `G_DoPlayDemo`.
pub(super) const ga_playdemo: c_int = 5;
/// Defer the end-of-level transition via `G_DoCompleted`.
pub(super) const ga_completed: c_int = 6;
/// Defer the end-of-game finale via `F_StartFinale`.
pub(super) const ga_victory: c_int = 7;
/// Defer the intermission-to-next-level transition via `G_DoWorldDone`.
pub(super) const ga_worlddone: c_int = 8;
/// Defer a screenshot grab via `V_ScreenShot`.
pub(super) const ga_screenshot: c_int = 9;

// ---------------------------------------------------------------------------
// Player state constants (from d_player.h)
// ---------------------------------------------------------------------------

/// `PST_LIVE` - player is alive and active.
pub(super) const PST_LIVE: c_int = 0;
/// `PST_DEAD` - player is dead, awaiting respawn input.
pub(super) const PST_DEAD: c_int = 1;
/// `PST_REBORN` - player should be respawned on the next tick.
pub(super) const PST_REBORN: c_int = 2;

// ---------------------------------------------------------------------------
// Weapon constants (from doomdef.h)
// ---------------------------------------------------------------------------

/// `wp_fist` weapon index (slot 1).
pub(super) const wp_fist: c_int = 0;
/// `wp_pistol` weapon index (slot 2).
pub(super) const wp_pistol: c_int = 1;
/// `wp_chainsaw` weapon index (also slot 1).
pub(super) const wp_chainsaw: c_int = 7;
/// `wp_supershotgun` weapon index (Doom II only, also slot 3).
pub(super) const wp_supershotgun: c_int = 8;
/// `wp_plasma` weapon index (slot 6).
pub(super) const wp_plasma: c_int = 5;
/// `wp_bfg` weapon index (slot 7).
pub(super) const wp_bfg: c_int = 6;
/// `wp_nochange` sentinel - pending weapon means "keep current".
pub(super) const wp_nochange: c_int = 9;

// ---------------------------------------------------------------------------
// Power types (from doomdef.h)
// ---------------------------------------------------------------------------

/// `pw_strength` index into `players[].powers` - berserk pack timer.
pub(super) const pw_strength: usize = 1;

// ---------------------------------------------------------------------------
// Button constants (from d_event.h)
// ---------------------------------------------------------------------------

/// `BT_ATTACK` ticcmd button bit - fire weapon.
pub(super) const BT_ATTACK: u8 = 1;
/// `BT_USE` ticcmd button bit - activate door / switch.
pub(super) const BT_USE: u8 = 2;
/// `BT_CHANGE` ticcmd button bit - request weapon change.
pub(super) const BT_CHANGE: u8 = 4;
/// Mask used to extract the weapon-change index encoded in `buttons`.
pub(super) const BT_WEAPONMASK: u8 = 8 + 16 + 32;
/// Left shift for encoding the weapon-change index into `buttons`.
pub(super) const BT_WEAPONSHIFT: u8 = 3;
/// `BT_SPECIAL` flag - the ticcmd carries a pause/save/load request.
pub(super) const BT_SPECIAL: u8 = 128;
/// Mask used to decode the special-button kind after `BT_SPECIAL`.
pub(super) const BT_SPECIALMASK: u8 = 3;
/// Special-button value for "pause toggle".
pub(super) const BTS_PAUSE: u8 = 1;
/// Special-button value for "savegame".
pub(super) const BTS_SAVEGAME: u8 = 2;
/// Mask for the savegame-slot field carried by a `BTS_SAVEGAME` request.
pub(super) const BTS_SAVEMASK: u8 = 4 + 8 + 16;
/// Left shift for the savegame-slot field carried by `BTS_SAVEGAME`.
pub(super) const BTS_SAVESHIFT: u8 = 2;

// ---------------------------------------------------------------------------
// Miscellaneous constants
// ---------------------------------------------------------------------------

/// Size of the `gamekeydown` keystate array - maximum supported key codes.
pub(super) const NUMKEYS: usize = 256;
/// Maximum number of mouse buttons recognised by the engine.
pub(super) const MAX_MOUSE_BUTTONS: usize = 8;
/// Maximum number of joystick buttons recognised by the engine.
pub(super) const MAX_JOY_BUTTONS: usize = 20;
/// Vanilla Doom savegame size cap (bytes); enforced when `vanilla_savegame_limit` is set.
pub(super) const SAVEGAMESIZE: c_long = 0x2c000;
/// End-of-stream sentinel byte written at the tail of every demo `.lmp`.
pub(super) const DEMOMARKER: byte = 0x80;
/// Size of the version-text field in some legacy savegame headers.
pub(super) const VERSIONSIZE: usize = 16;
/// Ammo-type index for the clip (bullets) ammo class.
pub(super) const am_clip: usize = 0;
/// `MT_TFOG` mobj type index - teleport fog spawned at player respawn spots.
pub(super) const MT_TFOG: c_int = 28;

/// Default starting health (100); patched by Dehacked in vanilla.
pub(super) const DEH_INITIAL_HEALTH: c_int = 100;
/// Default starting bullet count (50); patched by Dehacked in vanilla.
pub(super) const DEH_INITIAL_BULLETS: c_int = 50;

#[cfg(test)]
mod tests
{
    use super::*;

    /// `BT_*` / `BTS_*` button-bit constants match `d_event.h`.
    #[test]
    fn button_constants_match_d_event_h()
    {
        assert_eq!(BT_ATTACK, 1);
        assert_eq!(BT_USE, 2);
        assert_eq!(BT_CHANGE, 4);
        assert_eq!(BT_SPECIAL, 128);
        assert_eq!(BT_SPECIALMASK, 3);
        assert_eq!(BTS_PAUSE, 1);
        assert_eq!(BTS_SAVEGAME, 2);
        assert_eq!(BTS_SAVEMASK, 28);
        assert_eq!(BTS_SAVESHIFT, 2);
    }
}
