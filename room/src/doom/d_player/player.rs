//! The player vocabulary: `PlayerT` (the `player_t` FFI mirror), the
//! opaque `mobj_t` / `state_t` handles, the size constants, and the
//! cheat flags. The 328-byte `PlayerT` layout IS the savegame record
//! layout and the F9 `FinalState` capture surface -- the compile-time
//! layout guards below are load-bearing; never "modernize" the `c_int`
//! boolean fields (`bool` is 1 byte and would break the C ABI).

// The opaque C handles are lowercase (`mobj_t`, `state_t`); names are
// verbatim upstream data, so the type-name lint is silenced file-wide.
#![allow(non_camel_case_types)]

use std::ffi::c_char;
use std::os::raw::c_int;

use super::pspr::PspdefT;
use super::ticcmd::TiccmdT;

/// Number of power-up slots in `PlayerT::powers`.  Matches `NUMPOWERS` in `doomdef.h`.
pub const NUMPOWERS: usize = 6;

/// Number of key card slots in `PlayerT::cards`.  Matches `NUMCARDS` in `doomdef.h`.
pub const NUMCARDS: usize = 6;

/// Number of weapon slots in `PlayerT::weaponowned`.  Matches `NUMWEAPONS` in `doomdef.h`.
pub const NUMWEAPONS: usize = 9;

/// Number of ammo type slots in `PlayerT::ammo` / `maxammo`.  Matches `NUMAMMO` in `doomdef.h`.
pub const NUMAMMO: usize = 4;

/// Number of player-sprite (weapon overlay) slots in `PlayerT::psprites`.  Matches `NUMPSPRITES` in `p_pspr.h`.
pub const NUMPSPRITES: usize = 2;

/// Maximum number of simultaneously connected players.  Matches `MAXPLAYERS` in `doomdef.h`.
pub const MAXPLAYERS: usize = 4;

/// No-clip cheat flag: player passes through walls.  Matches `CF_NOCLIP` in `d_player.h`.
// Cheat flags (d_player.h)
pub const CF_NOCLIP: c_int = 1;

/// God-mode cheat flag: player takes no damage.  Matches `CF_GODMODE` in `d_player.h`.
pub const CF_GODMODE: c_int = 2;

/// No-momentum debug flag: player cannot move.  Matches `CF_NOMOMENTUM` in `d_player.h`.
pub const CF_NOMOMENTUM: c_int = 4;

/// Opaque handle for the moving-object (`mobj_t`) C type.
///
/// `mobj_t` is defined in `p_mobj.h` and is too large and complex to port
/// fully yet.  Using an empty enum prevents accidental construction while
/// still allowing `*mut mobj_t` pointers to cross the FFI boundary.
pub enum mobj_t {}

/// Opaque handle for the animation-state (`state_t`) C type.
///
/// `state_t` is defined in `info.h` (the mobjinfo table).  As with `mobj_t`,
/// represented as an empty enum so only pointers to it are valid at the
/// Rust call sites.
pub enum state_t {}

