//! The psprite state-machine engine: `P_SetPsprite` (the runner that
//! invokes the `A_*` action functions stored in the `states` table),
//! the raise/lower/fire orchestration (`P_BringUpWeapon`,
//! `P_CheckAmmo`, `P_FireWeapon`, `P_DropWeapon`), the level-start
//! setup, and the per-tic clock (`P_SetupPsprites`,
//! `P_MovePsprites`) -- bit-exact with the corresponding functions of
//! `vendor/doomgeneric/p_pspr.c`. Also carries the dead-but-kept
//! `P_CalcSwing` (zero callers in both trees; anchor-kept for C
//! parity).

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::c_ffi::WEAPONBOTTOM;
use crate::doom::d_items::weaponinfo;
use crate::doom::d_mode::{commercial, shareware};
use crate::doom::d_player::{PlayerT, PspdefT, NUMPSPRITES};
use crate::doom::doomstat::gamemode;
use crate::doom::info::{self, State, S_PLAY_ATK1};
use crate::doom::m_fixed::{FixedMul, FRACBITS};
use crate::doom::p_enemy::P_NoiseAlert;
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::leveltime;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finesine, FINEANGLES, FINEMASK};

use super::state::{
    am_cell, am_clip, am_misl, am_noammo, am_shell, swingx, swingy, wp_bfg, wp_chainsaw,
    wp_chaingun, wp_fist, wp_missile, wp_nochange, wp_pistol, wp_plasma, wp_shotgun,
    wp_supershotgun, DEH_DEFAULT_BFG_CELLS_PER_SHOT,
};

/// Transition a psprite slot to a new state, running action functions until a
/// non-zero tic count is reached or the state chain ends.
///
/// Mirrors `P_SetPsprite` in `p_pspr.c`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
/// `position` must be in `0..NUMPSPRITES`.  `stnum` must be a valid state
/// index or `S_NULL` (0).
#[no_mangle]
pub unsafe extern "C" fn P_SetPsprite(player: *mut PlayerT, position: c_int, stnum: c_int)
{
    let psp = (*player).psprites.as_mut_ptr().add(position as usize);

    let mut stnum = stnum;
    loop
    {
        if stnum == 0
        {
            (*psp).state = std::ptr::null_mut();
            break;
        }

        let state = &mut info::states[stnum as usize] as *mut State;
        (*psp).state = state as *mut crate::doom::d_player::state_t;
        (*psp).tics = (*state).tics;

        if (*state).misc1 != 0
        {
            (*psp).sx = (*state).misc1 << FRACBITS;
            (*psp).sy = (*state).misc2 << FRACBITS;
        }

        // Call action routine.
        if let Some(action) = (*state).action
        {
            let action: unsafe extern "C" fn(*mut PlayerT, *mut PspdefT) =
                std::mem::transmute(action);
            action(player, psp);
            if(*psp).state.is_null() { break; }
        }

        // Read nextstate from psp->state (action may have changed it).
        stnum = ((*psp).state as *mut State).read().nextstate;

        if(*psp).tics != 0 { break; }
    }
}

/// Recompute the horizontal (`swingx`) and vertical (`swingy`) weapon-bob
/// offsets for the current tic using the player's `bob` amplitude.
///
/// Dead in both trees (zero callers upstream and here); anchor-kept for
/// C parity -- see the module mapping table.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_CalcSwing(player: *mut PlayerT)
{
    let swing = (*player).bob;

    let mut angle = (FINEANGLES as c_int / 70 * leveltime) & FINEMASK;
    swingx = FixedMul(swing, finesine[angle as usize]);

    angle = (FINEANGLES as c_int / 70 * leveltime + FINEANGLES as c_int / 2) & FINEMASK;
    swingy = -FixedMul(swingx, finesine[angle as usize]);
}

/// Begin the raise animation for the pending weapon by setting the weapon
/// psprite to its `upstate` and positioning it at `WEAPONBOTTOM`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_BringUpWeapon(player: *mut PlayerT)
{
    if(*player).pendingweapon == wp_nochange { (*player).pendingweapon = (*player).readyweapon; }

    if(*player).pendingweapon == wp_chainsaw { S_StartSound((*player).mo as *mut c_void, Sfx::Sawup as c_int); }

    let newstate = weaponinfo[(*player).pendingweapon as usize].upstate;

    (*player).pendingweapon = wp_nochange;
    (*player).psprites[0].sy = WEAPONBOTTOM;

    P_SetPsprite(player, 0, newstate);
}

