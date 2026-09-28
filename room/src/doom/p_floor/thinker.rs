//! The floor thinker: `T_MoveFloor`, the per-tic updater that drives
//! a `floormove_t` through `T_MovePlane`, plays the movement sound on
//! the shared cadence, and applies texture/special changes on
//! arrival -- bit-exact with the `T_MoveFloor` half of
//! `vendor/doomgeneric/p_floor.c`.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};

use super::mover::T_MovePlane;
use super::state::{floor_donutRaise, floor_lowerAndChange, floormove_t, result_pastdest};
use crate::doom::p_tick::{leveltime, thinker_t, P_RemoveThinker};
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Per-tic update for a moving floor.
///
/// Calls `T_MovePlane` every tic and plays `sfx_stnmov` every 8 tics. When the
/// floor reaches `floordestheight` (`result_pastdest`), it applies any pending
/// texture/special changes, removes the thinker, and plays `sfx_pstop`.
///
/// # Safety
///
/// `floor` must be a valid, aligned, non-null pointer to a `floormove_t`
/// whose embedded `sector` pointer is also valid for the current map.
/// Called exclusively by the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_MoveFloor(floor: *mut floormove_t)
{
    let res = T_MovePlane(
        (*floor).sector,
        (*floor).speed,
        (*floor).floordestheight,
        (*floor).crush,
        0,
        (*floor).direction,
    );

    if(leveltime & 7) == 0
    {
        S_StartSound(
            &(*(*floor).sector).soundorg as *const [u8; 40] as *mut c_void,
            Sfx::Stnmov as c_int,
        );
    }

    if res == result_pastdest
    {
        (*(*floor).sector).specialdata = std::ptr::null_mut();

        if (*floor).direction == 1
        {
            if (*floor).r#type == floor_donutRaise
            {
                (*(*floor).sector).special = (*floor).newspecial as i16;
                (*(*floor).sector).floorpic = (*floor).texture;
            }
        }
        else if (*floor).direction == -1 && (*floor).r#type == floor_lowerAndChange
        {
            (*(*floor).sector).special = (*floor).newspecial as i16;
            (*(*floor).sector).floorpic = (*floor).texture;
        }
        P_RemoveThinker(&mut (*floor).thinker as *mut thinker_t);

        S_StartSound(
            &(*(*floor).sector).soundorg as *const [u8; 40] as *mut c_void,
            Sfx::Pstop as c_int,
        );
    }
}
