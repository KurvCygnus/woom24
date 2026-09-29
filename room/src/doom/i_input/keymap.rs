//! The shift transform: the US-layout `SHIFTXFORM` table and the key
//! translation / typed-character / shift-status helpers.

use std::os::raw::c_int;

use super::pump::shiftdown;
use crate::doom::doomkeys::KEY_RSHIFT;

/// US-layout shift transform table.
///
/// Maps an unshifted ASCII byte (0..=127) to the character produced when the
/// shift key is held. Mirrors `shiftxform[]` in `i_input.c` verbatim,
/// including the well-known Watcom quirk that maps shift-backslash to `'!'`.
static SHIFTXFORM: [u8; 128] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, b' ', b'!', b'"', b'#', b'$', b'%', b'&', b'"', // shift-'
    b'(', b')', b'*', b'+', b'<', // shift-,
    b'_', // shift--
    b'>', // shift-.
    b'?', // shift-/
    b')', // shift-0
    b'!', // shift-1
    b'@', // shift-2
    b'#', // shift-3
    b'$', // shift-4
    b'%', // shift-5
    b'^', // shift-6
    b'&', // shift-7
    b'*', // shift-8
    b'(', // shift-9
    b':', b':', // shift-;
    b'<', b'+', // shift-=
    b'>', b'?', b'@', b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M',
    b'N', b'O', b'P', b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X', b'Y', b'Z',
    b'[', // shift-[
    b'!', // shift-backslash
    b']', // shift-]
    b'"', b'_', b'\'', // shift-`
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N', b'O', b'P',
    b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X', b'Y', b'Z', b'{', b'|', b'}', b'~', 127,
];

/// Translates a raw platform key code to a Doom key code.
///
/// Identity function: `DG_GetKey` already returns Doom key codes, so no
/// remapping is needed. Preserved as a named call so the structure mirrors
/// the C source, where the body is also `return key;` followed by a commented
/// AT-to-Doom lookup.
pub(super) fn translate_key(key: u8) -> u8 {
    // The platform layer (DG_GetKey) already returns Doom key codes,
    // so this is an identity function (matching the active code in
    // the original C source).
    key
}

/// Returns the printable character produced by a key press, applying the
/// shift transform if shift is currently held.
///
/// Out-of-range key codes (`>= SHIFTXFORM.len()`) collapse to 0 when shift is
/// held, matching the C `arrlen(shiftxform)` guard. Reads the `shiftdown`
/// static, hence the `unsafe` block.
pub(super) fn typed_char(key: u8) -> u8 {
    let mut key = translate_key(key);
    unsafe {
        if shiftdown > 0 {
            if (key as usize) < SHIFTXFORM.len() {
                key = SHIFTXFORM[key as usize];
            } else {
                key = 0;
            }
        }
    }
    key
}

/// Updates `shiftdown` for a shift key event.
///
/// Increments on press, decrements on release. Only `KEY_RSHIFT` is tracked,
/// matching the C source. Mutates the `shiftdown` static.
pub(super) fn update_shift_status(pressed: c_int, key: u8) {
    let change = if pressed != 0 { 1 } else { -1 };
    if key == KEY_RSHIFT {
        unsafe {
            shiftdown += change;
        }
    }
}
