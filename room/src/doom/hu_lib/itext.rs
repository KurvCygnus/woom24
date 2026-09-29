//! The text-input widget primitives (`hu_itext_t`): margin-protected
//! editing, the key surface, draw, and the dead-but-exported erase.

use std::ffi::{c_char, c_int};

use super::textline::{
    add_char, clear_text_line, del_char, draw_text_line, init_text_line,
};
use super::types::hu_itext_t;
use crate::doom::doomkeys::{KEY_BACKSPACE, KEY_ENTER};

/// Initialize a text-input widget.
///
/// Zeroes the left margin (`lm = 0`), stores the visibility pointer, sets
/// `laston = 1`, and initialises the underlying text line via
/// [`init_text_line`].
///
/// # Preconditions
/// * `on` must be a valid non-null pointer for the widget's lifetime.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_initIText")]
#[export_name = "HUlib_initIText"]
pub extern "C" fn init_itext(
    it: *mut hu_itext_t,
    x: c_int,
    y: c_int,
    font: *mut *mut crate::doom::v_video::patch_t,
    startchar: c_int,
    on: *mut c_int,
) {
    unsafe {
        (*it).lm = 0;
        (*it).on = on;
        (*it).laston = 1;
        init_text_line(&mut (*it).l, x, y, font, startchar);
    }
}

/// Delete the last character from an input widget, respecting the left margin.
///
/// Calls [`del_char`] only when `l.len > lm`; characters
/// within the protected prefix are never removed.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_delCharFromIText")]
#[export_name = "HUlib_delCharFromIText"]
pub extern "C" fn del_char_respecting_margin(it: *mut hu_itext_t) {
    unsafe {
        if (*it).l.len != (*it).lm {
            del_char(&mut (*it).l);
        }
    }
}

/// Delete all user-entered characters from an input widget, stopping at the
/// left margin.
///
/// Repeatedly calls [`del_char`] until `l.len == lm`,
/// effectively clearing everything after the prefix.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_eraseLineFromIText")]
#[export_name = "HUlib_eraseLineFromIText"]
pub extern "C" fn erase_line_to_margin(it: *mut hu_itext_t) {
    unsafe {
        while (*it).lm != (*it).l.len {
            del_char(&mut (*it).l);
        }
    }
}

/// Reset an input widget to a fully empty state, including the prefix.
///
/// Sets `lm` to 0 and calls [`clear_text_line`], discarding both the
/// prefix and any user input. Used at the start of a new chat session.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_resetIText")]
#[export_name = "HUlib_resetIText"]
pub extern "C" fn reset_itext(it: *mut hu_itext_t) {
    unsafe {
        (*it).lm = 0;
        clear_text_line(&mut (*it).l);
    }
}

/// Append a prefix string to an input widget and lock it as the left margin.
///
/// Appends each byte of the NUL-terminated `str` to the underlying text line,
/// then sets `lm = l.len` so that subsequent delete operations cannot remove
/// those characters.
///
/// # Preconditions
/// * `str` must be a valid NUL-terminated C string.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_addPrefixToIText")]
#[export_name = "HUlib_addPrefixToIText"]
pub extern "C" fn add_prefix(it: *mut hu_itext_t, str: *mut c_char) {
    unsafe {
        let mut p = str;
        while *p != 0 {
            add_char(&mut (*it).l, *p);
            p = p.add(1);
        }
        (*it).lm = (*it).l.len;
    }
}

/// Process a keypress for an input text widget.
///
/// Uppercases `ch` before dispatch:
/// * Printable range `[' ', '_']`: appended via [`add_char`].
/// * `KEY_BACKSPACE`: deletes via [`del_char_respecting_margin`] (honours margin).
/// * `KEY_ENTER`: accepted as a terminator (no text change).
/// * Any other value: returns 0 to signal the key was not consumed.
///
/// Returns 1 if the key was consumed, 0 otherwise.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_keyInIText")]
#[export_name = "HUlib_keyInIText"]
pub extern "C" fn key_in_itext(it: *mut hu_itext_t, ch: u8) -> c_int {
    unsafe {
        let ch = ch.to_ascii_uppercase();
        if (b' '..=b'_').contains(&ch) {
            add_char(&mut (*it).l, ch as c_char);
        } else if ch == KEY_BACKSPACE {
            del_char_respecting_margin(it);
        } else if ch != KEY_ENTER {
            return 0;
        }
        1
    }
}

/// Render the input text widget, including the cursor glyph.
///
/// Skips drawing if `*it.on == 0`. Otherwise delegates to
/// [`draw_text_line`] with `drawcursor = 1`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_drawIText")]
#[export_name = "HUlib_drawIText"]
pub extern "C" fn draw_itext(it: *mut hu_itext_t) {
    unsafe {
        if *(*it).on == 0 {
            return;
        }
        draw_text_line(&mut (*it).l, 1);
    }
}

/// Erase the input text widget from the screen and update the visibility cache.
///
/// Marks the line dirty if the widget just became hidden (transition from
/// `laston != 0` to `*on == 0`), then calls
/// [`super::textline::erase_text_line`] and updates `laston`.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone (the only upstream caller
/// is the dead `HU_Erase` chain).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff`'s dead `erase` entry imports the upstream name through
/// the root shim.
#[doc(alias = "HUlib_eraseIText")]
#[export_name = "HUlib_eraseIText"]
pub extern "C" fn erase_itext(it: *mut hu_itext_t) {
    unsafe {
        if (*it).laston != 0 && *(*it).on == 0 {
            (*it).l.needsupdate = 4;
        }
        super::textline::erase_text_line(&mut (*it).l);
        (*it).laston = if *(*it).on != 0 { 1 } else { 0 };
    }
}
