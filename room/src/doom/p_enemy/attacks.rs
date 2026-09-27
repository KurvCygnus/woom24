//! Monster attack actions: target facing (with the shadow-invisibility
//! aim jitter) and the hitscan / projectile attacks of the former
//! human, shotgun guy, chaingunner, spider mastermind, arachnotron,
//! imp, demon, cacodemon, cyberdemon, and baron -- bit-exact with the
//! attack half of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::info::{
    MF_AMBUSH, MF_SHADOW, MobjInfo, MT_ARACHPLAZ, MT_BRUISERSHOT, MT_HEADSHOT, MT_ROCKET,
    MT_TROOPSHOT,
};
use crate::doom::m_fixed::fixed_t;
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_map::{P_AimLineAttack, P_LineAttack};
use crate::doom::p_mobj::{P_SpawnMissile, P_SetMobjState};
use crate::doom::p_sight::P_CheckSight;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::chase::check_melee_range;
use super::consts::{statenum_t, MISSILERANGE};

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = super::consts::CffiMobj;

/// Turns `actor` to face its `target` and clears the `MF_AMBUSH` flag.
///
/// If the target has `MF_SHADOW` (partial invisibility), the facing angle is
/// perturbed by a random ±21-bit BAM value to simulate aim confusion.
/// Returns immediately if `actor->target` is null.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_FaceTarget")]
#[export_name = "A_FaceTarget"]
pub unsafe extern "C" fn action_face_target(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    (*actor).flags &= !(MF_AMBUSH as c_int);
    (*actor).angle = R_PointToAngle2(
        (*actor).x,
        (*actor).y,
        (*(*actor).target).x,
        (*(*actor).target).y,
    );
    if (*(*actor).target).flags & MF_SHADOW as c_int != 0
    {
        (*actor).angle = (*actor)
            .angle
            .wrapping_add(((P_Random() - P_Random()) << 21 as c_int) as super::consts::angle_t);
    }
}

/// Attack action for the Former Human (Zombieman): single hitscan shot.
///
/// Faces the target, aims with `P_AimLineAttack`, plays `sfx_pistol`, then fires one
/// hitscan ray with horizontal spread of ±20 bits and damage of `(rnd%5+1)*3` (3-15).
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_PosAttack")]
#[export_name = "A_PosAttack"]
pub unsafe extern "C" fn action_pos_attack(actor: *mut mobj_t)
{
    let mut angle: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    angle = (*actor).angle as c_int;
    let slope: c_int =
        P_AimLineAttack(actor as *mut CffiMobj, angle as super::consts::angle_t, MISSILERANGE)
            as c_int;
    S_StartSound(actor as *mut c_void, Sfx::Pistol as c_int);
    angle = angle.wrapping_add((P_Random() - P_Random()) << 20 as c_int);
    let damage: c_int = (P_Random() % 5 as c_int + 1 as c_int) * 3 as c_int;
    P_LineAttack(
        actor as *mut CffiMobj,
        angle as super::consts::angle_t,
        MISSILERANGE,
        slope as fixed_t,
        damage,
    );
}

/// Attack action for the Shotgun Guy (Sergeant): three-pellet spread shot.
///
/// Plays `sfx_shotgn`, faces the target, aims once, then fires three independent
/// hitscan rays each with ±20-bit spread and `(rnd%5+1)*3` damage.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_SPosAttack")]
#[export_name = "A_SPosAttack"]
pub unsafe extern "C" fn action_spos_attack(actor: *mut mobj_t)
{
    let mut i: c_int;

    let mut angle: c_int;

    let mut damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    S_StartSound(actor as *mut c_void, Sfx::Shotgn as c_int);
    action_face_target(actor);
    let bangle: c_int = (*actor).angle as c_int;
    let slope: c_int =
        P_AimLineAttack(actor as *mut CffiMobj, bangle as super::consts::angle_t, MISSILERANGE)
            as c_int;
    i = 0 as c_int;
    while i < 3 as c_int
    {
        angle = bangle.wrapping_add((P_Random() - P_Random()) << 20 as c_int);
        damage = (P_Random() % 5 as c_int + 1 as c_int) * 3 as c_int;
        P_LineAttack(
            actor as *mut CffiMobj,
            angle as super::consts::angle_t,
            MISSILERANGE,
            slope as fixed_t,
            damage,
        );
        i += 1;
    }
}

/// Attack action for the Heavy Weapon Dude (Chaingunner): single-pellet burst fire.
///
/// Plays `sfx_shotgn`, faces the target, aims once, then fires one hitscan ray with
/// ±20-bit spread and `(rnd%5+1)*3` damage. The state machine calls this repeatedly
/// each tic to simulate chaingun fire; `A_CPosRefire` decides when to stop.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_CPosAttack")]
#[export_name = "A_CPosAttack"]
pub unsafe extern "C" fn action_cpos_attack(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    S_StartSound(actor as *mut c_void, Sfx::Shotgn as c_int);
    action_face_target(actor);
    let bangle: c_int = (*actor).angle as c_int;
    let slope: c_int =
        P_AimLineAttack(actor as *mut CffiMobj, bangle as super::consts::angle_t, MISSILERANGE)
            as c_int;
    let angle: c_int = bangle.wrapping_add((P_Random() - P_Random()) << 20 as c_int);
    let damage: c_int = (P_Random() % 5 as c_int + 1 as c_int) * 3 as c_int;
    P_LineAttack(
        actor as *mut CffiMobj,
        angle as super::consts::angle_t,
        MISSILERANGE,
        slope as fixed_t,
        damage,
    );
}

