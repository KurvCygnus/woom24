//! The player-side sim orchestrator: `P_PlayerThink`, the once-per-tic
//! per-player thinker that sequences cheat-flag propagation, the
//! chainsaw run override, death dispatch, movement, view height,
//! special sectors, weapon switching, use-lines, psprite animation,
//! power timers, and colormap selection -- bit-exact with the
//! `P_PlayerThink` half of `vendor/doomgeneric/p_user.c`.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};

use super::movement::{P_DeathThink, P_MovePlayer};
use super::state::{
    pw_infrared, pw_invulnerability, pw_ironfeet, pw_strength, pw_invisibility, wp_bfg, wp_chainsaw,
    wp_fist, wp_plasma, wp_shotgun, wp_supershotgun, BT_CHANGE, BT_SPECIAL, BT_USE, BT_WEAPONMASK,
    BT_WEAPONSHIFT, INVERSECOLORMAP, PST_DEAD,
};
use super::view::P_CalcHeight;
use crate::doom::d_mode::{commercial, shareware};
use crate::doom::d_player::{PlayerT, CF_NOCLIP};
use crate::doom::doomstat::gamemode;
use crate::doom::info::{MF_JUSTATTACKED, MF_NOCLIP, MF_SHADOW};
use crate::doom::p_map::P_UseLines;
use crate::doom::p_pspr::P_MovePsprites;
use crate::doom::p_spec::P_PlayerInSpecialSector;
use crate::doom::p_telept::mobj_t;

/// Main per-tic player thinker.
///
/// Called once per game tic for each player. Handles (in order):
/// - `CF_NOCLIP` cheat flag propagation to the mobj,
/// - chainsaw auto-run override (`MF_JUSTATTACKED`),
/// - death dispatch to `P_DeathThink`,
/// - movement via `P_MovePlayer` (suppressed during `reactiontime` after a
///   teleport),
/// - view height via `P_CalcHeight`,
/// - special sector effects via `P_PlayerInSpecialSector`,
/// - weapon switching (with shareware plasma/BFG guard and super-shotgun
///   upgrade logic),
/// - Use-key processing via `P_UseLines`,
/// - weapon sprite animation via `P_MovePsprites`,
/// - power-up timer countdown, and
/// - colormap selection for invulnerability and infrared.
///
/// Corresponds to `P_PlayerThink` in `p_user.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t` with a valid `subsector`
/// chain.
#[no_mangle]
pub extern "C" fn P_PlayerThink(player: *mut PlayerT)
{
    unsafe
    {
        let mo = (*player).mo as *mut mobj_t;
        let mut newweapon: c_int;

        // Fixme: do this in the cheat code
        if(*player).cheats & CF_NOCLIP != 0 { (*mo).flags |= MF_NOCLIP; }
        else { (*mo).flags &= !MF_NOCLIP; }

        // Chain saw run forward
        let cmd = &mut (*player).cmd;
        if (*mo).flags & MF_JUSTATTACKED != 0
        {
            cmd.angleturn = 0;
            cmd.forwardmove = (0xc800 / 512) as i8;
            cmd.sidemove = 0;
            (*mo).flags &= !MF_JUSTATTACKED;
        }

        if (*player).playerstate == PST_DEAD
        {
            P_DeathThink(player);
            return;
        }

        // Move around.
        // Reactiontime is used to prevent movement for a bit after a teleport.
        if(*mo).reactiontime != 0 { (*mo).reactiontime -= 1; }
        else { P_MovePlayer(player); }

        P_CalcHeight(player);
        if(*(*(*mo).subsector).sector).special != 0 { P_PlayerInSpecialSector(player); }
        // Check for weapon change.
        // A special event has no other buttons.
        if cmd.buttons & BT_SPECIAL != 0 { cmd.buttons = 0; }

        if cmd.buttons & BT_CHANGE != 0
        {
            // The actual changing of the weapon is done
            // when the weapon psprite can do it
            // (read: not in the middle of an attack).
            newweapon = (cmd.buttons as c_int & BT_WEAPONMASK as c_int) >> BT_WEAPONSHIFT;

            if newweapon == wp_fist
                && (*player).weaponowned[wp_chainsaw as usize] != 0
                && !((*player).readyweapon == wp_chainsaw && (*player).powers[pw_strength] != 0)
            {
                newweapon = wp_chainsaw;
            }

            if gamemode == commercial
                && newweapon == wp_shotgun
                && (*player).weaponowned[wp_supershotgun as usize] != 0
                && (*player).readyweapon != wp_supershotgun
            {
                newweapon = wp_supershotgun;
            }

            if (*player).weaponowned[newweapon as usize] != 0 && newweapon != (*player).readyweapon
            {
                // Do not go to plasma or BFG in shareware, even if cheated.
                if(newweapon != wp_plasma && newweapon != wp_bfg) || gamemode != shareware { (*player).pendingweapon = newweapon; }
            }
        }

        // Check for use
        if cmd.buttons & BT_USE != 0
        {
            if (*player).usedown == 0
            {
                P_UseLines(player as *mut c_void);
                (*player).usedown = 1;
            }
        }
        else { (*player).usedown = 0; }

        // Cycle psprites
        P_MovePsprites(player);

        // Counters, time dependent power ups.
        // Strength counts up to diminish fade.
        if(*player).powers[pw_strength] != 0 { (*player).powers[pw_strength] += 1; }

        if(*player).powers[pw_invulnerability] != 0 { (*player).powers[pw_invulnerability] -= 1; }

        if (*player).powers[pw_invisibility] != 0
        {
            (*player).powers[pw_invisibility] -= 1;
            if(*player).powers[pw_invisibility] == 0 { (*mo).flags &= !MF_SHADOW; }
        }

        if(*player).powers[pw_infrared] != 0 { (*player).powers[pw_infrared] -= 1; }

        if(*player).powers[pw_ironfeet] != 0 { (*player).powers[pw_ironfeet] -= 1; }

        if(*player).damagecount != 0 { (*player).damagecount -= 1; }

        if(*player).bonuscount != 0 { (*player).bonuscount -= 1; }

        // Handling colormaps.
        if (*player).powers[pw_invulnerability] != 0
        {
            if (*player).powers[pw_invulnerability] > 4 * 32
                || ((*player).powers[pw_invulnerability] & 8) != 0
            {
                (*player).fixedcolormap = INVERSECOLORMAP;
            }
            else { (*player).fixedcolormap = 0; }
        }
        else if (*player).powers[pw_infrared] != 0
        {
            if (*player).powers[pw_infrared] > 4 * 32 || ((*player).powers[pw_infrared] & 8) != 0
            {
                // Almost full bright
                (*player).fixedcolormap = 1;
            }
            else { (*player).fixedcolormap = 0; }
        }
        else { (*player).fixedcolormap = 0; }
    }
}
