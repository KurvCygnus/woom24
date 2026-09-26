//! The weapon action functions that roll dice and the hitscan helpers
//! they share: the melee attacks (`A_Punch`, `A_Saw`), the gunshots
//! (`P_BulletSlope`, `P_GunShot`, `A_FirePistol`, `A_FireShotgun`,
//! `A_FireShotgun2`, `A_FireCGun`), the projectiles (`A_FireMissile`,
//! `A_FirePlasma`, `A_FireBFG`), and the BFG tracer spray
//! (`A_BFGSpray`) -- bit-exact with the corresponding functions of
//! `vendor/doomgeneric/p_pspr.c`. The pure roll expressions live in
//! [`super::dtmc`]; every `P_Random` draw stays at these call sites.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::c_ffi::mobj_t as CffiMobj;
use crate::doom::d_items::weaponinfo;
use crate::doom::d_player::{PlayerT, PspdefT};
#[cfg(test)]
use crate::doom::info::{MT_PUFF, S_BFGEXP, S_BFGSHOT, S_PLASBALL, S_ROCKET};
use crate::doom::info::{
    self, State, MF_JUSTATTACKED, MT_BFG, MT_EXTRABFG, MT_PLASMA, MT_ROCKET, S_CHAIN1,
    S_PLAY_ATK2,
};
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_map::{linetarget, P_AimLineAttack, P_LineAttack};
use crate::doom::p_mobj::{P_SetMobjState, P_SpawnMobj, P_SpawnPlayerMissile};
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::actions::DecreaseAmmo;
use super::engine::P_SetPsprite;
use super::dtmc;
use super::state::{
    bulletslope, pw_strength, MELEERANGE, MISSILERANGE, ANG180, ANG90,
    DEH_DEFAULT_BFG_CELLS_PER_SHOT,
};


/// Weapon action: perform a fist punch in the player's facing direction.
///
/// Damage is 2-20 (×10 with Berserk).  If a target is hit, the player turns
/// to face it.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_Punch(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;

    let mut damage = dtmc::melee_roll(P_Random());
    if (*player).powers[pw_strength] != 0 {
        damage *= 10;
    }

    let mut angle = (*mo).angle;
    angle = angle.wrapping_add(dtmc::spread_angle(P_Random(), P_Random(), 18));
    let slope = P_AimLineAttack(mo as *mut _ as *mut CffiMobj, angle, MELEERANGE);
    P_LineAttack(
        mo as *mut _ as *mut CffiMobj,
        angle,
        MELEERANGE,
        slope,
        damage,
    );

    // Turn to face target.
    if !linetarget.is_null() {
        S_StartSound(mo as *mut c_void, Sfx::Punch as c_int);
        (*mo).angle = R_PointToAngle2((*mo).x, (*mo).y, (*linetarget).x, (*linetarget).y);
    }
}

/// Weapon action: perform a chainsaw attack.
///
/// Uses `MELEERANGE + 1` so the hit-puff does not skip the saw flash.  If
/// a target is hit, the player's angle is guided toward it gradually.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_Saw(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;

    let damage = dtmc::melee_roll(P_Random());
    let mut angle = (*mo).angle;
    angle = angle.wrapping_add(dtmc::spread_angle(P_Random(), P_Random(), 18));

    let slope = P_AimLineAttack(mo as *mut _ as *mut CffiMobj, angle, MELEERANGE + 1);
    P_LineAttack(
        mo as *mut _ as *mut CffiMobj,
        angle,
        MELEERANGE + 1,
        slope,
        damage,
    );

    if linetarget.is_null() {
        S_StartSound(mo as *mut c_void, Sfx::Sawful as c_int);
        return;
    }
    S_StartSound(mo as *mut c_void, Sfx::Sawhit as c_int);

    // Turn to face target.
    let angle = R_PointToAngle2((*mo).x, (*mo).y, (*linetarget).x, (*linetarget).y);
    let delta = angle.wrapping_sub((*mo).angle);
    if delta > ANG180 {
        let signed_delta = delta as i32;
        if signed_delta < -(ANG90 as i32) / 20 {
            (*mo).angle = angle.wrapping_add(ANG90 / 21);
        } else {
            (*mo).angle = (*mo).angle.wrapping_sub(ANG90 / 20);
        }
    } else {
        if delta > ANG90 / 20 {
            (*mo).angle = angle.wrapping_sub(ANG90 / 21);
        } else {
            (*mo).angle = (*mo).angle.wrapping_add(ANG90 / 20);
        }
    }
    (*mo).flags |= MF_JUSTATTACKED;
}

