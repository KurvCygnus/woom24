//! Weapon module state and vocabulary: the weapon-bob and bullet-slope
//! globals (intercepts-overrun trample targets) and the weapon, ammo,
//! button, angle, and range constants -- bit-exact with the data half
//! of `vendor/doomgeneric/p_pspr.c` and its headers.

#![allow(non_upper_case_globals)]

use std::os::raw::c_int;

use crate::doom::d_player::NUMWEAPONS;
use crate::doom::m_fixed::{fixed_t, FRACUNIT};

// Weapon type constants (from doomdef.h)
/// Weapon slot index for the fist (melee, no ammo).
pub(super) const wp_fist: c_int = 0;
/// Weapon slot index for the pistol.
pub(super) const wp_pistol: c_int = 1;
/// Weapon slot index for the shotgun.
pub(super) const wp_shotgun: c_int = 2;
/// Weapon slot index for the chaingun.
pub(super) const wp_chaingun: c_int = 3;
/// Weapon slot index for the rocket launcher.
pub(super) const wp_missile: c_int = 4;
/// Weapon slot index for the plasma rifle.
pub(super) const wp_plasma: c_int = 5;
/// Weapon slot index for the BFG 9000.
pub(super) const wp_bfg: c_int = 6;
/// Weapon slot index for the chainsaw (no ammo).
pub(super) const wp_chainsaw: c_int = 7;
/// Weapon slot index for the super shotgun (Doom II only).
pub(super) const wp_supershotgun: c_int = 8;
/// Sentinel value meaning no pending weapon change is queued.
pub(super) const wp_nochange: c_int = NUMWEAPONS as c_int;

// Ammo type constants
/// Ammo type index meaning the weapon uses no ammo.
pub(super) const am_noammo: c_int = 5;
/// Ammo type index for bullets (clip).
pub(super) const am_clip: c_int = 0;
/// Ammo type index for shells.
pub(super) const am_shell: c_int = 1;
/// Ammo type index for cells (plasma / BFG).
pub(super) const am_cell: c_int = 2;
/// Ammo type index for missiles (rockets).
pub(super) const am_misl: c_int = 3;

// Power type constants
/// Power-up slot index for Berserk (strength); multiplies fist damage by 10.
pub(super) const pw_strength: usize = 1;

// Button constants
/// Bit mask in `ticcmd_t::buttons` that signals the attack button is held.
pub(super) const BT_ATTACK: u8 = 1;

// Player state constants
/// Player state value for a dead player (`PST_DEAD`).
pub(super) const PST_DEAD: c_int = 1;

// Angle constants
/// Binary-angle for 90 degrees (0x40000000).
pub(super) const ANG90: u32 = 0x40000000;
/// Binary-angle for 180 degrees (0x80000000).
pub(super) const ANG180: u32 = 0x80000000;

// Range constants (from p_local.h)
/// Maximum reach for melee attacks, in fixed-point world units (64 map units).
pub(super) const MELEERANGE: c_int = 64 * FRACUNIT;
/// Maximum range for hitscan (bullet) attacks (32 * 64 map units).
pub(super) const MISSILERANGE: c_int = 32 * 64 * FRACUNIT;

// Dehacked default
/// Default number of cells consumed per BFG shot (Dehacked-patchable in C; hardcoded here).
pub(super) const DEH_DEFAULT_BFG_CELLS_PER_SHOT: c_int = 40;

//* The three statics below are trample targets of the intercepts-array
//* overrun emulation (`p_maputl.rs` `InterceptsMemoryOverrun`:
//* `bulletslope` is write slot 10, `swingx` / `swingy` are the skipped
//* slots 11/12 of the emulated BSS order) -- they must stay `pub`,
//* module-root reachable, `c_int`-sized, and in this emulated-BSS
//* order; any reorder is a p_maputl edit, not a state.rs one.

/// Horizontal weapon-bob offset; updated each tic by `P_CalcSwing`.
#[no_mangle]
pub static mut swingx: fixed_t = 0;

/// Vertical weapon-bob offset; updated each tic by `P_CalcSwing`.
#[no_mangle]
pub static mut swingy: fixed_t = 0;

/// Slope set by `P_BulletSlope` for near-miss aiming; read by `P_GunShot`.
#[no_mangle]
pub static mut bulletslope: fixed_t = 0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doom::c_ffi::{LOWERSPEED, RAISESPEED, WEAPONBOTTOM, WEAPONTOP};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Movement/range constants against the `c_ffi` mirrors and the
    /// `p_local.h` values (no engine statics touched; no lock).
    #[test]
    fn constants_match_c() {
        assert_eq!(LOWERSPEED, FRACUNIT * 6);
        assert_eq!(RAISESPEED, FRACUNIT * 6);
        assert_eq!(WEAPONBOTTOM, 128 * FRACUNIT);
        assert_eq!(WEAPONTOP, 32 * FRACUNIT);
        assert_eq!(MELEERANGE, 64 * FRACUNIT);
        assert_eq!(MISSILERANGE, 32 * 64 * FRACUNIT);
    }

    #[test]
    fn weapon_constants_match() {
        assert_eq!(wp_fist, 0);
        assert_eq!(wp_pistol, 1);
        assert_eq!(wp_shotgun, 2);
        assert_eq!(wp_chaingun, 3);
        assert_eq!(wp_missile, 4);
        assert_eq!(wp_plasma, 5);
        assert_eq!(wp_bfg, 6);
        assert_eq!(wp_chainsaw, 7);
        assert_eq!(wp_supershotgun, 8);
        assert_eq!(wp_nochange, NUMWEAPONS as c_int);
    }

    /// Reads the `swingx` / `swingy` engine statics; see
    /// `constants_match_c` for the lock note.
    #[test]
    fn swing_defaults_to_zero() {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            assert_eq!(swingx, 0);
            assert_eq!(swingy, 0);
        }
    }

    /// Reads the `bulletslope` engine static; see
    /// `constants_match_c` for the lock note.
    #[test]
    fn bulletslope_defaults_to_zero() {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            assert_eq!(bulletslope, 0);
        }
    }
}
