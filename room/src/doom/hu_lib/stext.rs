//! The scrolling-message widget primitives (`hu_stext_t`): ring
//! management, message append, draw, and the dead-but-exported erase.

use std::ffi::{c_char, c_int};

use super::short_swap;
use super::textline::{add_char, clear_text_line, draw_text_line, init_text_line};
use super::types::hu_stext_t;
use crate::doom::v_video::patch_t;

/// Initialize a scrolling text widget.
///
/// Sets height `h`, visibility pointer `on`, resets the current-line index
/// to 0, and initializes each of the `h` text lines. Lines are stacked
/// upward: line 0 is at `y`, line 1 at `y - font_height - 1`, and so on.
///
/// # Preconditions
/// * `h <= HU_MAXLINES`.
/// * `font` must point to valid patch data so the font height can be read.
/// * `on` must be a valid non-null pointer for the widget's lifetime.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_initSText")]
#[export_name = "HUlib_initSText"]
pub extern "C" fn init_stext(
    s: *mut hu_stext_t,
    x: c_int,
    y: c_int,
    h: c_int,
    font: *mut *mut patch_t,
    startchar: c_int,
    on: *mut c_int,
) {
    unsafe {
        (*s).h = h;
        (*s).on = on;
        (*s).laston = 1;
        (*s).cl = 0;
        let font_h = short_swap((**font).height) as c_int + 1;
        for i in 0..h as usize {
            init_text_line(&mut (*s).l[i], x, y - (i as c_int) * font_h, font, startchar);
        }
    }
}

/// Advance the ring-buffer cursor and clear the new current line.
///
/// Increments `cl` modulo `h`, clears the new current text line, and sets
/// `needsupdate` to 4 on every line so they are all redrawn.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_addLineToSText")]
#[export_name = "HUlib_addLineToSText"]
pub extern "C" fn add_line(s: *mut hu_stext_t) {
    unsafe {
        (*s).cl += 1;
        if (*s).cl == (*s).h {
            (*s).cl = 0;
        }
        clear_text_line(&mut (*s).l[(*s).cl as usize]);

        for i in 0..(*s).h as usize {
            (*s).l[i].needsupdate = 4;
        }
    }
}

/// Append a message (with optional prefix) to a scrolling text widget.
///
/// Calls [`add_line`] to advance the ring buffer, then appends
/// each character of `prefix` (if non-null) followed by each character of
/// `msg` to the current line using [`add_char`].
///
/// # Preconditions
/// * `msg` must be a valid NUL-terminated C string.
/// * `prefix` may be null; if non-null it must also be NUL-terminated.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_addMessageToSText")]
#[export_name = "HUlib_addMessageToSText"]
pub extern "C" fn add_message(s: *mut hu_stext_t, prefix: *mut c_char, msg: *mut c_char) {
    unsafe {
        add_line(s);
        if !prefix.is_null() {
            let mut p = prefix;
            while *p != 0 {
                add_char(&mut (*s).l[(*s).cl as usize], *p);
                p = p.add(1);
            }
        }
        let mut m = msg;
        while *m != 0 {
            add_char(&mut (*s).l[(*s).cl as usize], *m);
            m = m.add(1);
        }
    }
}

/// Render all lines of a scrolling text widget.
///
/// Skips drawing if `*s.on == 0`. Otherwise iterates `h` lines in ring order
/// (newest first) and calls [`draw_text_line`] for each without a cursor.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_drawSText")]
#[export_name = "HUlib_drawSText"]
pub extern "C" fn draw_stext(s: *mut hu_stext_t) {
    unsafe {
        if *(*s).on == 0 {
            return;
        }
        for i in 0..(*s).h as usize {
            let mut idx = (*s).cl as isize - i as isize;
            if idx < 0 {
                idx += (*s).h as isize;
            }
            draw_text_line(&mut (*s).l[idx as usize], 0);
        }
    }
}

/// Erase all lines of a scrolling text widget and update the visibility cache.
///
/// If the widget just transitioned from visible to hidden (`laston != 0` and
/// `*on == 0`), marks every line dirty so they are erased from the screen.
/// Then calls [`super::textline::erase_text_line`] on each line and updates
/// `laston`.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone (the only upstream caller
/// is the dead `HU_Erase` chain).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff`'s dead `erase` entry imports the upstream name through
/// the root shim.
#[doc(alias = "HUlib_eraseSText")]
#[export_name = "HUlib_eraseSText"]
pub extern "C" fn erase_stext(s: *mut hu_stext_t) {
    unsafe {
        for i in 0..(*s).h as usize {
            if (*s).laston != 0 && *(*s).on == 0 {
                (*s).l[i].needsupdate = 4;
            }
            super::textline::erase_text_line(&mut (*s).l[i]);
        }
        (*s).laston = if *(*s).on != 0 { 1 } else { 0 };
    }
}
