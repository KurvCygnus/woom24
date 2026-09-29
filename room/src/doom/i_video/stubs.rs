//! The no-op ABI hooks and the link anchor: entry points the engine's call
//! sites require but the generic platform does nothing with. Kept for
//! symbol-set parity, retiring with the freeze zone.

use std::ffi::c_char;
use std::ptr;

use super::buffer::{init_graphics, shutdown_graphics};
use super::palette::{get_palette_index, set_palette};
use super::present::{finish_update, read_screen, start_frame, start_tic, update_no_blit};
use crate::types::Boolean;

extern "C" {
    /// Platform hook called from `set_window_title` to update the
    /// host window title with a NUL-terminated C string.
    fn DG_SetWindowTitle(title: *const c_char);
}

/// SDL-era hook to signal the start of a disk read so a "loading"
/// icon can be displayed. No-op in this port.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers besides the link anchor,
/// kept for wasm symbol-set parity).
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_BeginRead")]
#[export_name = "I_BeginRead"]
pub unsafe extern "C" fn begin_read() {}

/// SDL-era hook to signal the end of a disk read. No-op here.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers besides the link anchor,
/// kept for wasm symbol-set parity).
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_EndRead")]
#[export_name = "I_EndRead"]
pub unsafe extern "C" fn end_read() {}

/// Set the host-window title via the platform `DG_SetWindowTitle`
/// callback. Mirrors `I_SetWindowTitle` from `i_video.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// `title` must be a valid NUL-terminated C string.
#[doc(alias = "I_SetWindowTitle")]
#[export_name = "I_SetWindowTitle"]
pub unsafe extern "C" fn set_window_title(title: *mut c_char) {
    DG_SetWindowTitle(title);
}

/// Parse video-related command-line switches. No-op in this port;
/// the SDL backend in chocolate-doom handles things like `-window`
/// here.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_GraphicsCheckCommandLine")]
#[export_name = "I_GraphicsCheckCommandLine"]
pub unsafe extern "C" fn graphics_check_command_line() {}

/// Register a callback that returns whether the mouse should be
/// grabbed by the window. No-op in this port (no mouse capture).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe; the callback argument is stored nowhere.
#[doc(alias = "I_SetGrabMouseCallback")]
#[export_name = "I_SetGrabMouseCallback"]
pub unsafe extern "C" fn set_grab_mouse_callback(_func: extern "C" fn() -> Boolean) {}

/// Enable the disk-activity icon overlay. No-op in this port.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_EnableLoadingDisk")]
#[export_name = "I_EnableLoadingDisk"]
pub unsafe extern "C" fn enable_loading_disk() {}

/// Bind video-related `m_config` variables. No-op in this port;
/// chocolate-doom binds things like `fullscreen`, `aspect_ratio_correct`,
/// etc. here.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/bind.rs` calls the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_BindVideoVariables")]
#[export_name = "I_BindVideoVariables"]
pub unsafe extern "C" fn bind_video_variables() {}

/// Toggle the on-screen FPS dot indicator. No-op in this port.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_DisplayFPSDots")]
#[export_name = "I_DisplayFPSDots"]
pub unsafe extern "C" fn display_fps_dots(_dots_on: Boolean) {}

/// Detect whether the engine was launched as a screensaver. No-op
/// in this port; `screensaver_mode` stays 0.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_CheckIsScreensaver")]
#[export_name = "I_CheckIsScreensaver"]
pub unsafe extern "C" fn check_is_screensaver() {}

/// Link anchor referencing every public C symbol in this module so
/// the linker keeps them all. Not part of the original Doom API.
///
/// # Safety
///
/// Passes null pointers everywhere and would crash if called.
/// Treat as link-only.
#[no_mangle]
pub unsafe extern "C" fn I_Video_Link_Anchor() {
    init_graphics();
    shutdown_graphics();
    start_frame();
    start_tic();
    update_no_blit();
    finish_update();
    read_screen(ptr::null_mut());
    set_palette(ptr::null_mut());
    get_palette_index(0, 0, 0);
    begin_read();
    end_read();
    set_window_title(ptr::null_mut());
    graphics_check_command_line();
    /// Inert mouse-grab callback used only to take a function pointer for
    /// `set_grab_mouse_callback`; never invoked at runtime.
    extern "C" fn _grab_anchor() -> Boolean {
        Boolean::FALSE
    }
    set_grab_mouse_callback(_grab_anchor);
    enable_loading_disk();
    bind_video_variables();
    display_fps_dots(Boolean::FALSE);
    check_is_screensaver();
}
