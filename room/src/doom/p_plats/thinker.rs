//! The plat thinker: `T_PlatRaise`, the per-tic updater that drives
//! a `plat_t` through `T_MovePlane`, flips direction on crush and
//! arrival, runs the wait countdown, and removes one-shot plats --
//! bit-exact with the `T_PlatRaise` half of
//! `vendor/doomgeneric/p_plats.c`.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};

use super::events::P_RemoveActivePlat;
use super::state::{
    blazeDWUS, down, downWaitUpStay, in_stasis, plat_t, raiseAndChange, raiseToNearestAndChange,
    result_crushed, result_pastdest, up, waiting,
};
use crate::doom::p_floor::T_MovePlane;
use crate::doom::p_tick::leveltime;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Per-tic update for a moving platform thinker.
///
/// Dispatches on `plat->status`:
///
/// - **`up`**: calls `T_MovePlane` toward `high`.
///   - If the result is `result_crushed` and `crush == 0`, resets the count,
///     reverses direction to `down`, and plays the start sound.
///   - If the result is `result_pastDest`, switches to `waiting`, plays the
///     stop sound, and for one-shot types (`downWaitUpStay`, `blazeDWUS`,
///     `raiseAndChange`, `raiseToNearestAndChange`) removes the platform.
///   - For `raiseAndChange`/`raiseToNearestAndChange`, plays the movement
///     sound every 8 tics during the rise.
/// - **`down`**: calls `T_MovePlane` toward `low` (no crush).
///   - On `result_pastDest`, switches to `waiting` and plays the stop sound.
/// - **`waiting`**: decrements `count`; when it reaches 0, determines the next
///   direction by comparing the current floor height to `low`, plays the start
///   sound, and transitions to `up` or `down`.
/// - **`in_stasis`**: no-op — the thinker function is nulled by
///   [`EV_StopPlat`](crate::doom::p_plats::EV_StopPlat) in practice, but this
///   arm handles the edge case where it is not.
///
/// Corresponds to `T_PlatRaise` in `p_plats.c`.
///
/// # Safety
///
/// `plat` must be a valid, non-null pointer to a `plat_t` that is currently
/// linked in the thinker list and whose `sector` pointer is valid.
#[no_mangle]
pub unsafe extern "C" fn T_PlatRaise(plat: *mut plat_t)
{
    match (*plat).status
    {
        x if x == up =>
        {
            let res = T_MovePlane(
                (*plat).sector,
                (*plat).speed,
                (*plat).high,
                (*plat).crush,
                0,
                1,
            );

            if ((*plat).r#type == raiseAndChange || (*plat).r#type == raiseToNearestAndChange)
                && (leveltime & 7) == 0
            {
                S_StartSound(
                    &(*(*plat).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Stnmov as c_int,
                );
            }

            if res == result_crushed && (*plat).crush == 0
            {
                (*plat).count = (*plat).wait;
                (*plat).status = down;
                S_StartSound(
                    &(*(*plat).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstart as c_int,
                );
            }
            else if res == result_pastdest
            {
                (*plat).count = (*plat).wait;
                (*plat).status = waiting;
                S_StartSound(
                    &(*(*plat).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstop as c_int,
                );

                match (*plat).r#type
                {
                    x if x == blazeDWUS
                        || x == downWaitUpStay
                        || x == raiseAndChange
                        || x == raiseToNearestAndChange =>
                    {
                        P_RemoveActivePlat(plat);
                    }
                    _ => {}
                }
            }
        }
        x if x == down =>
        {
            let res = T_MovePlane((*plat).sector, (*plat).speed, (*plat).low, 0, 0, -1);

            if res == result_pastdest
            {
                (*plat).count = (*plat).wait;
                (*plat).status = waiting;
                S_StartSound(
                    &(*(*plat).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstop as c_int,
                );
            }
        }
        x if x == waiting =>
        {
            (*plat).count -= 1;
            if (*plat).count == 0
            {
                if (*(*plat).sector).floorheight == (*plat).low
                {
                    (*plat).status = up;
                }
                else
                {
                    (*plat).status = down;
                }
                S_StartSound(
                    &(*(*plat).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstart as c_int,
                );
            }
        }
        x if x == in_stasis => {}
        _ => {}
    }
}
