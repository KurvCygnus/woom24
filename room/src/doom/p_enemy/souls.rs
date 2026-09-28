//! Lost Soul and Pain Elemental actions: the `MF_SKULLFLY` charge,
//! the Lost Soul spawn helper (with its 20-skull cap and telefrag
//! fallback), and the Pain Elemental attack/death pair -- bit-exact
//! with the skull/pain actions of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::info::{MF_SKULLFLY, MobjInfo, MT_SKULL};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_map::P_TryMove;
use crate::doom::p_maputl::P_AproxDistance;
use crate::doom::p_mobj::P_SpawnMobj;
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::{thinkercap, thinker_t};
use crate::doom::s_sound::S_StartSound;
use crate::doom::tables::{finecosine, finesine, ANG180, ANG270, ANG90, ANGLETOFINESHIFT};

use super::attacks::action_face_target;
use super::consts::{angle_t, CffiMobj, SKULLSPEED};

/// Lost Soul charge attack: sets `MF_SKULLFLY` and launches the actor as a living missile.
///
/// Sets horizontal momentum from `SKULLSPEED` scaled by the angle's fine-trig values.
/// Computes vertical momentum to reach the midpoint of the target's height over the
/// flight time (`dist / SKULLSPEED` tics, minimum 1).
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target` and valid `info`. Called from C.
#[doc(alias = "A_SkullAttack")]
#[export_name = "A_SkullAttack"]
pub unsafe extern "C" fn action_skull_attack(actor: *mut mobj_t)
{
    let mut dist: c_int;

    if(*actor).target.is_null() { return; }
    let dest: *mut mobj_t = (*actor).target;
    (*actor).flags |= MF_SKULLFLY as c_int;
    S_StartSound(
        actor as *mut c_void,
        (*((*actor).info as *mut MobjInfo)).attacksound as c_int,
    );
    action_face_target(actor);
    let an: angle_t = (*actor).angle >> ANGLETOFINESHIFT;
    (*actor).momx = FixedMul(SKULLSPEED, *finecosine.0.add(an as usize));
    (*actor).momy = FixedMul(SKULLSPEED, finesine[an as usize]);
    dist = P_AproxDistance((*dest).x - (*actor).x, (*dest).y - (*actor).y) as c_int;
    dist /= SKULLSPEED;
    if dist < 1 as c_int { dist = 1 as c_int; }
    (*actor).momz = (((*dest).z as c_int + ((*dest).height as c_int >> 1 as c_int) - (*actor).z as c_int) / dist) as fixed_t;
}

/// Spawns a Lost Soul (`MT_SKULL`) at `angle` from `actor` and launches it at the target.
///
/// Counts all live `MT_SKULL` thinkers; if more than 20 already exist, does nothing.
/// Computes the spawn point `prestep` units ahead (4 + 1.5 * combined radii) to avoid
/// spawning inside the Pain Elemental. If the skull cannot move at its spawn point
/// (`P_TryMove` fails), it is instantly killed with 10000 damage instead. On success,
/// the skull inherits `actor->target` and immediately calls `A_SkullAttack`.
///
/// # Safety
///
/// `actor` must be non-null with valid `info` and a valid or null `target`. Called from C.
#[doc(alias = "A_PainShootSkull")]
#[export_name = "A_PainShootSkull"]
pub unsafe extern "C" fn action_pain_shoot_skull(actor: *mut mobj_t, angle: angle_t)
{
    let mut count: c_int;

    let mut currentthinker: *mut thinker_t;

    count = 0 as c_int;
    currentthinker = thinkercap.next;
    while !std::ptr::eq(currentthinker, &raw const thinkercap)
    {
        if super::map_events::is_mobj_thinker(currentthinker) &&
            (*(currentthinker as *mut mobj_t)).mobjtype as c_uint == MT_SKULL as c_int as c_uint { count += 1; }
        currentthinker = (*currentthinker).next;
    }
    if count > 20 as c_int { return; }
    let an: angle_t = angle >> ANGLETOFINESHIFT;
    let prestep: c_int = 4 as c_int * FRACUNIT +
        3 as c_int * (
            (*((*actor).info as *mut MobjInfo)).radius +
            crate::doom::info::mobjinfo[MT_SKULL as c_int as usize].radius
        ) / 2 as c_int;
    let x: fixed_t = (*actor).x + FixedMul(prestep as fixed_t, *finecosine.0.add(an as usize));
    let y: fixed_t = (*actor).y + FixedMul(prestep as fixed_t, finesine[an as usize]);
    let z: fixed_t = ((*actor).z as c_int + 8 as c_int * FRACUNIT) as fixed_t;
    let newmobj: *mut mobj_t = P_SpawnMobj(x, y, z, MT_SKULL);
    if P_TryMove(newmobj as *mut CffiMobj, (*newmobj).x, (*newmobj).y) == 0
    {
        P_DamageMobj(newmobj, actor, actor, 10000 as c_int);
        return;
    }
    (*newmobj).target = (*actor).target;
    action_skull_attack(newmobj);
}

/// Pain Elemental attack: spawns a Lost Soul in the direction the actor is facing.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_PainAttack")]
#[export_name = "A_PainAttack"]
pub unsafe extern "C" fn action_pain_attack(actor: *mut mobj_t)
{
    if(*actor).target.is_null() { return; }
    action_face_target(actor);
    action_pain_shoot_skull(actor, (*actor).angle);
}

/// Pain Elemental death: drops to the ground and spits three Lost Souls in cardinal directions.
///
/// Calls `A_Fall` to clear `MF_SOLID`, then spawns skulls at `angle+90`, `angle+180`,
/// and `angle+270` relative to the current facing direction.
///
/// # Safety
///
/// `actor` must be non-null. Called from C.
#[doc(alias = "A_PainDie")]
#[export_name = "A_PainDie"]
pub unsafe extern "C" fn action_pain_die(actor: *mut mobj_t)
{
    super::death::action_fall(actor);
    action_pain_shoot_skull(actor, (*actor).angle.wrapping_add(ANG90 as angle_t));
    action_pain_shoot_skull(actor, (*actor).angle.wrapping_add(ANG180));
    action_pain_shoot_skull(actor, (*actor).angle.wrapping_add(ANG270));
}
