//! `@responsefile` expansion: scan `myargv` for arguments beginning
//! with `'@'` and splice the file's whitespace-separated tokens into
//! the argv pair in place.
//!
//! Upstream gates this whole machinery behind `#if ORIGCODE` (dead
//! there); this port keeps it live -- archaeology, not a
//! vanilla-behaviour emulation row.

#![allow(clippy::upper_case_acronyms)] // ! `FILE` mirrors the C stdio typedef name (pre-move allow carried over).

use std::ffi::{c_char, c_int, c_long, c_void};

use super::state::{myargc, myargv, MAXARGVS};
use crate::i_error;
use crate::doom::crt::{c_printf, c_printf1};
use crate::doom::m_misc::{M_FileLength, FILE as MiscFILE};

/// Opaque stdio `FILE` handle used by the local `fopen`/`fread`/`fclose`
/// declarations below. Mirrors the C `FILE` typedef without exposing its
/// internals to Rust.
enum FILE {}

extern "C"
{
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
    /// libc `isspace` — classify `c` as whitespace.
    fn isspace(c: c_int) -> c_int;
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
#[doc(alias = "LoadResponseFile")]
unsafe fn load_response_file(argv_index: c_int)
{
    let response_filename: *mut c_char = (*myargv.offset(argv_index as isize)).offset(1);

    let handle: *mut FILE = fopen(response_filename as *const c_char, c"rb".as_ptr());
    if handle.is_null()
    {
        c_printf(c"\nNo such response file!".as_ptr());
        return;
    }

    c_printf1(c"Found response file %s!\n".as_ptr(), response_filename);

    let size: c_long = M_FileLength(handle as *mut MiscFILE);

    let file: *mut c_char = malloc(size as usize + 1) as *mut c_char;
    let mut i: usize = 0;
    while i < size as usize
    {
        let k: usize = fread(file.add(i) as *mut c_void, 1, size as usize - i, handle);
        if k == 0
        {
            i_error!(
                "Failed to read full contents of '{}'",
                std::ffi::CStr::from_ptr(response_filename as *const c_char).to_string_lossy()
            );
        }
        i += k;
    }
    fclose(handle);
    *file.add(size as usize) = 0;

    let newargv: *mut *mut c_char = malloc(size_of::<*mut c_char>() * MAXARGVS) as *mut *mut c_char;
    let mut newargc: c_int = 0;
    memset(
        newargv as *mut c_void,
        0,
        size_of::<*mut c_char>() * MAXARGVS,
    );

    let mut idx: c_int = 0;
    while idx < argv_index
    {
        *newargv.offset(newargc as isize) = *myargv.offset(idx as isize);
        newargc += 1;
        idx += 1;
    }

    let infile: *mut c_char = file;
    let mut k: usize = 0;
    while (k as c_long) < size
    {
        while (k as c_long) < size && isspace(*infile.add(k) as c_int) != 0 { k += 1; }
        if (k as c_long) >= size { break; }

        if *infile.add(k) == b'"' as c_char
        {
            k += 1;
            *newargv.offset(newargc as isize) = infile.add(k);
            newargc += 1;
            while(k as c_long) < size &&
                *infile.add(k) != b'"' as c_char &&
                *infile.add(k) != b'\n' as c_char { k += 1; }
            
            if(k as c_long) >= size || *infile.add(k) == b'\n' as c_char
            {
                i_error!(
                    "Quotes unclosed in response file '{}'",
                    std::ffi::CStr::from_ptr(response_filename as *const c_char).to_string_lossy()
                );
            }
            *infile.add(k) = 0;
            k += 1;
        }
        else
        {
            *newargv.offset(newargc as isize) = infile.add(k);
            newargc += 1;
            while(k as c_long) < size && isspace(*infile.add(k) as c_int) == 0 { k += 1; }
            *infile.add(k) = 0;
            k += 1;
        }
    }

    idx = argv_index + 1;
    while idx < myargc
    {
        *newargv.offset(newargc as isize) = *myargv.offset(idx as isize);
        newargc += 1;
        idx += 1;
    }

    myargv = newargv;
    myargc = newargc;
}

/// `void M_FindResponseFile(void)` — scan `myargv` for arguments
/// beginning with `'@'` and expand each via `load_response_file`.
///
/// Freeze-zone legacy `extern "C"` blocks link this function by its
/// upstream C symbol (`doomgeneric.rs`, called from
/// `doomgeneric_Create` before `D_DoomMain`), so the symbol is pinned
/// with `#[export_name]` instead of being dropped with the rename.
///
/// # Safety
/// - `myargv` must either be null or point at `myargc` valid
///   NUL-terminated strings.
#[doc(alias = "M_FindResponseFile")]
#[export_name = "M_FindResponseFile"]
pub extern "C" fn find_response_file()
{
    unsafe
    {
        let mut i: c_int = 1;
        while i < myargc
        {
            if **myargv.offset(i as isize) == b'@' as c_char { load_response_file(i); }
            i += 1;
        }
    }
}
