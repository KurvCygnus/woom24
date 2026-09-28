//! Monster chase core: target acquisition and the per-tic state
//! machine (`A_Look` / `A_Chase`), melee/missile range checks, the
//! step-try walk, the eight-direction chase-dir chooser, and the
//! player scan -- bit-exact with the chase half of
//! `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::c_ffi::line_t;
use crate::doom::d_main::fastparm;
use crate::doom::d_player::{players, PlayerT};
use crate::doom::g_game::playeringame;
use crate::doom::g_game::{gameskill, netgame};
use crate::i_error;
use crate::doom::info::{
    MF_AMBUSH, MF_FLOAT, MF_INFLOAT, MF_JUSTATTACKED, MF_JUSTHIT, MF_SHOOTABLE, MobjInfo, MT_CYBORG,
    MT_SKULL, MT_SPIDER, MT_UNDEAD, MT_VILE,
};
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_map::{floatok, numspechit, spechit, tmfloorz, P_TryMove};
use crate::doom::p_maputl::P_AproxDistance;
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_sight::P_CheckSight;
use crate::doom::p_switch::P_UseSpecialLine;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{ANG90, ANG270};
use crate::types::Boolean;

use super::consts::{
    angle_t, statenum_t, DI_EAST, DI_NODIR, DI_NORTH, DI_NORTHEAST, DI_NORTHWEST, DI_SOUTH,
    DI_SOUTHEAST, DI_SOUTHWEST, DI_WEST, FLOATSPEED, MELEERANGE, sk_nightmare,
};

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = super::consts::CffiMobj;

/// Eight-direction movement type used by the monster pathfinding code; mirrors C `dirtype_t`.
type dirtype_t = c_int;

/// Lookup table mapping each of the eight directions to its 180-degree opposite, with
/// `DI_NODIR` mapping to `DI_NODIR`; indexed by `dirtype_t` in `P_NewChaseDir`.
/// Has C linkage (`#[no_mangle]`); referenced from `p_enemy.c` (C test harness).
#[no_mangle]
pub static mut opposite: [dirtype_t; 9] = [
    DI_WEST,
    DI_SOUTHWEST,
    DI_SOUTH,
    DI_SOUTHEAST,
    DI_EAST,
    DI_NORTHEAST,
    DI_NORTH,
    DI_NORTHWEST,
    DI_NODIR,
];

/// Lookup table mapping the two-bit index `(deltay<0)<<1 | (deltax>0)` to a diagonal
/// `dirtype_t`; used by `P_NewChaseDir` to prefer diagonal movement toward the target.
/// Has C linkage (`#[no_mangle]`); referenced from `p_enemy.c` (C test harness).
#[no_mangle]
pub static mut diags: [dirtype_t; 4] = [DI_NORTHWEST, DI_NORTHEAST, DI_SOUTHWEST, DI_SOUTHEAST];

/// Per-direction X-axis speed multiplier table indexed by `dirtype_t` (0=East..7=Southeast).
/// Each entry is a fixed-point unit (FRACUNIT or ~0.718*FRACUNIT for diagonals).
/// Has C linkage (`#[no_mangle]`); referenced from `p_enemy.c` (C test harness).
#[no_mangle]
pub static mut xspeed: [fixed_t; 8] = [
    FRACUNIT,
    47000 as c_int,
    0 as c_int,
    -47000 as c_int,
    -FRACUNIT,
    -47000 as c_int,
    0 as c_int,
    47000 as c_int,
];

/// Per-direction Y-axis speed multiplier table indexed by `dirtype_t` (0=East..7=Southeast).
/// Each entry is a fixed-point unit (FRACUNIT or ~0.718*FRACUNIT for diagonals).
/// Has C linkage (`#[no_mangle]`); referenced from `p_enemy.c` (C test harness).
#[no_mangle]
pub static mut yspeed: [fixed_t; 8] = [
    0 as c_int,
    47000 as c_int,
    FRACUNIT,
    47000 as c_int,
    0 as c_int,
    -47000 as c_int,
    -FRACUNIT,
    -47000 as c_int,
];