/// Weapon action: consume one rocket and spawn an `MT_ROCKET` projectile.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FireMissile(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 1);
    P_SpawnPlayerMissile(mo, MT_ROCKET);
}

/// Weapon action: consume `DEH_DEFAULT_BFG_CELLS_PER_SHOT` cells and spawn
/// an `MT_BFG` projectile.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FireBFG(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    DecreaseAmmo(
        player,
        weaponinfo[(*player).readyweapon as usize].ammo,
        DEH_DEFAULT_BFG_CELLS_PER_SHOT,
    );
    P_SpawnPlayerMissile(mo, MT_BFG);
}

/// Weapon action: consume one cell, randomise the flash frame, and spawn an
/// `MT_PLASMA` projectile.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FirePlasma(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 1);

    P_SetPsprite(
        player,
        1,
        weaponinfo[(*player).readyweapon as usize].flashstate + (P_Random() & 1),
    );

    P_SpawnPlayerMissile(mo, MT_PLASMA);
}

/// Compute `bulletslope` by aiming in the player's facing direction and two
/// small side offsets so that near-miss shots stay roughly at the intended
/// target's height.
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an initialised `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn P_BulletSlope(mo: *mut mobj_t) {
    let mut an = (*mo).angle;
    bulletslope = P_AimLineAttack(mo as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);

    if linetarget.is_null() {
        an = an.wrapping_add(1 << 26);
        bulletslope = P_AimLineAttack(mo as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);
        if linetarget.is_null() {
            an = an.wrapping_sub(2 << 26);
            bulletslope = P_AimLineAttack(mo as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);
        }
    }
}

/// Fire a single hitscan bullet from `mo`, using the pre-computed
/// `bulletslope`.  If `accurate` is zero, a random horizontal spread is
/// applied.
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an initialised `mobj_t`.
/// `P_BulletSlope` must have been called before this function so that
/// `bulletslope` is valid.
#[no_mangle]
pub unsafe extern "C" fn P_GunShot(mo: *mut mobj_t, accurate: c_int) {
    let damage = dtmc::gunshot_roll(P_Random());
    let mut angle = (*mo).angle;

    if accurate == 0 {
        angle = angle.wrapping_add(dtmc::spread_angle(P_Random(), P_Random(), 18));
    }

    P_LineAttack(
        mo as *mut _ as *mut CffiMobj,
        angle,
        MISSILERANGE,
        bulletslope,
        damage,
    );
}

/// Weapon action: fire the pistol (one accurate bullet, then spread on refire).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FirePistol(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    S_StartSound(mo as *mut c_void, Sfx::Pistol as c_int);

    P_SetMobjState(mo, S_PLAY_ATK2);
    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 1);

    P_SetPsprite(
        player,
        1,
        weaponinfo[(*player).readyweapon as usize].flashstate,
    );

    P_BulletSlope(mo);
    P_GunShot(mo, ((*player).refire == 0) as c_int);
}

/// Weapon action: fire the shotgun (7 inaccurate pellets, 1 shell consumed).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FireShotgun(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    S_StartSound(mo as *mut c_void, Sfx::Shotgn as c_int);
    P_SetMobjState(mo, S_PLAY_ATK2);

    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 1);

    P_SetPsprite(
        player,
        1,
        weaponinfo[(*player).readyweapon as usize].flashstate,
    );

    P_BulletSlope(mo);

    for _ in 0..7 {
        P_GunShot(mo, 0);
    }
}

/// Weapon action: fire the super shotgun (20 pellets with extra spread, 2
/// shells consumed).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_FireShotgun2(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    S_StartSound(mo as *mut c_void, Sfx::Dshtgn as c_int);
    P_SetMobjState(mo, S_PLAY_ATK2);

    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 2);

    P_SetPsprite(
        player,
        1,
        weaponinfo[(*player).readyweapon as usize].flashstate,
    );

    P_BulletSlope(mo);

    for _ in 0..20 {
        let damage = dtmc::gunshot_roll(P_Random());
        let mut angle = (*mo).angle;
        angle = angle.wrapping_add(dtmc::spread_angle(P_Random(), P_Random(), 19));
        P_LineAttack(
            mo as *mut _ as *mut CffiMobj,
            angle,
            MISSILERANGE,
            bulletslope + (((P_Random() - P_Random()) as c_int) << 5),
            damage,
        );
    }
}

/// Weapon action: fire one chaingun bullet, alternating the flash frame
/// between `S_CHAIN1` and `S_CHAIN2` to match the current psprite state.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.  `psp` must point to the
/// weapon psprite slot which must currently be in state `S_CHAIN1` or
/// `S_CHAIN2`.
#[no_mangle]
pub unsafe extern "C" fn A_FireCGun(player: *mut PlayerT, psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    S_StartSound(mo as *mut c_void, Sfx::Pistol as c_int);

    if (*player).ammo[weaponinfo[(*player).readyweapon as usize].ammo as usize] == 0 {
        return;
    }

    P_SetMobjState(mo, S_PLAY_ATK2);
    DecreaseAmmo(player, weaponinfo[(*player).readyweapon as usize].ammo, 1);

    let flashstate = weaponinfo[(*player).readyweapon as usize].flashstate;
    let state_offset = (*psp)
        .state
        .cast::<State>()
        .offset_from(&info::states[S_CHAIN1 as usize]);
    P_SetPsprite(player, 1, flashstate + state_offset as c_int);

    P_BulletSlope(mo);
    P_GunShot(mo, ((*player).refire == 0) as c_int);
}

/// Mobj action: spray the BFG tracers across a 90-degree arc centered on the
/// BFG ball's angle, dealing between 15 and 120 damage to each visible enemy.
///
/// Iterates 40 evenly-spaced angles, aims from `mo->target` (the originating
/// player), and spawns an `MT_EXTRABFG` explosion object on every hit target.
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` whose `target` field
/// points to a valid player `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_BFGSpray(mo: *mut mobj_t) {
    for i in 0..40 {
        let an = (*mo).angle - ANG90 / 2 + (ANG90 / 40) * i as u32;

        P_AimLineAttack(
            (*mo).target as *mut _ as *mut CffiMobj,
            an,
            16 * 64 * FRACUNIT,
        );

        if linetarget.is_null() {
            continue;
        }

        P_SpawnMobj(
            (*linetarget).x,
            (*linetarget).y,
            (*linetarget).z + ((*linetarget).height >> 2),
            MT_EXTRABFG,
        );

        let mut damage = 0;
        //* The 15-draw ladder bound has no unit vector (a hit target needs
        //* a blockmap world); it is guarded by the demo goldens alone -- do
        //* not retune `0..15` casually.
        for _ in 0..15 {
            damage = dtmc::spray_step(damage, P_Random());
        }

        P_DamageMobj(
            linetarget as *mut mobj_t,
            (*mo).target,
            (*mo).target,
            damage,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression tests for mobjtype constants used by weapon fire functions.
    /// These caught an off-by-one bug where MT_ROCKET/MT_PLASMA/MT_BFG were
    /// all 1 too high, causing A_FireMissile to spawn plasma balls instead
    /// of rockets (which then crashed on the missing PLSS sprite in the
    /// shareware WAD).
    #[test]
    fn mobjtype_constants_match_info() {
        assert_eq!(MT_ROCKET, 33);
        assert_eq!(MT_PLASMA, 34);
        assert_eq!(MT_BFG, 35);
        assert_eq!(MT_PUFF, 37);
        assert_eq!(MT_EXTRABFG, 42);
    }

    /// Verify that the mobjinfo table entries at the projectile indices
    /// have the expected spawnstates.  This catches index-vs-table drift.
    #[test]
    fn projectile_mobjinfo_entries() {
        unsafe {
            assert_eq!(
                info::mobjinfo[MT_ROCKET as usize].spawnstate,
                S_ROCKET,
                "mobjinfo[MT_ROCKET] should spawn S_ROCKET"
            );
            assert_eq!(
                info::mobjinfo[MT_PLASMA as usize].spawnstate,
                S_PLASBALL,
                "mobjinfo[MT_PLASMA] should spawn S_PLASBALL"
            );
            assert_eq!(
                info::mobjinfo[MT_BFG as usize].spawnstate,
                S_BFGSHOT,
                "mobjinfo[MT_BFG] should spawn S_BFGSHOT"
            );
            assert_eq!(
                info::mobjinfo[MT_EXTRABFG as usize].spawnstate,
                S_BFGEXP,
                "mobjinfo[MT_EXTRABFG] should spawn S_BFGEXP"
            );
        }
    }
}
