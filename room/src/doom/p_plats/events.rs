//! Linedef-triggered plat events and the active-plat table
//! maintenance: `EV_DoPlat` (the per-type spawner whose perpetualRaise
//! arm draws one `P_Random` byte), `EV_StopPlat`, `P_ActivateInStasis`,
//! and the `P_AddActivePlat` / `P_RemoveActivePlat` table operations --
//! bit-exact with the event half of `vendor/doomgeneric/p_plats.c`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};

use super::state::{
    activeplats, blazeDWUS, down, downWaitUpStay, in_stasis, MAXPLATS, perpetualRaise, plat_t,
    raiseAndChange, raiseToNearestAndChange, up, PLATSPEED, PLATWAIT,
};
use super::thinker::T_PlatRaise;
use crate::doom::c_ffi as cffi;
use crate::doom::i_timer::TICRATE;
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::m_random::P_Random;
use crate::doom::p_lights::{line_t, sector_t};
use crate::doom::p_setup::{sectors, sides};
use crate::doom::p_spec::{
    P_FindHighestFloorSurrounding, P_FindLowestFloorSurrounding, P_FindNextHighestFloor,
    P_FindSectorFromLineTag,
};
use crate::doom::p_tick::{P_AddThinker, P_RemoveThinker};
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};
use crate::i_error;

/// Activate a platform mover on every sector whose tag matches `line->tag`.
///
/// For `perpetualRaise`, first reactivates any in-stasis platforms with the
/// same tag via [`P_ActivateInStasis`].
///
/// For each eligible sector (no existing special data):
/// 1. Allocates and links a new `plat_t` thinker.
/// 2. Initialises its parameters (speed, height targets, wait time, initial
///    status) based on `plattype`.
/// 3. Registers it in [`activeplats`].
///
/// The `amount` parameter is only used by `raiseAndChange` to set the target
/// height to `floorheight + amount * FRACUNIT`.
///
/// Returns `1` if at least one platform was activated, `0` otherwise.
///
/// Corresponds to `EV_DoPlat` in `p_plats.c`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer.  The global `sectors` and `sides`
/// arrays must be initialised for the current level.
#[no_mangle]
pub unsafe extern "C" fn EV_DoPlat(line: *mut line_t, plattype: c_int, amount: c_int) -> c_int
{
    let mut secnum: c_int = -1;
    let mut rtn: c_int = 0;

    match plattype
    {
        x if x == perpetualRaise => { P_ActivateInStasis((*line).tag as c_int); }
        _ => {}
    }

    while
    {
        secnum = P_FindSectorFromLineTag(line as *mut cffi::line_t, secnum);
        secnum
    } >= 0
    {
        let sec = sectors.add(secnum as usize);

        if !(*sec).specialdata.is_null() { continue; }

        rtn = 1;
        let plat = Z_Malloc(
            std::mem::size_of::<plat_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut plat_t;
        P_AddThinker(&mut (*plat).thinker);

        (*plat).r#type = plattype;
        (*plat).sector = sec as *mut sector_t;
        (*plat).sector.as_mut().unwrap().specialdata = plat as *mut c_void;
        (*plat).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut plat_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_PlatRaise));
        (*plat).crush = 0;
        (*plat).tag = (*line).tag as c_int;

        match plattype
        {
            x if x == raiseToNearestAndChange =>
            {
                (*plat).speed = PLATSPEED / 2;
                let sidenum = (*line).sidenum[0] as isize;
                (*sec).floorpic = (*sides.offset(sidenum)).sector.as_mut().unwrap().floorpic;
                (*plat).high = P_FindNextHighestFloor(sec, (*sec).floorheight);
                (*plat).wait = 0;
                (*plat).status = up;
                (*sec).special = 0;
                S_StartSound(
                    &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Stnmov as c_int,
                );
            }
            x if x == raiseAndChange =>
            {
                (*plat).speed = PLATSPEED / 2;
                let sidenum = (*line).sidenum[0] as isize;
                (*sec).floorpic = (*sides.offset(sidenum)).sector.as_mut().unwrap().floorpic;
                (*plat).high = (*sec).floorheight + amount * FRACUNIT;
                (*plat).wait = 0;
                (*plat).status = up;
                S_StartSound(
                    &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Stnmov as c_int,
                );
            }
            x if x == downWaitUpStay =>
            {
                (*plat).speed = PLATSPEED * 4;
                (*plat).low = P_FindLowestFloorSurrounding(sec);
                if(*plat).low > (*sec).floorheight { (*plat).low = (*sec).floorheight; }
                (*plat).high = (*sec).floorheight;
                (*plat).wait = TICRATE * PLATWAIT;
                (*plat).status = down;
                S_StartSound(
                    &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstart as c_int,
                );
            }
            x if x == blazeDWUS =>
            {
                (*plat).speed = PLATSPEED * 8;
                (*plat).low = P_FindLowestFloorSurrounding(sec);
                if(*plat).low > (*sec).floorheight { (*plat).low = (*sec).floorheight; }
                (*plat).high = (*sec).floorheight;
                (*plat).wait = TICRATE * PLATWAIT;
                (*plat).status = down;
                S_StartSound(
                    &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstart as c_int,
                );
            }
            x if x == perpetualRaise =>
            {
                (*plat).speed = PLATSPEED;
                (*plat).low = P_FindLowestFloorSurrounding(sec);
                if(*plat).low > (*sec).floorheight { (*plat).low = (*sec).floorheight; }
                (*plat).high = P_FindHighestFloorSurrounding(sec);
                if(*plat).high < (*sec).floorheight { (*plat).high = (*sec).floorheight; }
                (*plat).wait = TICRATE * PLATWAIT;
                (*plat).status = P_Random() & 1;
                S_StartSound(
                    &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstart as c_int,
                );
            }
            _ => {}
        }
        P_AddActivePlat(plat);
    }
    rtn
}

