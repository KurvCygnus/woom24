//! Rust port of vendor/doomgeneric/m_argv.c.
//!
//! Command-line argument handling: parameter lookup, `@responsefile`
//! expansion, and basename extraction. `myargc`/`myargv` are exported
//! with C linkage so the C side and the Rust side share a single mutable
//! argv pair.

#![allow(non_upper_case_globals, non_snake_case, clippy::upper_case_acronyms)]

use crate::i_error;
use std::ffi::{c_char, c_int, c_long, c_void};

/// Opaque stdio `FILE` handle used by the local `fopen`/`fread`/`fclose`
/// declarations below. Mirrors the C `FILE` typedef without exposing its
/// internals to Rust.
enum FILE {}

/// Path separator used to extract the executable basename in
/// `M_GetExecutableName`. Matches `DIR_SEPARATOR` from the C headers
/// (always `'/'` in this port; the upstream code uses `'\\'` on Windows).
const DIR_SEPARATOR: c_char = b'/' as c_char;

/// Maximum number of arguments produced by a response-file expansion,
/// matching `MAXARGVS` in `m_argv.c`.
const MAXARGVS: usize = 100;

/// `int myargc` — process argument count. Exported with C linkage so the
/// vendored C TUs share the same definition.
#[no_mangle]
pub static mut myargc: c_int = 0;

/// `char **myargv` — process argument vector. Exported with C linkage so
/// the vendored C TUs share the same definition.
#[no_mangle]
pub static mut myargv: *mut *mut c_char = std::ptr::null_mut();

extern "C" {
    /// libc `fopen` — open a file by path with the given mode string.
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut FILE;
    /// libc `fread` — read up to `nmemb` elements of `size` bytes each.
    fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;
    /// libc `fclose` — close a file handle previously opened with `fopen`.
    fn fclose(stream: *mut FILE) -> c_int;
    /// libc `malloc` — uninitialised heap allocation.
    fn malloc(size: usize) -> *mut c_void;
    /// libc `memset` — fill `n` bytes at `s` with the byte value `c`.
    fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void;
    /// libc `strrchr` — locate the last occurrence of `c` in the string `s`.
    fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;
    /// libc `isspace` — classify `c` as whitespace.
    fn isspace(c: c_int) -> c_int;
}

use crate::doom::crt::{c_printf, c_printf1, strcasecmp};
use crate::doom::m_misc::{M_FileLength, FILE as MiscFILE};

/// `int M_CheckParmWithArgs(char *check, int num_args)` — search the
/// program command line for `check`, requiring at least `num_args`
/// remaining arguments after the match.
///
/// Returns the matched argument index (1..argc-num_args) or 0 if not
/// present. Comparison is case-insensitive.
#[no_mangle]
pub extern "C" fn M_CheckParmWithArgs(check: *const c_char, num_args: c_int) -> c_int {
    unsafe {
        let mut i: c_int = 1;
        while i < myargc - num_args {
            if strcasecmp(check, *myargv.offset(i as isize) as *const c_char) == 0 {
                return i;
            }
            i += 1;
        }
        0
    }
}

/// `boolean M_ParmExists(char *check)` — returns nonzero if `check` is
/// present on the command line, 0 otherwise.
#[no_mangle]
pub extern "C" fn M_ParmExists(check: *const c_char) -> c_int {
    (M_CheckParm(check) != 0) as c_int
}

/// `int M_CheckParm(char *check)` — convenience wrapper for
/// `M_CheckParmWithArgs(check, 0)`.
#[no_mangle]
pub extern "C" fn M_CheckParm(check: *const c_char) -> c_int {
    M_CheckParmWithArgs(check, 0)
}

