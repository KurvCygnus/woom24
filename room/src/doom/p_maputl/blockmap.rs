//! Blockmap cell iteration: `P_BlockLinesIterator` and
//! `P_BlockThingsIterator` -- bit-exact with the blockmap half of
//! `vendor/doomgeneric/p_maputl.c`.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_uint};

use crate::doom::c_ffi::{line_t, mobj_t};

/// Iterate all linedefs in blockmap cell `(x, y)`, calling `func` for each.
///
/// Returns 1 (true) if `func` returned non-zero for every line, 0 (false) if
/// `func` returned 0 for any line (early-out). Lines marked with the current
/// `validcount` are skipped to avoid processing the same line twice when it
/// spans multiple blockmap cells.
///
/// Out-of-bounds cell coordinates return 1 without calling `func`.
///
/// Matches `P_BlockLinesIterator` in `p_maputl.c`.
///
/// # Safety
/// The global blockmap arrays from `p_setup` must be initialised. `func` must
/// be a valid function pointer; it will be called with non-null `line_t`
/// pointers from the loaded map data.
#[no_mangle]
pub extern "C" fn P_BlockLinesIterator(
    x: c_int,
    y: c_int,
    func: Option<unsafe extern "C" fn(*mut line_t) -> c_uint>,
) -> c_uint {
    unsafe {
        if x < 0
            || y < 0
            || x >= crate::doom::p_setup::bmapwidth
            || y >= crate::doom::p_setup::bmapheight
        {
            return 1;
        }
        let offset = (y * crate::doom::p_setup::bmapwidth + x) as isize;
        let offset = *crate::doom::p_setup::blockmap.offset(offset) as isize;
        let mut list = crate::doom::p_setup::blockmaplump.offset(offset);
        while *list != -1 {
            let ld = crate::doom::p_setup::lines.offset(*list as isize);
            if (*ld).validcount == crate::doom::r_main::validcount {
                list = list.offset(1);
                continue;
            }
            (*ld).validcount = crate::doom::r_main::validcount;
            if func.unwrap()(ld) == 0 {
                return 0;
            }
            list = list.offset(1);
        }
        1
    }
}

/// Iterate all map objects in blockmap cell `(x, y)`, calling `func` for each.
///
/// Returns 1 (true) if `func` returned non-zero for every thing, 0 (false)
/// if `func` returned 0 for any thing (early-out). Out-of-bounds cell
/// coordinates return 1 without calling `func`.
///
/// Matches `P_BlockThingsIterator` in `p_maputl.c`.
///
/// # Safety
/// The global `blocklinks` array from `p_setup` must be initialised. `func`
/// must be a valid function pointer; it will be called with non-null `mobj_t`
/// pointers from the live thing-list.
#[no_mangle]
pub extern "C" fn P_BlockThingsIterator(
    x: c_int,
    y: c_int,
    func: Option<unsafe extern "C" fn(*mut mobj_t) -> c_uint>,
) -> c_uint {
    unsafe {
        if x < 0
            || y < 0
            || x >= crate::doom::p_setup::bmapwidth
            || y >= crate::doom::p_setup::bmapheight
        {
            return 1;
        }
        let idx = (y * crate::doom::p_setup::bmapwidth + x) as isize;
        let mut mobj = *crate::doom::p_setup::blocklinks.offset(idx) as *mut mobj_t;
        while !mobj.is_null() {
            if func.unwrap()(mobj) == 0 {
                return 0;
            }
            mobj = (*mobj).bnext as *mut mobj_t;
        }
        1
    }
}