/// Return `1` if the player has enough ammo to fire the ready weapon, `0`
/// otherwise.  When out of ammo, selects the next best weapon and begins
/// lowering the current one.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_CheckAmmo(player: *mut PlayerT) -> c_int
{
    let ammo = weaponinfo[(*player).readyweapon as usize].ammo;

    let count: c_int = if (*player).readyweapon == wp_bfg
    {
        DEH_DEFAULT_BFG_CELLS_PER_SHOT
    }
    else if(*player).readyweapon == wp_supershotgun { 2 }
    else { 1 };

    if ammo == am_noammo || (*player).ammo[ammo as usize] >= count { return 1; }

    // Out of ammo, pick a weapon to change to.
    loop
    {
        if (*player).weaponowned[wp_plasma as usize] != 0
            && (*player).ammo[am_cell as usize] != 0
            && gamemode != shareware
        {
            (*player).pendingweapon = wp_plasma;
        }
        else if(*player).weaponowned[wp_supershotgun as usize] != 0
            && (*player).ammo[am_shell as usize] > 2
            && gamemode == commercial
        {
            (*player).pendingweapon = wp_supershotgun;
        }
        else if(*player).weaponowned[wp_chaingun as usize] != 0
            && (*player).ammo[am_clip as usize] != 0
        {
            (*player).pendingweapon = wp_chaingun;
        }
        else if(*player).weaponowned[wp_shotgun as usize] != 0
            && (*player).ammo[am_shell as usize] != 0
        {
            (*player).pendingweapon = wp_shotgun;
        }
        else if(*player).ammo[am_clip as usize] != 0 { (*player).pendingweapon = wp_pistol; }
        else if(*player).weaponowned[wp_chainsaw as usize] != 0 { (*player).pendingweapon = wp_chainsaw; }
        else if(*player).weaponowned[wp_missile as usize] != 0
            && (*player).ammo[am_misl as usize] != 0
        {
            (*player).pendingweapon = wp_missile;
        }
        else if(*player).weaponowned[wp_bfg as usize] != 0
            && (*player).ammo[am_cell as usize] > 40
            && gamemode != shareware
        {
            (*player).pendingweapon = wp_bfg;
        }
        else { (*player).pendingweapon = wp_fist; }

        if(*player).pendingweapon != wp_nochange { break; }
    }

    P_SetPsprite(
        player,
        0,
        weaponinfo[(*player).readyweapon as usize].downstate,
    );

    0
}

/// Check ammo and, if sufficient, put the player mobj into the attack state
/// and transition the weapon psprite to its attack state.  Also alerts nearby
/// monsters via `P_NoiseAlert`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub unsafe extern "C" fn P_FireWeapon(player: *mut PlayerT)
{
    if P_CheckAmmo(player) == 0 { return; }

    P_SetMobjState((*player).mo as *mut mobj_t, S_PLAY_ATK1);
    let newstate = weaponinfo[(*player).readyweapon as usize].atkstate;
    P_SetPsprite(player, 0, newstate);
    P_NoiseAlert((*player).mo as *mut mobj_t, (*player).mo as *mut mobj_t);
}

/// Begin lowering the current weapon (called on player death or weapon switch).
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_DropWeapon(player: *mut PlayerT)
{
    P_SetPsprite(
        player,
        0,
        weaponinfo[(*player).readyweapon as usize].downstate,
    );
}

/// Initialise the player's psprite slots and begin raising the current weapon.
///
/// Called at the start of each level for every active player.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_SetupPsprites(player: *mut PlayerT)
{
    for i in 0..NUMPSPRITES { (*player).psprites[i].state = std::ptr::null_mut(); }

    (*player).pendingweapon = (*player).readyweapon;
    P_BringUpWeapon(player);
}

/// Advance the psprite state machine for all slots each tic and copy the
/// weapon slot's position to the flash slot.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_MovePsprites(player: *mut PlayerT)
{
    let mut psp = (*player).psprites.as_mut_ptr();

    for i in 0..NUMPSPRITES
    {
        let state = (*psp).state as *mut State;
        if !state.is_null()
        {
            // Drop tic count and possibly change state.
            // A -1 tic count never changes.
            if (*psp).tics != -1
            {
                (*psp).tics -= 1;
                if(*psp).tics == 0 { P_SetPsprite(player, i as c_int, (*state).nextstate); }
            }
        }
        psp = psp.add(1);
    }

    (*player).psprites[1].sx = (*player).psprites[0].sx;
    (*player).psprites[1].sy = (*player).psprites[0].sy;
}
