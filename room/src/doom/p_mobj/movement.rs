//! Momentum integration for map objects: the XY sub-step mover with
//! sliding, missile explosion, and friction, and the Z mover with view
//! smoothing, floating-monster tracking, gravity, and the version-
//! gated Lost Soul floor bounce -- bit-exact with the movement half of
//! `vendor/doomgeneric/p_mobj.c`.

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::d_player::{PlayerT, CF_NOMOMENTUM};
use crate::doom::doomstat::gameversion;
use crate::doom::info::{self, *};
use crate::doom::m_fixed::{FixedMul, FRACUNIT};
use crate::doom::p_map::{ceilingline, P_SlideMove, P_TryMove};
use crate::doom::p_maputl::P_AproxDistance;
use crate::doom::p_telept::{line_t, mobj_t};
use crate::doom::r_sky::skyflatnum;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = crate::doom::c_ffi::mobj_t;

use super::consts::{
    exe_ultimate, FLOATSPEED, FRICTION, GRAVITY, MAXMOVE, STOPSPEED, VIEWHEIGHT,
};
use super::lifecycle::remove_mobj;
use super::state::{explode_missile, set_mobj_state};

/// Apply one tic of XY momentum to `mo`, sub-stepping when the move exceeds
/// `MAXMOVE / 2`.  Handles player sliding, missile explosion against walls
/// and the sky, and floor friction / stopping.
///
/// ## Technical Details
///
/// The sub-step loop halves any momentum component above
/// `MAXMOVE / 2` and retries, so collision callbacks see the same
/// half-step sequence vanilla produced. On a blocked move the
/// missile/sky-hack arm order (ceilingline -> sky check -> remove,
/// else explode) and the player slide dispatch are statement-order
/// load-bearing, as is the friction ladder: the stop-snap arm (below
/// `STOPSPEED` with no input) resets the player run state via the
/// state-index probe before zeroing momentum, while the other arm
/// applies `FixedMul(mom, FRICTION)` -- draw-free but order-pinned by
/// the goldens.
///
/// ## On Calling
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` linked into
/// the active thinker list. The mobj may be removed inside the loop
/// (missile explosion); on return the caller must re-check via the
/// thinker sentinel (`mobj_thinker` does exactly that).
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` that is part of the
/// active thinker list.
#[doc(alias = "P_XYMovement")]
#[export_name = "P_XYMovement"]
pub unsafe extern "C" fn xy_movement(mo: *mut mobj_t)
{
    let mo = &mut *mo;
    let mut ptryx: c_int;
    let mut ptryy: c_int;
    let player = mo.player as *mut PlayerT;
    let mut xmove: c_int;
    let mut ymove: c_int;

    if mo.momx == 0 && mo.momy == 0
    {
        if mo.flags & MF_SKULLFLY != 0
        {
            let info = mo.info as *mut MobjInfo;
            mo.flags &= !MF_SKULLFLY;
            mo.momx = 0;
            mo.momy = 0;
            mo.momz = 0;
            set_mobj_state(mo as *mut mobj_t, (*info).spawnstate);
        }
        return;
    }

    mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
    mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);

    xmove = mo.momx;
    ymove = mo.momy;

    loop
    {
        if xmove > MAXMOVE / 2 || ymove > MAXMOVE / 2
        {
            ptryx = mo.x + xmove / 2;
            ptryy = mo.y + ymove / 2;
            xmove >>= 1;
            ymove >>= 1;
        }
        else
        {
            ptryx = mo.x + xmove;
            ptryy = mo.y + ymove;
            xmove = 0;
            ymove = 0;
        }

        if P_TryMove(mo as *mut _ as *mut CffiMobj, ptryx, ptryy) == 0
        {
            if !mo.player.is_null()
            {
                P_SlideMove(mo as *mut _ as *mut CffiMobj);
            }
            else if mo.flags & MF_MISSILE != 0
            {
                let cl = ceilingline;
                if !cl.is_null()
                {
                    let line = cl as *mut line_t;
                    if !(*line).backsector.is_null()
                        && (*(*line).backsector).ceilingpic == skyflatnum as i16
                    {
                        remove_mobj(mo as *mut mobj_t);
                        return;
                    }
                }
                explode_missile(mo as *mut mobj_t);
            }
            else
            {
                mo.momx = 0;
                mo.momy = 0;
            }
        }
        if xmove == 0 && ymove == 0
        {
            break;
        }
    }

    if !player.is_null() && (*player).cheats & CF_NOMOMENTUM != 0
    {
        mo.momx = 0;
        mo.momy = 0;
        return;
    }

    if mo.flags & (MF_MISSILE | MF_SKULLFLY) != 0
    {
        return;
    }

    if mo.z > mo.floorz
    {
        return;
    }

    if mo.flags & MF_CORPSE != 0
        && (mo.momx > FRACUNIT / 4
            || mo.momx < -FRACUNIT / 4
            || mo.momy > FRACUNIT / 4
            || mo.momy < -FRACUNIT / 4)
        && mo.floorz != (*(*mo.subsector).sector).floorheight
    {
        return;
    }

    if mo.momx > -STOPSPEED
        && mo.momx < STOPSPEED
        && mo.momy > -STOPSPEED
        && mo.momy < STOPSPEED
        && (player.is_null() || ((*player).cmd.forwardmove == 0 && (*player).cmd.sidemove == 0))
    {
        if !player.is_null()
        {
            let state_ptr = mo.state as *mut State;
            let states_ptr = std::ptr::addr_of!(info::states) as *const State;
            let state_idx = (state_ptr as usize - states_ptr as usize) / std::mem::size_of::<State>();
            let run_offset = state_idx as c_int - S_PLAY_RUN1;
            if (0..4).contains(&run_offset)
            {
                set_mobj_state(mo as *mut mobj_t, S_PLAY);
            }
        }
        mo.momx = 0;
        mo.momy = 0;
    }
    else
    {
        mo.momx = FixedMul(mo.momx, FRICTION);
        mo.momy = FixedMul(mo.momy, FRICTION);
    }
}

