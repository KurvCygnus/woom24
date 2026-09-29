//! The DOS scancode translation set: `SCANTOKEY` plus its local `KEY_*`
//! `c_int` constants. This is m_config's own copy (different type and role
//! from the `doomkeys` module set: these translate vanilla `.cfg` DOS
//! scancodes, they do not bind platform keys), kept separate by design.

use std::ffi::c_int;

// scantokey mapping from DOS keyboard scan codes to internal key codes.
// Key constant values from doomkeys.h:
//   KEY_BACKSPACE=0x7f, KEY_RCTRL=0x9D, KEY_RSHIFT=0xB6, KEYP_MULTIPLY='*',
//   KEY_RALT=0xB8, KEY_CAPSLOCK=0xBA, KEY_F1=0xBB, KEY_F2=0xBC, KEY_F3=0xBD,
//   KEY_F4=0xBE, KEY_F5=0xBF, KEY_F6=0xC0, KEY_F7=0xC1, KEY_F8=0xC2,
//   KEY_F9=0xC3, KEY_F10=0xC4, KEY_PAUSE=0xFF, KEY_SCRLCK=0xC6,
//   KEY_HOME=0xC7, KEY_UPARROW=0xAD, KEY_PGUP=0xC9, KEY_MINUS=0x2D,
//   KEY_LEFTARROW=0xAC, KEYP_5='5', KEY_RIGHTARROW=0xAE, KEYP_PLUS='+',
//   KEY_END=0xCF, KEY_DOWNARROW=0xAF, KEY_PGDN=0xD1, KEY_INS=0xD2,
//   KEY_DEL=0xD3, KEY_F11=0xD7, KEY_F12=0xD8, KEY_PRTSCR=0xD9

/// Doom internal code for the backspace key (C `KEY_BACKSPACE` from `doomkeys.h`).
const KEY_BACKSPACE: c_int = 0x7f;
/// Doom internal code for the right Ctrl key (C `KEY_RCTRL`).
const KEY_RCTRL: c_int = 0x9D;
/// Doom internal code for the right Shift key (C `KEY_RSHIFT`).
const KEY_RSHIFT: c_int = 0xB6;
/// Numpad `*` mapped to the ASCII `'*'`, matching C `KEYP_MULTIPLY`.
const KEYP_MULTIPLY: c_int = b'*' as c_int;
/// Doom internal code for the right Alt key (C `KEY_RALT`).
const KEY_RALT: c_int = 0xB8;
/// Doom internal code for the Caps Lock key (C `KEY_CAPSLOCK`).
const KEY_CAPSLOCK: c_int = 0xBA;
/// Doom internal code for F1 (C `KEY_F1`).
const KEY_F1: c_int = 0xBB;
/// Doom internal code for F2 (C `KEY_F2`).
const KEY_F2: c_int = 0xBC;
/// Doom internal code for F3 (C `KEY_F3`).
const KEY_F3: c_int = 0xBD;
/// Doom internal code for F4 (C `KEY_F4`).
const KEY_F4: c_int = 0xBE;
/// Doom internal code for F5 (C `KEY_F5`).
const KEY_F5: c_int = 0xBF;
/// Doom internal code for F6 (C `KEY_F6`).
const KEY_F6: c_int = 0xC0;
/// Doom internal code for F7 (C `KEY_F7`).
const KEY_F7: c_int = 0xC1;
/// Doom internal code for F8 (C `KEY_F8`).
const KEY_F8: c_int = 0xC2;
/// Doom internal code for F9 (C `KEY_F9`).
const KEY_F9: c_int = 0xC3;
/// Doom internal code for F10 (C `KEY_F10`).
const KEY_F10: c_int = 0xC4;
/// Doom internal code for the Pause key (C `KEY_PAUSE`).
const KEY_PAUSE: c_int = 0xFF;
/// Doom internal code for Scroll Lock (C `KEY_SCRLCK`).
const KEY_SCRLCK: c_int = 0xC6;
/// Doom internal code for the Home key (C `KEY_HOME`).
const KEY_HOME: c_int = 0xC7;
/// Doom internal code for the up arrow (C `KEY_UPARROW`).
const KEY_UPARROW: c_int = 0xAD;
/// Doom internal code for Page Up (C `KEY_PGUP`).
const KEY_PGUP: c_int = 0xC9;
/// Doom internal code for the `-` key (C `KEY_MINUS`).
const KEY_MINUS: c_int = 0x2D;
/// Doom internal code for the left arrow (C `KEY_LEFTARROW`).
const KEY_LEFTARROW: c_int = 0xAC;
/// Numpad `5` mapped to the ASCII `'5'`, matching C `KEYP_5`.
const KEYP_5: c_int = b'5' as c_int;
/// Doom internal code for the right arrow (C `KEY_RIGHTARROW`).
const KEY_RIGHTARROW: c_int = 0xAE;
/// Numpad `+` mapped to the ASCII `'+'`, matching C `KEYP_PLUS`.
const KEYP_PLUS: c_int = b'+' as c_int;
/// Doom internal code for the End key (C `KEY_END`).
const KEY_END: c_int = 0xCF;
/// Doom internal code for the down arrow (C `KEY_DOWNARROW`).
const KEY_DOWNARROW: c_int = 0xAF;
/// Doom internal code for Page Down (C `KEY_PGDN`).
const KEY_PGDN: c_int = 0xD1;
/// Doom internal code for the Insert key (C `KEY_INS`).
const KEY_INS: c_int = 0xD2;
/// Doom internal code for the Delete key (C `KEY_DEL`).
const KEY_DEL: c_int = 0xD3;
/// Doom internal code for F11 (C `KEY_F11`).
const KEY_F11: c_int = 0xD7;
/// Doom internal code for F12 (C `KEY_F12`).
const KEY_F12: c_int = 0xD8;
/// Doom internal code for Print Screen (C `KEY_PRTSCR`).
const KEY_PRTSCR: c_int = 0xD9;

