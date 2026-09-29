//! Environment / path helpers: integer parsing, the 8.3 lump-name
//! extractor, and the HOME / XDG config-directory resolution (Rust-side
//! additions centralising what vanilla scatters through `getenv` calls),
//! plus the Windows-only OEM-codepage stub.

use std::ffi::{c_char, c_int, CStr};

use super::files::{DIR_SEPARATOR, getenv, strlen, toupper};
use super::strings::string_join_array;
use crate::doom::crt::c_sscanf1;

/// Parse `str` as an integer into `*result`, returning `1` on success.
///
/// Recognised forms (matching C `M_StrToInt`): `0x` / `0X` hex, leading-zero
/// octal, and plain decimal. Returns `0` if none of the formats match. The C
/// original returns a `boolean`; the Rust port keeps the `c_int` ABI.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `i_system` / `p_map` / `p_spec` import the upstream name through the
/// root shim.
#[doc(alias = "M_StrToInt")]
#[export_name = "M_StrToInt"]
pub extern "C" fn str_to_int(str: *const c_char, result: *mut c_int) -> c_int {
    unsafe {
        if c_sscanf1(str, c" 0x%x".as_ptr(), result) == 1 {
            return 1;
        }
        if c_sscanf1(str, c" 0X%x".as_ptr(), result) == 1 {
            return 1;
        }
        if c_sscanf1(str, c" 0%o".as_ptr(), result) == 1 {
            return 1;
        }
        if c_sscanf1(str, c" %d".as_ptr(), result) == 1 {
            return 1;
        }
        0
    }
}

/// Extract the upper-cased 8.3 base name of `path` into the 8-byte `dest`.
///
/// Scans backwards from the end of `path` to find the final directory
/// separator, copies characters from that point until the next `.` or NUL,
/// up to a maximum of 8 bytes, converting each to upper case. The
/// destination is zero-padded to 8 bytes. Names longer than 8 characters
/// are truncated with a warning printed to stderr.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `w_wad` imports the upstream name through the root shim.
#[doc(alias = "M_ExtractFileBase")]
#[export_name = "M_ExtractFileBase"]
pub extern "C" fn extract_file_base(path: *mut c_char, dest: *mut c_char) {
    unsafe {
        let len = strlen(path);
        let mut src = path.add(len).sub(1);
        while src != path && *src.sub(1) != DIR_SEPARATOR {
            src = src.sub(1);
        }
        let filename = src;
        let mut length: usize = 0;
        std::ptr::write_bytes(dest, 0, 8);
        while *src != 0 && *src != b'.' as c_char {
            if length >= 8 {
                let filename_cstr = CStr::from_ptr(filename);
                let dest_cstr = CStr::from_ptr(dest);
                eprintln!(
                    "Warning: Truncated '{}.????????' lump name to '{}'.",
                    filename_cstr.to_string_lossy(),
                    dest_cstr.to_string_lossy()
                );
                break;
            }
            *dest.add(length) = toupper(*src as c_int) as c_char;
            src = src.add(1);
            length += 1;
        }
    }
}

/// Returns `getenv("HOME")` - the user's home directory, or NULL if unset.
///
/// Note: not present in vanilla `m_misc.c` (that file's per-user paths are
/// resolved inline via `getenv` calls scattered through `M_GetSaveGameDir`
/// and friends); this port lifts the lookup into a named helper.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_HomeDir")]
#[export_name = "M_HomeDir"]
pub extern "C" fn home_dir() -> *const c_char {
    unsafe { getenv(c"HOME".as_ptr()) }
}

/// Returns the default configuration directory.
///
/// Resolution order:
/// 1. If `HOME` is unset, returns the static `"."`.
/// 2. If `XDG_CONFIG_HOME` is set, returns `"<XDG_CONFIG_HOME>/doom"`.
/// 3. Otherwise returns `"<HOME>/.config/doom"`.
///
/// Cases 2 and 3 return a heap-allocated path (via `M_StringJoinA` /
/// `malloc`); case 1 returns a static literal. Callers cannot distinguish
/// the two, so the returned pointer must NOT be freed.
///
/// Note: not present in vanilla `m_misc.c` (`M_SetConfigDir` in the C port
/// computes a fallback inline). This Rust helper centralises XDG-aware
/// resolution.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_DefaultConfigDir")]
#[export_name = "M_DefaultConfigDir"]
pub extern "C" fn default_config_dir() -> *const c_char {
    unsafe {
        let home = home_dir();
        if home.is_null() {
            return c".".as_ptr();
        }
        let xdg = getenv(c"XDG_CONFIG_HOME".as_ptr());
        if !xdg.is_null() {
            let strs: [*const c_char; 3] = [xdg, c"/doom".as_ptr(), std::ptr::null()];
            // SAFETY: null-terminated pointer array; result intentionally leaked (see above).
            return string_join_array(strs.as_ptr());
        }
        let strs: [*const c_char; 3] = [home, c"/.config/doom".as_ptr(), std::ptr::null()];
        // SAFETY: null-terminated pointer array; result intentionally leaked (see above).
        string_join_array(strs.as_ptr())
    }
}

/// Stub for the Windows-only `M_OEMToUTF8` (always returns NULL on this port).
///
/// The C original lives behind `#ifdef _WIN32` and converts OEM-encoded
/// strings to UTF-8 via `MultiByteToWideChar` / `WideCharToMultiByte`. The
/// non-Windows build has no such call site; this stub exists only for ABI
/// completeness.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "M_OEMToUTF8")]
#[export_name = "M_OEMToUTF8"]
pub extern "C" fn oem_to_utf8(_oem: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}
