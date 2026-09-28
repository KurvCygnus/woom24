//! Kill accounting and damage application: `P_KillMobj` (kill/frag
//! counters, corpse transition, death-tic roll, weapon drops) and
//! `P_DamageMobj` (knockback thrust, god/invulnerability gates, armor
//! absorption, pain and target acquisition) -- bit-exact with the
//! corresponding half of `vendor/doomgeneric/p_inter.c`. The genuinely
//! pure computations are extracted in [`super::dtmc`]; the draws stay
//! at these call sites.

#![allow(non_snake_case)]

use std::os::raw::c_int;

use crate::doom::am_map::{automapactive, AM_Stop};
use crate::doom::d_player::{consoleplayer, players, PlayerT, CF_GODMODE};
use crate::doom::doomstat::gameversion;
use crate::doom::g_game::{gameskill, netgame};
use crate::doom::i_system::I_Tactile;
use crate::doom::info;
use crate::doom::info::{
    MF_CORPSE, MF_COUNTKILL, MF_DROPPED, MF_DROPOFF, MF_FLOAT, MF_JUSTHIT, MF_NOCLIP,
    MF_NOGRAVITY, MF_SHOOTABLE, MF_SKULLFLY, MF_SOLID, MT_CHAINGUN, MT_CHAINGUY, MT_CLIP,
    MT_SHOTGUN, MT_SHOTGUY, MT_SKULL, MT_POSSESSED, MT_VILE, MT_WOLFSS, S_NULL, State, MobjInfo,
};
use crate::doom::m_fixed::{FixedMul, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_mobj::{P_SetMobjState, P_SpawnMobj};
use crate::doom::p_pspr::P_DropWeapon;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::tables::{finecosine, finesine, ANG180, ANGLETOFINESHIFT};

use super::consts::{exe_chex, sk_baby, wp_chainsaw, BASETHRESHOLD, ONFLOORZ, pw_invulnerability};
use super::dtmc;

// ---------------------------------------------------------------------------
// P_KillMobj
// ---------------------------------------------------------------------------

/// Kill map object `target`, optionally crediting `source` with the kill.
///
/// Clears movement flags (`MF_SHOOTABLE`, `MF_FLOAT`, `MF_SKULLFLY`),
/// sets `MF_CORPSE | MF_DROPOFF`, halves the height, transitions to the
/// appropriate death state (normal or extra-gory `xdeathstate`), and
/// randomises the initial death-animation tic offset by up to 3 tics.
///
/// Kill counters: if `source` is a player, `killcount` and `frags` are
/// updated.  If `source` is null in a single-player game, `players[0]`
/// still gets the kill credit (e.g. barrel chain-kills).
///
/// Weapon drops: `MT_WOLFSS` / `MT_POSSESSED` drop `MT_CLIP`;
/// `MT_SHOTGUY` drops `MT_SHOTGUN`; `MT_CHAINGUY` drops `MT_CHAINGUN`.
/// No items are dropped in Chex Quest.
///
/// If `target` is a player, the player enters `PST_DEAD`, the automap is
/// stopped for the console player, and `P_DropWeapon` is called.
///
/// # Safety
///
/// `target` must be a valid, non-null pointer to a live `mobj_t`.
/// `source` may be null (environmental kill).  All global game-state
/// statics must only be accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_KillMobj(source: *mut mobj_t, target: *mut mobj_t)
{
    let info = (*target).info as *mut MobjInfo;
    (*target).flags &= !(MF_SHOOTABLE | MF_FLOAT | MF_SKULLFLY);
    if(*target).mobjtype != MT_SKULL { (*target).flags &= !MF_NOGRAVITY; }
    (*target).flags |= MF_CORPSE | MF_DROPOFF;
    (*target).height >>= 2;

    if !source.is_null() && !(*source).player.is_null()
    {
        let source_player = (*source).player as *mut PlayerT;
        if(*target).flags & MF_COUNTKILL != 0 { (*source_player).killcount += 1; }
        if !(*target).player.is_null()
        {
            let target_player = (*target).player as *mut PlayerT;
            let idx = target_player.offset_from(std::ptr::addr_of_mut!(players[0])) as usize;
            (*source_player).frags[idx] += 1;
        }
    }
    else if netgame == 0 && (*target).flags & MF_COUNTKILL != 0 { players[0].killcount += 1; }

    if !(*target).player.is_null()
    {
        let target_player = (*target).player as *mut PlayerT;
        if source.is_null()
        {
            let idx = target_player.offset_from(std::ptr::addr_of_mut!(players[0])) as usize;
            (*target_player).frags[idx] += 1;
        }
        (*target).flags &= !MF_SOLID;
        (*target_player).playerstate = 1; // PST_DEAD
        P_DropWeapon(target_player);
        if std::ptr::eq(
            target_player,
            std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize),
        ) && automapactive != 0
        {
            AM_Stop();
        }
    }

    if(*target).health < -(*info).spawnhealth && (*info).xdeathstate != 0 { P_SetMobjState(target, (*info).xdeathstate); }
    else { P_SetMobjState(target, (*info).deathstate); }
    (*target).tics = dtmc::death_tic_roll((*target).tics, P_Random());

    if gameversion == exe_chex { return; }

    let item: c_int = match (*target).mobjtype
    {
        MT_WOLFSS | MT_POSSESSED => MT_CLIP,
        MT_SHOTGUY => MT_SHOTGUN,
        MT_CHAINGUY => MT_CHAINGUN,
        _ => return,
    };

    let mo = P_SpawnMobj((*target).x, (*target).y, ONFLOORZ, item);
    (*mo).flags |= MF_DROPPED;
}