/// Returns `TRUE` when `actor`'s target is within melee striking range and line of sight.
///
/// Range test: approximate distance must be less than `MELEERANGE - 20 + target.radius`.
/// Returns `FALSE` immediately if `actor->target` is null, if the distance check fails,
/// or if `P_CheckSight` reports no clear line of sight.
///
/// # Safety
///
/// `actor` must be a valid, non-null `mobj_t`. If `actor->target` is non-null it must
/// also point to a valid `mobj_t` with a valid `info` pointer. Called from C.
#[doc(alias = "P_CheckMeleeRange")]
#[export_name = "P_CheckMeleeRange"]
pub unsafe extern "C" fn check_melee_range(actor: *mut mobj_t) -> Boolean
{
    if(*actor).target.is_null() { return Boolean::FALSE; }
    let pl: *mut mobj_t = (*actor).target;
    let dist: fixed_t = P_AproxDistance((*pl).x - (*actor).x, (*pl).y - (*actor).y);
    if dist >= MELEERANGE - 20 as c_int * FRACUNIT + (*((*pl).info as *mut MobjInfo)).radius { return Boolean::FALSE; }
    if P_CheckSight(actor, (*actor).target) == 0 { return Boolean::FALSE; }
    Boolean::TRUE
}

/// Returns `TRUE` when `actor` is permitted to fire a missile at its current target.
///
/// Checks line of sight first; immediately grants permission if the actor was just hit
/// (`MF_JUSTHIT`, clears the flag). Blocks attacks during `reactiontime` countdown.
/// Distance is scaled to a 0-200 range and compared against a random threshold so
/// closer targets are more reliably hit. Per-monster special cases:
/// - Archvile (`MT_VILE`): blocked beyond 14*64 units.
/// - Revenant (`MT_UNDEAD`): blocked below 196 units (prefers melee); range halved.
/// - Cyberdemon, Spider Mastermind, Lost Soul: effective range halved and capped at 160
///   (Cyberdemon) or 200 (others).
///
/// # Safety
///
/// `actor` must be non-null and have a valid `target`, `info`, and `reactiontime`.
/// Called from C.
#[doc(alias = "P_CheckMissileRange")]
#[export_name = "P_CheckMissileRange"]
pub unsafe extern "C" fn check_missile_range(actor: *mut mobj_t) -> Boolean
{
    let mut dist: fixed_t;

    if P_CheckSight(actor, (*actor).target) == 0 { return Boolean::FALSE; }
    if(*actor).flags & MF_JUSTHIT as c_int != 0
    {
        (*actor).flags &= !(MF_JUSTHIT as c_int);
        return Boolean::TRUE;
    }
    if(*actor).reactiontime != 0
    {
        return Boolean::FALSE;
    }
    dist = (
        P_AproxDistance(
            (*actor).x - (*(*actor).target).x,
            (*actor).y - (*(*actor).target).y,
        ) as c_int - 64 as c_int * FRACUNIT
    ) as fixed_t;
    if(*((*actor).info as *mut MobjInfo)).meleestate == 0 { dist -= 128 as c_int * FRACUNIT; }
    dist >>= 16 as c_int;
    if(*actor).mobjtype as c_uint == MT_VILE as c_int as c_uint && dist > 14 as c_int * 64 as c_int { return Boolean::FALSE; }
    if(*actor).mobjtype as c_uint == MT_UNDEAD as c_int as c_uint
    {
        if dist < 196 as c_int { return Boolean::FALSE; }
        dist >>= 1 as c_int;
    }
    if(*actor).mobjtype as c_uint == MT_CYBORG as c_int as c_uint ||
        (*actor).mobjtype as c_uint == MT_SPIDER as c_int as c_uint ||
        (*actor).mobjtype as c_uint == MT_SKULL as c_int as c_uint { dist >>= 1 as c_int; }
    if dist > 200 as c_int { dist = 200 as c_int as fixed_t; }
    if(*actor).mobjtype as c_uint == MT_CYBORG as c_int as c_uint && dist > 160 as c_int { dist = 160 as c_int as fixed_t; }
    if P_Random() < dist { return Boolean::FALSE; }
    Boolean::TRUE
}

