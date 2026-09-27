//! The teleporter event: `EV_Teleport`, the linedef special that moves a
//! thing to its sector's `MT_TELEPORTMAN` marker -- bit-exact with
//! `vendor/doomgeneric/p_telept.c`.

use std::ffi::c_void;
use std::os::raw::c_int;

use super::types::{line_t, mobj_t, sector_t, EXE_FINAL};
use crate::doom::d_player::PlayerT;
use crate::doom::info::*;
use crate::doom::p_map::P_TeleportMove;
use crate::doom::p_mobj::{P_MobjThinker, P_SpawnMobj};
use crate::doom::p_setup::{numsectors, sectors};
use crate::doom::p_tick::thinkercap;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};
use crate::doom::doomstat::gameversion;

// Type aliases for cross-module pointer casts (#[repr(C)] identical layout).
type CffiMobj = crate::doom::c_ffi::mobj_t;
type CffiSector = crate::doom::c_ffi::sector_t;
// We already have PlayerT in d_player.rs but can't use it here
// because the C player_s is different. Use pointer casts.

/// Teleport `thing` across the linedef special `line` if all conditions are met.
///
/// Searches all sectors whose tag matches `line->tag` for an `MT_TELEPORTMAN`
/// marker.  When one is found:
///
/// 1. Calls `P_TeleportMove` to clip-move `thing` to the marker's position,
///    telefragging any blocking enemies.
/// 2. Sets `thing->z` to floor height (skipped for the first Final Doom
///    executable — `gameversion == EXE_FINAL` (7); see `doomstat.h`).
/// 3. Adjusts the player's `viewz` if `thing` is a player.
/// 4. Spawns `MT_TFOG` at both the source and destination, playing
///    `sfx_telept` at each.
/// 5. Sets `reactiontime = 18` to freeze player movement briefly.
/// 6. Zeros `thing`'s momentum and copies the marker's angle.
///
/// Returns `1` on a successful teleport, `0` if no suitable destination was
/// found or if `P_TeleportMove` failed.  Missiles (`MF_MISSILE`) and things
/// that hit the back of the line (`side == 1`) are rejected immediately.
///
/// Corresponds to `EV_Teleport` in `p_telept.c`.
///
/// # Safety
///
/// `line` and `thing` must be valid, non-null pointers for the duration of
/// the call.  The global arrays `sectors` and `thinkercap` must be initialised
/// (i.e. a level must be loaded).
#[no_mangle]
pub extern "C" fn EV_Teleport(line: *mut line_t, side: c_int, thing: *mut mobj_t) -> c_int
{
    unsafe
    {
        // Don't teleport missiles
        if (*thing).flags & MF_MISSILE != 0
        {
            return 0;
        }

        // Don't teleport if hit back of line
        if side == 1
        {
            return 0;
        }

        let tag = (*line).tag;

        for i in 0..numsectors as usize
        {
            if (*sectors.add(i)).tag != tag
            {
                continue;
            }

            let mut thinker = thinkercap.next;
            while !std::ptr::eq(thinker, &raw const thinkercap)
            {
                // Not a mobj
                if (*thinker).function.acp1
                    != Some(core::mem::transmute::<
                        unsafe extern "C" fn(*mut mobj_t),
                        unsafe extern "C" fn(*mut c_void),
                    >(P_MobjThinker))
                {
                    thinker = (*thinker).next;
                    continue;
                }

                let m = thinker as *mut mobj_t;

                // Not a teleportman
                if (*m).mobjtype != MT_TELEPORTMAN
                {
                    thinker = (*thinker).next;
                    continue;
                }

                let sector = (*(*m).subsector).sector;
                // Wrong sector
                if sector.offset_from(sectors as *mut sector_t) != i as isize
                {
                    thinker = (*thinker).next;
                    continue;
                }

                let oldx = (*thing).x;
                let oldy = (*thing).y;
                let oldz = (*thing).z;

                if P_TeleportMove(thing as *mut CffiMobj, (*m).x, (*m).y) == 0
                {
                    return 0;
                }

                // Final Doom quirk: don't set z
                if gameversion != EXE_FINAL
                {
                    (*thing).z = (*thing).floorz;
                }

                if !(*thing).player.is_null()
                {
                    let player = (*thing).player as *mut PlayerT;
                    (*player).viewz = (*thing).z + (*player).viewheight;
                }

                // Spawn teleport fog at source
                let fog = P_SpawnMobj(oldx, oldy, oldz, MT_TFOG);
                S_StartSound(fog as *mut c_void, Sfx::Telept as c_int);

                // Spawn teleport fog at destination
                let an = ((*m).angle >> ANGLETOFINESHIFT) as usize;
                let fog = P_SpawnMobj(
                    (*m).x + 20 * *finecosine.0.add(an),
                    (*m).y + 20 * finesine[an],
                    (*thing).z,
                    MT_TFOG,
                );
                S_StartSound(fog as *mut c_void, Sfx::Telept as c_int);

                // Don't move for a bit
                if !(*thing).player.is_null()
                {
                    (*thing).reactiontime = 18;
                }

                (*thing).angle = (*m).angle;
                (*thing).momx = 0;
                (*thing).momy = 0;
                (*thing).momz = 0;
                return 1;
            }
        }
    }
    0
}
