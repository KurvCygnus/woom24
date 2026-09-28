//! Player view height: `P_CalcHeight`, the per-tic view-bob
//! computation that stores the momentum amplitude into `player.bob`
//! (via the extracted `dtmc::bob_amplitude`), advances the
//! `viewheight` step machine, and writes `viewz` with the sinusoidal
//! bob offset (phase via `dtmc::bob_phase`) -- bit-exact with the
//! `P_CalcHeight` half of `vendor/doomgeneric/p_user.c`.

#![allow(non_snake_case)]

use super::dtmc::{bob_amplitude, bob_phase};
use super::state::{onground, PST_LIVE, VIEWHEIGHT};
use crate::doom::d_player::{PlayerT, CF_NOMOMENTUM};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::leveltime;
use crate::doom::tables::finesine;

/// Compute and set the player's view height (`viewz`) for the current tic.
///
/// Calculates the view-bob amplitude from the player's momentum, applies a
/// sinusoidal bob offset (unless `CF_NOMOMENTUM` is set or the player is
/// airborne), and clamps `viewz` to avoid clipping through the ceiling.
/// Also advances `viewheight` toward `VIEWHEIGHT` via `deltaviewheight`.
///
/// Corresponds to `P_CalcHeight` in `p_user.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to an initialised `PlayerT`
/// whose `mo` field points to a valid `mobj_t`.
#[no_mangle]
pub extern "C" fn P_CalcHeight(player: *mut PlayerT)
{
    unsafe
    {
        let mo = (*player).mo as *mut mobj_t;

        // Regular movement bobbing
        (*player).bob = bob_amplitude((*mo).momx, (*mo).momy);

        if ((*player).cheats & CF_NOMOMENTUM) != 0 || onground == 0
        {
            (*player).viewz = (*mo).z + VIEWHEIGHT;

            if(*player).viewz > (*mo).ceilingz - 4 * FRACUNIT { (*player).viewz = (*mo).ceilingz - 4 * FRACUNIT; }

            // vanilla-faithful double write: this OVERWRITES the clamped
            // `z + VIEWHEIGHT` above, so the final viewz is always
            // `z + viewheight` on this branch (`p_user.c:92-97`). Carry
            // verbatim; never "simplify".
            (*player).viewz = (*mo).z + (*player).viewheight;
            return;
        }

        let angle: usize = bob_phase(leveltime);
        let bob: fixed_t = FixedMul((*player).bob / 2, finesine[angle]);

        // Move viewheight
        if (*player).playerstate == PST_LIVE
        {
            (*player).viewheight += (*player).deltaviewheight;

            if (*player).viewheight > VIEWHEIGHT
            {
                (*player).viewheight = VIEWHEIGHT;
                (*player).deltaviewheight = 0;
            }

            if (*player).viewheight < VIEWHEIGHT / 2
            {
                (*player).viewheight = VIEWHEIGHT / 2;
                if(*player).deltaviewheight <= 0 { (*player).deltaviewheight = 1; }
            }

            if (*player).deltaviewheight != 0
            {
                (*player).deltaviewheight += FRACUNIT / 4;
                if(*player).deltaviewheight == 0 { (*player).deltaviewheight = 1; }
            }
        }
        (*player).viewz = (*mo).z + (*player).viewheight + bob;

        if(*player).viewz > (*mo).ceilingz - 4 * FRACUNIT { (*player).viewz = (*mo).ceilingz - 4 * FRACUNIT; }
    }
}
