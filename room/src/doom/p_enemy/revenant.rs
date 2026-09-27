//! Revenant attack actions: the homing `MT_TRACER` missile launch and
//! its per-tic steering, plus the melee whoosh/fist pair -- bit-exact
//! with the Revenant actions of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::d_loop::gametic;
use crate::doom::info::{MobjInfo, MT_SMOKE, MT_TRACER};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_maputl::P_AproxDistance;
use crate::doom::p_mobj::{P_SpawnMissile, P_SpawnMobj, P_SpawnPuff};
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};

use super::chase::check_melee_range;
use super::consts::angle_t;

/// Maximum angular correction applied per active tic by `A_Tracer` (approx 11.25 degrees in BAM).
/// Has C linkage (`#[no_mangle]`); referenced from `p_enemy.c` (C test harness).
#[no_mangle]
pub static mut TRACEANGLE: c_int = 0xc000000 as c_int;

/// Attack action for the Revenant: launches a homing `MT_TRACER` missile.
///
/// Temporarily raises the actor's Z by 16 units so the missile spawns at shoulder
/// height. After spawning, advances the missile one tic forward and stores the
/// current target in `tracer` so `A_Tracer` can home in.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_SkelMissile")]
#[export_name = "A_SkelMissile"]
pub unsafe extern "C" fn action_skel_missile(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    super::attacks::action_face_target(actor);
    (*actor).z += 16 as c_int * FRACUNIT;
    let mo: *mut mobj_t = P_SpawnMissile(actor, (*actor).target, MT_TRACER);
    (*actor).z -= 16 as c_int * FRACUNIT;
    (*mo).x += (*mo).momx;
    (*mo).y += (*mo).momy;
    (*mo).tracer = (*actor).target;
}

/// Per-tic homing update for the Revenant's tracer missile.
///
/// Only executes on tics where `gametic & 3 == 0` (every fourth tic). Each active tic:
/// 1. Spawns a smoke puff at the current position and a `MT_SMOKE` particle behind it.
/// 2. Steers the missile angle toward `tracer` by at most `TRACEANGLE` per tic, snapping
///    exactly when the correction would overshoot.
/// 3. Recomputes `momx`/`momy` from the new angle and the missile's `info->speed`.
/// 4. Adjusts `momz` by ±`FRACUNIT/8` to converge on `tracer->z + 40` units.
///
/// Returns immediately if `tracer` is null or dead.
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t` with valid `info`. If `actor->tracer` is
/// non-null it must point to a valid `mobj_t`. Called from C.
#[doc(alias = "A_Tracer")]
#[export_name = "A_Tracer"]
pub unsafe extern "C" fn action_tracer(actor: *mut mobj_t)
{
    let mut exact: angle_t;

    let mut dist: fixed_t;

    if gametic & 3 as c_int != 0
    {
        return;
    }
    P_SpawnPuff((*actor).x, (*actor).y, (*actor).z);
    let th: *mut mobj_t = P_SpawnMobj(
        (*actor).x - (*actor).momx,
        (*actor).y - (*actor).momy,
        (*actor).z,
        MT_SMOKE,
    );
    (*th).momz = FRACUNIT as fixed_t;
    (*th).tics -= P_Random() & 3 as c_int;
    if (*th).tics < 1 as c_int
    {
        (*th).tics = 1 as c_int;
    }
    let dest: *mut mobj_t = (*actor).tracer;
    if dest.is_null() || (*dest).health <= 0 as c_int
    {
        return;
    }
    exact = R_PointToAngle2((*actor).x, (*actor).y, (*dest).x, (*dest).y);
    if exact != (*actor).angle
    {
        if exact.wrapping_sub((*actor).angle) > 0x80000000 as c_uint
        {
            (*actor).angle = (*actor).angle.wrapping_sub(TRACEANGLE as angle_t);
            if exact.wrapping_sub((*actor).angle) < 0x80000000 as c_uint
            {
                (*actor).angle = exact;
            }
        }
        else
        {
            (*actor).angle = (*actor).angle.wrapping_add(TRACEANGLE as angle_t);
            if exact.wrapping_sub((*actor).angle) > 0x80000000 as c_uint
            {
                (*actor).angle = exact;
            }
        }
    }
    exact = (*actor).angle >> ANGLETOFINESHIFT;
    (*actor).momx = FixedMul(
        (*((*actor).info as *mut MobjInfo)).speed as fixed_t,
        *finecosine.0.add(exact as usize),
    );
    (*actor).momy = FixedMul(
        (*((*actor).info as *mut MobjInfo)).speed as fixed_t,
        finesine[exact as usize],
    );
    dist = P_AproxDistance((*dest).x - (*actor).x, (*dest).y - (*actor).y);
    dist = (dist as c_int / (*((*actor).info as *mut MobjInfo)).speed) as fixed_t;
    if dist < 1 as c_int
    {
        dist = 1 as c_int as fixed_t;
    }
    let slope: fixed_t = ((*dest).z + 40 as fixed_t * FRACUNIT - (*actor).z) / dist;
    if slope < (*actor).momz
    {
        (*actor).momz -= FRACUNIT / 8 as c_int;
    }
    else
    {
        (*actor).momz += FRACUNIT / 8 as c_int;
    };
}

/// Revenant melee wind-up: faces the target and plays the whoosh sound.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_SkelWhoosh")]
#[export_name = "A_SkelWhoosh"]
pub unsafe extern "C" fn action_skel_whoosh(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    super::attacks::action_face_target(actor);
    S_StartSound(actor as *mut c_void, Sfx::Skeswg as c_int);
}

/// Revenant melee strike: deals `(rnd%10+1)*6` damage (6-60) when in melee range.
///
/// Plays `sfx_skepch` on a successful hit.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_SkelFist")]
#[export_name = "A_SkelFist"]
pub unsafe extern "C" fn action_skel_fist(actor: *mut mobj_t)
{
    let damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    super::attacks::action_face_target(actor);
    if check_melee_range(actor).is_truthy()
    {
        damage = (P_Random() % 10 as c_int + 1 as c_int) * 6 as c_int;
        S_StartSound(actor as *mut c_void, Sfx::Skepch as c_int);
        P_DamageMobj((*actor).target, actor, actor, damage);
    }
}
