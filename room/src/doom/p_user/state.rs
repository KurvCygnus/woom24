//! Player module state and vocabulary: the shared `onground` global
//! and the button, weapon, power, player-state, view, and colormap
//! constants -- bit-exact with the data half of
//! `vendor/doomgeneric/p_user.c` and its headers.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::tables::ANG90;

/// Maximum view-bob amplitude in fixed-point units (16 pixels = 0x100000).
/// Matches `MAXBOB` in `p_user.c`.
pub(super) const MAXBOB: c_int = 0x100000;

/// Default player eye height above the floor in fixed-point units (41 map
/// units). Matches `VIEWHEIGHT` in `p_local.h`.
pub(super) const VIEWHEIGHT: fixed_t = 41 * FRACUNIT;

/// 5 degrees expressed as a binary angle (BAM). Used in `P_DeathThink` to
/// rotate the dead player's camera toward the killer.
pub(super) const ANG5: u32 = ANG90 / 18;

// ---------------------------------------------------------------------------
// Button constants (from d_event.h)
// ---------------------------------------------------------------------------

/// Button flag: this `ticcmd` carries a special (menu/cheat) event rather
/// than a normal game button. When set, all other button bits are ignored.
pub(super) const BT_SPECIAL: u8 = 128;

/// Button flag: the player wants to switch weapons.
pub(super) const BT_CHANGE: u8 = 4;

/// Bitmask that extracts the requested weapon index from `buttons` when
/// `BT_CHANGE` is set. Three bits wide (weapons 0-7), shifted by `BT_WEAPONSHIFT`.
pub(super) const BT_WEAPONMASK: u8 = 8 + 16 + 32;

/// Number of bits to right-shift `buttons & BT_WEAPONMASK` to obtain the
/// raw weapon index.
pub(super) const BT_WEAPONSHIFT: u8 = 3;

/// Button flag: the player pressed the Use/Open key.
pub(super) const BT_USE: u8 = 2;

/// Button flag: the player is holding the attack button.
pub(super) const BT_ATTACK: u8 = 1;

// ---------------------------------------------------------------------------
// Weapon type indices (from doomdef.h / info.h)
// ---------------------------------------------------------------------------

/// Fist weapon index.
pub(super) const wp_fist: c_int = 0;
/// Pistol weapon index.
pub(super) const wp_pistol: c_int = 1;
/// Single-barrel shotgun weapon index.
pub(super) const wp_shotgun: c_int = 2;
/// Chaingun weapon index.
pub(super) const wp_chaingun: c_int = 3;
/// Rocket launcher weapon index.
pub(super) const wp_missile: c_int = 4;
/// Plasma rifle weapon index.
pub(super) const wp_plasma: c_int = 5;
/// BFG 9000 weapon index.
pub(super) const wp_bfg: c_int = 6;
/// Chainsaw weapon index.
pub(super) const wp_chainsaw: c_int = 7;
/// Super shotgun (double-barrel) weapon index. Commercial/Doom 2 only.
pub(super) const wp_supershotgun: c_int = 8;

// ---------------------------------------------------------------------------
// Power-up indices (from doomdef.h)
// ---------------------------------------------------------------------------

/// Index into `player.powers[]` for the invulnerability sphere.
pub(super) const pw_invulnerability: usize = 0;
/// Index into `player.powers[]` for the berserk pack (strength).
pub(super) const pw_strength: usize = 1;
/// Index into `player.powers[]` for the partial-invisibility sphere.
pub(super) const pw_invisibility: usize = 2;
/// Index into `player.powers[]` for the radiation shielding suit (iron feet).
pub(super) const pw_ironfeet: usize = 3;
/// Index into `player.powers[]` for the computer area map.
pub(super) const pw_allmap: usize = 4;
/// Index into `player.powers[]` for the light-amplification visor (infrared).
pub(super) const pw_infrared: usize = 5;

// ---------------------------------------------------------------------------
// Player state constants (from d_player.h)
// ---------------------------------------------------------------------------

