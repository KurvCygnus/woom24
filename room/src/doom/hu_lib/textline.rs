//! The text-line primitives: init/clear, character add/delete, the
//! patch-font draw, and the dead-but-exported erase.

use std::ffi::{c_char, c_int};

use super::types::{hu_textline_t, HU_MAXLINELENGTH};
use super::{automapactive, short_swap, viewheight, viewwindowx, viewwindowy, viewwidth};
use crate::doom::i_video::SCREENWIDTH;

/// Initialize the heads-up widget library (no-op in this port, matching C).
///
/// Called once at startup from `load_font` (the graduated `hu_stuff`'s
/// `HU_Init`). The C original also had no body. `doomgeneric.rs` takes
/// this function's address as the module's link anchor; the C symbol
/// is pinned below.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `doomgeneric.rs:35/:263` reaches the upstream name through the root
/// shim.
#[doc(alias = "HUlib_init")]
#[export_name = "HUlib_init"]
pub extern "C" fn init_library() {}

/// Reset a text line to empty, marking it for redisplay.
///
/// Sets `len` to 0, NUL-terminates `l[0]`, and sets `needsupdate` to 1.
/// Called from `load_font`, `add_line`, and `reset_itext`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_clearTextLine")]
#[export_name = "HUlib_clearTextLine"]
pub extern "C" fn clear_text_line(t: *mut hu_textline_t) {
    unsafe {
        (*t).len = 0;
        (*t).l[0] = 0;
        (*t).needsupdate = 1;
    }
}

/// Initialize a text line widget with its screen position and font.
///
/// Sets the position (`x`, `y`), font pointer `f`, and start character `sc`,
/// then calls [`clear_text_line`] to zero the text buffer.
///
/// # Preconditions
/// * `t` must be a valid non-null pointer.
/// * `f` must point to a valid array of at least `('_' - sc + 1)` patch
///   pointers when drawing is later requested.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_initTextLine")]
#[export_name = "HUlib_initTextLine"]
pub extern "C" fn init_text_line(
    t: *mut hu_textline_t,
    x: c_int,
    y: c_int,
    f: *mut *mut crate::doom::v_video::patch_t,
    sc: c_int,
) {
    unsafe {
        (*t).x = x;
        (*t).y = y;
        (*t).f = f;
        (*t).sc = sc;
        clear_text_line(t);
    }
}

/// Append a character to a text line.
///
/// Returns 1 (true) on success, 0 (false) if the line is already at
/// `HU_MAXLINELENGTH` (80) characters. On success, sets `needsupdate` to 4.
/// The buffer remains NUL-terminated after the call.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_addCharToTextLine")]
#[export_name = "HUlib_addCharToTextLine"]
pub extern "C" fn add_char(t: *mut hu_textline_t, ch: c_char) -> c_int {
    unsafe {
        if (*t).len == HU_MAXLINELENGTH as c_int {
            0
        } else {
            (*t).l[(*t).len as usize] = ch;
            (*t).len += 1;
            (*t).l[(*t).len as usize] = 0;
            (*t).needsupdate = 4;
            1
        }
    }
}

/// Delete the last character from a text line.
///
/// Returns 1 (true) on success, 0 (false) if the line is already empty.
/// On success, NUL-terminates the new end and sets `needsupdate` to 4.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_delCharFromTextLine")]
#[export_name = "HUlib_delCharFromTextLine"]
pub extern "C" fn del_char(t: *mut hu_textline_t) -> c_int {
    unsafe {
        if (*t).len == 0 {
            0
        } else {
            (*t).len -= 1;
            (*t).l[(*t).len as usize] = 0;
            (*t).needsupdate = 4;
            1
        }
    }
}

