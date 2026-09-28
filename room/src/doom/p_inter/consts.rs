//! Pickup/damage vocabulary: the numeric constants, the `GOT*` pickup
//! message pointers, and the `DEH_DEFAULT_*` fallbacks behind the
//! runtime `deh_*` globals -- bit-exact with the data half of
//! `vendor/doomgeneric/p_inter.c` and its headers.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_char;
use std::os::raw::c_int;

/// Bonus-count increment added to `player.bonuscount` on most pickups,
/// causing a brief gold screen-flash. Matches `BONUSADD` in `p_inter.c`.
pub(super) const BONUSADD: c_int = 6;

/// Number of distinct ammo types (clip, shell, cell, missile).
pub(super) const NUMAMMO: usize = 4;

/// Maximum health for normal health items; over-100 bonuses use
/// `DEH_DEFAULT_MAX_HEALTH` instead.
pub(super) const MAXHEALTH: c_int = 100;

/// Special Z value meaning "place mobj at floor level of its sector".
/// Stored as `i32::MIN` to match the C `ONFLOORZ` sentinel.
pub(super) const ONFLOORZ: c_int = i32::MIN;

/// Initial `target.threshold` assigned when a monster acquires a new target.
pub(super) const BASETHRESHOLD: c_int = 100;

// Skill levels

/// Skill 0 — "I'm Too Young to Die" / baby mode. Damage is halved.
pub(super) const sk_baby: c_int = 0;

/// Skill 4 — Nightmare. Ammo doublers apply and monsters are fast.
pub(super) const sk_nightmare: c_int = 4;

// Power-up durations (TICRATE = 35 tics/second)

/// Duration of the invulnerability sphere power-up: 30 seconds.
pub(super) const INVULNTICS: c_int = 30 * 35;

/// Duration of the partial-invisibility power-up: 60 seconds.
pub(super) const INVISTICS: c_int = 60 * 35;

/// Duration of the light-amplification visor power-up: 120 seconds.
pub(super) const INFRATICS: c_int = 120 * 35;

/// Duration of the radiation-shielding suit power-up: 60 seconds.
pub(super) const IRONTICS: c_int = 60 * 35;

// Weapon type indices (match `weapontype_t` in `info.h`)

/// Fist — the starting melee weapon.
pub(super) const wp_fist: c_int = 0;

/// Pistol — the starting ranged weapon.
pub(super) const wp_pistol: c_int = 1;

/// Single-barrelled shotgun.
pub(super) const wp_shotgun: c_int = 2;

/// Chaingun.
pub(super) const wp_chaingun: c_int = 3;

/// Rocket launcher.
pub(super) const wp_missile: c_int = 4;

/// Plasma gun.
pub(super) const wp_plasma: c_int = 5;

/// BFG 9000.
pub(super) const wp_bfg: c_int = 6;

/// Chainsaw.
pub(super) const wp_chainsaw: c_int = 7;

/// Super shotgun (Doom II only).
pub(super) const wp_supershotgun: c_int = 8;

// Ammo type indices (match `ammotype_t`)

/// Sentinel value meaning "this weapon uses no ammo".
pub(super) const am_noammo: c_int = 5;

/// Bullet clip ammo (pistol / chaingun).
pub(super) const am_clip: c_int = 0;

/// Shell ammo (shotgun / super shotgun).
pub(super) const am_shell: c_int = 1;

/// Energy cell ammo (plasma gun / BFG).
pub(super) const am_cell: c_int = 2;

/// Rocket ammo.
pub(super) const am_misl: c_int = 3;

// Card/key type indices (match `card_t`)

/// Blue keycard index.
pub(super) const it_bluecard: c_int = 0;

/// Yellow keycard index.
pub(super) const it_yellowcard: c_int = 1;

/// Red keycard index.
pub(super) const it_redcard: c_int = 2;

/// Blue skull key index.
pub(super) const it_blueskull: c_int = 3;

/// Yellow skull key index.
pub(super) const it_yellowskull: c_int = 4;

/// Red skull key index.
pub(super) const it_redskull: c_int = 5;

