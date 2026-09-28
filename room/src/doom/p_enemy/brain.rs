//! Icon of Sin (brain) actions: boss-target collection, the cube
//! spit with its easy-skill alternation, the cube flight and arrival
//! (the weighted random-monster materialization), and the Romero-head
//! death sequence -- bit-exact with the brain half of
//! `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::g_game::{gameskill, G_ExitLevel};
use crate::doom::info::{
    MT_BOSSTARGET, MT_ROCKET, MT_SPAWNFIRE, MT_SPAWNSHOT, MobjInfo, State, S_BRAINEXPLODE1,
};
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_map::P_TeleportMove;
use crate::doom::p_mobj::{P_RemoveMobj, P_SpawnMissile, P_SpawnMobj, P_SetMobjState, P_SubstNullMobj};
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::{thinkercap, thinker_t};
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::chase::look_for_players;
use super::consts::{mobjtype_t, statenum_t, CffiMobj, sk_easy};
use super::dtmc::spawn_fly_pick;

/// Array of `MT_BOSSTARGET` map objects (Icon of Sin target spots) populated by `A_BrainAwake`.
/// The brain cycles through these to choose spawn destinations. Has C linkage.
#[no_mangle]
pub static mut braintargets: [*mut mobj_t; 32] = [std::ptr::null_mut::<mobj_t>(); 32];

/// Count of valid entries in `braintargets`; set by `A_BrainAwake`. Has C linkage.
#[no_mangle]
pub static mut numbraintargets: c_int = 0;

/// Round-robin index into `braintargets` for the next cube launch; advanced by `A_BrainSpit`.
/// Has C linkage.
#[no_mangle]
pub static mut braintargeton: c_int = 0 as c_int;

/// Icon of Sin wake-up: scans the thinker list for `MT_BOSSTARGET` spots and plays `sfx_bossit`.
///
/// Populates `braintargets` and resets `numbraintargets`/`braintargeton` to 0.
/// The target spots are placed by the level designer in the map; the brain cycles
/// through them to choose where to spit monster cubes.
///
/// # Safety
///
/// The thinker list must be consistent. Called from C via state-machine action pointer.
#[doc(alias = "A_BrainAwake")]
#[export_name = "A_BrainAwake"]
pub unsafe extern "C" fn action_brain_awake(_mo: *mut mobj_t)
{
    let mut thinker: *mut thinker_t;

    let mut m: *mut mobj_t;

    numbraintargets = 0 as c_int;
    braintargeton = 0 as c_int;
    thinker = thinkercap.next;
    while !std::ptr::eq(thinker, &raw const thinkercap)
    {
        if super::map_events::is_mobj_thinker(thinker)
        {
            m = thinker as *mut mobj_t;
            if(*m).mobjtype as c_uint == MT_BOSSTARGET as c_int as c_uint
            {
                braintargets[numbraintargets as usize] = m;
                numbraintargets += 1;
            }
        }
        thinker = (*thinker).next;
    }
    S_StartSound(std::ptr::null_mut::<c_void>(), Sfx::Bossit as c_int);
}

/// Icon of Sin pain reaction: plays `sfx_bospn` at full (global) volume.
///
/// # Safety
///
/// Called from C. No pointer dereference; safe with any (even null) `_mo`.
#[doc(alias = "A_BrainPain")]
#[export_name = "A_BrainPain"]
pub unsafe extern "C" fn action_brain_pain(_mo: *mut mobj_t) { S_StartSound(std::ptr::null_mut::<c_void>(), Sfx::Bospn as c_int); }

/// Icon of Sin death scream: spawns a row of exploding rockets across the brain's width.
///
/// Spawns `MT_ROCKET` objects every 8 units from `mo->x - 196` to `mo->x + 320`,
/// each at a random height and with a random upward `momz`. Each rocket is immediately
/// set to `S_BRAINEXPLODE1` with a randomised tic count for visual variety.
/// Plays `sfx_bosdth` at full volume to conclude.
///
/// # Safety
///
/// `mo` must be non-null. Called from C via state-machine action pointer.
#[doc(alias = "A_BrainScream")]
#[export_name = "A_BrainScream"]
pub unsafe extern "C" fn action_brain_scream(mo: *mut mobj_t)
{
    let mut x: c_int;

    let mut y: c_int;

    let mut z: c_int;

    let mut th: *mut mobj_t;

    x = (*mo).x as c_int - 196 as c_int * FRACUNIT;
    while x < (*mo).x as c_int + 320 as c_int * FRACUNIT
    {
        y = (*mo).y as c_int - 320 as c_int * FRACUNIT;
        z = 128 as c_int + P_Random() * 2 as c_int * FRACUNIT;
        th = P_SpawnMobj(x as fixed_t, y as fixed_t, z as fixed_t, MT_ROCKET);
        (*th).momz = (P_Random() * 512 as c_int) as fixed_t;
        P_SetMobjState(th, S_BRAINEXPLODE1);
        (*th).tics -= P_Random() & 7 as c_int;
        if(*th).tics < 1 as c_int { (*th).tics = 1 as c_int; }
        x += FRACUNIT * 8 as c_int;
    }
    S_StartSound(std::ptr::null_mut::<c_void>(), Sfx::Bosdth as c_int);
}

