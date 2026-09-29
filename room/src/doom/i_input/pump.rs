//! The event pump: config statics, the `DG_GetKey` reverse contract, the
//! `I_InitInput` / `I_GetEvent` entry points, and the link anchor.

#![allow(non_snake_case, non_upper_case_globals)]

use std::os::raw::c_int;

use super::keymap::{translate_key, typed_char, update_shift_status};
use crate::doom::d_event::event_t;

/// Configuration flag mirroring the C global `vanilla_keyboard_mapping`.
///
/// When non-zero, classic Doom keyboard mapping is used. Exported with C
/// linkage so the rest of the (still C) code base can read and write it.
#[no_mangle]
pub static mut vanilla_keyboard_mapping: c_int = 1;

/// Net count of shift presses minus shift releases.
///
/// Treated as boolean-ish: `> 0` means a shift key is currently held. Using
/// a counter rather than a boolean tolerates lost key-up events without
/// permanently latching the shift state, matching the C source.
pub(super) static mut shiftdown: c_int = 0;

extern "C" {
    /// Platform-layer key poll provided by `doomgeneric.c`.
    ///
    /// Fills `*pressed` with 1 for keydown / 0 for keyup and `*key` with the
    /// Doom key code. Returns non-zero while events are available.
    //*
    //* Reverse contract: implemented by the shells; never re-pointed at a
    //* Rust path.
    fn DG_GetKey(pressed: *mut c_int, key: *mut u8) -> c_int;
}

/// One-time input subsystem initialisation hook.
///
/// No-op in the doomgeneric platform layer (all input setup happens in
/// `DG_Init` / `DG_GetKey`). Exported with C linkage because `i_video.c`
/// still calls it.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `i_video.rs` imports the upstream name through the root shim.
#[doc(alias = "I_InitInput")]
#[export_name = "I_InitInput"]
pub extern "C" fn init_input() {}

/// Pumps the platform key queue, posting one `ev_keydown` event per pressed
/// key and a single `ev_keyup` event before returning.
///
/// Mirrors the C control flow exactly: keydown events are posted in a loop
/// (so multiple keys pressed in the same frame all flow through), but the
/// first keyup event ends the call. Suppresses events whose `data1` is 0.
/// Exported with C linkage because `d_main.c` calls it from the main loop.
///
//* The `break` on the first keyup is upstream behaviour (`i_input.c`'s
//* loop), not a bug to fix -- do not "repair" it into a full drain.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `i_video.rs` imports the upstream name through the root shim.
#[doc(alias = "I_GetEvent")]
#[export_name = "I_GetEvent"]
pub extern "C" fn pump_key_events() {
    unsafe {
        let mut pressed: c_int = 0;
        let mut key: u8 = 0;

        while DG_GetKey(&mut pressed, &mut key) != 0 {
            update_shift_status(pressed, key);

            let mut event = event_t {
                type_: 0,
                data1: 0,
                data2: 0,
                data3: 0,
                data4: 0,
            };

            if pressed != 0 {
                event.type_ = 0; // ev_keydown
                event.data1 = translate_key(key) as c_int;
                event.data2 = typed_char(key) as c_int;
                if event.data1 != 0 {
                    crate::doom::d_event::D_PostEvent(&event);
                }
            } else {
                event.type_ = 1; // ev_keyup
                event.data1 = translate_key(key) as c_int;
                event.data2 = 0;
                if event.data1 != 0 {
                    crate::doom::d_event::D_PostEvent(&event);
                }
                break;
            }
        }
    }
}

/// Link-anchor symbol: forces the linker to keep the `extern "C"` exports
/// in this module even when nothing in the Rust crate references them.
///
/// Doomgeneric's still-C call sites pull in `I_InitInput` and `I_GetEvent`
/// by name, so this thunk takes their addresses to defeat dead-code
/// elimination. Called from the platform layer's link table.
///
/// Kept verbatim (name, symbol, body); retires with the freeze zone.
#[no_mangle]
pub extern "C" fn I_Input_Link_Anchor() {
    let _ = init_input as *const () as usize;
    let _ = pump_key_events as *const () as usize;
}