/// Render a text line to the screen using its patch font.
///
/// Iterates over `l[0..len]`, uppercases each character, and calls
/// `V_DrawPatchDirect` for characters in the range `[sc, '_']`; spaces and
/// out-of-range characters advance the x cursor by 4 pixels. Rendering stops
/// early if the cursor would exceed `SCREENWIDTH`.
///
/// If `drawcursor` is non-zero, the `'_'` patch is drawn at the current
/// position (provided it fits), giving a text-entry cursor appearance.
///
/// Called from `hu_stuff::drawer` and `draw_stext`/`draw_itext`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff` imports the upstream name through the root shim.
#[doc(alias = "HUlib_drawTextLine")]
#[export_name = "HUlib_drawTextLine"]
pub extern "C" fn draw_text_line(l: *mut hu_textline_t, drawcursor: c_int) {
    unsafe {
        let mut x = (*l).x;
        let f = (*l).f;
        let sc = (*l).sc;

        for i in 0..(*l).len as usize {
            let c = ((*l).l[i] as u8).to_ascii_uppercase();
            if c != b' ' && c >= sc as u8 && c <= b'_' {
                let patch = *f.add((c as c_int - sc) as usize);
                let w = short_swap((*patch).width) as c_int;
                if x + w > SCREENWIDTH {
                    break;
                }
                super::V_DrawPatchDirect(x, (*l).y, patch);
                x += w;
            } else {
                x += 4;
                if x >= SCREENWIDTH {
                    break;
                }
            }
        }

        if drawcursor != 0 {
            let cursor_patch = *f.add((b'_' as c_int - sc) as usize);
            let cursor_w = short_swap((*cursor_patch).width) as c_int;
            if x + cursor_w <= SCREENWIDTH {
                super::V_DrawPatchDirect(x, (*l).y, cursor_patch);
            }
        }
    }
}