// Power type indices (match `powertype_t`)

/// Invulnerability sphere power-up slot.
pub(super) const pw_invulnerability: usize = 0;

/// Berserk pack power-up slot (also boosts fist damage).
pub(super) const pw_strength: usize = 1;

/// Partial-invisibility power-up slot.
pub(super) const pw_invisibility: usize = 2;

/// Radiation-shielding suit power-up slot.
pub(super) const pw_ironfeet: usize = 3;

/// Computer area map power-up slot (reveals the automap).
pub(super) const pw_allmap: usize = 4;

/// Light-amplification visor power-up slot.
pub(super) const pw_infrared: usize = 5;

// Game version / mode constants

/// Chex Quest game-version code. Monsters drop no items in Chex Quest.
pub(super) const exe_chex: c_int = 9;

/// Commercial game-mode code (Doom II / TNT / Plutonia). Required for
/// MegaSphere pickup.
pub(super) const commercial: c_int = 2;

// DEH defaults — used when FEATURE_DEHACKED is not compiled in.

/// Maximum health achievable via bonus health spheres (not normal medikits).
pub(super) const DEH_DEFAULT_MAX_HEALTH: c_int = 200;

/// Maximum armor points achievable via armor bonuses.
pub(super) const DEH_DEFAULT_MAX_ARMOR: c_int = 200;

/// Armor class granted by the green security armor shirt.
pub(super) const DEH_DEFAULT_GREEN_ARMOR_CLASS: c_int = 1;

/// Armor class granted by the blue mega-armor.
pub(super) const DEH_DEFAULT_BLUE_ARMOR_CLASS: c_int = 2;

/// Upper health limit imposed by the soulsphere.
pub(super) const DEH_DEFAULT_MAX_SOULSPHERE: c_int = 200;

/// Health points added by the soulsphere.
pub(super) const DEH_DEFAULT_SOULSPHERE_HEALTH: c_int = 100;

/// Health set to when the megasphere is picked up.
pub(super) const DEH_DEFAULT_MEGASPHERE_HEALTH: c_int = 200;

// ---------------------------------------------------------------------------
// Pick-up message strings
// ---------------------------------------------------------------------------

/// "Picked up the armor." — displayed when the green armor is collected.
pub(super) const GOTARMOR: *mut c_char = c"Picked up the armor.".as_ptr().cast_mut();
/// "Picked up the MegaArmor!" — displayed when the blue mega-armor is collected.
pub(super) const GOTMEGA: *mut c_char = c"Picked up the MegaArmor!".as_ptr().cast_mut();
/// "Picked up a health bonus." — displayed for the health-bonus helmet.
pub(super) const GOTHTHBONUS: *mut c_char = c"Picked up a health bonus.".as_ptr().cast_mut();
/// "Picked up an armor bonus." — displayed for the armor-bonus helmet.
pub(super) const GOTARMBONUS: *mut c_char = c"Picked up an armor bonus.".as_ptr().cast_mut();
/// "Picked up a stimpack." — displayed for the stimpack.
pub(super) const GOTSTIM: *mut c_char = c"Picked up a stimpack.".as_ptr().cast_mut();
/// Urgent medikit message when health is critically low (below 25).
pub(super) const GOTMEDINEED: *mut c_char = c"Picked up a medikit that you REALLY need!"
    .as_ptr()
    .cast_mut();
