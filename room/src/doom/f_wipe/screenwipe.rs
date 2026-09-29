//! The public wipe entry points: start/end screen capture and the
//! per-tick wipe driver with its `GO` latch.

use std::os::raw::c_int;

use super::{GO, WIPE_SCR, WIPE_SCR_END, WIPE_SCR_START, WIPES};
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::z_zone::PU_STATIC;

/// Capture the current screen as the wipe start ("old") frame.
///
/// Allocates `SCREENWIDTH * SCREENHEIGHT` bytes via `Z_Malloc` and fills it
/// with a copy of the video buffer.  The `x`, `y`, `width`, and `height`
/// parameters are accepted for ABI compatibility but unused; the full screen is
/// always captured.  Returns 0.
///
/// Called by C code in `d_main.c` before the scene transition.
/// C origin: `wipe_StartScreen` in f_wipe.c.
///
/// # Safety
///
/// The video subsystem must be initialised; `Z_Malloc` must succeed.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim and
/// the differential oracle declares the symbol directly.
#[doc(alias = "wipe_StartScreen")]
#[export_name = "wipe_StartScreen"]
pub extern "C" fn start_screen(_x: c_int, _y: c_int, _width: c_int, _height: c_int) -> c_int {
    unsafe {
        WIPE_SCR_START =
            super::Z_Malloc(SCREENWIDTH * SCREENHEIGHT, PU_STATIC, std::ptr::null_mut()) as *mut u8;
        super::I_ReadScreen(WIPE_SCR_START);
    }
    0
}

/// Capture the current screen as the wipe end ("new") frame, then restore the
/// start frame to the video buffer.
///
/// Allocates `SCREENWIDTH * SCREENHEIGHT` bytes via `Z_Malloc`, captures the
/// video buffer into it, and then blits the start frame back so that the
/// display still shows the old scene until the wipe begins.  Returns 0.
///
/// Precondition: [`start_screen`] must have been called first.
///
/// Called by C code in `d_main.c` after drawing the new scene.
/// C origin: `wipe_EndScreen` in f_wipe.c.
///
/// # Safety
///
/// The video subsystem must be initialised and [`start_screen`] must have
/// allocated `WIPE_SCR_START`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim and
/// the differential oracle declares the symbol directly.
#[doc(alias = "wipe_EndScreen")]
#[export_name = "wipe_EndScreen"]
pub extern "C" fn end_screen(x: c_int, y: c_int, width: c_int, height: c_int) -> c_int {
    unsafe {
        WIPE_SCR_END =
            super::Z_Malloc(SCREENWIDTH * SCREENHEIGHT, PU_STATIC, std::ptr::null_mut()) as *mut u8;
        super::I_ReadScreen(WIPE_SCR_END);
        super::V_DrawBlock(x, y, width, height, WIPE_SCR_START);
    }
    0
}

/// Drive the screen wipe one frame forward and return whether the wipe is
/// complete.
///
/// On the first call (`GO == 0`), initialises the chosen algorithm, sets
/// `WIPE_SCR` to the live video buffer, and calls the init function.
/// Subsequently calls the step function; if it signals completion, calls the
/// exit function and resets `GO`.  Marks the updated rectangle as dirty via
/// `V_MarkRect` each call.
///
/// Returns 1 when the wipe is finished, 0 while it is still running.
///
/// The `_x` and `_y` parameters are accepted for ABI compatibility but are
/// unused; the full `(0, 0, width, height)` rectangle is always marked.
///
/// Called by C code in `d_main.c` each game tick while a wipe is active.
/// C origin: `wipe_ScreenWipe` in f_wipe.c.
///
/// # Safety
///
/// `start_screen`/`end_screen` must have captured both snapshots; the video
/// subsystem must be initialised.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim and
/// the differential oracle declares the symbol directly.
#[doc(alias = "wipe_ScreenWipe")]
#[export_name = "wipe_ScreenWipe"]
pub extern "C" fn screen_wipe(
    wipeno: c_int,
    _x: c_int,
    _y: c_int,
    width: c_int,
    height: c_int,
    ticks: c_int,
) -> c_int {
    unsafe {
        if GO == 0 {
            GO = 1;
            WIPE_SCR = super::I_VideoBuffer;
            WIPES[(wipeno * 3) as usize](width, height, ticks);
        }

        super::V_MarkRect(0, 0, width, height);
        let rc = WIPES[(wipeno * 3 + 1) as usize](width, height, ticks);

        if rc != 0 {
            GO = 0;
            WIPES[(wipeno * 3 + 2) as usize](width, height, ticks);
        }

        if GO == 0 {
            1
        } else {
            0
        }
    }
}