/// Erase the screen region occupied by a text line and decrement the dirty
/// counter.
///
/// Erasing only occurs when the automap is inactive and the view window is
/// reduced (`viewwindowx != 0`). For each scanline in the font-height range,
/// the function erases either the full line (if it is outside the view window)
/// or the left and right border strips (if it falls within the view window).
/// `needsupdate` is decremented by 1 each call (never below 0), so the line
/// stays "dirty" for up to 4 frames after a change.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The only live callers
/// upstream are through `HU_Erase` (vendor `d_main.c:208`), which the
/// ported `d_main/display.rs` never wired in. The `viewwindowx != 0`
/// gate is a crispy-era conditional kept verbatim.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff`'s dead `erase` entry imports the upstream name through
/// the root shim.
#[doc(alias = "HUlib_eraseTextLine")]
#[export_name = "HUlib_eraseTextLine"]
pub extern "C" fn erase_text_line(l: *mut hu_textline_t) {
    unsafe {
        if automapactive == 0 && viewwindowx != 0 && (*l).needsupdate != 0 {
            let lh = short_swap((**(*l).f).height) as c_int + 1;
            for y in (*l).y..(*l).y + lh {
                let yoffset = y * SCREENWIDTH;
                if y < viewwindowy || y >= viewwindowy + viewheight {
                    super::R_VideoErase(yoffset as std::ffi::c_uint, SCREENWIDTH);
                } else {
                    super::R_VideoErase(yoffset as std::ffi::c_uint, viewwindowx);
                    super::R_VideoErase(
                        (yoffset + viewwindowx + viewwidth) as std::ffi::c_uint,
                        viewwindowx,
                    );
                }
            }
        }

        if (*l).needsupdate != 0 {
            (*l).needsupdate -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doom::hu_lib::hu_itext_t;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    /// Build a zero-initialised hu_textline_t without a valid font pointer.
    /// Safe to use with clear/add/del because those don't dereference `f`.
    fn make_textline() -> hu_textline_t {
        hu_textline_t {
            x: 0,
            y: 0,
            f: std::ptr::null_mut(),
            sc: 0,
            l: [0; HU_MAXLINELENGTH + 1],
            len: 0,
            needsupdate: 0,
        }
    }

    #[test]
    fn clear_text_line_resets_fields() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        tl.len = 5;
        tl.needsupdate = 0;
        tl.l[0] = b'X' as c_char;

        clear_text_line(&mut tl);

        assert_eq!(tl.len, 0, "len must be reset to 0");
        assert_eq!(tl.l[0], 0, "first char must be nul after clear");
        assert_eq!(tl.needsupdate, 1, "needsupdate must be set to 1");
    }

    #[test]
    fn add_char_increments_len_and_nul_terminates() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        clear_text_line(&mut tl);

        let ret = add_char(&mut tl, b'A' as c_char);

        assert_eq!(ret, 1, "successful add must return 1");
        assert_eq!(tl.len, 1);
        assert_eq!(tl.l[0], b'A' as c_char);
        assert_eq!(tl.l[1], 0, "character after the last must be nul");
        assert_eq!(tl.needsupdate, 4);
    }

    #[test]
    fn add_multiple_chars_builds_string() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        clear_text_line(&mut tl);

        for ch in b"HI" {
            add_char(&mut tl, *ch as c_char);
        }
        assert_eq!(tl.len, 2);
        assert_eq!(tl.l[0], b'H' as c_char);
        assert_eq!(tl.l[1], b'I' as c_char);
        assert_eq!(tl.l[2], 0);
    }

    #[test]
    fn add_char_at_max_capacity_returns_zero() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        clear_text_line(&mut tl);

        // Fill to HU_MAXLINELENGTH
        for _ in 0..HU_MAXLINELENGTH {
            add_char(&mut tl, b'X' as c_char);
        }
        assert_eq!(tl.len, HU_MAXLINELENGTH as c_int);

        // One more must be rejected
        let ret = add_char(&mut tl, b'Y' as c_char);
        assert_eq!(ret, 0, "add beyond max length must return 0");
        assert_eq!(tl.len, HU_MAXLINELENGTH as c_int, "len must not change");
    }

    #[test]
    fn del_char_decrements_len_and_nul_terminates() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        clear_text_line(&mut tl);
        add_char(&mut tl, b'A' as c_char);
        add_char(&mut tl, b'B' as c_char);

        let ret = del_char(&mut tl);

        assert_eq!(ret, 1, "successful delete must return 1");
        assert_eq!(tl.len, 1);
        assert_eq!(tl.l[1], 0, "position after new end must be nul");
        assert_eq!(tl.needsupdate, 4);
    }

    #[test]
    fn del_char_on_empty_line_returns_zero() {
        let _g = LOCK.lock().unwrap();
        let mut tl = make_textline();
        clear_text_line(&mut tl);

        let ret = del_char(&mut tl);
        assert_eq!(ret, 0, "delete on empty line must return 0");
        assert_eq!(tl.len, 0, "len must stay 0");
    }

    /// HUlib_keyInIText with a printable character in [' ', '_'] must add it.
    #[test]
    fn key_in_itext_printable_adds_char() {
        let _g = LOCK.lock().unwrap();
        let mut on: c_int = 1;
        let mut it = hu_itext_t {
            l: make_textline(),
            lm: 0,
            _pad0: [0; 4],
            on: &mut on,
            laston: 0,
            _pad1: [0; 4],
        };
        clear_text_line(&mut it.l);

        let ret = super::super::itext::key_in_itext(&mut it, b'a'); // lowercase → uppercased to 'A'
        assert_eq!(ret, 1);
        assert_eq!(it.l.len, 1);
        assert_eq!(it.l.l[0], b'A' as c_char);
    }

    /// HUlib_keyInIText with KEY_BACKSPACE removes the last character.
    #[test]
    fn key_in_itext_backspace_removes_char() {
        use crate::doom::doomkeys::KEY_BACKSPACE;
        let _g = LOCK.lock().unwrap();
        let mut on: c_int = 1;
        let mut it = hu_itext_t {
            l: make_textline(),
            lm: 0,
            _pad0: [0; 4],
            on: &mut on,
            laston: 0,
            _pad1: [0; 4],
        };
        clear_text_line(&mut it.l);
        add_char(&mut it.l, b'Z' as c_char);
        assert_eq!(it.l.len, 1);

        let ret = super::super::itext::key_in_itext(&mut it, KEY_BACKSPACE);
        assert_eq!(ret, 1);
        assert_eq!(it.l.len, 0);
    }

    /// Characters outside [' ', '_'] (except Enter/Backspace) return 0.
    #[test]
    fn key_in_itext_unknown_key_returns_zero() {
        let _g = LOCK.lock().unwrap();
        let mut on: c_int = 1;
        let mut it = hu_itext_t {
            l: make_textline(),
            lm: 0,
            _pad0: [0; 4],
            on: &mut on,
            laston: 0,
            _pad1: [0; 4],
        };
        clear_text_line(&mut it.l);

        // 0x01 is below ' ' (0x20) and is not Enter or Backspace
        let ret = super::super::itext::key_in_itext(&mut it, 0x01);
        assert_eq!(ret, 0, "unknown control char must return 0");
        assert_eq!(it.l.len, 0, "no char should be added");
    }
}
