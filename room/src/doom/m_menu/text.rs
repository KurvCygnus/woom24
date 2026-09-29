//! The HUD-font text family: width/height measurement and the menu
//! text renderer, plus the modifier-key filter. Static-reading glue
//! (no dtmc extraction, report §8.4).

use std::ffi::{c_char, c_int};

use super::consts::{HU_FONTSIZE, HU_FONTSTART, KEY_CAPSLOCK, KEY_NUMLOCK, KEY_PAUSE, KEY_SCRLCK};
use super::types::patch_stub;
use crate::doom::hu_stuff::hu_font;
use crate::doom::i_video::SCREENWIDTH;
use crate::doom::v_video::V_DrawPatchDirect;

/// Return the pixel width of `string` rendered in the HUD font.
///
/// Non-printable characters (outside `HU_FONTSTART`..`HU_FONTEND`) contribute 4 pixels each.
#[doc(alias = "M_StringWidth")]
pub(super) unsafe fn string_width(string: *mut c_char) -> c_int {
    let len = super::strlen(string);
    let mut w: c_int = 0;
    for i in 0..len {
        let c = super::toupper(*string.add(i) as c_int) - HU_FONTSTART;
        if c < 0 || c as usize >= HU_FONTSIZE {
            w += 4;
        } else {
            let patch = hu_font[c as usize] as *const patch_stub;
            w += (*patch).width as c_int;
        }
    }
    w
}

/// Return the pixel height of `string` rendered in the HUD font, accounting for newlines.
#[doc(alias = "M_StringHeight")]
pub(super) unsafe fn string_height(string: *mut c_char) -> c_int {
    let patch = hu_font[0] as *const patch_stub;
    let height = (*patch).height as c_int;
    let mut h = height;
    let len = super::strlen(string);
    for i in 0..len {
        if *string.add(i) == b'\n' as c_char {
            h += height;
        }
    }
    h
}

/// Render `string` using the HUD font at screen position `(x, y)`.
///
/// Newline characters reset the x cursor and advance y by 12 pixels.
/// Characters outside the font range are rendered as 4-pixel spaces.
/// Rendering stops at the screen right edge.
#[doc(alias = "M_WriteText")]
pub(super) unsafe fn write_text(x: c_int, y: c_int, string: *mut c_char) {
    let mut ch_ptr = string;
    let mut cx = x;
    let mut cy = y;

    loop {
        let c = *ch_ptr;
        if c == 0 {
            break;
        }
        ch_ptr = ch_ptr.add(1);
        if c == b'\n' as c_char {
            cx = x;
            cy += 12;
            continue;
        }

        let c = super::toupper(c as c_int) - HU_FONTSTART;
        if c < 0 || c as usize >= HU_FONTSIZE {
            cx += 4;
            continue;
        }

        let patch = hu_font[c as usize] as *const patch_stub;
        let w = (*patch).width as c_int;
        if cx + w > SCREENWIDTH {
            break;
        }
        V_DrawPatchDirect(cx, cy, hu_font[c as usize]);
        cx += w;
    }
}

/// Return `true` if `key` is a modifier key that should not terminate shortcut search.
///
/// Pause, Caps Lock, Scroll Lock, and Num Lock are treated as null keys.
#[doc(alias = "IsNullKey")]
pub(super) fn is_null_key(key: c_int) -> bool {
    key == KEY_PAUSE || key == KEY_CAPSLOCK || key == KEY_SCRLCK || key == KEY_NUMLOCK
}
