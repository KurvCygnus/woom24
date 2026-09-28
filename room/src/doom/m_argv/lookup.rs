//! The pure command-line scan: locate a parameter on the command line,
//! optionally requiring trailing arguments, plus the two convenience
//! wrappers.

use std::ffi::{c_char, c_int};

use super::state::{myargc, myargv};
use crate::doom::crt::strcasecmp;

/// `int M_CheckParmWithArgs(char *check, int num_args)` — search the
/// program command line for `check`, requiring at least `num_args`
/// remaining arguments after the match.
///
/// Returns the matched argument index (1..argc-num_args) or 0 if not
/// present. Comparison is case-insensitive.
///
/// Freeze-zone legacy `extern "C"` blocks link this function by its
/// upstream C symbol (`d_iwad.rs`), so the symbol is pinned with
/// `#[export_name]` instead of being dropped with the rename.
///
/// # Safety
/// - `myargv` must either be null (empty command line) or point at
///   `myargc` valid NUL-terminated strings.
/// - `check` must point to a readable NUL-terminated string.
#[doc(alias = "M_CheckParmWithArgs")]
#[export_name = "M_CheckParmWithArgs"]
pub extern "C" fn check_parm_with_args(check: *const c_char, num_args: c_int) -> c_int
{
    unsafe
    {
        let mut i: c_int = 1;
        while i < myargc - num_args
        {
            if strcasecmp(check, *myargv.offset(i as isize) as *const c_char) == 0 { return i; }
            i += 1;
        }
        0
    }
}

/// `boolean M_ParmExists(char *check)` — returns nonzero if `check` is
/// present on the command line, 0 otherwise.
///
/// Called from `I_Error`'s `-nogui` scan (`i_system.rs`), i.e. during
/// abort handling -- keep it allocation-free as today.
///
/// # Safety
/// - Same preconditions as [`check_parm_with_args`].
#[doc(alias = "M_ParmExists")]
pub extern "C" fn parm_exists(check: *const c_char) -> c_int { (check_parm(check) != 0) as c_int }

/// `int M_CheckParm(char *check)` — convenience wrapper for
/// `M_CheckParmWithArgs(check, 0)`.
///
//* Freeze-zone legacy `extern "C"` blocks link this function by its
//* upstream C symbol (`d_net/mod.rs:155`, called from `D_ConnectNetGame`
//* at every boot) -- a declarer missed by the investigation report's
//* four-pin list -- so the symbol is pinned with `#[export_name]`
//* instead of being dropped with the rename.
///
/// # Safety
/// - Same preconditions as [`check_parm_with_args`].
#[doc(alias = "M_CheckParm")]
#[export_name = "M_CheckParm"]
pub extern "C" fn check_parm(check: *const c_char) -> c_int { check_parm_with_args(check, 0) }

/// Baseline vectors for the pure command-line parm lookup, written
/// against the pre-move `M_CheckParmWithArgs` body and re-pointed to
/// the graduated names after the split -- same vectors, same results
/// (F10 wave B5). They pin the case-insensitive match, the `num_args`
/// window bound (a match at index `myargc - num_args` must NOT count),
/// and the zero-on-miss contract shared by the `check_parm` /
/// `parm_exists` wrappers.
#[cfg(test)]
mod tests
{
    use std::ffi::CString;

    use super::super::state::{myargc, myargv};
    use super::*;

    //* Serialises argv installation: the tests mutate the process-wide
    //* `myargc` / `myargv` statics.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    //* Installs a fake argv whose storage leaks for the process
    //* lifetime (test-scale allocations); every test resets the
    //* globals afterwards.
    unsafe fn install_argv(args: &[&str])
    {
        let ptrs: Vec<*mut c_char> = args.
            iter().
            map(|s| CString::new(*s).unwrap().into_raw()).
            collect();
        myargv = Vec::leak(ptrs).as_mut_ptr();
        myargc = args.len() as c_int;
    }

    unsafe fn reset_argv()
    {
        myargv = std::ptr::null_mut();
        myargc = 0;
    }

    /// Exact and case-insensitive matches return the argument index;
    /// a miss returns 0.
    #[test]
    fn check_parm_with_args_matches_case_insensitively()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            install_argv(&["woom", "-FILE", "x.wad", "-iwad", "y.wad"]);

            assert_eq!(check_parm_with_args(c"-FILE".as_ptr(), 0), 1);
            assert_eq!(check_parm_with_args(c"-file".as_ptr(), 0), 1);
            assert_eq!(check_parm_with_args(c"-IWAD".as_ptr(), 0), 3);
            assert_eq!(check_parm_with_args(c"-nomiss".as_ptr(), 0), 0);
            assert_eq!(
                check_parm_with_args(c"woom".as_ptr(), 0),
                0,
                "argv[0] is never searched"
            );

            reset_argv();
        }
    }

    /// The `num_args` window: a match at the last index that leaves
    /// `num_args` followers counts; a match at index
    /// `myargc - num_args` (not enough followers) must not.
    #[test]
    fn check_parm_with_args_respects_num_args_window()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            install_argv(&["woom", "-b", "y", "-iwad", "doom.wad"]);

            // "-iwad" at index 3 with one follower: last valid index
            // under the `i < myargc - num_args` bound.
            assert_eq!(check_parm_with_args(c"-iwad".as_ptr(), 1), 3);
            // "-b" at index 1 with three followers is fine too.
            assert_eq!(check_parm_with_args(c"-b".as_ptr(), 3), 1);
            // "-iwad" would need two followers but only one exists.
            assert_eq!(check_parm_with_args(c"-iwad".as_ptr(), 2), 0);
            // "-b" with four followers: impossible (only three other args).
            assert_eq!(check_parm_with_args(c"-b".as_ptr(), 4), 0);

            reset_argv();
        }
    }

    /// Degenerate windows: `num_args` at or beyond the whole command
    /// line returns 0 even for a present parm, and the empty scan
    /// (argc == 1) returns 0.
    #[test]
    fn check_parm_with_args_degenerate_windows()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            install_argv(&["woom", "-file", "x.wad"]);

            assert_eq!(check_parm_with_args(c"-file".as_ptr(), 2), 0);
            assert_eq!(check_parm_with_args(c"-file".as_ptr(), 3), 0);
            assert_eq!(check_parm_with_args(c"-nomatch".as_ptr(), 2), 0);

            install_argv(&["woom"]);
            assert_eq!(check_parm_with_args(c"-file".as_ptr(), 0), 0);

            reset_argv();
        }
    }

    /// The wrapper contracts: `check_parm(check)` equals
    /// `check_parm_with_args(check, 0)`, and `parm_exists` maps the
    /// same scan to 1/0.
    #[test]
    fn check_parm_and_parm_exists_wrap_the_scan()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            install_argv(&["woom", "-nomusic", "-file", "x.wad"]);

            assert_eq!(check_parm(c"-nomusic".as_ptr()), 1);
            assert_eq!(
                check_parm(c"-nomusic".as_ptr()),
                check_parm_with_args(c"-nomusic".as_ptr(), 0)
            );
            assert_eq!(parm_exists(c"-file".as_ptr()), 1);
            assert_eq!(parm_exists(c"-nope".as_ptr()), 0);

            reset_argv();
        }
    }
}