/// Load a response file (`@filename` on the command line) and splice its
/// tokens into `myargv` in place of the response-file argument.
///
/// Behaviour mirrors `LoadResponseFile` in `m_argv.c`: words are
/// whitespace-separated, double-quoted runs become a single argument, and
/// missing-file / unclosed-quote conditions raise an `I_Error`.
///
/// # Safety
/// - `argv_index` must be a valid index into the current `myargv`.
/// - `myargv[argv_index]` must be a NUL-terminated C string whose first
///   character is `'@'`; the response-file path is the remainder.
/// - All entries in `myargv` must remain readable for the duration of the
///   call.
unsafe fn LoadResponseFile(argv_index: c_int) {
    let response_filename: *mut c_char = (*myargv.offset(argv_index as isize)).offset(1);

    let handle: *mut FILE = fopen(response_filename as *const c_char, c"rb".as_ptr());
    if handle.is_null() {
        c_printf(c"\nNo such response file!".as_ptr());
        return;
    }

    c_printf1(c"Found response file %s!\n".as_ptr(), response_filename);

    let size: c_long = M_FileLength(handle as *mut MiscFILE);

    let file: *mut c_char = malloc(size as usize + 1) as *mut c_char;
    let mut i: usize = 0;
    while i < size as usize {
        let k: usize = fread(file.add(i) as *mut c_void, 1, size as usize - i, handle);
        if k == 0 {
            i_error!(
                "Failed to read full contents of '{}'",
                std::ffi::CStr::from_ptr(response_filename as *const c_char).to_string_lossy()
            );
        }
        i += k;
    }
    fclose(handle);
    *file.add(size as usize) = 0;

    let newargv: *mut *mut c_char =
        malloc(std::mem::size_of::<*mut c_char>() * MAXARGVS) as *mut *mut c_char;
    let mut newargc: c_int = 0;
    memset(
        newargv as *mut c_void,
        0,
        std::mem::size_of::<*mut c_char>() * MAXARGVS,
    );

    let mut idx: c_int = 0;
    while idx < argv_index {
        *newargv.offset(newargc as isize) = *myargv.offset(idx as isize);
        newargc += 1;
        idx += 1;
    }

    let infile: *mut c_char = file;
    let mut k: usize = 0;
    while (k as c_long) < size {
        while (k as c_long) < size && isspace(*infile.add(k) as c_int) != 0 {
            k += 1;
        }
        if (k as c_long) >= size {
            break;
        }

        if *infile.add(k) == b'"' as c_char {
            k += 1;
            *newargv.offset(newargc as isize) = infile.add(k);
            newargc += 1;
            while (k as c_long) < size
                && *infile.add(k) != b'"' as c_char
                && *infile.add(k) != b'\n' as c_char
            {
                k += 1;
            }
            if (k as c_long) >= size || *infile.add(k) == b'\n' as c_char {
                i_error!(
                    "Quotes unclosed in response file '{}'",
                    std::ffi::CStr::from_ptr(response_filename as *const c_char).to_string_lossy()
                );
            }
            *infile.add(k) = 0;
            k += 1;
        } else {
            *newargv.offset(newargc as isize) = infile.add(k);
            newargc += 1;
            while (k as c_long) < size && isspace(*infile.add(k) as c_int) == 0 {
                k += 1;
            }
            *infile.add(k) = 0;
            k += 1;
        }
    }

    idx = argv_index + 1;
    while idx < myargc {
        *newargv.offset(newargc as isize) = *myargv.offset(idx as isize);
        newargc += 1;
        idx += 1;
    }

    myargv = newargv;
    myargc = newargc;
}

/// `void M_FindResponseFile(void)` — scan `myargv` for arguments
/// beginning with `'@'` and expand each via `LoadResponseFile`.
#[no_mangle]
pub extern "C" fn M_FindResponseFile() {
    unsafe {
        let mut i: c_int = 1;
        while i < myargc {
            if **myargv.offset(i as isize) == b'@' as c_char {
                LoadResponseFile(i);
            }
            i += 1;
        }
    }
}

