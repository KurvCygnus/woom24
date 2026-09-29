//! The `STlib_draw*`/`STlib_update*` entry points: framebuffer work
//! only, all state arriving through caller-supplied widget pointers.

use std::os::raw::c_int;

use super::short_swap;
use super::sttminus;
use super::widgets::{st_binicon_t, st_multicon_t, st_number_t, st_percent_t};
use super::ST_Y;
use crate::doom::st_stuff::st_backing_screen;
use crate::doom::v_video::{V_CopyRect, V_DrawPatch};
use crate::i_error;

/// Draw a number widget unconditionally.
///
/// Algorithm:
/// 1. Clamp negative values only for `width == 2` (floor at -9) and
///    `width == 3` (floor at -99); other widths are not clamped.
/// 2. Erase the current field by blitting from `st_backing_screen`.
/// 3. If the value equals `1994`, skip rendering (magic "inactive" sentinel).
/// 4. Draw digits right-to-left using `p[digit]` patches.
/// 5. If negative, draw the [`sttminus`] patch 8 pixels left of the field.
///
/// The `_refresh` parameter is accepted for ABI compatibility but is currently
/// unused; erasing and redrawing always happen unconditionally.
/// Called by [`update_number_widget`].
///
/// Panics via `i_error!` if the widget's Y position is above the status bar
/// (`n->y - ST_Y < 0`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_drawNum")]
#[export_name = "STlib_drawNum"]
pub extern "C" fn draw_number_widget(n: *mut st_number_t, _refresh: c_int) {
    unsafe {
        let mut numdigits = (*n).width;
        let num = *(*n).num;

        let w = short_swap((**(*n).p).width) as c_int;
        let h = short_swap((**(*n).p).height) as c_int;
        (*n).oldnum = num;

        let neg = num < 0;
        let mut num = num;

        if neg {
            if numdigits == 2 && num < -9 {
                num = -9;
            } else if numdigits == 3 && num < -99 {
                num = -99;
            }
            num = -num;
        }

        // clear the area
        let mut x = (*n).x - numdigits * w;

        if (*n).y - ST_Y < 0 {
            i_error!("drawNum: n->y - ST_Y < 0");
        }

        V_CopyRect(
            x,
            (*n).y - ST_Y,
            st_backing_screen,
            w * numdigits,
            h,
            x,
            (*n).y,
        );

        // if non-number, do not draw it
        if num == 1994 {
            return;
        }

        x = (*n).x;

        // in the special case of 0, you draw 0
        if num == 0 {
            V_DrawPatch(x - w, (*n).y, *(*n).p);
        }

        // draw the new number
        while num != 0 && numdigits > 0 {
            x -= w;
            V_DrawPatch(x, (*n).y, *(*n).p.offset((num % 10) as isize));
            num /= 10;
            numdigits -= 1;
        }

        // draw a minus sign if necessary
        if neg {
            V_DrawPatch(x - 8, (*n).y, sttminus);
        }
    }
}

/// Update a number widget, redrawing it if the widget is visible.
///
/// Calls [`draw_number_widget`] when `*n.on != 0`. In the C original this function
/// also checked whether the value had changed before drawing; this port always
/// redraws when visible (matching the `refresh` path).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_updateNum")]
#[export_name = "STlib_updateNum"]
pub extern "C" fn update_number_widget(n: *mut st_number_t, refresh: c_int) {
    unsafe {
        if *(*n).on != 0 {
            draw_number_widget(n, refresh);
        }
    }
}

/// Update a percent widget, drawing the `%` patch and updating the number.
///
/// When `refresh != 0` and the widget is visible, draws the `%` patch at the
/// number's position before delegating to [`update_number_widget`].
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_updatePercent")]
#[export_name = "STlib_updatePercent"]
pub extern "C" fn update_percent_widget(per: *mut st_percent_t, refresh: c_int) {
    unsafe {
        if refresh != 0 && *(*per).n.on != 0 {
            V_DrawPatch((*per).n.x, (*per).n.y, (*per).p);
        }
        update_number_widget(&mut (*per).n, refresh);
    }
}

/// Update a multi-icon widget, redrawing if the index changed or refresh is
/// requested.
///
/// When the widget is visible and (`oldinum != *inum` or `refresh != 0`) and
/// `*inum != -1`:
/// * If there was a previous icon (`oldinum != -1`), erases it by blitting its
///   patch area from `st_backing_screen`.
/// * Draws the new icon patch using `V_DrawPatch`.
/// * Updates `oldinum`.
///
/// Skips all work when `*on == 0` or `*inum == -1`.
///
/// Panics via `i_error!` if the widget's Y position is above the status bar
/// (`y - ST_Y < 0`) when erasing a previous icon.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_updateMultIcon")]
#[export_name = "STlib_updateMultIcon"]
pub extern "C" fn update_multicon_widget(mi: *mut st_multicon_t, refresh: c_int) {
    unsafe {
        if *(*mi).on != 0 && ((*mi).oldinum != *(*mi).inum || refresh != 0) && *(*mi).inum != -1 {
            if (*mi).oldinum != -1 {
                let old_patch = *(*mi).p.offset((*mi).oldinum as isize);
                let x = (*mi).x - short_swap((*old_patch).leftoffset) as c_int;
                let y = (*mi).y - short_swap((*old_patch).topoffset) as c_int;
                let w = short_swap((*old_patch).width) as c_int;
                let h = short_swap((*old_patch).height) as c_int;

                if y - ST_Y < 0 {
                    i_error!("updateMultIcon: y - ST_Y < 0");
                }

                V_CopyRect(x, y - ST_Y, st_backing_screen, w, h, x, y);
            }
            V_DrawPatch((*mi).x, (*mi).y, *(*mi).p.offset(*(*mi).inum as isize));
            (*mi).oldinum = *(*mi).inum;
        }
    }
}

/// Update a binary icon widget, toggling the patch when the value changes.
///
/// When the widget is visible and (`oldval != *val` or `refresh != 0`):
/// * Computes the patch's top-left corner using its `leftoffset` / `topoffset`.
/// * If `*val != 0`, draws the patch with `V_DrawPatch`.
/// * If `*val == 0`, erases the patch area by blitting from `st_backing_screen`.
/// * Updates `oldval`.
///
/// Skips all work when `*on == 0`.
///
/// Panics via `i_error!` if the widget's Y position is above the status bar
/// (`y - ST_Y < 0`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_updateBinIcon")]
#[export_name = "STlib_updateBinIcon"]
pub extern "C" fn update_binicon_widget(bi: *mut st_binicon_t, refresh: c_int) {
    unsafe {
        if *(*bi).on != 0 && ((*bi).oldval != *(*bi).val || refresh != 0) {
            let p = (*bi).p;
            let x = (*bi).x - short_swap((*p).leftoffset) as c_int;
            let y = (*bi).y - short_swap((*p).topoffset) as c_int;
            let w = short_swap((*p).width) as c_int;
            let h = short_swap((*p).height) as c_int;

            if y - ST_Y < 0 {
                i_error!("updateBinIcon: y - ST_Y < 0");
            }

            if *(*bi).val != 0 {
                V_DrawPatch((*bi).x, (*bi).y, p);
            } else {
                V_CopyRect(x, y - ST_Y, st_backing_screen, w, h, x, y);
            }

            (*bi).oldval = *(*bi).val;
        }
    }
}