// ---------------------------------------------------------------------------
// P_DamageMobj
// ---------------------------------------------------------------------------

/// Apply `damage` points to map object `target`.
///
/// `inflictor` is the projectile or object that physically caused the
/// damage (used to compute knockback direction); it may be null for
/// environmental damage such as slime floors or barrel explosions.
/// `source` is the actor to blame for the damage and to set as
/// `target.target`; it may also be null.  `source` and `inflictor` are
/// the same for hitscan and melee attacks.
///
/// Behavior summary:
/// - On skill `sk_baby`, player damage is halved.
/// - Knockback thrust is applied unless the source is using the chainsaw or
///   the target has `MF_NOCLIP`.  A random forward-fall is possible when the
///   target is damaged from below and has low remaining health.
/// - In the end-of-game hell sector (special 11) damage is capped so the
///   player cannot be killed.
/// - `CF_GODMODE` and the invulnerability power-up block damage below 1000.
/// - Green armor absorbs 1/3 of damage; blue armor absorbs 1/2.  Armor is
///   consumed when points are exhausted.
/// - If health drops to zero `P_KillMobj` is called.
/// - On surviving hits a pain state may be entered and the monster's target
///   is updated to `source`.
///
/// # Safety
///
/// `target` must be a valid, non-null pointer to a live `mobj_t`.
/// `inflictor` and `source` may be null.  If `target.player` is non-null
/// it must point to a valid `PlayerT`.  All global game-state statics must
/// only be accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_DamageMobj(
    target: *mut mobj_t,
    inflictor: *mut mobj_t,
    source: *mut mobj_t,
    mut damage: c_int,
)
{
    if((*target).flags & MF_SHOOTABLE) == 0 { return; }
    if(*target).health <= 0 { return; }
    if ((*target).flags & MF_SKULLFLY) != 0
    {
        (*target).momx = 0;
        (*target).momy = 0;
        (*target).momz = 0;
    }

    let player = (*target).player as *mut PlayerT;
    if !player.is_null() && gameskill == sk_baby { damage >>= 1; }

    if !inflictor.is_null()
        && ((*target).flags & MF_NOCLIP) == 0
        && (source.is_null()
            || (*source).player.is_null()
            || (*((*source).player as *mut PlayerT)).readyweapon != wp_chainsaw)
    {
        let mut ang = R_PointToAngle2((*inflictor).x, (*inflictor).y, (*target).x, (*target).y);
        let info = (*target).info as *mut MobjInfo;
        let mut thrust = dtmc::thrust_for(damage, (*info).mass);

        //* Short-circuit order is load-bearing: the three cheap arms
        //* stay ahead of the call so the `P_Random()` argument is only
        //* DRAWN when the original guard would draw (the helper
        //* re-verifies all four arms -- the no-draw property is pinned
        //* by the `baseline_damage_fall_forward_flip_draw_pins`
        //* vectors in `dtmc.rs`).
        if damage < 40
            && damage > (*target).health
            && (*target).z - (*inflictor).z > 64 * FRACUNIT
            && dtmc::fall_forward_flip(
                damage,
                (*target).health,
                (*target).z - (*inflictor).z,
                P_Random(),
            )
        {
            ang = ang.wrapping_add(ANG180);
            thrust *= 4;
        }

        ang >>= ANGLETOFINESHIFT;
        (*target).momx += FixedMul(thrust, *finecosine.0.add(ang as usize));
        (*target).momy += FixedMul(thrust, finesine[ang as usize]);
    }

    if !player.is_null()
    {
        // end of game hell hack
        if(*(*(*target).subsector).sector).special == 11 && damage >= (*target).health { damage = (*target).health - 1; }

        if damage < 1000
            && (((*player).cheats & CF_GODMODE) != 0 || (*player).powers[pw_invulnerability] != 0)
        {
            return;
        }

        if (*player).armortype != 0
        {
            let (damage_after, armorpoints_after, armortype_after) =
                dtmc::armor_absorption(damage, (*player).armortype, (*player).armorpoints);
            (*player).armorpoints = armorpoints_after;
            (*player).armortype = armortype_after;
            damage = damage_after;
        }
        (*player).health -= damage;
        if(*player).health < 0 { (*player).health = 0; }
        (*player).attacker = source as *mut crate::doom::d_player::mobj_t;
        (*player).damagecount += damage;
        if(*player).damagecount > 100 { (*player).damagecount = 100; }
        let temp = if damage < 100 { damage } else { 100 };
        if std::ptr::eq(
            player,
            std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize),
        )
        {
            I_Tactile(40, 10, 40 + temp * 2);
        }
    }

    (*target).health -= damage;
    if (*target).health <= 0
    {
        P_KillMobj(source, target);
        return;
    }

    let info = (*target).info as *mut MobjInfo;
    if P_Random() < (*info).painchance && ((*target).flags & MF_SKULLFLY) == 0
    {
        (*target).flags |= MF_JUSTHIT;
        P_SetMobjState(target, (*info).painstate);
    }

    (*target).reactiontime = 0;

    if ((*target).threshold == 0 || (*target).mobjtype == MT_VILE)
        && !source.is_null()
        && source != target
        && (*source).mobjtype != MT_VILE
    {
        (*target).target = source;
        (*target).threshold = BASETHRESHOLD;
        let state_ptr = (*target).state as *mut State;
        let spawnstate_ptr = &info::states[(*info).spawnstate as usize] as *const State;
        if std::ptr::eq(state_ptr, spawnstate_ptr) && (*info).seestate != S_NULL { P_SetMobjState(target, (*info).seestate); }
    }
}
