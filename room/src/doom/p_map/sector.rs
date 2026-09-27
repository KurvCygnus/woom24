//! Sector height-change propagation: the crush-state statics and the
//! blockmap sweep that gibs, drops, or crushes things which no longer fit
//! after a floor/ceiling move -- bit-exact with the sector-changing half
//! of `vendor/doomgeneric/p_map.c`.

use std::ffi::{c_int, c_uint};
use std::ptr;

use crate::doom::c_ffi::{mobj_t, sector_t};
use crate::doom::m_bbox::BBox;
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_maputl::P_BlockThingsIterator;
use crate::doom::p_mobj::{P_RemoveMobj, P_SetMobjState, P_SpawnMobj};
use crate::doom::p_tick::leveltime;

use super::consts::{MF_DROPPED, MF_SHOOTABLE, MF_SOLID, MT_BLOOD, S_GIBS};
use super::move_::thing_height_clip;
use super::state::TeleptMobj;

/// Non-zero when things that do not fit during a sector-height change should
/// take crush damage (10 HP every 4 tics).
///
/// Set by `change_sector` from the `crunch` parameter.
#[no_mangle]
pub static mut crushchange: c_int = 0; // boolean

/// Set to non-zero by `pit_change_sector` if any thing no longer fits
/// after a sector-height change.
///
/// Returned by `change_sector`; a truthy value tells the caller to
/// either continue crushing or revert the sector height.
#[no_mangle]
pub static mut nofit: c_int = 0; // boolean

/// Blockmap thing iterator callback for `change_sector`.
///
/// Calls `thing_height_clip` on each thing. If the thing fits, returns 1.
/// If it does not fit: dead things are gibbed, dropped items are removed,
/// non-shootable things are ignored (assumed decorative), and live shootable
/// things set `nofit` and optionally receive crush damage with a blood
/// spray every 4 tics.
///
/// Always returns 1 to continue checking other things.
///
/// ## Technical Details
///
/// The gib/drop/crush ladder is decision-order demo surface, and the
/// blood-spray momentum pair `(P_Random() - P_Random()) << 12` is two
/// RNG draws whose COUNT and ORDER are pinned by the F9 goldens (the
/// `leveltime & 3` gate decides only whether the pair fires). The
/// `crushchange`/`nofit` statics are also the spechit overrun emulation's
/// write targets 13/14 (catalog entry 1 / G1) -- a trampled value here is
/// demo-observable by design.
///
/// ## On Calling
///
/// C ABI callback passed to `P_BlockThingsIterator` by `change_sector`
/// (retained via the `PIT_ChangeSector` export pin); never call it
/// directly. `thing` must be a valid, non-null pointer to an initialised
/// `mobj_t`; `crushchange` must be set by `change_sector` before
/// iteration. Single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and global crush-state mutation; see On
/// Calling.
#[doc(alias = "PIT_ChangeSector")]
#[export_name = "PIT_ChangeSector"]
pub unsafe extern "C" fn pit_change_sector(thing: *mut mobj_t) -> c_uint
{
    if thing_height_clip(thing) != 0
    {
        return 1;
    }
    let thing = &mut *thing;
    if thing.health <= 0
    {
        P_SetMobjState(thing as *const _ as *mut TeleptMobj, S_GIBS);
        thing.flags &= !MF_SOLID;
        thing.height = 0;
        thing.radius = 0;
        return 1;
    }
    if thing.flags & MF_DROPPED != 0
    {
        P_RemoveMobj(thing as *const _ as *mut TeleptMobj);
        return 1;
    }
    if thing.flags & MF_SHOOTABLE == 0
    {
        return 1;
    }
    nofit = 1;
    if crushchange != 0 && leveltime & 3 == 0
    {
        P_DamageMobj(
            thing as *const _ as *mut TeleptMobj,
            ptr::null_mut(),
            ptr::null_mut(),
            10,
        );
        let mo = P_SpawnMobj(thing.x, thing.y, thing.z + thing.height / 2, MT_BLOOD);
        (*mo).momx = (P_Random() - P_Random()) << 12;
        (*mo).momy = (P_Random() - P_Random()) << 12;
    }
    1
}

/// Propagate a floor or ceiling height change in `sector` to all nearby things.
///
/// Re-checks height constraints for every thing in the blockmap cells that
/// overlap `sector->blockbox`. Returns non-zero if any thing no longer fits
/// (i.e. `nofit` was set). If `crunch` is zero and this returns non-zero,
/// the caller should revert the sector height and call this again.
///
/// ## Technical Details
///
/// The `nofit = 0` / `crushchange = crunch` reset order and the
/// x-outer/y-inner `blockbox` sweep order decide the visit order and the
/// return value the mover logic (p_floor/p_ceilng/p_plats/p_doors) acts
/// on; the return feeds their revert/retry loop, so it is
/// movement-visible.
///
/// ## On Calling
///
/// `sector` must be a valid, non-null pointer to an initialised
/// `sector_t` whose `blockbox` indices are within the blockmap bounds.
/// Consumed by the shared mover logic through the `P_ChangeSector`
/// root shim. Single-threaded sim only.
///
/// # Safety
///
/// Raw `sector_t` / `mobj_t` dereferences; see On Calling.
#[doc(alias = "P_ChangeSector")]
#[export_name = "P_ChangeSector"]
pub unsafe extern "C" fn change_sector(sector: *mut sector_t, crunch: c_int) -> c_int
{
    nofit = 0;
    crushchange = crunch;
    let sec = &*sector;
    for x in sec.blockbox[BBox::LEFT]..=sec.blockbox[BBox::RIGHT]
    {
        for y in sec.blockbox[BBox::BOTTOM]..=sec.blockbox[BBox::TOP]
        {
            P_BlockThingsIterator(x, y, Some(pit_change_sector));
        }
    }
    nofit
}