/// Refire check for the Chaingunner: keeps firing unless the target is gone or hidden.
///
/// Faces the target. With a 40/256 chance returns early (keeps firing regardless).
/// Otherwise, if the target is null, dead, or out of sight, transitions back to
/// `seestate` to stop the burst.
///
/// # Safety
///
/// `actor` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_CPosRefire")]
#[export_name = "A_CPosRefire"]
pub unsafe extern "C" fn action_cpos_refire(actor: *mut mobj_t)
{
    action_face_target(actor);
    if P_Random() < 40 as c_int
    {
        return;
    }
    if (*actor).target.is_null()
        || (*(*actor).target).health <= 0 as c_int
        || P_CheckSight(actor, (*actor).target) == 0
    {
        P_SetMobjState(
            actor,
            (*((*actor).info as *mut MobjInfo)).seestate as statenum_t,
        );
    }
}

/// Refire check for the Spider Mastermind: keeps firing unless the target is gone or hidden.
///
/// Same logic as `A_CPosRefire` but with a lower 10/256 early-return chance, making the
/// Spider Mastermind more persistent.
///
/// # Safety
///
/// `actor` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_SpidRefire")]
#[export_name = "A_SpidRefire"]
pub unsafe extern "C" fn action_spid_refire(actor: *mut mobj_t)
{
    action_face_target(actor);
    if P_Random() < 10 as c_int
    {
        return;
    }
    if (*actor).target.is_null()
        || (*(*actor).target).health <= 0 as c_int
        || P_CheckSight(actor, (*actor).target) == 0
    {
        P_SetMobjState(
            actor,
            (*((*actor).info as *mut MobjInfo)).seestate as statenum_t,
        );
    }
}

/// Attack action for the Arachnotron: launches one `MT_ARACHPLAZ` plasma ball.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_BspiAttack")]
#[export_name = "A_BspiAttack"]
pub unsafe extern "C" fn action_bspi_attack(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    P_SpawnMissile(actor, (*actor).target, MT_ARACHPLAZ);
}

/// Attack action for the Imp: claw swipe in melee range, fireball at distance.
///
/// If in melee range plays `sfx_claw` and deals `(rnd%8+1)*3` damage (3-24).
/// Otherwise launches a `MT_TROOPSHOT` fireball.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_TroopAttack")]
#[export_name = "A_TroopAttack"]
pub unsafe extern "C" fn action_troop_attack(actor: *mut mobj_t)
{
    let damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    if check_melee_range(actor).is_truthy()
    {
        S_StartSound(actor as *mut c_void, Sfx::Claw as c_int);
        damage = (P_Random() % 8 as c_int + 1 as c_int) * 3 as c_int;
        P_DamageMobj((*actor).target, actor, actor, damage);
        return;
    }
    P_SpawnMissile(actor, (*actor).target, MT_TROOPSHOT);
}

/// Attack action for the Demon (Sarg): melee-only bite dealing `(rnd%10+1)*4` damage (4-40).
///
/// Only damages the target when within melee range; no ranged fallback.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_SargAttack")]
#[export_name = "A_SargAttack"]
pub unsafe extern "C" fn action_sarg_attack(actor: *mut mobj_t)
{
    let damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    if check_melee_range(actor).is_truthy()
    {
        damage = (P_Random() % 10 as c_int + 1 as c_int) * 4 as c_int;
        P_DamageMobj((*actor).target, actor, actor, damage);
    }
}

/// Attack action for the Cacodemon: bite in melee range, fireball at distance.
///
/// Melee deals `(rnd%6+1)*10` damage (10-60). Ranged fires `MT_HEADSHOT`.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_HeadAttack")]
#[export_name = "A_HeadAttack"]
pub unsafe extern "C" fn action_head_attack(actor: *mut mobj_t)
{
    let damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    if check_melee_range(actor).is_truthy()
    {
        damage = (P_Random() % 6 as c_int + 1 as c_int) * 10 as c_int;
        P_DamageMobj((*actor).target, actor, actor, damage);
        return;
    }
    P_SpawnMissile(actor, (*actor).target, MT_HEADSHOT);
}

/// Attack action for the Cyberdemon: launches one `MT_ROCKET`.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_CyberAttack")]
#[export_name = "A_CyberAttack"]
pub unsafe extern "C" fn action_cyber_attack(actor: *mut mobj_t)
{
    if (*actor).target.is_null()
    {
        return;
    }
    action_face_target(actor);
    P_SpawnMissile(actor, (*actor).target, MT_ROCKET);
}

/// Attack action for the Baron of Hell / Hell Knight: claw in melee, plasma ball at distance.
///
/// Melee plays `sfx_claw` and deals `(rnd%8+1)*10` damage (10-80). Ranged fires
/// `MT_BRUISERSHOT`.
///
/// # Safety
///
/// `actor` must be non-null with a valid or null `target`. Called from C.
#[doc(alias = "A_BruisAttack")]
#[export_name = "A_BruisAttack"]
pub unsafe extern "C" fn action_bruis_attack(actor: *mut mobj_t)
{
    let damage: c_int;

    if (*actor).target.is_null()
    {
        return;
    }
    if check_melee_range(actor).is_truthy()
    {
        S_StartSound(actor as *mut c_void, Sfx::Claw as c_int);
        damage = (P_Random() % 8 as c_int + 1 as c_int) * 10 as c_int;
        P_DamageMobj((*actor).target, actor, actor, damage);
        return;
    }
    P_SpawnMissile(actor, (*actor).target, MT_BRUISERSHOT);
}