/// "Picked up a medikit." — normal medikit pickup message.
pub(super) const GOTMEDIKIT: *mut c_char = c"Picked up a medikit.".as_ptr().cast_mut();
/// "Supercharge!" — displayed when the soulsphere is collected.
pub(super) const GOTSUPER: *mut c_char = c"Supercharge!".as_ptr().cast_mut();
/// "MegaSphere!" — displayed when the megasphere is collected (Doom II only).
pub(super) const GOTMSPHERE: *mut c_char = c"MegaSphere!".as_ptr().cast_mut();
/// "Picked up a blue keycard." — displayed when the blue keycard is collected.
pub(super) const GOTBLUECARD: *mut c_char = c"Picked up a blue keycard.".as_ptr().cast_mut();
/// "Picked up a yellow keycard." — displayed when the yellow keycard is collected.
pub(super) const GOTYELWCARD: *mut c_char = c"Picked up a yellow keycard.".as_ptr().cast_mut();
/// "Picked up a red keycard." — displayed when the red keycard is collected.
pub(super) const GOTREDCARD: *mut c_char = c"Picked up a red keycard.".as_ptr().cast_mut();
/// "Picked up a blue skull key." — displayed when the blue skull key is collected.
pub(super) const GOTBLUESKUL: *mut c_char = c"Picked up a blue skull key.".as_ptr().cast_mut();
/// "Picked up a yellow skull key." — displayed when the yellow skull key is collected.
pub(super) const GOTYELWSKUL: *mut c_char = c"Picked up a yellow skull key.".as_ptr().cast_mut();
/// "Picked up a red skull key." — displayed when the red skull key is collected.
pub(super) const GOTREDSKULL: *mut c_char = c"Picked up a red skull key.".as_ptr().cast_mut();
/// "Invulnerability!" — displayed when the invulnerability sphere is collected.
pub(super) const GOTINVUL: *mut c_char = c"Invulnerability!".as_ptr().cast_mut();
/// "Berserk!" — displayed when the berserk pack is collected.
pub(super) const GOTBERSERK: *mut c_char = c"Berserk!".as_ptr().cast_mut();
/// "Partial Invisibility" — displayed when the blur-sphere is collected.
pub(super) const GOTINVIS: *mut c_char = c"Partial Invisibility".as_ptr().cast_mut();
/// "Radiation Shielding Suit" — displayed when the rad suit is collected.
pub(super) const GOTSUIT: *mut c_char = c"Radiation Shielding Suit".as_ptr().cast_mut();
/// "Computer Area Map" — displayed when the automap power-up is collected.
pub(super) const GOTMAP: *mut c_char = c"Computer Area Map".as_ptr().cast_mut();
/// "Light Amplification Visor" — displayed when the visor is collected.
pub(super) const GOTVISOR: *mut c_char = c"Light Amplification Visor".as_ptr().cast_mut();
/// "Picked up a clip." — bullet clip pickup message.
pub(super) const GOTCLIP: *mut c_char = c"Picked up a clip.".as_ptr().cast_mut();
/// "Picked up a box of bullets." — ammo box pickup message.
pub(super) const GOTCLIPBOX: *mut c_char = c"Picked up a box of bullets.".as_ptr().cast_mut();
/// "Picked up a rocket." — single rocket pickup message.
pub(super) const GOTROCKET: *mut c_char = c"Picked up a rocket.".as_ptr().cast_mut();
/// "Picked up a box of rockets." — rocket box pickup message.
pub(super) const GOTROCKBOX: *mut c_char = c"Picked up a box of rockets.".as_ptr().cast_mut();
/// "Picked up an energy cell." — single energy cell pickup message.
pub(super) const GOTCELL: *mut c_char = c"Picked up an energy cell.".as_ptr().cast_mut();
/// "Picked up an energy cell pack." — energy cell pack pickup message.
pub(super) const GOTCELLBOX: *mut c_char = c"Picked up an energy cell pack.".as_ptr().cast_mut();
/// "Picked up 4 shotgun shells." — shotgun shell pickup message.
pub(super) const GOTSHELLS: *mut c_char = c"Picked up 4 shotgun shells.".as_ptr().cast_mut();
/// "Picked up a box of shotgun shells." — shell box pickup message.
pub(super) const GOTSHELLBOX: *mut c_char = c"Picked up a box of shotgun shells."
    .as_ptr()
    .cast_mut();
/// "Picked up a backpack full of ammo!" — backpack pickup message.
pub(super) const GOTBACKPACK: *mut c_char = c"Picked up a backpack full of ammo!"
    .as_ptr()
    .cast_mut();
