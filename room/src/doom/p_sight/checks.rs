//! The line-of-sight entry point: `P_CheckSight` with its REJECT-table
//! fast path -- bit-exact with `P_CheckSight` in
//! `vendor/doomgeneric/p_sight.c`.

#![allow(non_snake_case)]

use std::os::raw::c_int;

use crate::doom::p_setup::{numnodes, numsectors, rejectmatrix, sectors as p_setup_sectors};
use crate::doom::r_main::validcount;

use super::bsp::P_CrossBSPNode;
use super::state::{bottomslope, sightcounts, sightzstart, strace, t2x, t2y, topslope};
use super::types::{mobj_t, sector_t};

/// Test whether there is an unobstructed line of sight between two map objects.
///
/// Performs a two-stage check:
/// 1. REJECT table lookup: if the sector pair is flagged as mutually invisible,
///    return 0 immediately (no BSP traversal needed).
/// 2. Full BSP traversal via `P_CrossBSPNode`, narrowing `topslope` /
///    `bottomslope` at each two-sided portal until the ray is either confirmed
///    clear or blocked.
///
/// Returns a C `boolean` (`c_int`, non-zero = visible) rather than Rust `bool`
/// because Doom's `typedef unsigned int boolean` makes the C caller read a
/// 32-bit value. Returning `bool` (1 byte) would leave the upper 24 bits
/// undefined and cause sight checks to randomly succeed or fail, desynchronising
/// demos and RNG.
///
/// Matches `P_CheckSight` in `p_sight.c`.
///
/// # Safety
/// Both `t1` and `t2` must be valid, non-null pointers to initialised
/// `mobj_t` instances. Their `subsector` fields must point into the live
/// subsector/sector arrays loaded by `p_setup`.
#[no_mangle]
pub extern "C" fn P_CheckSight(t1: *mut mobj_t, t2: *mut mobj_t) -> c_int
{
    unsafe
    {
        // Cast both pointers through *const u8 to sidestep the fact that
        // mobj_t (from p_telept) references a different sector_t type than
        // the one declared here. Sector size is the same in both.
        let sec_size = std::mem::size_of::<sector_t>() as isize;
        let s1 = (((*(*t1).subsector).sector as *const u8)
            .offset_from(p_setup_sectors as *const u8)
            / sec_size) as c_int;
        let s2 = (((*(*t2).subsector).sector as *const u8)
            .offset_from(p_setup_sectors as *const u8)
            / sec_size) as c_int;
        let pnum = s1 * numsectors + s2;
        let bytenum = pnum >> 3;
        let bitnum = 1 << (pnum & 7);

        // Check in REJECT table.
        if *rejectmatrix.add(bytenum as usize) & bitnum != 0
        {
            sightcounts[0] += 1;
            return 0;
        }

        sightcounts[1] += 1;
        validcount += 1;

        sightzstart = (*t1).z + (*t1).height - ((*t1).height >> 2);
        topslope = (*t2).z + (*t2).height - sightzstart;
        bottomslope = (*t2).z - sightzstart;

        strace.x = (*t1).x;
        strace.y = (*t1).y;
        t2x = (*t2).x;
        t2y = (*t2).y;
        strace.dx = (*t2).x - (*t1).x;
        strace.dy = (*t2).y - (*t1).y;

        P_CrossBSPNode(numnodes - 1) as c_int
    }
}
