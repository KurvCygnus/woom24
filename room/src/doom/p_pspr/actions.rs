//! The weapon action functions that do not roll dice: idle bob and
//! fire gating (`A_WeaponReady`), refire/reload checks (`A_ReFire`,
//! `A_CheckReload`), the raise/lower animation (`A_Lower`, `A_Raise`),
//! the muzzle-flash overlays (`A_GunFlash`, `A_Light0/1/2`), the BFG
//! charge sound (`A_BFGsound`), and the vanilla ammo-array overflow
//! emulation in `DecreaseAmmo` -- bit-exact with the corresponding
//! functions of `vendor/doomgeneric/p_pspr.c`.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::c_ffi::{LOWERSPEED, RAISESPEED, WEAPONBOTTOM, WEAPONTOP};
use crate::doom::d_items::weaponinfo;
use crate::doom::d_player::{PlayerT, PspdefT, NUMAMMO};
use crate::doom::info::{self, State, S_PLAY, S_PLAY_ATK1, S_PLAY_ATK2, S_SAW, S_NULL};
use crate::doom::m_fixed::{FixedMul, FRACUNIT};
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::leveltime;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, FINEANGLES, FINEMASK};

use super::engine::{P_CheckAmmo, P_FireWeapon, P_SetPsprite, P_BringUpWeapon};
use super::state::{wp_bfg, wp_chainsaw, wp_missile, wp_nochange, BT_ATTACK, PST_DEAD};

/// Weapon action: idle state — bob the weapon, check for fire or weapon
/// change, and return the player mobj to the walking state if it is still in
/// an attack state.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.  `psp` must point to the
/// weapon psprite slot.
#[no_mangle]
pub unsafe extern "C" fn A_WeaponReady(player: *mut PlayerT, psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;

    // Get out of attack state.
    let state_ptr = (*mo).state as *mut State;
    if std::ptr::eq(state_ptr, &info::states[S_PLAY_ATK1 as usize])
        || std::ptr::eq(state_ptr, &info::states[S_PLAY_ATK2 as usize])
    {
        P_SetMobjState(mo, S_PLAY);
    }

    if (*player).readyweapon == wp_chainsaw
        && std::ptr::eq((*psp).state as *mut State, &info::states[S_SAW as usize])
    {
        S_StartSound(mo as *mut c_void, Sfx::Sawidl as c_int);
    }

    // Check for change: if player is dead, put the weapon away.
    if (*player).pendingweapon != wp_nochange || (*player).health == 0 {
        let newstate = weaponinfo[(*player).readyweapon as usize].downstate;
        P_SetPsprite(player, 0, newstate);
        return;
    }

    // Check for fire: the missile launcher and bfg do not auto fire.
    if (*player).cmd.buttons & BT_ATTACK != 0 {
        if (*player).attackdown == 0
            || ((*player).readyweapon != wp_missile && (*player).readyweapon != wp_bfg)
        {
            (*player).attackdown = 1;
            P_FireWeapon(player);
            return;
        }
    } else {
        (*player).attackdown = 0;
    }

    // Bob the weapon based on movement speed.
    let mut angle = (128 * leveltime) as u32 & FINEMASK as u32;
    (*psp).sx = FRACUNIT + FixedMul((*player).bob, *finecosine.0.add(angle as usize));
    angle &= (FINEANGLES / 2 - 1) as u32;
    (*psp).sy = WEAPONTOP + FixedMul((*player).bob, finesine[angle as usize]);
}

/// Weapon action: allow re-firing without fully lowering the weapon.
///
/// If the attack button is still held and no weapon change is pending, increment
/// `refire` and call `P_FireWeapon`; otherwise reset `refire` and check ammo.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn A_ReFire(player: *mut PlayerT, _psp: *mut PspdefT) {
    if (*player).cmd.buttons & BT_ATTACK != 0
        && (*player).pendingweapon == wp_nochange
        && (*player).health != 0
    {
        (*player).refire += 1;
        P_FireWeapon(player);
    } else {
        (*player).refire = 0;
        P_CheckAmmo(player);
    }
}