/// Restart all in-stasis platforms whose tag equals `tag`.
///
/// Restores `status` from `oldstatus` and reinstates `T_PlatRaise` as the
/// thinker callback.  Called by [`EV_DoPlat`] for the `perpetualRaise` type.
///
/// Corresponds to `P_ActivateInStasis` in `p_plats.c`.
#[no_mangle]
pub extern "C" fn P_ActivateInStasis(tag: c_int)
{
    unsafe
    {
        for i in 0..MAXPLATS
        {
            if !activeplats[i].is_null()
                && (*activeplats[i]).tag == tag
                && (*activeplats[i]).status == in_stasis
            {
                (*activeplats[i]).status = (*activeplats[i]).oldstatus;
                (*activeplats[i]).thinker.function.acp1 = Some(core::mem::transmute::<
                    unsafe extern "C" fn(*mut plat_t),
                    unsafe extern "C" fn(*mut c_void),
                >(T_PlatRaise));
            }
        }
    }
}

/// Suspend all active platforms whose tag matches `line->tag`.
///
/// Saves `status` to `oldstatus`, sets `status` to `in_stasis`, and sets the
/// thinker function to `None` so the callback is skipped.  The platform
/// remains in [`activeplats`] and can be resumed by [`P_ActivateInStasis`].
///
/// Corresponds to `EV_StopPlat` in `p_plats.c`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer.
#[no_mangle]
pub extern "C" fn EV_StopPlat(line: *mut line_t)
{
    unsafe
    {
        for j in 0..MAXPLATS
        {
            if !activeplats[j].is_null()
                && (*activeplats[j]).status != in_stasis
                && (*activeplats[j]).tag == (*line).tag as c_int
            {
                (*activeplats[j]).oldstatus = (*activeplats[j]).status;
                (*activeplats[j]).status = in_stasis;
                (*activeplats[j]).thinker.function.acv = None;
            }
        }
    }
}

/// Register `plat` in the first available slot of [`activeplats`].
///
/// If all `MAXPLATS` (30) slots are occupied, calls `I_Error` and aborts — unlike
/// `P_AddActiveCeiling` which silently discards overflows.
///
/// Corresponds to `P_AddActivePlat` in `p_plats.c`.
#[no_mangle]
pub extern "C" fn P_AddActivePlat(plat: *mut plat_t)
{
    unsafe
    {
        for i in 0..MAXPLATS
        {
            if activeplats[i].is_null()
            {
                activeplats[i] = plat;
                return;
            }
        }
        i_error!("P_AddActivePlat: no more plats!");
    }
}

/// Unlink and schedule deallocation of the active platform `plat`.
///
/// Clears `sector->specialdata`, calls [`P_RemoveThinker`] (which marks the
/// thinker for deferred `Z_Free`), and nulls the matching slot in
/// [`activeplats`].  Calls `I_Error` if the platform is not found.
///
/// Corresponds to `P_RemoveActivePlat` in `p_plats.c`.
#[no_mangle]
pub extern "C" fn P_RemoveActivePlat(plat: *mut plat_t)
{
    unsafe
    {
        for i in 0..MAXPLATS
        {
            if plat == activeplats[i]
            {
                (*activeplats[i]).sector.as_mut().unwrap().specialdata = std::ptr::null_mut();
                P_RemoveThinker(&mut (*activeplats[i]).thinker);
                activeplats[i] = std::ptr::null_mut();
                return;
            }
        }
        i_error!("P_RemoveActivePlat: can't find plat!");
    }
}
