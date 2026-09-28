//! Archvile resurrection: the blockmap corpse probe (`PIT_VileCheck`
//! callback) and the `A_VileChase` search-and-raise sequence --
//! bit-exact with the vile-chase half of
//! `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::c_ffi::MAPBLOCKSHIFT;
use crate::doom::info::{mobjinfo, MF_CORPSE, MobjInfo, MT_VILE, S_NULL, S_VILE_HEAL1};
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_map::P_CheckPosition;
use crate::doom::p_maputl::P_BlockThingsIterator;
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_setup::{bmaporgx, bmaporgy};
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::types::Boolean;

use super::chase::{action_chase, xspeed, yspeed};
use super::consts::{statenum_t, DI_NODIR};

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = super::consts::CffiMobj;

/// Returns the absolute value of `x`; local substitute for `stdlib.h` `abs` used in `p_enemy.c`.
#[inline]
fn abs(x: c_int) -> c_int
{
    if x < 0 { -x }
    else { x }
}

/// The corpse most recently selected by `PIT_VileCheck` for resurrection; written by
/// `PIT_VileCheck`, read and mutated by `A_VileChase`. Has C linkage.
#[no_mangle]
pub static mut corpsehit: *mut mobj_t = std::ptr::null_mut::<mobj_t>();

/// The Archvile actor currently searching for a corpse to raise; set by `A_VileChase`
/// before calling `P_BlockThingsIterator`. Has C linkage.
#[no_mangle]
pub static mut vileobj: *mut mobj_t = std::ptr::null_mut::<mobj_t>();

/// X coordinate of the position the Archvile is moving toward, used as the centre of the
/// corpse search radius in `PIT_VileCheck`. Has C linkage.
#[no_mangle]
pub static mut viletryx: fixed_t = 0;

/// Y coordinate of the position the Archvile is moving toward. Has C linkage.
#[no_mangle]
pub static mut viletryy: fixed_t = 0;

/// Blockmap iterator callback: tests whether `thing` is a raiseable corpse near the Archvile.
///
/// Returns `TRUE` (keep iterating) unless `thing` is a fully-settled corpse (`MF_CORPSE`,
/// `tics == -1`) with a valid `raisestate`, within `thing->radius + MT_VILE->radius` of
/// (`viletryx`, `viletryy`), and with enough headroom to stand at full height. If all
/// conditions are met, stores `thing` in `corpsehit` and returns `FALSE` to stop iteration.
///
/// # Safety
///
/// `thing` must be a non-null, valid `mobj_t`. `viletryx`/`viletryy` and `vileobj` must
/// have been set by `A_VileChase`. Called from C via `P_BlockThingsIterator`.
#[doc(alias = "PIT_VileCheck")]
#[export_name = "PIT_VileCheck"]
pub unsafe extern "C" fn pit_vile_check(thing: *mut mobj_t) -> Boolean
{
    if(*thing).flags & MF_CORPSE as c_int == 0 { return Boolean::TRUE; }
    if(*thing).tics != -1 as c_int { return Boolean::TRUE; }
    if(*((*thing).info as *mut MobjInfo)).raisestate == S_NULL as c_int { return Boolean::TRUE; }
    let maxdist: c_int = (*((*thing).info as *mut MobjInfo)).radius + mobjinfo[MT_VILE as c_int as usize].radius;
    
    if((*thing).x as c_int - viletryx as c_int).abs() > maxdist || ((*thing).y as c_int - viletryy as c_int).abs() > maxdist { return Boolean::TRUE; }
    corpsehit = thing;
    (*corpsehit).momy = 0 as c_int as fixed_t;
    (*corpsehit).momx = (*corpsehit).momy;
    (*corpsehit).height <<= 2 as c_int;
    let check: Boolean = Boolean::from_raw(
        P_CheckPosition(
            corpsehit as *mut CffiMobj,
            (*corpsehit).x,
            (*corpsehit).y,
        )
    );
    (*corpsehit).height >>= 2 as c_int;
    if check.is_false() { return Boolean::TRUE; }
    Boolean::FALSE
}

/// Chase action for the Archvile: hunts for corpses to resurrect while pursuing the player.
///
/// If the actor has a movement direction, computes a look-ahead position and searches the
/// surrounding blockmap with `PIT_VileCheck`. When a raiseable corpse is found, the Archvile
/// faces it, enters `S_VILE_HEAL1`, plays the resurrection sound, restores the corpse's
/// flags/health/height, and clears its `target`. Then falls through to `A_Chase`.
///
/// # Safety
///
/// `actor` must be non-null with valid `info` and `movedir`. Called from C.
#[doc(alias = "A_VileChase")]
#[export_name = "A_VileChase"]
pub unsafe extern "C" fn action_vile_chase(actor: *mut mobj_t)
{
    let xl: c_int;

    let xh: c_int;

    let yl: c_int;

    let yh: c_int;

    let mut bx: c_int;

    let mut by: c_int;

    let info: *mut MobjInfo;

    let temp: *mut mobj_t;

    if(*actor).movedir != DI_NODIR as c_int
    {
        viletryx = (*actor).x + (*((*actor).info as *mut MobjInfo)).speed as fixed_t             * xspeed[(*actor).movedir as usize];
        viletryy = (*actor).y + (*((*actor).info as *mut MobjInfo)).speed as fixed_t * yspeed[(*actor).movedir as usize];
        xl = (viletryx as c_int - bmaporgx as c_int - 32 as c_int * FRACUNIT * 2 as c_int) >> MAPBLOCKSHIFT;
        xh = (viletryx as c_int - bmaporgx as c_int + 32 as c_int * FRACUNIT * 2 as c_int) >> MAPBLOCKSHIFT;
        yl = (viletryy as c_int - bmaporgy as c_int - 32 as c_int * FRACUNIT * 2 as c_int) >> MAPBLOCKSHIFT;
        yh = (viletryy as c_int - bmaporgy as c_int + 32 as c_int * FRACUNIT * 2 as c_int) >> MAPBLOCKSHIFT;
        vileobj = actor;
        bx = xl;
        while bx <= xh
        {
            by = yl;
            while by <= yh
            {
                if P_BlockThingsIterator(
                    bx,
                    by,
                    Some(
                        core::mem::transmute::<
                            unsafe extern "C" fn(*mut mobj_t) -> Boolean,
                            unsafe extern "C" fn(*mut CffiMobj) -> c_uint,
                        >(pit_vile_check)
                    ),
                ) == 0
                {
                    temp = (*actor).target;
                    (*actor).target = corpsehit;
                    super::attacks::action_face_target(actor);
                    (*actor).target = temp;
                    P_SetMobjState(actor, S_VILE_HEAL1);
                    S_StartSound(corpsehit as *mut std::ffi::c_void, Sfx::Slop as c_int);
                    info = (*corpsehit).info as *mut MobjInfo;
                    P_SetMobjState(corpsehit, (*info).raisestate as statenum_t);
                    (*corpsehit).height <<= 2 as c_int;
                    (*corpsehit).flags = (*info).flags;
                    (*corpsehit).health = (*info).spawnhealth;
                    (*corpsehit).target = core::ptr::null_mut::<mobj_t>();
                    return;
                }
                by += 1;
            }
            bx += 1;
        }
    }
    action_chase(actor);
}
