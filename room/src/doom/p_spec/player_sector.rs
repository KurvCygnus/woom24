//! The per-tic environmental damage a player standing in a special
//! sector takes: `player_in_special_sector` -- bit-exact with the
//! `P_PlayerInSpecialSector` function of `vendor/doomgeneric/p_spec.c`.

use std::ffi::c_int;
use std::ptr;

use crate::doom::c_ffi::{mobj_t, sector_t, subsector_t};
use crate::doom::d_player::{PlayerT, CF_GODMODE};
use crate::doom::g_game::G_ExitLevel;
use crate::i_error;
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_tick::leveltime;

use super::consts::pw_ironfeet;

/// `mobj_t` as seen by `p_telept` - `#[repr(C)]` layout identical to `c_ffi::mobj_t`.
type TeleptMobj = crate::doom::p_telept::mobj_t;

/// Applies the environmental effect of a special sector to the player each tic.
///
/// Mirrors `P_PlayerInSpecialSector` in `p_spec.c`; called per-tic from
/// p_user's `P_PlayerThink`.
///
/// ## Technical Details
///
/// The `leveltime & 0x1f` damage cadence and the `P_Random() < 5`
/// radiation-suit bypass draw are the demo surface: the draw happens only
/// for specials 4/16, only when a suit is worn, and only after the suit
/// check short-circuits false -- its exact position in the per-tic RNG
/// stream is pinned by the F9 goldens. Handled specials: 5 (Hellslime,
/// 10 hp / 32 tics), 7 (Nukage, 5 hp / 32 tics), 4/16 (Strobe
/// Hurt / Super Hellslime, 20 hp / 32 tics, suit bypassed when the draw
/// is under 5), 9 (Secret: `secretcount` increment then `special = 0`,
/// one-shot), 11 (end-level damage: strips God Mode, 20 hp / 32 tics,
/// exits at health <= 10). Unknown specials `i_error!` like C `I_Error`.
/// Airborne players (`mo->z != floorheight`) return before any effect.
///
/// ## On Calling
///
/// `player` navigates to its sector via `mo->subsector` (the Rust port's
/// documented divergence from the C pointer chain, same result). The
/// `leveltime` gate stays at the call sites -- only the suit-bypass draw
/// is module-visible RNG.
///
/// # Safety
///
/// `player` must be a valid, non-null player whose `mo` is linked into a
/// loaded subsector/sector.
#[doc(alias = "P_PlayerInSpecialSector")]
#[export_name = "P_PlayerInSpecialSector"]
pub unsafe extern "C" fn player_in_special_sector(player: *mut PlayerT)
{
    let mo = (*player).mo as *mut mobj_t;
    let sub = (*mo).subsector as *mut subsector_t;
    let sector = (*sub).sector as *mut sector_t;

    if(*mo).z != (*sector).floorheight { return; }

    match (*sector).special as c_int
    {
        5 =>
        {
            if (*player).powers[pw_ironfeet] == 0 && leveltime & 0x1f == 0
            {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    10,
                );
            }
        }
        7 =>
        {
            if (*player).powers[pw_ironfeet] == 0 && leveltime & 0x1f == 0
            {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    5,
                );
            }
        }
        16 | 4 =>
        {
            if ((*player).powers[pw_ironfeet] == 0 || P_Random() < 5) && leveltime & 0x1f == 0
            {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    20,
                );
            }
        }
        9 =>
        {
            (*player).secretcount += 1;
            (*sector).special = 0;
        }
        11 =>
        {
            (*player).cheats &= !CF_GODMODE;
            if leveltime & 0x1f == 0
            {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    20,
                );
            }
            if(*player).health <= 10 { G_ExitLevel(); }
        }
        _ =>
        {
            i_error!(
                "P_PlayerInSpecialSector: unknown special {}",
                (*sector).special
            );
        }
    }
}
