//! The directory-level IWAD scan: filename-component matching,
//! per-directory existence probing, the mask-filtered `IWADS` walk, and
//! filename-based mission identification. Verbatim ports of the
//! corresponding `d_iwad.c` file-static helpers. The two pure string
//! helpers carry the pre-move baseline vectors (F10 wave C1).

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use super::table::IWADS;
use crate::doom::crt::{c_printf1, strcasecmp, strdup};
use crate::doom::d_mode::none;
use crate::doom::m_misc::M_StringJoinA;

use super::{free, M_FileExists, strcmp, strlen, strrchr};

/// Path component separator character (`/`).
///
/// Used when constructing full file paths from a directory and a filename.
/// Housed beside its heaviest users (`dir_is_file`,
/// `check_directory_has_iwad`); `find` reads `DIR_SEPARATOR_S` too.
pub(super) const DIR_SEPARATOR: c_char = b'/' as c_char;

/// Null-terminated string form of [`DIR_SEPARATOR`], suitable for passing to
/// C string-joining functions.
pub(super) const DIR_SEPARATOR_S: &[u8] = b"/\0";

/// Returns non-zero if `path` is a file path whose final component equals
/// `filename` (case-insensitive comparison).
///
/// For example, `DirIsFile("/games/doom.wad", "doom.wad")` returns `1`.
/// Corresponds to `DirIsFile` in `d_iwad.c`.
///
/// # Safety
/// Both `path` and `filename` must be valid, non-null pointers to
/// null-terminated C strings. The strings must remain valid for the duration
/// of the call. The function passes these pointers directly to the C FFI
/// functions `strlen` and `strcasecmp`, which impose the same requirements.
pub(super) unsafe fn dir_is_file(path: *mut c_char, filename: *mut c_char) -> c_int
{
    let path_len = strlen(path);
    let filename_len = strlen(filename);

    if path_len > filename_len &&
        *path.add(path_len - filename_len - 1) == DIR_SEPARATOR &&
        strcasecmp(path.add(path_len - filename_len), filename) == 0
        { 1 }
    else { 0 }
}

/// Checks whether `dir` contains the IWAD named `iwadname`, returning a
/// heap-allocated full path on success or null on failure.
///
/// Two cases are handled:
/// - If `dir` is itself a path to the IWAD file (i.e. `DirIsFile` returns
///   true and the file exists), a `strdup` of `dir` is returned.
/// - Otherwise the path `dir/iwadname` is constructed and tested. The special
///   case `dir == "."` elides the directory prefix.
///
/// The caller is responsible for `free`-ing the returned string.
/// Corresponds to `CheckDirectoryHasIWAD` in `d_iwad.c`.
///
/// # Safety
/// Both `dir` and `iwadname` must be valid, non-null pointers to
/// null-terminated C strings that remain valid for the duration of the call.
/// These pointers are passed to `strlen`, `strcasecmp`, `strcmp`, `strdup`,
/// and `free` via C FFI, all of which require the same pointer-validity
/// guarantee. The heap-allocated string returned on success must be freed by
/// the caller using `free`.
pub(super) unsafe fn check_directory_has_iwad(
    dir: *mut c_char,
    iwadname: *mut c_char,
) -> *mut c_char
{
    if dir_is_file(dir, iwadname) != 0 && M_FileExists(dir) != 0 { return strdup(dir); }

    let filename = if strcmp(dir, c".".as_ptr()) == 0 { strdup(iwadname) }
    else
    {
        let strs: [*const c_char; 4] = [
            dir as *const c_char,
            DIR_SEPARATOR_S.as_ptr() as *const c_char,
            iwadname as *const c_char,
            ptr::null(),
        ];
        // SAFETY: null-terminated pointer array; ownership transferred to caller via return.
        M_StringJoinA(strs.as_ptr())
    };

    c_printf1(c"Trying IWAD file:%s\n".as_ptr(), filename);

    if M_FileExists(filename) != 0 { return filename; }

    free(filename as *mut c_void);
    ptr::null_mut()
}

/// Searches a single directory `dir` for the first IWAD entry in `IWADS`
/// that passes the `mask` filter and exists on disk.
///
/// On success, writes the matched mission to `*mission` and returns a
/// heap-allocated path string (caller must `free` it). Returns null if no
/// matching IWAD is found. Corresponds to `SearchDirectoryForIWAD` in
/// `d_iwad.c`.
///
/// # Safety
/// `mission` must be a valid, non-null pointer.
pub(super) unsafe fn search_directory_for_iwad(
    dir: *mut c_char,
    mask: c_int,
    mission: *mut c_int,
) -> *mut c_char
{
    for i in 0..IWADS.len()
    {
        if ((1 << IWADS[i].mission) & mask) == 0 { continue; }

        let filename = check_directory_has_iwad(dir, IWADS[i].name);

        if !filename.is_null()
        {
            *mission = IWADS[i].mission;
            return filename;
        }
    }

    ptr::null_mut()
}