/// Attempts to advance `actor` one step in its current `movedir`.
///
/// Returns `FALSE` if `movedir` is `DI_NODIR`. Computes the target position using
/// `xspeed`/`yspeed` scaled by the actor's `info->speed`, then calls `P_TryMove`.
/// On success, clears `MF_INFLOAT` and snaps non-floating actors to the floor.
/// On failure:
/// - If the actor has `MF_FLOAT` and `floatok` is set, adjusts Z by `FLOATSPEED`
///   toward `tmfloorz` and sets `MF_INFLOAT`, returning `TRUE`.
/// - Otherwise iterates `spechit` in reverse, calling `P_UseSpecialLine` on each;
///   returns `TRUE` if any special line was successfully activated.
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t` with valid `info`. All globals
/// `floatok`, `tmfloorz`, `numspechit`, and `spechit` must be consistent with the
/// most recent `P_TryMove` call. Called from C.
#[doc(alias = "P_Move")]
#[export_name = "P_Move"]
pub unsafe extern "C" fn move_step(actor: *mut mobj_t) -> Boolean
{
    let mut ld: *mut line_t;

    let mut good: Boolean;

    if(*actor).movedir == DI_NODIR as c_int { return Boolean::FALSE; }
    if(*actor).movedir as c_uint >= 8 as c_uint { i_error!("Weird actor->movedir!"); }
    let tryx: fixed_t = (*actor).x + (*((*actor).info as *mut MobjInfo)).speed as fixed_t * xspeed[(*actor).movedir as usize];
    let tryy: fixed_t = (*actor).y + (*((*actor).info as *mut MobjInfo)).speed as fixed_t * yspeed[(*actor).movedir as usize];
    let try_ok: Boolean = Boolean::from_raw(P_TryMove(actor as *mut CffiMobj, tryx, tryy));
    if try_ok.is_false()
    {
        if(*actor).flags & MF_FLOAT as c_int != 0 && floatok != 0
        {
            if(*actor).z < tmfloorz { (*actor).z += FLOATSPEED; }
            else { (*actor).z -= FLOATSPEED; }
            (*actor).flags |= MF_INFLOAT as c_int;
            return Boolean::TRUE;
        }
        if numspechit == 0 { return Boolean::FALSE; }
        (*actor).movedir = DI_NODIR as c_int;
        good = Boolean::FALSE;
        loop
        {
            let c2rust_fresh0 = numspechit;
            numspechit -= 1;
            if c2rust_fresh0 == 0 { break; }
            //? Entries beyond the array were never stored (see the guarded
            //? push in PIT_CheckLine); skip them instead of reading OOB.
            if numspechit < 0 || numspechit as usize >= crate::doom::p_map::MAXSPECIALCROSS { continue; }
            ld = spechit[numspechit as usize];
            if P_UseSpecialLine(
                actor as *mut c_void,
                ld as *mut crate::doom::p_lights::line_t,
                0 as c_int,
            ) != 0 { good = Boolean::TRUE; }
        }
        return good;
    }
    else { (*actor).flags &= !(MF_INFLOAT as c_int); }
    if(*actor).flags & MF_FLOAT as c_int == 0 { (*actor).z = (*actor).floorz; }
    Boolean::TRUE
}

/// Attempts to move `actor` in its current direction; on success randomises `movecount`.
///
/// Calls `P_Move`; returns `FALSE` immediately if blocked. On success sets
/// `actor->movecount` to a random value in 0..=15, giving the monster a random
/// number of tics before it reconsiders its direction.
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t`. Called from C.
#[doc(alias = "P_TryWalk")]
#[export_name = "P_TryWalk"]
pub unsafe extern "C" fn try_walk(actor: *mut mobj_t) -> Boolean
{
    if move_step(actor).is_false() { return Boolean::FALSE; }
    (*actor).movecount = P_Random() & 15 as c_int;
    Boolean::TRUE
}

