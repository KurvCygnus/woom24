//! Map-load special spawning: `spawn_specials`, the once-per-level scan
//! that seeds the sector/line special thinkers, the deathmatch timer, and
//! the switch/plat/ceiling registries -- bit-exact with the
//! `P_SpawnSpecials` function of `vendor/doomgeneric/p_spec.c`.

use std::ffi::c_int;
use std::ptr;

use crate::i_error;
use crate::doom::g_game::{deathmatch, timelimit, totalsecret};
use crate::doom::i_timer::TICRATE;
use crate::doom::p_ceilng::activeceilings;
use crate::doom::p_doors::{P_SpawnDoorCloseIn30, P_SpawnDoorRaiseIn5Mins};
use crate::doom::p_lights::{
    P_SpawnFireFlicker, P_SpawnGlowingLight, P_SpawnLightFlash, P_SpawnStrobeFlash,
};
use crate::doom::p_plats::activeplats;
use crate::doom::p_setup::{lines, numlines, numsectors, sectors};
use crate::doom::p_switch::{buttonlist, MAXBUTTONS};

use super::consts::MAXLINEANIMS;
use super::ticker::{levelTimeCount, levelTimer, linespeciallist, numlinespecials};

/// `sector_t` as seen by `p_lights` - `#[repr(C)]` layout identical to `c_ffi::sector_t`.
type LightsSector = crate::doom::p_lights::sector_t;

/// Scans all sectors and linedefs at map load time and spawns thinkers for specials.
///
/// Mirrors `P_SpawnSpecials` in `p_spec.c`; called once after the map is
/// loaded (from `G_DoLoadLevel` via p_setup).
///
/// ## Technical Details
///
/// The map-load spawn ORDER is the demo-relevant surface (all thinker
/// spawns happen in one sweep, in sector order, before the first tic):
/// 1. **Deathmatch timer**: if `timelimit > 0` and `deathmatch != 0`, sets
///    `levelTimer` and `levelTimeCount` (in tics = `timelimit * 60 * TICRATE`).
/// 2. **Sector specials** (sector order): 1 random light flash, 2 fast
///    strobe, 3 slow strobe, 4 fast strobe with the special re-armed to 4,
///    8 glowing light, 9 secret (`totalsecret` increment), 10 door closes
///    in 30 s, 12 slow sync strobe, 13 fast sync strobe, 14 door raises in
///    5 min, 17 fire flicker.
/// 3. **Line specials**: special-48 (first-column scroll) lines register
///    into `linespeciallist` in line order; more than `MAXLINEANIMS` (64)
///    aborts with `i_error!` (the vanilla limit).
/// 4. **Misc cleanup**: clears `activeceilings`, `activeplats`, and
///    `buttonlist`.
///
/// ## On Calling
///
/// Runs once per level load, after map setup and before the first
/// `P_Ticker`. Not per-tic: its outputs are consumed by
/// `ticker::update_specials` (the statics live in one data home,
/// `ticker.rs`, per the module's Deterministic Aspects note).
///
/// # Safety
///
/// Requires the map arrays (`sectors`, `lines`, counts) to be loaded.
#[doc(alias = "P_SpawnSpecials")]
#[export_name = "P_SpawnSpecials"]
pub unsafe extern "C" fn spawn_specials()
{
    if timelimit > 0 && deathmatch != 0
    {
        levelTimer = 1;
        levelTimeCount = timelimit * 60 * TICRATE;
    }
    else { levelTimer = 0; }

    for i in 0..numsectors
    {
        let sector = sectors.offset(i as isize);
        if(*sector).special == 0 { continue; }
        match (*sector).special as c_int
        {
            1 => P_SpawnLightFlash(sector as *mut LightsSector),
            2 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 0),
            3 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::SLOWDARK, 0),
            4 =>
            {
                P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 0);
                (*sector).special = 4;
            }
            8 => P_SpawnGlowingLight(sector as *mut LightsSector),
            9 => { totalsecret += 1; }
            10 => P_SpawnDoorCloseIn30(sector as *mut LightsSector),
            12 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::SLOWDARK, 1),
            13 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 1),
            14 => P_SpawnDoorRaiseIn5Mins(sector as *mut LightsSector, i),
            17 => P_SpawnFireFlicker(sector as *mut LightsSector),
            _ => {}
        }
    }

    numlinespecials = 0;
    for i in 0..numlines
    {
        if (*lines.offset(i as isize)).special as c_int == 48
        {
            if numlinespecials as c_int >= MAXLINEANIMS as c_int { i_error!("Too many scrolling wall linedefs! (Vanilla limit is 64)"); }
            linespeciallist[numlinespecials as usize] = lines.offset(i as isize);
            numlinespecials += 1;
        }
    }

    for i in 0..crate::doom::c_ffi::MAXCEILINGS as usize { activeceilings[i] = ptr::null_mut(); }
    for i in 0..crate::doom::c_ffi::MAXPLATS as usize { activeplats[i] = ptr::null_mut(); }
    for i in 0..MAXBUTTONS { buttonlist[i] = std::mem::zeroed(); }
}
