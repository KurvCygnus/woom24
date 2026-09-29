//! The fatal-error path: the `LAST_I_ERROR` diagnostics channel, the
//! `I_Error` entry with its recursion guard, log-facade emission and
//! Zenity popup, the varargs-era wrapper, and the Zenity process pair.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, CStr};
use std::sync::Mutex;

use crate::doom::m_argv::M_ParmExists;

use super::banner::console_stdout;
use super::exit::{atexit_listentry_t, exit_funcs};

/// Probe whether the Zenity GUI dialog binary is available on this
/// system by invoking `/usr/bin/zenity --help`.
///
/// # Safety
///
/// Marked `unsafe` only for symmetry with `zenity_error_box`; the body
/// itself uses safe `std::process` APIs. Always safe to call.
unsafe fn zenity_available() -> bool
{
    std::process::Command::new("/usr/bin/zenity")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Display `message` in a native error dialog using Zenity. Returns 1
/// if Zenity launched successfully and exited cleanly, 0 otherwise.
///
/// Unlike the C source's `EscapeShellString` + `system()` approach,
/// this implementation passes the message as an argv element to
/// `std::process::Command`, so no shell-escaping is required.
///
/// # Safety
///
/// `message` must point to a valid NUL-terminated C string.
unsafe fn zenity_error_box(message: *const c_char) -> c_int
{
    if !zenity_available()
    {
        return 0;
    }

    let msg = CStr::from_ptr(message).to_string_lossy();
    let status = std::process::Command::new("/usr/bin/zenity")
        .arg("--error")
        .arg("--text")
        .arg(&*msg)
        .status();

    match status
    {
        Ok(s) =>
        {
            if s.success()
            {
                1
            }
            else
            {
                0
            }
        }
        Err(_) => 0,
    }
}

/// Last message passed to `I_Error`, for hosts that render crash state
/// (wasm page overlay, headless harness assertions). Diagnostics only:
/// never read by simulation.
static LAST_I_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Record `msg` as the last `I_Error` message. Poisoning-tolerant by
/// design: a lost message on the fatal path is acceptable, a second
/// panic there is not.
fn set_last_i_error(msg: String)
{
    if let Ok(mut slot) = LAST_I_ERROR.lock()
    {
        *slot = Some(msg);
    }
}

/// Host-facing reader for the last `I_Error` message. `None` = no fatal
/// error has fired this process. Read by the web shell's crash overlay
/// (`shells/web/src/lib.rs`) through the module-root path kept below.
pub fn last_i_error() -> Option<String> { LAST_I_ERROR.lock().ok().and_then(|s| s.clone()) }

/// Fatal error handler: prints `msg` to stderr, runs all
/// `run_on_error` exit callbacks, optionally pops up a GUI dialog,
/// then terminates with exit code -1. Never returns.
///
/// Recursive calls are detected (via the function-local
/// `already_quitting` static) and trigger only a warning print before
/// continuing; this matches the safety net in `I_Error` from
/// `i_system.c`. Unlike the C source, which takes a printf-style
/// format string and varargs, this entry point accepts a single
/// pre-formatted C string - the `i_error!` macro takes care of
/// formatting on the caller side.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// `i_error!` macro (`m_misc/format.rs`) and six `g_game` files reach
/// it through the root shim, so the `crate::doom::i_system::I_Error`
/// path stays valid.
///
/// # Safety
///
/// `msg` must point to a valid NUL-terminated C string.
#[doc(alias = "I_Error")]
#[export_name = "I_Error"]
pub extern "C" fn fatal_error(msg: *const c_char) -> !
{
    unsafe
    {
        static mut already_quitting: bool = false;

        if already_quitting
        {
            eprintln!("Warning: recursive call to I_Error detected.");
        }
        else
        {
            already_quitting = true;
        }

        let msg_cstr = CStr::from_ptr(msg);
        let msg_str = msg_cstr.to_string_lossy().into_owned();
        //* wasm targets have no stderr, so the message would be lost
        //* entirely; the log facade (routed to the host console by the
        //* shells) is the only channel that reaches the user there.
        //* The last-error channel additionally survives the exit trap, so
        //* the page overlay and harness can read it after death.
        set_last_i_error(msg_str.clone());
        log::error!("{}", msg_str);
        eprintln!("{}", msg_str);
        eprintln!();

        let mut entry: *mut atexit_listentry_t = exit_funcs;
        while !entry.is_null()
        {
            if (*entry).run_on_error.is_truthy()
            {
                ((*entry).func)();
            }
            entry = (*entry).next;
        }

        let exit_gui_popup = M_ParmExists(c"-nogui".as_ptr().cast_mut()) == 0;
        if exit_gui_popup && console_stdout() == 0
        {
            zenity_error_box(msg);
        }

        std::process::exit(-1)
    }
}

/// Wrapper called by the C preprocessor macro for I_Error.
///
/// The original C source expands `I_Error(...)` into a varargs call;
/// in this port the `i_error!` macro produces a formatted C string
/// and forwards to `fatal_error`. Identical behaviour to `I_Error`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers besides the link anchor,
/// kept for wasm symbol-set parity).
///
/// # Safety
///
/// `msg` must point to a valid NUL-terminated C string.
#[doc(alias = "I_ErrorV")]
#[export_name = "I_ErrorV"]
pub extern "C" fn fatal_error_var(msg: *const c_char)
{
    fatal_error(msg);
}

#[cfg(test)]
mod tests
{
    /// Host-safe: this exercises the set/last pair directly - calling
    /// `fatal_error` itself would exit the test process.
    #[test]
    fn last_i_error_roundtrip()
    {
        super::set_last_i_error("Z_Malloc: failed on allocation".to_string());
        assert_eq!(
            super::last_i_error().as_deref(),
            Some("Z_Malloc: failed on allocation"),
            "the shell overlay and harness must read the exact I_Error text"
        );
        super::set_last_i_error("second".to_string());
        assert_eq!(super::last_i_error().as_deref(), Some("second"), "latest wins");
    }
}
