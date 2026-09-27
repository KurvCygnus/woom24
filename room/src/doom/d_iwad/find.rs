//! The public WAD/IWAD finders: `D_FindWADByName`, `D_TryFindWADByName`,
//! and `D_FindIWAD`, verbatim ports of the corresponding `d_iwad.c`
//! functions. The returned strings are CRT-allocated and freed through
//! the platform `free` (on wasm, through the VFS shim) -- allocation
//! ownership is load-bearing, never modernise it.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use super::dirs::{build_iwad_dir_list, iwad_dirs, num_iwad_dirs};
use super::search::{dir_is_file, identify_iwad_by_name, search_directory_for_iwad, DIR_SEPARATOR_S};
use crate::doom::crt::{c_printf, strdup};
use crate::doom::m_misc::M_StringJoinA;
use crate::i_error;

use super::{free, M_FileExists, myargv, M_CheckParmWithArgs};

/// Searches IWAD search paths for a WAD file with the given `name`.
///
/// If `name` already refers to an existing file on disk, it is returned as-is
/// (no allocation). Otherwise the function calls `build_iwad_dir_list` and
/// iterates over every candidate directory, trying both the directory path
/// itself (when `name` is the final component) and the concatenated path
/// `dir/name`. Returns a heap-allocated path string on success, or null if the
/// file cannot be found anywhere. The caller is responsible for `free`-ing any
/// returned string that is not identical to `name`.
///
/// Called from `d_main.c` and `w_main.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism; no in-tree extern declarer exists --
//* the web shell cites this function in comments only).
///
/// # Safety
/// `name` must be a valid, null-terminated C string.
#[doc(alias = "D_FindWADByName")]
#[export_name = "D_FindWADByName"]
pub unsafe extern "C" fn find_wad_by_name(name: *mut c_char) -> *mut c_char
{
    if M_FileExists(name) != 0
    {
        return name;
    }

    build_iwad_dir_list();

    for i in 0..num_iwad_dirs
    {
        if dir_is_file(iwad_dirs[i as usize], name) != 0 && M_FileExists(iwad_dirs[i as usize]) != 0
        {
            return strdup(iwad_dirs[i as usize]);
        }

        let strs: [*const c_char; 4] = [
            iwad_dirs[i as usize] as *const c_char,
            DIR_SEPARATOR_S.as_ptr() as *const c_char,
            name as *const c_char,
            ptr::null(),
        ];
        // SAFETY: null-terminated pointer array; freed below on miss, transferred to caller on hit.
        let path = M_StringJoinA(strs.as_ptr());

        if M_FileExists(path) != 0
        {
            return path;
        }

        free(path as *mut c_void);
    }

    ptr::null_mut()
}

/// Searches for a WAD by filename, falling back to the original `filename`
/// pointer if the file cannot be found.
///
/// Unlike [`find_wad_by_name`], this function always returns a non-null
/// pointer: either a heap-allocated path string on success, or the original
/// `filename` argument unchanged on failure. Called from `w_main.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below;
//* the freeze-zone caller `w_main.rs` imports the upstream name
//* through the root shim.
///
/// # Safety
/// `filename` must be a valid, null-terminated C string.
#[doc(alias = "D_TryFindWADByName")]
#[export_name = "D_TryFindWADByName"]
pub unsafe extern "C" fn try_find_wad_by_name(filename: *mut c_char) -> *mut c_char
{
    let result = find_wad_by_name(filename);
    if !result.is_null()
    {
        result
    }
    else
    {
        filename
    }
}

/// Locates an IWAD file on disk and identifies its game mission.
///
/// If the `-iwad <file>` command-line argument is present, the specified file
/// is located via [`find_wad_by_name`] (aborting with `I_Error` if not found)
/// and its mission is identified by filename. Otherwise, the function scans
/// all IWAD search directories for any IWAD whose mission bit is set in `mask`.
///
/// Returns a heap-allocated path to the found IWAD, or null if none was found
/// in the auto-scan path. Writes the identified mission to `*mission`.
///
/// Called from `d_main.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below;
//* the freeze-zone caller `d_main.rs` imports the upstream name
//* through the root shim. The `i_error!` abort path and the
//* `-iwad` argv read are carried verbatim.
///
/// # Safety
/// `mission` must be a valid, non-null pointer.
#[doc(alias = "D_FindIWAD")]
#[export_name = "D_FindIWAD"]
pub unsafe extern "C" fn find_iwad(mask: c_int, mission: *mut c_int) -> *mut c_char
{
    let iwadparm = M_CheckParmWithArgs(c"-iwad".as_ptr(), 1);

    if iwadparm != 0
    {
        let iwadfile = *myargv.offset((iwadparm + 1) as isize);
        let result = find_wad_by_name(iwadfile);

        if result.is_null()
        {
            i_error!(
                "IWAD file '{}' not found!",
                std::ffi::CStr::from_ptr(iwadfile).to_string_lossy()
            );
        }

        *mission = identify_iwad_by_name(result, mask);
        result
    }
    else
    {
        c_printf(c"-iwad not specified, trying a few iwad names\n".as_ptr());

        let mut result: *mut c_char = ptr::null_mut();

        build_iwad_dir_list();

        for i in 0..num_iwad_dirs
        {
            if !result.is_null()
            {
                break;
            }
            result = search_directory_for_iwad(iwad_dirs[i as usize], mask, mission);
        }

        result
    }
}
