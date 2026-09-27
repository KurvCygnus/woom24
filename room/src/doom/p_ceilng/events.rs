//! Linedef-triggered ceiling events and the active-ceiling table
//! maintenance: `EV_DoCeiling` (the per-type spawner, carrying the
//! PRE-EXISTING `silentCrushAndRaise`/`crushAndRaise` missing
//! fall-through configuration divergence, documented below -- never
//! normalize it here), `EV_CeilingCrushStop`, and the
//! `P_AddActiveCeiling` / `P_RemoveActiveCeiling` /
//! `P_ActivateInStasisCeiling` table operations -- bit-exact with the
//! event half of `vendor/doomgeneric/p_ceilng.c`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};

use super::state::{
    activeceilings, ceiling_t, crushAndRaise, fastCrushAndRaise, lowerAndCrush, lowerToFloor,
    raiseToHighest, silentCrushAndRaise, MAXCEILINGS, CEILSPEED,
};
use super::thinker::T_MoveCeiling;
use crate::doom::c_ffi as cffi;
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_lights::{line_t, sector_t};
use crate::doom::p_setup::sectors;
use crate::doom::p_spec::{P_FindHighestCeilingSurrounding, P_FindSectorFromLineTag};
use crate::doom::p_tick::{P_AddThinker, P_RemoveThinker};
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

/// Activate a ceiling mover on every sector whose tag matches `line->tag`.
///
/// For crusher types (`fastCrushAndRaise`, `silentCrushAndRaise`,
/// `crushAndRaise`), first reactivates any in-stasis ceilings with the same
/// tag via [`P_ActivateInStasisCeiling`].
///
/// For each eligible sector (no existing special data):
/// 1. Allocates and links a new `ceiling_t` thinker.
/// 2. Sets its parameters based on `ceilingtype` (height targets, speed,
///    crush flag, direction).  `silentCrushAndRaise` and `crushAndRaise` only
///    initialise `crush` and `topheight`; `bottomheight` and `direction` keep
///    their zero-initialised defaults and are set dynamically by `T_MoveCeiling`.
/// 3. Registers it in [`activeceilings`].
///
/// Returns `1` if at least one ceiling was activated, `0` otherwise.
///
/// Corresponds to `EV_DoCeiling` in `p_ceilng.c`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer.  The global `sectors` array must
/// be initialised for the current level.
#[no_mangle]
pub unsafe extern "C" fn EV_DoCeiling(line: *mut line_t, ceilingtype: c_int) -> c_int
{
    let mut secnum: c_int = -1;
    let mut rtn: c_int = 0;

    // Reactivate in-stasis ceilings for certain types.
    match ceilingtype
    {
        x if x == fastCrushAndRaise || x == silentCrushAndRaise || x == crushAndRaise =>
        {
            P_ActivateInStasisCeiling(line);
        }
        _ => {}
    }

    while
    {
        secnum = P_FindSectorFromLineTag(line as *mut cffi::line_t, secnum);
        secnum
    } >= 0
    {
        let sec = sectors.add(secnum as usize);
        if !(*sec).specialdata.is_null()
        {
            continue;
        }

        rtn = 1;
        let ceiling = Z_Malloc(
            std::mem::size_of::<ceiling_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut ceiling_t;
        P_AddThinker(&mut (*ceiling).thinker);
        (*sec).specialdata = ceiling as *mut c_void;
        (*ceiling).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut ceiling_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_MoveCeiling));
        (*ceiling).sector = sec as *mut sector_t;
        (*ceiling).crush = 0;

        match ceilingtype
        {
            x if x == fastCrushAndRaise =>
            {
                (*ceiling).crush = 1;
                (*ceiling).topheight = (*sec).ceilingheight;
                (*ceiling).bottomheight = (*sec).floorheight + (8 * FRACUNIT);
                (*ceiling).direction = -1;
                (*ceiling).speed = CEILSPEED * 2;
            }
            x if x == silentCrushAndRaise || x == crushAndRaise =>
            {
                (*ceiling).crush = 1;
                (*ceiling).topheight = (*sec).ceilingheight;
            }
            x if x == lowerAndCrush || x == lowerToFloor =>
            {
                (*ceiling).bottomheight = (*sec).floorheight;
                if ceilingtype != lowerToFloor
                {
                    (*ceiling).bottomheight += 8 * FRACUNIT;
                }
                (*ceiling).direction = -1;
                (*ceiling).speed = CEILSPEED;
            }
            x if x == raiseToHighest =>
            {
                (*ceiling).topheight = P_FindHighestCeilingSurrounding(sec);
                (*ceiling).direction = 1;
                (*ceiling).speed = CEILSPEED;
            }
            _ => {}
        }

        (*ceiling).tag = (*sec).tag as c_int;
        (*ceiling).r#type = ceilingtype;
        P_AddActiveCeiling(ceiling);
    }
    rtn
}