/// DOS PC keyboard scan-code -> Doom internal key-code lookup table.
///
/// Indexed by the 7-bit DOS scancode (0-127) read out of a vanilla
/// `default.cfg`; the entry is the corresponding internal key code used
/// throughout the engine. Used by `set_variable` to translate the
/// `untranslated` value stored on disk into a runtime key constant.
pub(super) const SCANTOKEY: [c_int; 128] = [
    0,
    27,
    b'1' as c_int,
    b'2' as c_int,
    b'3' as c_int,
    b'4' as c_int,
    b'5' as c_int,
    b'6' as c_int,
    b'7' as c_int,
    b'8' as c_int,
    b'9' as c_int,
    b'0' as c_int,
    b'-' as c_int,
    b'=' as c_int,
    KEY_BACKSPACE,
    9,
    b'q' as c_int,
    b'w' as c_int,
    b'e' as c_int,
    b'r' as c_int,
    b't' as c_int,
    b'y' as c_int,
    b'u' as c_int,
    b'i' as c_int,
    b'o' as c_int,
    b'p' as c_int,
    b'[' as c_int,
    b']' as c_int,
    13,
    KEY_RCTRL,
    b'a' as c_int,
    b's' as c_int,
    b'd' as c_int,
    b'f' as c_int,
    b'g' as c_int,
    b'h' as c_int,
    b'j' as c_int,
    b'k' as c_int,
    b'l' as c_int,
    b';' as c_int,
    b'\'' as c_int,
    b'`' as c_int,
    KEY_RSHIFT,
    b'\\' as c_int,
    b'z' as c_int,
    b'x' as c_int,
    b'c' as c_int,
    b'v' as c_int,
    b'b' as c_int,
    b'n' as c_int,
    b'm' as c_int,
    b',' as c_int,
    b'.' as c_int,
    b'/' as c_int,
    KEY_RSHIFT,
    KEYP_MULTIPLY,
    KEY_RALT,
    b' ' as c_int,
    KEY_CAPSLOCK,
    KEY_F1,
    KEY_F2,
    KEY_F3,
    KEY_F4,
    KEY_F5,
    KEY_F6,
    KEY_F7,
    KEY_F8,
    KEY_F9,
    KEY_F10,
    KEY_PAUSE,
    KEY_SCRLCK,
    KEY_HOME,
    KEY_UPARROW,
    KEY_PGUP,
    KEY_MINUS,
    KEY_LEFTARROW,
    KEYP_5,
    KEY_RIGHTARROW,
    KEYP_PLUS,
    KEY_END,
    KEY_DOWNARROW,
    KEY_PGDN,
    KEY_INS,
    KEY_DEL,
    0,
    0,
    0,
    KEY_F11,
    KEY_F12,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    KEY_PRTSCR,
    0,
];

/// Baseline pin written before the graduation move (F10 wave F2-b):
/// `SCANTOKEY` covers the full 7-bit DOS scancode range; out-of-range
/// scancodes must keep resolving to `0` in `set_variable`.
#[cfg(test)]
mod tests {
    use super::*;

    /// Table length covers the 7-bit scancode range.
    #[test]
    fn scantokey_covers_7bit_range() {
        assert_eq!(SCANTOKEY.len(), 128);
    }
}
