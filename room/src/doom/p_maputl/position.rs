//! Thing-position management: unlinking and relinking a map object
//! against the sector thing-lists and the blockmap cell lists --
//! bit-exact with the position half of
//! `vendor/doomgeneric/p_maputl.c`.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::ptr;

use crate::doom::c_ffi::{mobj_t, sector_t, subsector_t, MAPBLOCKSHIFT};
use crate::doom::info::{MF_NOBLOCKMAP, MF_NOSECTOR};

/// Unlink a map object from the sector thing-list and the blockmap.
///
/// Removes `thing` from:
/// - the doubly-linked sector thing-list (unless `MF_NOSECTOR` is set), and
/// - the blockmap cell's singly-linked thing-list (unless `MF_NOBLOCKMAP` is
///   set).
///
/// Must be called before moving an mobj; `P_SetThingPosition` re-inserts it
/// at the new location.
///
/// Matches `P_UnsetThingPosition` in `p_maputl.c`.
///
/// # Safety
/// `thing` must be a valid, non-null pointer to an initialised `mobj_t`. All
/// linked-list pointers (`snext`, `sprev`, `bnext`, `bprev`) must be either
/// null or valid `mobj_t` pointers. The global blockmap arrays from
/// `p_setup` must be initialised.
#[no_mangle]
pub extern "C" fn P_UnsetThingPosition(thing: *mut mobj_t)
{
    unsafe
    {
        let thing = &mut *thing;
        if (thing.flags & MF_NOSECTOR) == 0
        {
            if !thing.snext.is_null()
            {
                let snext = thing.snext as *mut mobj_t;
                (*snext).sprev = thing.sprev;
            }
            if !thing.sprev.is_null()
            {
                let sprev = thing.sprev as *mut mobj_t;
                (*sprev).snext = thing.snext;
            }
            else
            {
                let subsector = thing.subsector as *mut subsector_t;
                let sector = (*subsector).sector as *mut sector_t;
                (*sector).thinglist = thing.snext;
            }
        }
        if (thing.flags & MF_NOBLOCKMAP) == 0
        {
            if !thing.bnext.is_null()
            {
                let bnext = thing.bnext as *mut mobj_t;
                (*bnext).bprev = thing.bprev;
            }
            if !thing.bprev.is_null()
            {
                let bprev = thing.bprev as *mut mobj_t;
                (*bprev).bnext = thing.bnext;
            }
            else
            {
                let blockx = (thing.x - crate::doom::p_setup::bmaporgx) >> MAPBLOCKSHIFT;
                let blocky = (thing.y - crate::doom::p_setup::bmaporgy) >> MAPBLOCKSHIFT;
                if blockx >= 0
                    && blockx < crate::doom::p_setup::bmapwidth
                    && blocky >= 0
                    && blocky < crate::doom::p_setup::bmapheight
                {
                    let idx = (blocky * crate::doom::p_setup::bmapwidth + blockx) as isize;
                    *crate::doom::p_setup::blocklinks.offset(idx) = thing.bnext;
                }
            }
        }
    }
}

/// Insert a map object into the sector thing-list and the blockmap.
///
/// Calls `R_PointInSubsector` to determine the correct subsector and sector,
/// then prepends `thing` to:
/// - the sector's doubly-linked thing-list (unless `MF_NOSECTOR` is set), and
/// - the blockmap cell's singly-linked thing-list (unless `MF_NOBLOCKMAP` is
///   set or the thing is outside the map bounds).
///
/// Also sets `thing->subsector` to the computed subsector pointer.
///
/// Matches `P_SetThingPosition` in `p_maputl.c`.
///
/// # Safety
/// `thing` must be a valid, non-null pointer to an initialised `mobj_t`. The
/// global map data (sectors, blockmap) from `p_setup` must be loaded.
#[no_mangle]
pub extern "C" fn P_SetThingPosition(thing: *mut mobj_t)
{
    unsafe
    {
        let thing = &mut *thing;
        let ss = crate::doom::r_main::R_PointInSubsector(thing.x, thing.y) as *mut subsector_t;
        thing.subsector = ss as *mut c_void;
        if (thing.flags & MF_NOSECTOR) == 0
        {
            let sec = (*ss).sector as *mut sector_t;
            thing.sprev = ptr::null_mut();
            thing.snext = (*sec).thinglist;
            if !(*sec).thinglist.is_null()
            {
                let thinglist = (*sec).thinglist as *mut mobj_t;
                (*thinglist).sprev = thing as *mut mobj_t as *mut c_void;
            }
            (*sec).thinglist = thing as *mut mobj_t as *mut c_void;
        }
        if (thing.flags & MF_NOBLOCKMAP) == 0
        {
            let blockx = (thing.x - crate::doom::p_setup::bmaporgx) >> MAPBLOCKSHIFT;
            let blocky = (thing.y - crate::doom::p_setup::bmaporgy) >> MAPBLOCKSHIFT;
            if blockx >= 0
                && blockx < crate::doom::p_setup::bmapwidth
                && blocky >= 0
                && blocky < crate::doom::p_setup::bmapheight
            {
                let idx = (blocky * crate::doom::p_setup::bmapwidth + blockx) as isize;
                let link = crate::doom::p_setup::blocklinks.offset(idx);
                thing.bprev = ptr::null_mut();
                thing.bnext = *link;
                if !(*link).is_null()
                {
                    let l = *link as *mut mobj_t;
                    (*l).bprev = thing as *mut mobj_t as *mut c_void;
                }
                *link = thing as *mut mobj_t as *mut c_void;
            }
            else
            {
                thing.bnext = ptr::null_mut();
                thing.bprev = ptr::null_mut();
            }
        }
    }
}
