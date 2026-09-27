//! The IWAD search-directory list: the lazily built `iwad_dirs` array,
//! its build guard and counter, the `add_iwad_dir` appender, and
//! `build_iwad_dir_list`. Verbatim ports of the corresponding
//! `d_iwad.c` file-statics and file-static helpers.

// The C statics are lowercase (`iwad_dirs`, ...); names are verbatim
// upstream data, so the name lints are silenced file-wide.
#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};
use std::ptr;

use super::table::MAX_IWAD_DIRS;

/// Whether `build_iwad_dir_list` has already been called.
///
/// Guards against rebuilding the directory list on repeated calls.
pub(super) static mut iwad_dirs_built: bool = false;

/// Array of directories to search for IWAD files.
///
/// Populated lazily by `build_iwad_dir_list`. At most [`MAX_IWAD_DIRS`]
/// entries are stored. Corresponds to `iwad_dirs[]` in `d_iwad.c`.
pub(super) static mut iwad_dirs: [*mut c_char; MAX_IWAD_DIRS] = [ptr::null_mut(); MAX_IWAD_DIRS];

/// Number of valid entries in [`iwad_dirs`].
pub(super) static mut num_iwad_dirs: c_int = 0;

/// Appends `dir` to the global IWAD search directory list if the list is not
/// already full.
///
/// Silently drops `dir` when [`MAX_IWAD_DIRS`] has been reached.
/// Corresponds to `AddIWADDir` in `d_iwad.c`.
///
/// # Safety
/// `dir` must be a valid, non-null pointer to a null-terminated C string that
/// remains valid for as long as it may be read from `iwad_dirs`. Callers must
/// only invoke this function from the single-threaded game-startup path, as it
/// writes to the mutable globals `iwad_dirs` and `num_iwad_dirs` without
/// synchronisation.
unsafe fn add_iwad_dir(dir: *mut c_char)
{
    if num_iwad_dirs < MAX_IWAD_DIRS as c_int
    {
        iwad_dirs[num_iwad_dirs as usize] = dir;
        num_iwad_dirs += 1;
    }
}

/// Populates the global IWAD directory list if it has not been built yet.
///
/// This simplified port always adds only the current directory (`"."`).  The
/// full Chocolate Doom implementation (guarded by `ORIGCODE` and `_WIN32` in
/// the C source) additionally checks `DOOMWADDIR`, `DOOMWADPATH`, Windows
/// registry keys, and standard Unix paths -- none of which are supported here.
/// Corresponds to `BuildIWADDirList` in `d_iwad.c`.
///
/// # Safety
/// Must be called from the single-threaded game-startup path only. The
/// function writes to the mutable globals `iwad_dirs`, `num_iwad_dirs`, and
/// `iwad_dirs_built` without synchronisation, and calls `add_iwad_dir` which
/// imposes the same requirement.
pub(super) unsafe fn build_iwad_dir_list()
{
    add_iwad_dir(c".".as_ptr().cast_mut());
    iwad_dirs_built = true;
}
