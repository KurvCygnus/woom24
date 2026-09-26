//! Player movement and the death camera: `P_Thrust` (fine-angle
//! momentum impulse), `P_MovePlayer` (per-tic command application and
//! the `onground` write), and `P_DeathThink` (death-camera turn and
//! respawn gate) -- bit-exact with the movement half of
//! `vendor/doomgeneric/p_user.c`.

#![allow(non_snake_case)]

use std::ffi::c_int;

use super::state::{onground, ANG5, BT_USE, PST_REBORN};
use super::view::P_CalcHeight;
use crate::doom::d_player::PlayerT;
use crate::doom::info::{states, S_PLAY, S_PLAY_RUN1};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_pspr::P_MovePsprites;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::tables::{finecosine, finesine, ANG180, ANG90, ANGLETOFINESHIFT};

/// Apply a momentum impulse to the player's map object along `angle`.
///
/// Converts `angle` (binary angle) to a fine-angle table index, then adds
/// `move_ * cos(angle)` to `momx` and `move_ * sin(angle)` to `momy`.
/// Corresponds to `P_Thrust` in `p_user.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub extern "C" fn P_Thrust(player: *mut PlayerT, angle: u32, move_: fixed_t)
{
    unsafe
    {
        let mo = (*player).mo as *mut mobj_t;
        let angle = (angle >> ANGLETOFINESHIFT) as usize;
        (*mo).momx += FixedMul(move_, *finecosine.0.add(angle));
        (*mo).momy += FixedMul(move_, finesine[angle]);
    }
}

/// Apply the player's movement command for one tic.
///
/// Rotates `mo->angle` by `cmd.angleturn`, sets `onground`, and calls
/// `P_Thrust` for forward and side movement when the player is on the ground.
/// Also transitions the player mobj to the `S_PLAY_RUN1` state when moving.
///
/// Corresponds to `P_MovePlayer` in `p_user.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub extern "C" fn P_MovePlayer(player: *mut PlayerT)
{
    unsafe
    {
        let mo = (*player).mo as *mut mobj_t;
        let cmd = &(*player).cmd;

        (*mo).angle = (*mo)
            .angle
            .wrapping_add(((cmd.angleturn) as u32).wrapping_shl(16));

        // Do not let the player control movement if not onground.
        onground = ((*mo).z <= (*mo).floorz) as c_int;

        if cmd.forwardmove != 0 && onground != 0
        {
            P_Thrust(player, (*mo).angle, cmd.forwardmove as fixed_t * 2048);
        }

        if cmd.sidemove != 0 && onground != 0
        {
            P_Thrust(
                player,
                (*mo).angle.wrapping_sub(ANG90),
                cmd.sidemove as fixed_t * 2048,
            );
        }

        // C: player->mo->state == &states[S_PLAY]
        let p_play = std::ptr::addr_of!(states[S_PLAY as usize]) as *const u8;
        if (cmd.forwardmove != 0 || cmd.sidemove != 0) && (*mo).state as *const u8 == p_play
        {
            P_SetMobjState(mo, S_PLAY_RUN1);
        }
    }
}

/// Per-tic camera and respawn logic for a dead player.
///
/// Drops `viewheight` to floor level, calls `P_CalcHeight`, and slowly
/// rotates the camera toward the attacker (if one exists). Pressing the Use
/// key transitions the player to `PST_REBORN`.
///
/// Corresponds to `P_DeathThink` in `p_user.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub extern "C" fn P_DeathThink(player: *mut PlayerT)
{
    unsafe
    {
        let mo = (*player).mo as *mut mobj_t;
        let angle: u32;
        let delta: u32;

        P_MovePsprites(player);

        // Fall to the ground
        if (*player).viewheight > 6 * FRACUNIT
        {
            (*player).viewheight -= FRACUNIT;
        }

        if (*player).viewheight < 6 * FRACUNIT
        {
            (*player).viewheight = 6 * FRACUNIT;
        }

        (*player).deltaviewheight = 0;
        onground = ((*mo).z <= (*mo).floorz) as c_int;
        P_CalcHeight(player);

        if !(*player).attacker.is_null() && (*player).attacker != (*player).mo
        {
            let atk = (*player).attacker as *mut mobj_t;
            angle = R_PointToAngle2((*mo).x, (*mo).y, (*atk).x, (*atk).y);

            delta = angle.wrapping_sub((*mo).angle);

            // `delta > (!ANG5).wrapping_add(1)` is the C `(unsigned)-ANG5`
            // unsigned-negation idiom: any rewrite to signed arithmetic
            // breaks the wrap.
            if delta < ANG5 || delta > (!ANG5).wrapping_add(1)
            {
                // Looking at killer, so fade damage flash down.
                (*mo).angle = angle;

                if (*player).damagecount != 0
                {
                    (*player).damagecount -= 1;
                }
            }
            else if delta < ANG180
            {
                (*mo).angle = (*mo).angle.wrapping_add(ANG5);
            }
            else
            {
                (*mo).angle = (*mo).angle.wrapping_sub(ANG5);
            }
        }
        else if (*player).damagecount != 0
        {
            (*player).damagecount -= 1;
        }

        if (*player).cmd.buttons & BT_USE != 0
        {
            (*player).playerstate = PST_REBORN;
        }
    }
}
