//! Line-opening state and computation: the opening quartet statics
//! (`opentop`, `openbottom`, `openrange`, `lowfloor`) and
//! `P_LineOpening` -- bit-exact with the corresponding half of
//! `vendor/doomgeneric/p_maputl.c`.

#![allow(non_snake_case, non_upper_case_globals)]

use crate::doom::c_ffi::{line_t, sector_t};
use crate::doom::m_fixed::fixed_t;

/// Top of the open vertical range through the last line tested by
/// `P_LineOpening` (fixed-point map units). Read by `p_map.c`.
#[no_mangle]
pub static mut opentop: fixed_t = 0;

/// Bottom of the open vertical range through the last line tested by
/// `P_LineOpening` (fixed-point map units). Read by `p_map.c`.
#[no_mangle]
pub static mut openbottom: fixed_t = 0;

/// Height of the open range (`opentop - openbottom`). Zero means the line is
/// closed (impassable). Read by `p_map.c`.
#[no_mangle]
pub static mut openrange: fixed_t = 0;

/// The lower of the two floor heights on either side of the last line tested
/// by `P_LineOpening`. Used for step-up logic in `p_map.c`.
#[no_mangle]
pub static mut lowfloor: fixed_t = 0;

/// Compute the open vertical portal range for a two-sided linedef.
///
/// Sets the module globals `opentop`, `openbottom`, `openrange`, and
/// `lowfloor` based on the floor/ceiling heights of the front and back sectors.
/// If the linedef has no back side (`sidenum[1] == -1`) then `openrange` is
/// set to 0 (no passage).
///
/// These globals are read by `p_map.c` after each crossing test to determine
/// whether a moving object fits through the gap.
///
/// Matches `P_LineOpening` in `p_maputl.c`.
///
/// # Safety
/// `linedef` must be a valid, non-null pointer to an initialised `line_t`.
/// If the line is two-sided, `frontsector` and `backsector` must also be
/// valid non-null pointers to initialised `sector_t` values.
#[no_mangle]
pub extern "C" fn P_LineOpening(linedef: *mut line_t)
{
    unsafe
    {
        let linedef = &*linedef;
        if linedef.sidenum[1] == -1
        {
            openrange = 0;
            return;
        }
        let front = linedef.frontsector as *mut sector_t;
        let back = linedef.backsector as *mut sector_t;
        if (*front).ceilingheight < (*back).ceilingheight
        {
            opentop = (*front).ceilingheight;
        }
        else
        {
            opentop = (*back).ceilingheight;
        }
        if (*front).floorheight > (*back).floorheight
        {
            openbottom = (*front).floorheight;
            lowfloor = (*back).floorheight;
        }
        else
        {
            openbottom = (*back).floorheight;
            lowfloor = (*front).floorheight;
        }
        openrange = opentop - openbottom;
    }
}
