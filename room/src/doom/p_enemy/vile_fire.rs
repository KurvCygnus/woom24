//! Archvile fire-pillar actions: the attack startup, the flame
//! start/crackle sustain, the target-locking spawn (site of the
//! vanilla fire-coordinate bug, catalog entry 8), and the finishing
//! payload -- bit-exact with the vile-fire actions of
//! `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::info::{MobjInfo, MT_FIRE};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::p_map::P_RadiusAttack;
use crate::doom::p_maputl::{P_SetThingPosition, P_UnsetThingPosition};
use crate::doom::p_mobj::{P_SpawnMobj, P_SubstNullMobj};
use crate::doom::p_sight::P_CheckSight;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};

use super::attacks::action_face_target;

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = super::consts::CffiMobj;

/// Archvile attack wind-up: plays the attack start sound (`sfx_vilatk`).
///
/// # Safety
///
/// `actor` must be non-null. Called from C.
#[doc(alias = "A_VileStart")]
#[export_name = "A_VileStart"]
pub unsafe extern "C" fn action_vile_start(actor: *mut mobj_t)
{
    S_StartSound(actor as *mut c_void, Sfx::Vilatk as c_int);
}

/// Archvile fire start: plays the flame start sound then positions the fire object.
///
/// # Safety
///
/// `actor` must be non-null with valid `tracer` and `target` fields (or null). Called from C.
#[doc(alias = "A_StartFire")]
#[export_name = "A_StartFire"]
pub unsafe extern "C" fn action_start_fire(actor: *mut mobj_t)
{
    S_StartSound(actor as *mut c_void, Sfx::Flamst as c_int);
    action_fire(actor);
}

/// Archvile fire crackle: plays the sustained flame sound then repositions the fire object.
///
/// # Safety
///
/// `actor` must be non-null with valid `tracer` and `target` fields (or null). Called from C.
#[doc(alias = "A_FireCrackle")]
#[export_name = "A_FireCrackle"]
pub unsafe extern "C" fn action_fire_crackle(actor: *mut mobj_t)
{
    S_StartSound(actor as *mut c_void, Sfx::Flame as c_int);
    action_fire(actor);
}

/// Repositions the Archvile fire object 24 units in front of its tracer (the victim).
///
/// Does nothing if `actor->tracer` is null. Checks sight from the Archvile's `target`
/// to the tracer; if the vile lost sight of its victim, the fire does not move.
/// Uses `P_UnsetThingPosition`/`P_SetThingPosition` to maintain blockmap consistency.
///
/// # Safety
///
/// `actor` must be non-null. `actor->tracer` if non-null must be a valid `mobj_t`.
/// `actor->target` if non-null must be a valid `mobj_t`. Called from C.
#[doc(alias = "A_Fire")]
#[export_name = "A_Fire"]
pub unsafe extern "C" fn action_fire(actor: *mut mobj_t)
{
    let dest: *mut mobj_t = (*actor).tracer;
    if dest.is_null()
    {
        return;
    }
    let target: *mut mobj_t = P_SubstNullMobj((*actor).target);
    if P_CheckSight(target, dest) == 0
    {
        return;
    }
    let an: c_uint = ((*dest).angle >> ANGLETOFINESHIFT) as c_uint;
    P_UnsetThingPosition(actor as *mut CffiMobj);
    (*actor).x = (*dest).x + FixedMul(24 as fixed_t * FRACUNIT, *finecosine.0.add(an as usize));
    (*actor).y = (*dest).y + FixedMul(24 as fixed_t * FRACUNIT, finesine[an as usize]);
    (*actor).z = (*dest).z;
    P_SetThingPosition(actor as *mut CffiMobj);
}

/// Archvile attack setup: spawns the `MT_FIRE` hellfire object at the target's location.
///
/// Links the fire into the Archvile/target triangle: `actor->tracer = fire`,
/// `fire->target = actor`, `fire->tracer = actor->target`, then calls `A_Fire` to
/// position it immediately.
///
/// Note: the C source passes `actor->target->x` for both X and Y of `P_SpawnMobj`,
/// which is a vanilla Doom bug (Y should be `actor->target->y`). This port
/// faithfully reproduces the original behavior.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_VileTarget")]
#[export_name = "A_VileTarget"]
pub unsafe extern "C" fn action_vile_target(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    // FIXME: p_enemy.c passes `actor->target->x` for the Y argument instead of
    // `actor->target->y` - this is a vanilla Doom bug reproduced faithfully here.
    let fog: *mut mobj_t = P_SpawnMobj(
        (*(*actor).target).x,
        (*(*actor).target).x,
        (*(*actor).target).z,
        MT_FIRE,
    );
    (*actor).tracer = fog;
    (*fog).target = actor;
    (*fog).tracer = (*actor).target;
    action_fire(fog);
}

/// Archvile attack payload: direct damage plus radius explosion via the fire object.
///
/// Faces the target; aborts if line of sight is lost. Deals 20 direct damage and
/// applies an upward momentum impulse of `1000*FRACUNIT / target->mass` to the target.
/// Repositions the fire object 24 units behind the target (opposite the Archvile's angle)
/// and calls `P_RadiusAttack` with radius 70 to inflict blast damage in the area.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target` and `tracer`. Called from C.
#[doc(alias = "A_VileAttack")]
#[export_name = "A_VileAttack"]
pub unsafe extern "C" fn action_vile_attack(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    if P_CheckSight(actor, (*actor).target) == 0
    {
        return;
    }
    S_StartSound(actor as *mut c_void, Sfx::Barexp as c_int);
    crate::doom::p_inter::P_DamageMobj((*actor).target, actor, actor, 20 as c_int);
    (*(*actor).target).momz =
        (1000 as c_int * FRACUNIT / (*((*(*actor).target).info as *mut MobjInfo)).mass) as fixed_t;
    let an: c_int = ((*actor).angle >> ANGLETOFINESHIFT) as c_int;
    let fire: *mut mobj_t = (*actor).tracer;
    if fire.is_null()
    {
        return;
    }
    (*fire).x =
        (*(*actor).target).x - FixedMul(24 as fixed_t * FRACUNIT, *finecosine.0.add(an as usize));
    (*fire).y = (*(*actor).target).y - FixedMul(24 as fixed_t * FRACUNIT, finesine[an as usize]);
    P_RadiusAttack(fire as *mut CffiMobj, actor as *mut CffiMobj, 70 as c_int);
}