/// Individual brain explosion particle: spawns one `MT_ROCKET` at a random horizontal offset.
///
/// Called repeatedly by the `S_BRAINEXPLODE` state chain. Each invocation spawns a rocket
/// with random X offset (±`P_Random*2048`) at the same Y as `mo`, at a random height
/// (128 + `P_Random*2*FRACUNIT`). The rocket transitions immediately to `S_BRAINEXPLODE1`
/// with a randomised tic count.
///
/// # Safety
///
/// `mo` must be non-null. Called from C via state-machine action pointer.
#[doc(alias = "A_BrainExplode")]
#[export_name = "A_BrainExplode"]
pub unsafe extern "C" fn action_brain_explode(mo: *mut mobj_t)
{
    let x: c_int = (*mo).x as c_int + (P_Random() - P_Random()) * 2048 as c_int;
    let y: c_int = (*mo).y as c_int;
    let z: c_int = 128 as c_int + P_Random() * 2 as c_int * FRACUNIT;
    let th: *mut mobj_t = P_SpawnMobj(
        x as fixed_t,
        y as fixed_t,
        z as fixed_t,
        MT_ROCKET,
    );
    (*th).momz = (P_Random() * 512 as c_int) as fixed_t;
    P_SetMobjState(th, S_BRAINEXPLODE1);
    (*th).tics -= P_Random() & 7 as c_int;
    if(*th).tics < 1 as c_int { (*th).tics = 1 as c_int; }
}

/// Icon of Sin death: ends the level via `G_ExitLevel`.
///
/// # Safety
///
/// Called from C. No pointer dereference beyond the ignored `_mo`.
#[doc(alias = "A_BrainDie")]
#[export_name = "A_BrainDie"]
pub unsafe extern "C" fn action_brain_die(_mo: *mut mobj_t) { G_ExitLevel(); }

/// Icon of Sin attack: launches a monster cube (`MT_SPAWNSHOT`) toward the next target spot.
///
/// Alternates with a static `easy` flag so on easy skill every other attack is skipped.
/// Picks `braintargets[braintargeton]` as the destination and advances `braintargeton`
/// modulo `numbraintargets`. Sets the cube's `reactiontime` to the travel time (in state
/// tics) so `A_SpawnFly` knows when to materialize the monster. Plays `sfx_bospit` globally.
///
/// # Safety
///
/// `mo` must be non-null. `braintargets` must have been populated by `A_BrainAwake` and
/// `numbraintargets` must be > 0. Called from C via state-machine action pointer.
#[doc(alias = "A_BrainSpit")]
#[export_name = "A_BrainSpit"]
pub unsafe extern "C" fn action_brain_spit(mo: *mut mobj_t)
{
    static mut easy: c_int = 0 as c_int;
    easy ^= 1 as c_int;
    if gameskill as c_int <= sk_easy as c_int && easy == 0 { return; }
    let targ: *mut mobj_t = braintargets[braintargeton as usize];
    braintargeton = (braintargeton + 1 as c_int) % numbraintargets;
    let newmobj: *mut mobj_t = P_SpawnMissile(mo, targ, MT_SPAWNSHOT);
    (*newmobj).target = targ;
    (*newmobj).reactiontime = ((*targ).y as c_int - (*mo).y as c_int) /
        (*newmobj).momy as c_int /
        (*((*newmobj).state as *mut State)).tics;
    S_StartSound(std::ptr::null_mut::<c_void>(), Sfx::Bospit as c_int);
}

/// In-flight cube sound: plays `sfx_boscub` while the cube travels, then calls `A_SpawnFly`.
///
/// # Safety
///
/// `mo` must be non-null. Called from C via state-machine action pointer.
#[doc(alias = "A_SpawnSound")]
#[export_name = "A_SpawnSound"]
pub unsafe extern "C" fn action_spawn_sound(mo: *mut mobj_t)
{
    S_StartSound(mo as *mut c_void, Sfx::Boscub as c_int);
    action_spawn_fly(mo);
}

/// Monster cube arrival: materializes a random monster at the target spot when `reactiontime` hits 0.
///
/// Decrements `reactiontime` each tic; returns immediately while still > 0. On arrival:
/// 1. Spawns a `MT_SPAWNFIRE` teleport fog with `sfx_telept` at the target's location.
/// 2. Picks a random monster type via weighted probability table (Imp 50/256 through
///    Baron of Hell for top range).
/// 3. Spawns the monster, calls `P_LookForPlayers` to awaken it.
/// 4. Calls `P_TeleportMove` to telefrág anything at the spawn point.
/// 5. Removes the cube (`mo`) via `P_RemoveMobj`.
///
/// # Safety
///
/// `mo` must be non-null with a valid `target`. `mo->target` must point to a valid
/// `mobj_t` (the boss target spot). Called from C via state-machine action pointer.
#[doc(alias = "A_SpawnFly")]
#[export_name = "A_SpawnFly"]
pub unsafe extern "C" fn action_spawn_fly(mo: *mut mobj_t)
{
    (*mo).reactiontime -= 1;
    if(*mo).reactiontime != 0 { return; }
    let targ: *mut mobj_t = P_SubstNullMobj((*mo).target);
    let fog: *mut mobj_t = P_SpawnMobj((*targ).x, (*targ).y, (*targ).z, MT_SPAWNFIRE);
    S_StartSound(fog as *mut c_void, Sfx::Telept as c_int);
    let r: c_int = P_Random();
    let type_0: mobjtype_t = spawn_fly_pick(r);
    let newmobj: *mut mobj_t = P_SpawnMobj((*targ).x, (*targ).y, (*targ).z, type_0);
    if look_for_players(newmobj, crate::types::Boolean::TRUE).is_truthy()
    {
        P_SetMobjState(
            newmobj,
            (*((*newmobj).info as *mut MobjInfo)).seestate as statenum_t,
        );
    }
    P_TeleportMove(newmobj as *mut CffiMobj, (*newmobj).x, (*newmobj).y);
    P_RemoveMobj(mo);
}