/// Selects a new movement direction for `actor` based on the vector to its target.
///
/// Algorithm (mirrors `P_NewChaseDir` in `p_enemy.c`):
/// 1. Compute axis-aligned directions toward the target (`d[1]`, `d[2]`).
/// 2. Prefer the diagonal that combines both axes; try it first if not a U-turn.
/// 3. With 20% probability (or if |dy|>|dx|) swap x/y preference.
/// 4. Filter out the reverse direction and try each axis individually.
/// 5. Fall back to continuing the previous direction.
/// 6. Last resort: sweep all eight directions (randomly forward or backward).
/// 7. If still blocked, set `movedir = DI_NODIR`.
///
/// Panics (via `i_error!`) if `actor->target` is null.
///
/// # Safety
///
/// `actor` must be non-null with a valid, non-null `target`. Called from C.
#[doc(alias = "P_NewChaseDir")]
#[export_name = "P_NewChaseDir"]
pub unsafe extern "C" fn new_chase_dir(actor: *mut mobj_t)
{
    let mut d: [dirtype_t; 3] = [DI_EAST; 3];
    let mut tdir: c_int;

    if(*actor).target.is_null() { i_error!("P_NewChaseDir: called with no target"); }
    let olddir: dirtype_t = (*actor).movedir as dirtype_t;
    let turnaround: dirtype_t = opposite[olddir as usize];
    let deltax: fixed_t = (*(*actor).target).x - (*actor).x;
    let deltay: fixed_t = (*(*actor).target).y - (*actor).y;
    if deltax > 10 as c_int * FRACUNIT { d[1 as c_int as usize] = DI_EAST; }
    else if deltax < -10 as c_int * FRACUNIT { d[1 as c_int as usize] = DI_WEST; }
    else { d[1 as c_int as usize] = DI_NODIR; }
    if deltay < -10 as c_int * FRACUNIT { d[2 as c_int as usize] = DI_SOUTH; }
    else if deltay > 10 as c_int * FRACUNIT { d[2 as c_int as usize] = DI_NORTH; }
    else { d[2 as c_int as usize] = DI_NODIR; }
    if d[1 as c_int as usize] as c_uint != DI_NODIR as c_int as c_uint &&
        d[2 as c_int as usize] as c_uint != DI_NODIR as c_int as c_uint
        {
            (*actor).movedir = diags[((((deltay < 0 as c_int) as c_int) << 1 as c_int)
                + (deltax > 0 as c_int) as c_int) as usize] as c_int;
            if(*actor).movedir != turnaround as c_int && try_walk(actor).is_truthy() { return; }
        }
    
    if P_Random() > 200 as c_int || (deltay as c_int).abs() > (deltax as c_int).abs()
    {
        tdir = d[1 as c_int as usize] as c_int;
        d[1 as c_int as usize] = d[2 as c_int as usize];
        d[2 as c_int as usize] = tdir as dirtype_t;
    }
    
    if d[1 as c_int as usize] as c_uint == turnaround as c_uint { d[1 as c_int as usize] = DI_NODIR; }
    
    if d[2 as c_int as usize] as c_uint == turnaround as c_uint { d[2 as c_int as usize] = DI_NODIR; }
    
    if d[1 as c_int as usize] as c_uint != DI_NODIR as c_int as c_uint
    {
        (*actor).movedir = d[1 as c_int as usize] as c_int;
        if try_walk(actor).is_truthy() { return; }
    }
    if d[2 as c_int as usize] as c_uint != DI_NODIR as c_int as c_uint
    {
        (*actor).movedir = d[2 as c_int as usize] as c_int;
        if try_walk(actor).is_truthy() { return; }
    }
    if olddir as c_uint != DI_NODIR as c_int as c_uint
    {
        (*actor).movedir = olddir as c_int;
        if try_walk(actor).is_truthy() { return; }
    }
    if P_Random() & 1 as c_int != 0
    {
        tdir = DI_EAST as c_int;
        while tdir <= DI_SOUTHEAST as c_int
        {
            if tdir != turnaround as c_int
            {
                (*actor).movedir = tdir;
                if try_walk(actor).is_truthy() { return; }
            }
            tdir += 1;
        }
    }
    else
    {
        tdir = DI_SOUTHEAST as c_int;
        while tdir != DI_EAST as c_int - 1 as c_int
        {
            if tdir != turnaround as c_int
            {
                (*actor).movedir = tdir;
                if try_walk(actor).is_truthy() { return; }
            }
            tdir -= 1;
        }
    }
    if turnaround as c_uint != DI_NODIR as c_int as c_uint
    {
        (*actor).movedir = turnaround as c_int;
        if try_walk(actor).is_truthy() { return; }
    }
    (*actor).movedir = DI_NODIR as c_int;
}