/// Weapon action: check ammo and switch weapons if insufficient.
///
/// Used by the super shotgun after firing to ensure the player still has
/// shells before returning to the ready state.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn A_CheckReload(player: *mut PlayerT, _psp: *mut PspdefT) {
    P_CheckAmmo(player);
}

/// Weapon action: scroll the weapon psprite down by `LOWERSPEED` each tic.
///
/// When the sprite reaches `WEAPONBOTTOM`, switches to the pending weapon
/// (or parks the weapon off-screen if the player is dead).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
/// `psp` must point to the weapon psprite slot.
#[no_mangle]
pub unsafe extern "C" fn A_Lower(player: *mut PlayerT, psp: *mut PspdefT) {
    (*psp).sy += LOWERSPEED;

    // Is already down.
    if (*psp).sy < WEAPONBOTTOM {
        return;
    }

    // Player is dead.
    if (*player).playerstate == PST_DEAD {
        (*psp).sy = WEAPONBOTTOM;
        return;
    }

    // The old weapon has been lowered off the screen, so change the weapon
    // and start raising it.
    if (*player).health == 0 {
        P_SetPsprite(player, 0, S_NULL);
        return;
    }

    (*player).readyweapon = (*player).pendingweapon;
    P_BringUpWeapon(player);
}

/// Weapon action: scroll the weapon psprite up by `RAISESPEED` each tic.
///
/// When the sprite reaches `WEAPONTOP`, transitions to the weapon's ready
/// state.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
/// `psp` must point to the weapon psprite slot.
#[no_mangle]
pub unsafe extern "C" fn A_Raise(player: *mut PlayerT, psp: *mut PspdefT) {
    (*psp).sy -= RAISESPEED;

    if (*psp).sy > WEAPONTOP {
        return;
    }

    (*psp).sy = WEAPONTOP;

    let newstate = weaponinfo[(*player).readyweapon as usize].readystate;
    P_SetPsprite(player, 0, newstate);
}

/// Weapon action: set the player mobj to attack state 2 and activate the
/// muzzle-flash psprite.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_GunFlash(player: *mut PlayerT, _psp: *mut PspdefT) {
    let mo = (*player).mo as *mut mobj_t;
    P_SetMobjState(mo, S_PLAY_ATK2);
    P_SetPsprite(
        player,
        1,
        weaponinfo[(*player).readyweapon as usize].flashstate,
    );
}

/// Weapon action: clear the extra-light boost (normal sector lighting).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn A_Light0(player: *mut PlayerT, _psp: *mut PspdefT) {
    (*player).extralight = 0;
}

/// Weapon action: set the extra-light boost to +1 (used by pistol/shotgun
/// muzzle flash).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn A_Light1(player: *mut PlayerT, _psp: *mut PspdefT) {
    (*player).extralight = 1;
}

/// Weapon action: set the extra-light boost to +2 (used by plasma / BFG
/// muzzle flash).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn A_Light2(player: *mut PlayerT, _psp: *mut PspdefT) {
    (*player).extralight = 2;
}

/// Weapon action: play the BFG charging sound.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn A_BFGsound(player: *mut PlayerT, _psp: *mut PspdefT) {
    S_StartSound((*player).mo as *mut c_void, Sfx::Bfg as c_int);
}

/// Subtract `amount` from the player's ammo for slot `ammonum`, emulating
/// the original C array-overflow behaviour: if `ammonum >= NUMAMMO` the
/// excess indexes into `maxammo` instead (Dehacked compatibility).
///
/// Kept beside the action set it feeds: every caller is a weapon action
/// in `weapons.rs`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
/// `ammonum` may legally exceed `NUMAMMO - 1`; the function handles that case.
pub(super) unsafe fn DecreaseAmmo(player: *mut PlayerT, ammonum: c_int, amount: c_int) {
    if ammonum < NUMAMMO as c_int {
        (*player).ammo[ammonum as usize] -= amount;
    } else {
        (*player).maxammo[(ammonum - NUMAMMO as c_int) as usize] -= amount;
    }
}
