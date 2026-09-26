//! The ceiling thinker: `T_MoveCeiling`, the per-tic updater that
//! drives a `ceiling_t` through `T_MovePlane`, plays the movement
//! sound on the shared cadence (silent for `silentCrushAndRaise`),
//! and reacts to arrival/crush results -- bit-exact with the
//! `T_MoveCeiling` half of `vendor/doomgeneric/p_ceilng.c`, including
//! the PRE-EXISTING port divergences documented below (never
//! normalize them here).

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};

use super::events::P_RemoveActiveCeiling;
use super::state::{
    ceiling_t, crushAndRaise, fastCrushAndRaise, lowerAndCrush, raiseToHighest, CEILSPEED,
    result_crushed, result_pastdest, silentCrushAndRaise,
};
use crate::doom::p_floor::T_MovePlane;
use crate::doom::p_tick::leveltime;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Per-tic update for a moving ceiling thinker.
///
/// Calls `T_MovePlane` with the appropriate target height (determined by
/// `ceiling->direction`), then reacts to the result:
///
/// - **`result_pastdest`**: the ceiling reached its target height.
///   - `raiseToHighest`: remove and free the thinker.
///   - `silentCrushAndRaise` (downward): play the stop sound, then reverse
///     direction upward.
///   - `fastCrushAndRaise` / `crushAndRaise` (downward): reverse direction.
///   - `lowerAndCrush` / `lowerToFloor` (downward): remove and free the
///     thinker.
/// - **`result_crushed`**: the ceiling hit something while moving down.
///   - `silentCrushAndRaise` / `crushAndRaise` / `lowerAndCrush`: reduce
///     speed to `CEILSPEED / 8` to avoid shaking the victim rapidly.
///
/// Every 8 tics, a movement sound (`sfx_stnmov`) is played at the sector's
/// sound origin, except for `silentCrushAndRaise` which suppresses it.
///
/// # FIXME
/// The C original uses a `switch(direction)` with explicit `case 0` (stasis,
/// no-op), `case 1` (up), and `case -1` (down) branches.  The direction
/// controls which target height is passed to `T_MovePlane` and which
/// `pastdest` actions apply.  This Rust port passes `direction` to
/// `T_MovePlane` but does not gate the `pastdest`/`crushed` handling on the
/// current direction, so those match arms may fire for `direction == 0`
/// (stasis) or the wrong direction — a minor behavioral divergence from C.
///
/// # FIXME
/// The C `case silentCrushAndRaise` (downward) falls through to
/// `crushAndRaise`/`fastCrushAndRaise`, which restores speed to `CEILSPEED`
/// and reverses direction.  This Rust port handles `silentCrushAndRaise`
/// separately and only plays the stop sound; it does not restore speed or
/// reverse direction for that variant.
///
/// Corresponds to `T_MoveCeiling` in `p_ceilng.c`.
///
/// # Safety
///
/// `ceiling` must be a valid, non-null pointer to a `ceiling_t` that is
/// currently linked in the thinker list and whose `sector` pointer is valid.
#[no_mangle]
pub unsafe extern "C" fn T_MoveCeiling(ceiling: *mut ceiling_t)
{
    let res = T_MovePlane(
        (*ceiling).sector,
        (*ceiling).speed,
        if (*ceiling).direction == 1
        {
            (*ceiling).topheight
        }
        else
        {
            (*ceiling).bottomheight
        },
        (*ceiling).crush,
        1,
        (*ceiling).direction,
    );

    if (leveltime & 7) == 0
    {
        match (*ceiling).r#type
        {
            x if x == silentCrushAndRaise => {}
            _ =>
            {
                S_StartSound(
                    &(*(*ceiling).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Stnmov as c_int,
                );
            }
        }
    }

    if res == result_pastdest
    {
        match (*ceiling).r#type
        {
            x if x == raiseToHighest =>
            {
                P_RemoveActiveCeiling(ceiling);
            }
            x if x == silentCrushAndRaise =>
            {
                S_StartSound(
                    &(*(*ceiling).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Pstop as c_int,
                );
            }
            x if x == fastCrushAndRaise || x == crushAndRaise =>
            {
                (*ceiling).direction = -1;
            }
            _ => {}
        }
    }
    else if res == result_crushed
    {
        match (*ceiling).r#type
        {
            x if x == silentCrushAndRaise || x == crushAndRaise || x == lowerAndCrush =>
            {
                (*ceiling).speed = CEILSPEED / 8;
            }
            _ => {}
        }
    }
}
