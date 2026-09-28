//! Executable-basename extraction from `myargv[0]`.

use std::ffi::{c_char, c_int};

use super::state::DIR_SEPARATOR;
use super::state::myargv;

extern "C"
{
    /// libc `strrchr` — locate the last occurrence of `c` in the string `s`.
    fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;
}

/// `char *M_GetExecutableName(void)` — return the basename portion of
/// `myargv[0]` (everything after the last `DIR_SEPARATOR`), or the whole
/// `argv[0]` if no separator is found.
///
/// # Safety
/// - `myargv` must be non-null with `myargv[0]` a valid NUL-terminated
///   string; the returned pointer aliases it.
#[doc(alias = "M_GetExecutableName")]
pub extern "C" fn get_executable_name() -> *mut c_char
{
    unsafe
    {
        let sep: *mut c_char = strrchr(*myargv as *const c_char, DIR_SEPARATOR as c_int);
        if sep.is_null() { *myargv }
        else { sep.offset(1) }
    }
}