/// Scans active players to find a visible target for `actor`; returns `TRUE` if one is found.
///
/// Iterates up to four player slots starting from `actor->lastlook`, cycling with `& 3`.
/// Stops after examining two live players or looping back to the starting slot.
/// Skips dead players and players with no line of sight.
/// If `allaround` is `FALSE`, also skips players that are more than 90 degrees behind
/// the actor (angle difference in ANG90..ANG270) unless they are within melee range.
/// On success sets `actor->target` to the found player's map object.
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t`. The global `players` array and
/// `playeringame` flags must be consistent. Called from C.
#[doc(alias = "P_LookForPlayers")]
#[export_name = "P_LookForPlayers"]
pub unsafe extern "C" fn look_for_players(actor: *mut mobj_t, allaround: Boolean) -> Boolean
{
    let mut c: c_int;

    let mut player: *mut PlayerT;

    let mut an: angle_t;

    let mut dist: fixed_t;

    c = 0 as c_int;
    let stop: c_int = ((*actor).lastlook - 1 as c_int) & 3 as c_int;
    loop
    {
        's_20:
        {
            if playeringame[(*actor).lastlook as usize] != 0
            {
                let c2rust_fresh1 = c;
                c += 1;
                if c2rust_fresh1 == 2 as c_int || (*actor).lastlook == stop { return Boolean::FALSE; }
                player = (&raw mut players as *mut PlayerT).offset((*actor).lastlook as isize);
                if(*player).health > 0 as c_int
                {
                    let sight = P_CheckSight(actor, (*player).mo as *mut mobj_t);
                    if sight != 0
                    {
                        if allaround.is_false()
                        {
                            an = R_PointToAngle2(
                                (*actor).x,
                                (*actor).y,
                                (*((*player).mo as *mut mobj_t)).x,
                                (*((*player).mo as *mut mobj_t)).y,
                            ).wrapping_sub((*actor).angle);
                            
                            if an > ANG90 as angle_t && an < ANG270
                            {
                                dist = P_AproxDistance(
                                    (*((*player).mo as *mut mobj_t)).x - (*actor).x,
                                    (*((*player).mo as *mut mobj_t)).y - (*actor).y,
                                );
                                if dist > MELEERANGE { break 's_20; }
                            }
                        }
                        (*actor).target = (*player).mo as *mut mobj_t;
                        return Boolean::TRUE;
                    }
                }
            }
        }
        (*actor).lastlook = ((*actor).lastlook + 1 as c_int) & 3 as c_int;
    }
}

/// State-machine action: monster idle look, waiting to spot a player.
///
/// Resets `threshold` to 0 so any hit will wake it. First checks the sector's
/// `soundtarget`; if a shootable target is present the monster wakes immediately
/// (ambush monsters additionally require line of sight). Falls back to
/// `P_LookForPlayers`. On waking, plays the monster's `seesound` (randomised for
/// Former Human and Demon variants) and transitions to `seestate`.
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t` with valid `subsector`, `info`, and
/// `flags`. Called from C via state-machine action pointer.
#[doc(alias = "A_Look")]
#[export_name = "A_Look"]
pub unsafe extern "C" fn action_look(actor: *mut mobj_t)
{
    (*actor).threshold = 0 as c_int;
    let targ: *mut mobj_t = (*(*(*actor).subsector).sector).soundtarget as *mut mobj_t;
    '_seeyou:
    {
        if !targ.is_null() && (*targ).flags & MF_SHOOTABLE as c_int != 0
        {
            (*actor).target = targ;
            if(*actor).flags & MF_AMBUSH as c_int != 0 { if P_CheckSight(actor, (*actor).target) != 0 { break '_seeyou; } }
            else { break '_seeyou; }
        }
        if look_for_players(actor, Boolean::FALSE).is_false() { return; }
    }
    if(*((*actor).info as *mut MobjInfo)).seesound != Sfx::None
    {
        let sound = match (*((*actor).info as *mut MobjInfo)).seesound
        {
            Sfx::Posit1 | Sfx::Posit2 | Sfx::Posit3 => { Sfx::Posit1 as c_int + P_Random() % 3 as c_int }
            Sfx::Bgsit1 | Sfx::Bgsit2 => Sfx::Bgsit1 as c_int + P_Random() % 2 as c_int,
            s => s as c_int,
        };
        if(*actor).mobjtype as c_uint == MT_SPIDER as c_int as c_uint ||
            (*actor).mobjtype as c_uint == MT_CYBORG as c_int as c_uint { S_StartSound(std::ptr::null_mut::<c_void>(), sound); }
        else { S_StartSound(actor as *mut c_void, sound); }
    }
    P_SetMobjState(
        actor,
        (*((*actor).info as *mut MobjInfo)).seestate as statenum_t,
    );
}

/// State-machine action: monster actively chases its target and attacks when able.
///
/// Each call:
/// 1. Decrements `reactiontime` if non-zero (initial delay after spawning).
/// 2. Decrements `threshold` toward zero (persistence on current target).
/// 3. Snaps `angle` toward the current `movedir` (±ANG90/2 per tic).
/// 4. If the target is gone/dead, searches for a new player; falls back to `spawnstate`.
/// 5. If `MF_JUSTATTACKED` is set, clears it and redirects movement (except Nightmare).
/// 6. Triggers melee attack if in range (`meleestate`).
/// 7. Triggers missile attack if in range and the cooldown permits (`missilestate`).
/// 8. In netgame, may switch targets if the current one is out of sight.
/// 9. Advances movement; calls `P_NewChaseDir` if blocked or `movecount` expired.
/// 10. Occasionally plays `activesound` (random < 3).
///
/// # Safety
///
/// `actor` must be a non-null, valid `mobj_t` with valid `info`. All referenced
/// globals (`gameskill`, `fastparm`, `netgame`) must be initialised. Called from C.
#[doc(alias = "A_Chase")]
#[export_name = "A_Chase"]
pub unsafe extern "C" fn action_chase(actor: *mut mobj_t)
{
    let delta: c_int;

    if(*actor).reactiontime != 0 { (*actor).reactiontime -= 1; }
    if(*actor).threshold != 0
    {
        if(*actor).target.is_null() || (*(*actor).target).health <= 0 as c_int { (*actor).threshold = 0 as c_int; }
        else { (*actor).threshold -= 1; }
    }
    if(*actor).movedir < 8 as c_int
    {
        (*actor).angle &= ((7 as c_int) << 29 as c_int) as angle_t;
        delta = (*actor).angle.wrapping_sub(((*actor).movedir << 29 as c_int) as angle_t) as c_int;
        if delta > 0 as c_int { (*actor).angle = (*actor).angle.wrapping_sub((ANG90 / 2) as angle_t); }
        else if delta < 0 as c_int { (*actor).angle = (*actor).angle.wrapping_add((ANG90 / 2) as angle_t); }
    }
    if(*actor).target.is_null() || (*(*actor).target).flags & MF_SHOOTABLE as c_int == 0
    {
        if look_for_players(actor, Boolean::TRUE).is_truthy() { return; }
        P_SetMobjState(
            actor,
            (*((*actor).info as *mut MobjInfo)).spawnstate as statenum_t,
        );
        return;
    }
    if(*actor).flags & MF_JUSTATTACKED as c_int != 0
    {
        (*actor).flags &= !(MF_JUSTATTACKED as c_int);
        if gameskill as c_int != sk_nightmare as c_int && fastparm == 0 { new_chase_dir(actor); }
        return;
    }
    if(*((*actor).info as *mut MobjInfo)).meleestate != 0 && check_melee_range(actor).is_truthy()
    {
        if(*((*actor).info as *mut MobjInfo)).attacksound != Sfx::None
        {
            S_StartSound(
                actor as *mut c_void,
                (*((*actor).info as *mut MobjInfo)).attacksound as c_int,
            );
        }
        P_SetMobjState(
            actor,
            (*((*actor).info as *mut MobjInfo)).meleestate as statenum_t,
        );
        return;
    }
    if(*((*actor).info as *mut MobjInfo)).missilestate != 0 &&
        !(
            (gameskill as c_int) < sk_nightmare as c_int &&
            fastparm == 0 &&
            (*actor).movecount != 0
        ) && check_missile_range(actor).is_truthy()
        {
            P_SetMobjState(
                actor,
                (*((*actor).info as *mut MobjInfo)).missilestate as statenum_t,
            );
            (*actor).flags |= MF_JUSTATTACKED as c_int;
            return;
        }
    
    if netgame != 0 && (*actor).threshold == 0 && P_CheckSight(actor, (*actor).target) == 0 && look_for_players(actor, Boolean::TRUE).is_truthy() { return; }
    (*actor).movecount -= 1;
    if(*actor).movecount < 0 as c_int || move_step(actor).is_false() { new_chase_dir(actor); }
    if(*((*actor).info as *mut MobjInfo)).activesound != Sfx::None && P_Random() < 3 as c_int
    {
        S_StartSound(
            actor as *mut c_void,
            (*((*actor).info as *mut MobjInfo)).activesound as c_int,
        );
    }
}