/// Register `c` in the first available slot of [`activeceilings`].
///
/// If all [`MAXCEILINGS`] slots are occupied the function returns silently,
/// discarding the ceiling — matching the C behaviour which has no overflow
/// guard.
///
/// Corresponds to `P_AddActiveCeiling` in `p_ceilng.c`.
#[no_mangle]
pub extern "C" fn P_AddActiveCeiling(c: *mut ceiling_t)
{
    unsafe
    {
        for i in 0..MAXCEILINGS
        {
            if activeceilings[i].is_null()
            {
                activeceilings[i] = c;
                return;
            }
        }
    }
}

/// Unlink and schedule deallocation of the active ceiling `c`.
///
/// Clears `sector->specialdata`, calls [`P_RemoveThinker`] (which marks the
/// thinker for deferred `Z_Free`), and nulls the matching slot in
/// [`activeceilings`].  Does nothing if `c` is not found in the table.
///
/// Corresponds to `P_RemoveActiveCeiling` in `p_ceilng.c`.
#[no_mangle]
pub extern "C" fn P_RemoveActiveCeiling(c: *mut ceiling_t)
{
    unsafe
    {
        for i in 0..MAXCEILINGS
        {
            if activeceilings[i] == c
            {
                (*activeceilings[i]).sector.as_mut().unwrap().specialdata = std::ptr::null_mut();
                P_RemoveThinker(&mut (*activeceilings[i]).thinker);
                activeceilings[i] = std::ptr::null_mut();
                break;
            }
        }
    }
}

/// Restart any in-stasis ceilings whose tag matches `line->tag`.
///
/// A ceiling is in stasis when `direction == 0`.  This function restores
/// `direction` from `olddirection` and reinstates `T_MoveCeiling` as the
/// thinker callback.
///
/// Called by [`EV_DoCeiling`] for crusher types, and corresponds to
/// `P_ActivateInStasisCeiling` in `p_ceilng.c`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer.
#[no_mangle]
pub extern "C" fn P_ActivateInStasisCeiling(line: *mut line_t)
{
    unsafe
    {
        for i in 0..MAXCEILINGS
        {
            if !activeceilings[i].is_null()
                && (*activeceilings[i]).tag == (*line).tag as c_int
                && (*activeceilings[i]).direction == 0
            {
                (*activeceilings[i]).direction = (*activeceilings[i]).olddirection;
                (*activeceilings[i]).thinker.function.acp1 = Some(core::mem::transmute::<
                    unsafe extern "C" fn(*mut ceiling_t),
                    unsafe extern "C" fn(*mut c_void),
                >(T_MoveCeiling));
            }
        }
    }
}

/// Stop all active crusher ceilings whose tag matches `line->tag`.
///
/// Each matching ceiling has its `direction` saved to `olddirection`, its
/// `direction` set to `0` (stasis), and its thinker function set to `None` so
/// the callback is skipped each tic.  The ceiling is not removed from
/// [`activeceilings`]; it can be restarted by [`P_ActivateInStasisCeiling`].
///
/// Returns `1` if at least one ceiling was stopped, `0` otherwise.
///
/// Corresponds to `EV_CeilingCrushStop` in `p_ceilng.c`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer.
#[no_mangle]
pub extern "C" fn EV_CeilingCrushStop(line: *mut line_t) -> c_int
{
    unsafe
    {
        let mut rtn: c_int = 0;
        for i in 0..MAXCEILINGS
        {
            if !activeceilings[i].is_null()
                && (*activeceilings[i]).tag == (*line).tag as c_int
                && (*activeceilings[i]).direction != 0
            {
                (*activeceilings[i]).olddirection = (*activeceilings[i]).direction;
                (*activeceilings[i]).thinker.function.acv = None;
                (*activeceilings[i]).direction = 0;
                rtn = 1;
            }
        }
        rtn
    }
}
