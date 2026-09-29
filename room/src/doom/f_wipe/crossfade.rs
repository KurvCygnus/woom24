//! The colour cross-fade wipe (`wipeno` 0): palette indices nudged
//! toward the end frame a few units per call. Private callbacks --
//! never exported; the `WIPES` table at the module root
//! references them by item.

use std::os::raw::c_int;

use super::WIPE_SCR;
use super::WIPE_SCR_END;
use super::WIPE_SCR_START;

/// Initialise the colour cross-fade wipe by copying the start screen into the
/// working buffer.
///
/// Returns 0 on success (matches the C convention: non-zero means done).
/// C origin: `wipe_initColorXForm` in f_wipe.c.
///
/// # Safety
///
/// `WIPE_SCR_START` and `WIPE_SCR` must both be valid pointers to at least
/// `width * height` bytes.
#[doc(alias = "wipe_initColorXForm")]
pub(super) unsafe extern "C" fn init_color_xform(width: c_int, height: c_int, _ticks: c_int) -> c_int {
    let len = (width * height) as usize;
    std::ptr::copy(WIPE_SCR_START, WIPE_SCR, len);
    0
}

/// Advance the colour cross-fade wipe by `ticks` steps.
///
/// Each pixel in the working buffer is nudged toward the corresponding pixel in
/// the end screen by `ticks` palette index units per call.  Returns 0 while
/// the transition is still in progress and 1 when every pixel has reached its
/// target value.
///
/// C origin: `wipe_doColorXForm` in f_wipe.c.
///
/// # Safety
///
/// `WIPE_SCR`, `WIPE_SCR_END` must both be valid pointers to at least
/// `width * height` bytes.
#[doc(alias = "wipe_doColorXForm")]
pub(super) unsafe extern "C" fn do_color_xform(width: c_int, height: c_int, ticks: c_int) -> c_int {
    let len = (width * height) as usize;
    let mut changed = false;

    for i in 0..len {
        let w = *WIPE_SCR.add(i);
        let e = *WIPE_SCR_END.add(i);

        if w != e {
            let newval: c_int;
            if w > e {
                newval = (w as c_int) - ticks;
                if newval < e as c_int {
                    *WIPE_SCR.add(i) = e;
                } else {
                    *WIPE_SCR.add(i) = newval as u8;
                }
                changed = true;
            } else {
                newval = (w as c_int) + ticks;
                if newval > e as c_int {
                    *WIPE_SCR.add(i) = e;
                } else {
                    *WIPE_SCR.add(i) = newval as u8;
                }
                changed = true;
            }
        }
    }

    if changed {
        0
    } else {
        1
    }
}

/// Clean up after the colour cross-fade wipe; a no-op that always returns 0.
///
/// C origin: `wipe_exitColorXForm` in f_wipe.c.
///
/// # Safety
///
/// No preconditions beyond those required by `extern "C"` calling convention.
#[doc(alias = "wipe_exitColorXForm")]
pub(super) unsafe extern "C" fn exit_color_xform(_width: c_int, _height: c_int, _ticks: c_int) -> c_int {
    0
}
