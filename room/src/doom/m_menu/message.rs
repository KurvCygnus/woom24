//! The modal overlay message state machine: display a centered message
//! over the menu, optionally requiring a y/n (or specific key) response
//! routed through the stored callback.

use std::ffi::c_int;

use super::state::{menuactive, messageLastMenuActive, messageNeedsInput, messageRoutine, messageString, messageToPrint};

/// Display a modal overlay message.
///
/// `string` is the message to display.
/// `routine` is an optional callback invoked with the key pressed to dismiss.
/// `input` is non-zero when a specific key (y/n or confirm/abort) is required.
#[doc(alias = "M_StartMessage")]
pub(super) fn start_message(string: *mut std::ffi::c_char, routine: Option<extern "C" fn(c_int)>, input: c_int) {
    unsafe {
        messageLastMenuActive = menuactive;
        messageToPrint = 1;
        messageString = string;
        messageRoutine = routine;
        messageNeedsInput = input;
        menuactive = 1;
    }
}

/// Dismiss the current modal overlay message and restore the previous `menuactive` state.
#[doc(alias = "M_StopMessage")]
pub(super) fn stop_message() {
    unsafe {
        menuactive = messageLastMenuActive;
        messageToPrint = 0;
    }
}
