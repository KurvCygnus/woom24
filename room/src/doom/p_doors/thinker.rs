//! The door thinker: `T_VerticalDoor`, the per-tic updater that drives
//! a `vldoor_t` through `T_MovePlane` on its sector's ceiling, runs
//! the direction / countdown machine, and removes finished doors --
//! bit-exact with the `T_VerticalDoor` half of
//! `vendor/doomgeneric/p_doors.c`.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use super::state::{
    result_crushed, result_pastdest, vld_blazeClose, vld_blazeOpen, vld_blazeRaise, vld_close,
    vld_close30ThenOpen, vld_normal, vld_open, vld_raiseIn5Mins, vldoor_t,
};
use crate::doom::i_timer::TICRATE;
use crate::doom::p_floor::T_MovePlane;
use crate::doom::p_tick::P_RemoveThinker;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Per-tic update for a vertical door.
///
/// Dispatches on `door->direction`:
/// - `0` (waiting): counts down `topcountdown`, then starts closing or re-opening.
/// - `2` (initial wait): counts down `topcountdown`, then transitions to open.
/// - `-1` (moving down): calls `T_MovePlane`; handles crush, fully-closed, and
///   the `vld_close30ThenOpen` re-open delay.
/// - `1` (moving up): calls `T_MovePlane`; parks at top or removes thinker when done.
///
/// # Safety
///
/// `door` must be a valid, aligned, non-null pointer to a `vldoor_t` whose
/// `sector` pointer is also valid for the current map. Called exclusively by
/// the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_VerticalDoor(door: *mut vldoor_t)
{
    let res: c_int;

    match(*door).direction
    {
        0 =>
        {
            // WAITING
            (*door).topcountdown -= 1;
            if(*door).topcountdown == 0
            {
                match(*door).r#type
                {
                    x if x == vld_blazeRaise =>
                    {
                        (*door).direction = -1;
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Bdcls as c_int,
                        );
                    }
                    x if x == vld_normal =>
                    {
                        (*door).direction = -1;
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Dorcls as c_int,
                        );
                    }
                    x if x == vld_close30ThenOpen =>
                    {
                        (*door).direction = 1;
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Doropn as c_int,
                        );
                    }
                    _ => {}
                }
            }
        }
        2 =>
        {
            // INITIAL WAIT
            (*door).topcountdown -= 1;
            if(*door).topcountdown == 0
            {
                match(*door).r#type
                {
                    x if x == vld_raiseIn5Mins =>
                    {
                        (*door).direction = 1;
                        (*door).r#type = vld_normal;
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Doropn as c_int,
                        );
                    }
                    _ => {}
                }
            }
        }
        -1 =>
        {
            // DOWN
            res = T_MovePlane(
                (*door).sector,
                (*door).speed,
                (*(*door).sector).floorheight,
                0,
                1,
                -1,
            );
            if res == result_pastdest
            {
                match(*door).r#type
                {
                    x if x == vld_blazeRaise || x == vld_blazeClose =>
                    {
                        (*(*door).sector).specialdata = std::ptr::null_mut();
                        P_RemoveThinker(&mut (*door).thinker);
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Bdcls as c_int,
                        );
                    }
                    x if x == vld_normal || x == vld_close =>
                    {
                        (*(*door).sector).specialdata = std::ptr::null_mut();
                        P_RemoveThinker(&mut (*door).thinker);
                    }
                    x if x == vld_close30ThenOpen =>
                    {
                        (*door).direction = 0;
                        (*door).topcountdown = TICRATE * 30;
                    }
                    _ => {}
                }
            }
            else if res == result_crushed
            {
                match (*door).r#type
                {
                    x if x == vld_blazeClose || x == vld_close => { /* DO NOT GO BACK UP! */ }
                    _ =>
                    {
                        (*door).direction = 1;
                        S_StartSound(
                            &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                            Sfx::Doropn as c_int,
                        );
                    }
                }
            }
        }
        1 =>
        {
            // UP
            res = T_MovePlane((*door).sector, (*door).speed, (*door).topheight, 0, 1, 1);
            if res == result_pastdest
            {
                match(*door).r#type
                {
                    x if x == vld_blazeRaise || x == vld_normal =>
                    {
                        (*door).direction = 0;
                        (*door).topcountdown = (*door).topwait;
                    }
                    x if x == vld_close30ThenOpen || x == vld_blazeOpen || x == vld_open =>
                    {
                        (*(*door).sector).specialdata = std::ptr::null_mut();
                        P_RemoveThinker(&mut (*door).thinker);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