/// Apply one tic of Z (vertical) movement to `mo`.
///
/// Handles player view-height smoothing on step-ups, floating-monster altitude
/// tracking, gravity, floor and ceiling collisions, and Lost Soul bounce
/// (with version-correct bug emulation for the original v1.9 desync).
///
/// ## Technical Details
///
/// The Lost Soul floor bounce is emulated twice on purpose: at or
/// above `exe_ultimate` the flip happens when the floor is reached
/// (the corrected order), below it the flip happens after the
/// `momz = 0` clamp (the v1.9 order whose accidental double-negation
/// demos depend on). The float-tracking `dist`/`delta` gates, the
/// gravity init `-GRAVITY * 2` vs the `-GRAVITY` step, and the
/// view-height writes all run in the pinned statement order.
///
/// ## On Calling
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` linked into
/// the active thinker list. May explode the mobj on a ceiling hit
/// (missiles); the caller re-checks via the thinker sentinel.
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` that is part of the
/// active thinker list.
#[doc(alias = "P_ZMovement")]
#[export_name = "P_ZMovement"]
pub unsafe extern "C" fn z_movement(mo: *mut mobj_t)
{
    let mo = &mut *mo;
    let dist: c_int;
    let delta: c_int;

    if !mo.player.is_null() && mo.z < mo.floorz
    {
        let player = mo.player as *mut PlayerT;
        (*player).viewheight -= mo.floorz - mo.z;
        (*player).deltaviewheight = (VIEWHEIGHT - (*player).viewheight) >> 3;
    }

    mo.z += mo.momz;

    if mo.flags & MF_FLOAT != 0
        && !mo.target.is_null()
        && mo.flags & MF_SKULLFLY == 0
        && mo.flags & MF_INFLOAT == 0
    {
        let target = mo.target;
        dist = P_AproxDistance(mo.x - (*target).x, mo.y - (*target).y);
        delta = ((*target).z + (mo.height >> 1)) - mo.z;
        if delta < 0 && dist < -(delta * 3)
        {
            mo.z -= FLOATSPEED;
        }
        else if delta > 0 && dist < delta * 3
        {
            mo.z += FLOATSPEED;
        }
    }

    if mo.z <= mo.floorz
    {
        let correct_lost_soul_bounce = gameversion >= exe_ultimate;
        if correct_lost_soul_bounce && mo.flags & MF_SKULLFLY != 0
        {
            mo.momz = -mo.momz;
        }
        if mo.momz < 0
        {
            if !mo.player.is_null() && mo.momz < -GRAVITY * 8
            {
                let player = mo.player as *mut PlayerT;
                (*player).deltaviewheight = mo.momz >> 3;
                S_StartSound(mo as *mut mobj_t as *mut c_void, Sfx::Oof as c_int);
            }
            mo.momz = 0;
        }
        mo.z = mo.floorz;
        if !correct_lost_soul_bounce && mo.flags & MF_SKULLFLY != 0
        {
            mo.momz = -mo.momz;
        }
        if mo.flags & MF_MISSILE != 0 && mo.flags & MF_NOCLIP == 0
        {
            explode_missile(mo as *mut mobj_t);
            return;
        }
    }
    else if mo.flags & MF_NOGRAVITY == 0
    {
        if mo.momz == 0
        {
            mo.momz = -GRAVITY * 2;
        }
        else
        {
            mo.momz -= GRAVITY;
        }
    }

    if mo.z + mo.height > mo.ceilingz
    {
        if mo.momz > 0
        {
            mo.momz = 0;
        }
        mo.z = mo.ceilingz - mo.height;
        if mo.flags & MF_SKULLFLY != 0
        {
            mo.momz = -mo.momz;
        }
        if mo.flags & MF_MISSILE != 0 && mo.flags & MF_NOCLIP == 0
        {
            explode_missile(mo as *mut mobj_t);
        }
    }
}
