//! Mancubus attack actions: the raise/taunt and the three two-shot
//! `MT_FATSHOT` spread phases -- bit-exact with the Mancubus actions
//! of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::info::{MobjInfo, MT_FATSHOT};
use crate::doom::m_fixed::{fixed_t, FixedMul};
use crate::doom::p_mobj::{P_SpawnMissile, P_SubstNullMobj};
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};

use super::attacks::action_face_target;
use super::consts::{angle_t, FATSPREAD};

/// Mancubus raise/taunt: faces the target and plays the attack preparation sound.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_FatRaise")]
#[export_name = "A_FatRaise"]
pub unsafe extern "C" fn action_fat_raise(actor: *mut mobj_t)
{
    action_face_target(actor);
    S_StartSound(actor as *mut c_void, Sfx::Manatk as c_int);
}

/// Mancubus attack phase 1: two `MT_FATSHOT` fireballs spread to the right.
///
/// Rotates the actor's angle by `+FATSPREAD` (ANG90/8), fires one shot directly,
/// then fires a second shot additionally rotated by `+FATSPREAD` with velocity
/// recomputed from the new angle.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_FatAttack1")]
#[export_name = "A_FatAttack1"]
pub unsafe extern "C" fn action_fat_attack1(actor: *mut mobj_t)
{
    action_face_target(actor);
    (*actor).angle = (*actor).angle.wrapping_add(FATSPREAD as angle_t);
    let target: *mut mobj_t = P_SubstNullMobj((*actor).target);
    P_SpawnMissile(actor, target, MT_FATSHOT);
    let mo: *mut mobj_t = P_SpawnMissile(actor, target, MT_FATSHOT);
    (*mo).angle = (*mo).angle.wrapping_add(FATSPREAD as angle_t);
    let an: c_int = ((*mo).angle >> ANGLETOFINESHIFT) as c_int;
    (*mo).momx = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        *finecosine.0.add(an as usize),
    );
    (*mo).momy = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        finesine[an as usize],
    );
}

/// Mancubus attack phase 2: two `MT_FATSHOT` fireballs spread to the left.
///
/// Rotates the actor's angle by `-FATSPREAD`, fires one shot directly, then fires a
/// second shot additionally rotated by `-2*FATSPREAD` with velocity recomputed.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_FatAttack2")]
#[export_name = "A_FatAttack2"]
pub unsafe extern "C" fn action_fat_attack2(actor: *mut mobj_t)
{
    action_face_target(actor);
    (*actor).angle = (*actor).angle.wrapping_sub(FATSPREAD as angle_t);
    let target: *mut mobj_t = P_SubstNullMobj((*actor).target);
    P_SpawnMissile(actor, target, MT_FATSHOT);
    let mo: *mut mobj_t = P_SpawnMissile(actor, target, MT_FATSHOT);
    (*mo).angle = (*mo)
        .angle
        .wrapping_sub((FATSPREAD * 2 as c_int) as angle_t);
    let an: c_int = ((*mo).angle >> ANGLETOFINESHIFT) as c_int;
    (*mo).momx = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        *finecosine.0.add(an as usize),
    );
    (*mo).momy = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        finesine[an as usize],
    );
}

/// Mancubus attack phase 3: two `MT_FATSHOT` fireballs spread symmetrically ±FATSPREAD/2.
///
/// Fires one shot rotated `-FATSPREAD/2` and a second at `+FATSPREAD/2`; velocities are
/// recomputed for each so they actually travel in the spread directions.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_FatAttack3")]
#[export_name = "A_FatAttack3"]
pub unsafe extern "C" fn action_fat_attack3(actor: *mut mobj_t)
{
    let mut mo: *mut mobj_t;

    let mut an: c_int;

    action_face_target(actor);
    let target: *mut mobj_t = P_SubstNullMobj((*actor).target);
    mo = P_SpawnMissile(actor, target, MT_FATSHOT);
    (*mo).angle = (*mo)
        .angle
        .wrapping_sub((FATSPREAD / 2 as c_int) as angle_t);
    an = ((*mo).angle >> ANGLETOFINESHIFT) as c_int;
    (*mo).momx = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        *finecosine.0.add(an as usize),
    );
    (*mo).momy = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        finesine[an as usize],
    );
    mo = P_SpawnMissile(actor, target, MT_FATSHOT);
    (*mo).angle = (*mo)
        .angle
        .wrapping_add((FATSPREAD / 2 as c_int) as angle_t);
    an = ((*mo).angle >> ANGLETOFINESHIFT) as c_int;
    (*mo).momx = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        *finecosine.0.add(an as usize),
    );
    (*mo).momy = FixedMul(
        (*((*mo).info as *mut MobjInfo)).speed as fixed_t,
        finesine[an as usize],
    );
}