/// Player state: alive and playing.
pub(super) const PST_LIVE: c_int = 0;
/// Player state: dead (playing the death sequence / death-camera).
pub(super) const PST_DEAD: c_int = 1;
/// Player state: ready to be reborn (respawn requested).
pub(super) const PST_REBORN: c_int = 2;

// ---------------------------------------------------------------------------
// Colormap index
// ---------------------------------------------------------------------------

/// Colormap index for the full-bright inverse palette used during
/// invulnerability. Matches `INVERSECOLORMAP` in `p_user.c`.
pub(super) const INVERSECOLORMAP: c_int = 32;

/// Whether the player is standing on the floor (`mo->z == mo->floorz`).
///
/// A single shared global, NOT per-player: `P_MovePlayer` and
/// `P_DeathThink` write it once per player every tic, so in a multiplayer
/// tic each player's value overwrites the previous one and the next reader
/// sees the leftover -- exactly what the DOS original does
/// (`p_user.c:44`). Never thread-localize it or fold it into `PlayerT`:
/// that would move golden demo recordings of multiplayer sessions while
/// changing single-player behavior not at all. The `#[no_mangle]` export
/// keeps the C symbol.
#[no_mangle]
pub static mut onground: c_int = 0;

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::d_mode::{commercial, registered, retail, shareware};
    use crate::doom::d_player::{PspdefT, NUMPOWERS};
    use crate::doom::info::{MF_JUSTATTACKED, MF_NOCLIP, MF_SHADOW};
    use crate::doom::tables::ANG270;
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    const PSPDEF_T_SIZEOF: usize = 24;

    /// Actually pins `d_player::PspdefT` (24 bytes), not a p_user item:
    /// the test predates the graduation and rides with the module that
    /// read the psprite state; kept where it landed per the B1 report.
    #[test]
    fn pspdef_t_size_matches_c()
    {
        assert_eq!(
            std::mem::size_of::<PspdefT>(),
            PSPDEF_T_SIZEOF,
            "PspdefT size mismatch: Rust={}, expected={}",
            std::mem::size_of::<PspdefT>(),
            PSPDEF_T_SIZEOF,
        );
    }

    /// Reads the `onground` engine static, so it holds the shared
    /// engine-statics lock (the pre-graduation flat file used a
    /// module-local mutex; the shared lock is the mandatory helper once
    /// any other test in the suite mutates the same static).
    #[test]
    fn onground_defaults_to_zero()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            assert_eq!(onground, 0);
        }
    }

    #[test]
    fn constants_match_c_header_values()
    {
        // Verify button constants match d_event.h
        assert_eq!(BT_SPECIAL, 128);
        assert_eq!(BT_CHANGE, 4);
        assert_eq!(BT_WEAPONMASK, 56); // 8+16+32
        assert_eq!(BT_WEAPONSHIFT, 3);
        assert_eq!(BT_USE, 2);
        assert_eq!(BT_ATTACK, 1);

        // Verify game mode enum values match d_mode.rs
        assert_eq!(shareware, 0);
        assert_eq!(registered, 1);
        assert_eq!(commercial, 2);
        assert_eq!(retail, 3);

        // Verify angle constants match tables.h
        assert_eq!(ANG90, 0x40000000);
        assert_eq!(ANG270, 0xc0000000);

        // Verify player state enum values match d_player.h
        assert_eq!(PST_LIVE, 0);
        assert_eq!(PST_DEAD, 1);
        assert_eq!(PST_REBORN, 2);

        // Verify mobj flags match p_mobj.h
        assert_eq!(MF_NOCLIP, 0x1000);
        assert_eq!(MF_JUSTATTACKED, 128);
        assert_eq!(MF_SHADOW, 0x40000);

        // Verify power indices match doomdef.h (pw_allmap sits between
        // pw_ironfeet and pw_infrared; NUMPOWERS is 6).
        assert_eq!(pw_invulnerability, 0);
        assert_eq!(pw_strength, 1);
        assert_eq!(pw_invisibility, 2);
        assert_eq!(pw_ironfeet, 3);
        assert_eq!(pw_allmap, 4);
        assert_eq!(pw_infrared, 5);
        assert_eq!(NUMPOWERS, 6);
    }
}