/// `char *M_GetExecutableName(void)` — return the basename portion of
/// `myargv[0]` (everything after the last `DIR_SEPARATOR`), or the whole
/// `argv[0]` if no separator is found.
#[no_mangle]
pub extern "C" fn M_GetExecutableName() -> *mut c_char {
    unsafe {
        let sep: *mut c_char = strrchr(*myargv as *const c_char, DIR_SEPARATOR as c_int);
        if sep.is_null() {
            *myargv
        } else {
            sep.offset(1)
        }
    }
}

/// Baseline vectors for the pure command-line parm lookup, written
/// against the pre-move `M_CheckParmWithArgs` body (F10 wave B5 --
/// first vectors this module has ever had). They pin the
/// case-insensitive match, the `num_args` window bound (a match at
/// index `myargc - num_args` must NOT count), and the zero-on-miss
/// contract shared by the `M_CheckParm` / `M_ParmExists` wrappers.
#[cfg(test)]
mod tests
{
    use std::ffi::CString;

    use super::*;

    //* Serialises argv installation: the tests mutate the process-wide
    //* `myargc` / `myargv` statics.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    //* Installs a fake argv whose storage leaks for the process
    //* lifetime (test-scale allocations); every test resets the
    //* globals afterwards.
    unsafe fn install_argv(args: &[&str])
    {
        let ptrs: Vec<*mut c_char> = args
            .iter()
            .map(|s| CString::new(*s).unwrap().into_raw())
            .collect();
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

            assert_eq!(M_CheckParmWithArgs(c"-FILE".as_ptr(), 0), 1);
            assert_eq!(M_CheckParmWithArgs(c"-file".as_ptr(), 0), 1);
            assert_eq!(M_CheckParmWithArgs(c"-IWAD".as_ptr(), 0), 3);
            assert_eq!(M_CheckParmWithArgs(c"-nomiss".as_ptr(), 0), 0);
            assert_eq!(M_CheckParmWithArgs(c"woom".as_ptr(), 0), 0, "argv[0] is never searched");

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
            assert_eq!(M_CheckParmWithArgs(c"-iwad".as_ptr(), 1), 3);
            // "-b" at index 1 with three followers is fine too.
            assert_eq!(M_CheckParmWithArgs(c"-b".as_ptr(), 3), 1);
            // "-iwad" would need two followers but only one exists.
            assert_eq!(M_CheckParmWithArgs(c"-iwad".as_ptr(), 2), 0);
            // "-b" with four followers: impossible (only three other args).
            assert_eq!(M_CheckParmWithArgs(c"-b".as_ptr(), 4), 0);

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

            assert_eq!(M_CheckParmWithArgs(c"-file".as_ptr(), 2), 0);
            assert_eq!(M_CheckParmWithArgs(c"-file".as_ptr(), 3), 0);
            assert_eq!(M_CheckParmWithArgs(c"-nomatch".as_ptr(), 2), 0);

            install_argv(&["woom"]);
            assert_eq!(M_CheckParmWithArgs(c"-file".as_ptr(), 0), 0);

            reset_argv();
        }
    }

    /// The wrapper contracts: `M_CheckParm(check)` equals
    /// `M_CheckParmWithArgs(check, 0)`, and `M_ParmExists` maps the
    /// same scan to 1/0.
    #[test]
    fn check_parm_and_parm_exists_wrap_the_scan()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            install_argv(&["woom", "-nomusic", "-file", "x.wad"]);

            assert_eq!(M_CheckParm(c"-nomusic".as_ptr()), 1);
            assert_eq!(M_CheckParm(c"-nomusic".as_ptr()), M_CheckParmWithArgs(c"-nomusic".as_ptr(), 0));
            assert_eq!(M_ParmExists(c"-file".as_ptr()), 1);
            assert_eq!(M_ParmExists(c"-nope".as_ptr()), 0);

            reset_argv();
        }
    }
}