/// Complete per-player state, updated every tic by the game logic.
///
/// Corresponds to `player_t` in `d_player.h`.  One `PlayerT` exists for each
/// active player (up to [`MAXPLAYERS`]); the local player is always
/// `players[consoleplayer]`.  The struct is large (328 bytes on x86-64) and
/// encapsulates everything the engine needs to simulate, render, and save a
/// single player's participation in a game session.
///
/// Layout is verified at compile time by the tests below; the expected values
/// were derived from a `layout_probe.c` run on x86-64 Linux.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PlayerT
{
    /// Pointer to the player's map object (position, velocity, health during play).
    pub mo: *mut mobj_t,
    /// Current lifecycle state: alive (`PST_LIVE`), dead (`PST_DEAD`), or respawning (`PST_REBORN`).
    pub playerstate: c_int,
    /// The tic command built for this player this tic.
    pub cmd: TiccmdT,
    /// Camera height above the floor in fixed-point units (`fixed_t`).
    pub viewz: c_int,
    /// Base eye height above the floor; normally 41 units.
    pub viewheight: c_int,
    /// Per-tic adjustment to `viewheight` for landing/bobbing smoothness.
    pub deltaviewheight: c_int,
    /// Bounded total momentum magnitude used to drive weapon and view bob.
    pub bob: c_int,
    /// Player health between levels (during a level, `mo->health` is authoritative).
    pub health: c_int,
    /// Armor point total; 0 = no armor.
    pub armorpoints: c_int,
    /// Armor type: 0 = none, 1 = green armor (33%), 2 = blue armor (50%).
    pub armortype: c_int,
    /// Active power-up tic counters (`pw_*` index); invulnerability/invisibility count down.
    pub powers: [c_int; NUMPOWERS],
    /// Key card / skull key possession flags (`it_*` index); non-zero means owned.
    pub cards: [c_int; NUMCARDS],
    /// Non-zero if the player has picked up a backpack (doubles max ammo).
    pub backpack: c_int,
    /// Per-player frag counts indexed by player number.
    pub frags: [c_int; MAXPLAYERS],
    /// Currently active weapon (`weapontype_t` value).
    pub readyweapon: c_int,
    /// Weapon the player has requested to switch to; `wp_nochange` if none.
    pub pendingweapon: c_int,
    /// Weapon ownership flags; non-zero at index `i` means weapon `i` is owned.
    pub weaponowned: [c_int; NUMWEAPONS],
    /// Current ammo count per ammo type (`am_*` index).
    pub ammo: [c_int; NUMAMMO],
    /// Maximum ammo capacity per ammo type; doubled after picking up a backpack.
    pub maxammo: [c_int; NUMAMMO],
    /// Non-zero if the attack button was held last tic (prevents auto-fire restart).
    pub attackdown: c_int,
    /// Non-zero if the use button was held last tic (prevents repeated use on hold).
    pub usedown: c_int,
    /// Bitmask of active cheat flags (`CF_NOCLIP`, `CF_GODMODE`, `CF_NOMOMENTUM`).
    pub cheats: c_int,
    /// Refiring counter; incremented while the fire button is held; accuracy decreases while non-zero.
    pub refire: c_int,
    /// Total kills this level (used by intermission screen).
    pub killcount: c_int,
    /// Total items picked up this level (used by intermission screen).
    pub itemcount: c_int,
    /// Total secrets found this level (used by intermission screen).
    pub secretcount: c_int,
    /// Pointer to the current HUD hint message string, or null if none.
    pub message: *mut c_char,
    /// Tic counter for red damage flash; decrements toward 0 each tic.
    pub damagecount: c_int,
    /// Tic counter for yellow bonus flash; decrements toward 0 each tic.
    pub bonuscount: c_int,
    /// Pointer to the `mobj_t` that last damaged this player; null for environment damage.
    pub attacker: *mut mobj_t,
    /// Extra lighting bonus for gun flashes (added to sector light level during render).
    pub extralight: c_int,
    /// If non-zero, overrides the colormap used to render the player's view (e.g., `REDCOLORMAP` for pain).
    pub fixedcolormap: c_int,
    /// Player skin color shift index (0-3), used by the renderer to select the palette range.
    pub colormap: c_int,
    /// Weapon and muzzle-flash overlay sprites drawn on top of the 3-D view.
    pub psprites: [PspdefT; NUMPSPRITES],
    /// Non-zero if this player has completed a secret level this episode.
    pub didsecret: c_int,
}

/// The in-module layout guards (pre-move `d_player.rs:254-285`):
/// the expected `player_t` values as emitted by `layout_probe.c` on
/// x86_64 Linux, cross-checked C-side by `c_tests/struct_layouts.rs`
/// via `test_helpers.c`. These are the F10 §2.3 baseline for this
/// module -- they moved with the type unchanged and rerun after the
/// move, same vectors.
#[cfg(test)]
mod tests
{
    use super::*;

    /// Expected layout of `player_t` as emitted by layout_probe.c
    /// on x86_64 Linux.  Kept in-source so tests don't depend on the
    /// linker pulling symbols out of a static archive.
    const PLAYER_T_SIZEOF: usize = 328;
    const PLAYER_T_MESSAGE_OFFSET: usize = 232;

    #[test]
    fn player_t_size_matches_c()
    {
        assert_eq!(
            size_of::<PlayerT>(),
            PLAYER_T_SIZEOF,
            "PlayerT size mismatch: Rust={}, expected={}",
            size_of::<PlayerT>(),
            PLAYER_T_SIZEOF
        );
    }

    #[test]
    fn player_t_message_offset_matches_c()
    {
        assert_eq!(
            std::mem::offset_of!(PlayerT, message),
            PLAYER_T_MESSAGE_OFFSET,
            "PlayerT.message offset mismatch: Rust={}, expected={}",
            std::mem::offset_of!(PlayerT, message),
            PLAYER_T_MESSAGE_OFFSET
        );
    }
}
