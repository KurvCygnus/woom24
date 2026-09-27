//! Pickup/damage module state: the `maxammo` / `clipammo` ammo tables
//! mutated in place by the pickup flow and the runtime
//! DEHacked-tunable `deh_*` globals -- bit-exact with the globals of
//! `vendor/doomgeneric/p_inter.c`.

#![allow(non_upper_case_globals)]

use std::os::raw::c_int;

use super::consts::{
    DEH_DEFAULT_BLUE_ARMOR_CLASS, DEH_DEFAULT_GREEN_ARMOR_CLASS, DEH_DEFAULT_MAX_ARMOR,
    DEH_DEFAULT_MAX_HEALTH, DEH_DEFAULT_MAX_SOULSPHERE, DEH_DEFAULT_MEGASPHERE_HEALTH,
    DEH_DEFAULT_SOULSPHERE_HEALTH, NUMAMMO,
};

// ---------------------------------------------------------------------------
// Runtime DEHacked-tunable globals.
//
// These mirror the `deh_*` variables in upstream Chocolate Doom's
// `p_inter.c`.  They are initialized to the constants above and may be
// overridden at runtime by the DEHacked loader.  Pickup code must read
// the global, not the `DEH_DEFAULT_*` constant, so DEH overrides take
// effect.
// ---------------------------------------------------------------------------

/// Runtime maximum health achievable via bonus health spheres.
#[no_mangle]
pub static mut deh_max_health: c_int = DEH_DEFAULT_MAX_HEALTH;

/// Runtime maximum armor achievable via armor bonuses.
#[no_mangle]
pub static mut deh_max_armor: c_int = DEH_DEFAULT_MAX_ARMOR;

/// Runtime armor class granted by the green security armor shirt.
#[no_mangle]
pub static mut deh_green_armor_class: c_int = DEH_DEFAULT_GREEN_ARMOR_CLASS;

/// Runtime armor class granted by the blue mega-armor.
#[no_mangle]
pub static mut deh_blue_armor_class: c_int = DEH_DEFAULT_BLUE_ARMOR_CLASS;

/// Runtime upper health limit imposed by the soulsphere.
#[no_mangle]
pub static mut deh_max_soulsphere: c_int = DEH_DEFAULT_MAX_SOULSPHERE;

/// Runtime health points added by the soulsphere.
#[no_mangle]
pub static mut deh_soulsphere_health: c_int = DEH_DEFAULT_SOULSPHERE_HEALTH;

/// Runtime health set when the megasphere is picked up.
#[no_mangle]
pub static mut deh_megasphere_health: c_int = DEH_DEFAULT_MEGASPHERE_HEALTH;

// ---------------------------------------------------------------------------
// Ammo tables
// ---------------------------------------------------------------------------

/// Maximum ammo capacity for each ammo type when the player has no backpack.
/// Indexed by `am_clip`, `am_shell`, `am_cell`, `am_misl` (0-3).
/// Matches `maxammo[]` in `p_inter.c`.
#[no_mangle]
pub static mut maxammo: [c_int; NUMAMMO] = [200, 50, 300, 50];

/// Base ammo count per pickup for each ammo type.
/// A weapon pickup grants 2× this amount; a dropped weapon grants 1×;
/// passing `num=0` to `P_GiveAmmo` grants half a clip.
/// Matches `clipammo[]` in `p_inter.c`.
#[no_mangle]
pub static mut clipammo: [c_int; NUMAMMO] = [10, 4, 20, 1];

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Reads the `maxammo` engine static, so it holds the shared
    /// engine-statics lock (the pre-graduation flat file used a
    /// module-local mutex; the shared lock is the mandatory helper
    /// once any other test in the suite mutates the same statics
    /// family).
    #[test]
    fn maxammo_defaults()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            assert_eq!(maxammo, [200, 50, 300, 50]);
        }
    }

    /// Reads the `clipammo` engine static; see `maxammo_defaults` for
    /// the lock note.
    #[test]
    fn clipammo_defaults()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            assert_eq!(clipammo, [10, 4, 20, 1]);
        }
    }

    /// Reads the `deh_*` engine statics; see `maxammo_defaults` for
    /// the lock note.
    #[test]
    fn deh_runtime_globals_default_to_deh_values()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            assert_eq!(deh_max_health, DEH_DEFAULT_MAX_HEALTH);
            assert_eq!(deh_max_armor, DEH_DEFAULT_MAX_ARMOR);
            assert_eq!(deh_green_armor_class, DEH_DEFAULT_GREEN_ARMOR_CLASS);
            assert_eq!(deh_blue_armor_class, DEH_DEFAULT_BLUE_ARMOR_CLASS);
            assert_eq!(deh_max_soulsphere, DEH_DEFAULT_MAX_SOULSPHERE);
            assert_eq!(deh_soulsphere_health, DEH_DEFAULT_SOULSPHERE_HEALTH);
            assert_eq!(deh_megasphere_health, DEH_DEFAULT_MEGASPHERE_HEALTH);
        }
    }
}