/// "You got the BFG9000!  Oh, yes." — BFG pickup message.
pub(super) const GOTBFG9000: *mut c_char = c"You got the BFG9000!  Oh, yes."
    .as_ptr()
    .cast_mut();
/// "You got the chaingun!" — chaingun pickup message.
pub(super) const GOTCHAINGUN: *mut c_char = c"You got the chaingun!".as_ptr().cast_mut();
/// "A chainsaw!  Find some meat!" — chainsaw pickup message.
pub(super) const GOTCHAINSAW: *mut c_char = c"A chainsaw!  Find some meat!"
    .as_ptr()
    .cast_mut();
/// "You got the rocket launcher!" — rocket launcher pickup message.
pub(super) const GOTLAUNCHER: *mut c_char = c"You got the rocket launcher!"
    .as_ptr()
    .cast_mut();
/// "You got the plasma gun!" — plasma gun pickup message.
pub(super) const GOTPLASMA: *mut c_char = c"You got the plasma gun!".as_ptr().cast_mut();
/// "You got the shotgun!" — shotgun pickup message.
pub(super) const GOTSHOTGUN: *mut c_char = c"You got the shotgun!".as_ptr().cast_mut();
/// "You got the super shotgun!" — super shotgun pickup message (Doom II only).
pub(super) const GOTSHOTGUN2: *mut c_char = c"You got the super shotgun!".as_ptr().cast_mut();

// ---------------------------------------------------------------------------
// DEH_String shim — identity when dehacked is disabled.
// ---------------------------------------------------------------------------

/// Passes `s` through unchanged.
///
/// When DEHacked support is compiled in this function would look up a
/// patched string replacement. Here it is a no-op identity shim because
/// `FEATURE_DEHACKED` is not defined.
#[inline(always)]
pub(super) unsafe fn DEH_String(s: *mut c_char) -> *mut c_char { s }

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::d_player::CF_GODMODE;
    use crate::doom::info::{MF_COUNTITEM, MF_DROPPED, MF_SHOOTABLE, MF_SKULLFLY};
    use crate::doom::m_fixed::FRACUNIT;
    use crate::doom::tables::{ANG180, ANGLETOFINESHIFT};

    #[test]
    fn constants_match()
    {
        assert_eq!(BONUSADD, 6);
        assert_eq!(NUMAMMO, 4);
        assert_eq!(MAXHEALTH, 100);
        assert_eq!(ONFLOORZ, i32::MIN);
        assert_eq!(FRACUNIT, 65536);
        assert_eq!(ANG180, 0x80000000);
        assert_eq!(ANGLETOFINESHIFT, 19);
        assert_eq!(BASETHRESHOLD, 100);
        assert_eq!(CF_GODMODE, 2);
        assert_eq!(sk_baby, 0);
        assert_eq!(sk_nightmare, 4);
        assert_eq!(INVULNTICS, 1050);
        assert_eq!(INVISTICS, 2100);
        assert_eq!(INFRATICS, 4200);
        assert_eq!(IRONTICS, 2100);
        assert_eq!(am_noammo, 5);
        assert_eq!(MF_DROPPED, 0x00020000);
        assert_eq!(MF_COUNTITEM, 0x00800000);
        assert_eq!(MF_SHOOTABLE, 4);
        assert_eq!(MF_SKULLFLY, 0x01000000);
        assert_eq!(exe_chex, 9);
        assert_eq!(commercial, 2);
    }

    #[test]
    fn deh_defaults_match()
    {
        assert_eq!(DEH_DEFAULT_MAX_HEALTH, 200);
        assert_eq!(DEH_DEFAULT_MAX_ARMOR, 200);
        assert_eq!(DEH_DEFAULT_GREEN_ARMOR_CLASS, 1);
        assert_eq!(DEH_DEFAULT_BLUE_ARMOR_CLASS, 2);
        assert_eq!(DEH_DEFAULT_MAX_SOULSPHERE, 200);
        assert_eq!(DEH_DEFAULT_SOULSPHERE_HEALTH, 100);
        assert_eq!(DEH_DEFAULT_MEGASPHERE_HEALTH, 200);
    }
}