/// Identifies the game mission for an IWAD given its filename.
///
/// Strips any leading directory components from `name`, then performs a
/// case-insensitive comparison against each entry in `IWADS` that passes the
/// `mask` filter. Returns the matching `d_mode::*` mission constant, or
/// `none` if the filename is not recognised.
///
/// Corresponds to `IdentifyIWADByName` in `d_iwad.c`.
///
/// # Safety
/// `name` must be a valid, non-null pointer to a null-terminated C string that
/// remains valid for the duration of the call. The pointer is passed to the C
/// FFI functions `strrchr` and `strcasecmp`, which require a valid
/// null-terminated string. The pointer returned by `strrchr` (if non-null) is
/// an interior pointer into the same string and is used only within this call.
pub(super) unsafe fn identify_iwad_by_name(mut name: *mut c_char, mask: c_int) -> c_int
{
    let p = strrchr(name, DIR_SEPARATOR as c_int);
    if !p.is_null() { name = p.add(1); }

    let mut mission = none;

    for i in 0..IWADS.len()
    {
        if ((1 << IWADS[i].mission) & mask) == 0 { continue; }

        if strcasecmp(name, IWADS[i].name) == 0
        {
            mission = IWADS[i].mission;
            break;
        }
    }

    mission
}

/// The d_iwad baseline vectors (F10 wave C1, pre-move commit
/// `9baf5ff`): written against the pre-move `DirIsFile` /
/// `CheckDirectoryHasIWAD` bodies and re-pointed to the graduated names
/// after the split -- same vectors, same results. They pin the
/// trailing-component match of `dir_is_file` (case-insensitive,
/// separator required, partial names rejected) and the three branches
/// of `check_directory_has_iwad` (the `dir == "."` elision probes the
/// bare name, the join branch builds `dir/name`, and the `dir_is_file`
/// + `M_FileExists` branch returns a copy of the directory itself).
#[cfg(test)]
mod tests
{
    use std::ffi::{CStr, CString};

    use super::*;

    /// Creates an empty probe file and returns its full path with
    /// forward-slash separators (the separator this module hardcodes),
    /// so the vectors behave identically on Windows and Unix hosts.
    fn make_probe_path(tag: &str) -> String
    {
        let mut path = std::env::temp_dir();
        path.push(format!("woom24_c1_probe_{tag}.wad"));
        std::fs::write(&path, b"").unwrap();
        path.to_str().unwrap().replace('\\', "/")
    }

    /// Converts a Rust string to an owned C string pointer.
    fn cstr(s: &str) -> CString { CString::new(s).unwrap() }

    /// The trailing path component matches case-insensitively.
    #[test]
    fn dir_is_file_trailing_component_matches()
    {
        unsafe
        {
            let path = cstr("/games/doom.wad");
            let name = cstr("doom.wad");
            assert_eq!(dir_is_file(path.as_ptr().cast_mut(), name.as_ptr().cast_mut()), 1);
        }
    }

    /// A shared prefix that does not end at a path separator must not
    /// count as a match (`mydoom.wad` is not `doom.wad`).
    #[test]
    fn dir_is_file_partial_name_no_match()
    {
        unsafe
        {
            let path = cstr("/games/mydoom.wad");
            let name = cstr("doom.wad");
            assert_eq!(dir_is_file(path.as_ptr().cast_mut(), name.as_ptr().cast_mut()), 0);
        }
    }

    /// A bare filename (no directory separator before the final
    /// component) never matches, even against itself.
    #[test]
    fn dir_is_file_bare_name_no_separator_no_match()
    {
        unsafe
        {
            let path = cstr("doom.wad");
            let name = cstr("doom.wad");
            assert_eq!(dir_is_file(path.as_ptr().cast_mut(), name.as_ptr().cast_mut()), 0);
        }
    }

    /// With `dir == "."` the directory prefix is elided: the candidate
    /// probed is the bare name, so an absolute existing path passed as
    /// `iwadname` comes back verbatim (never `./`-joined).
    #[test]
    fn check_directory_elides_dot_prefix()
    {
        let probe = make_probe_path("elide");
        unsafe
        {
            let dir = cstr(".");
            let name = cstr(&probe);
            let result =
                check_directory_has_iwad(dir.as_ptr().cast_mut(), name.as_ptr().cast_mut());
            assert!(!result.is_null());
            assert_eq!(CStr::from_ptr(result).to_str().unwrap(), probe);
            free(result as *mut c_void);
        }
    }

    /// A real directory joins `dir/name` with the hardcoded `/`
    /// separator and returns the existing path.
    #[test]
    fn check_directory_joins_dir_and_name()
    {
        let probe_file = make_probe_path("join");
        let dir = std::path::Path::new(&probe_file).
            parent().
            unwrap().
            to_str().
            unwrap().
            to_string();
        let joined = format!("{dir}/woom24_c1_probe_join.wad");
        unsafe
        {
            let dir = cstr(&dir);
            let name = cstr("woom24_c1_probe_join.wad");
            let result = check_directory_has_iwad(dir.as_ptr().cast_mut(), name.as_ptr().cast_mut());
            assert!(!result.is_null());
            assert_eq!(CStr::from_ptr(result).to_str().unwrap(), joined);
            free(result as *mut c_void);
        }
    }

    /// When `dir` itself is the IWAD file, a copy of `dir` is returned
    /// (the `dir_is_file` + `M_FileExists` first branch).
    #[test]
    fn check_directory_dir_is_file_branch_returns_copy_of_dir()
    {
        let probe = make_probe_path("self");
        unsafe
        {
            let dir = cstr(&probe);
            let name = cstr("woom24_c1_probe_self.wad");
            let result = check_directory_has_iwad(dir.as_ptr().cast_mut(), name.as_ptr().cast_mut());
            assert!(!result.is_null());
            assert_eq!(CStr::from_ptr(result).to_str().unwrap(), probe);
            free(result as *mut c_void);
        }
    }
}
