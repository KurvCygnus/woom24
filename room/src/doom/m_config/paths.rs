//! Configuration paths: the global `configdir` directory pointer, its
//! `-savedir`-era setter, and the savegame-directory computation. The
//! Windows-only discovery paths of the C source are skipped here; only the
//! "current directory" fallback remains.

#![allow(non_upper_case_globals, clippy::manual_c_str_literals)]

use std::ffi::c_char;
use std::ptr;

use super::vars::{malloc, strcmp};
use crate::doom::crt::{c_printf1, strdup};
use crate::doom::m_misc::{M_MakeDirectory, M_StringJoinA};

/// Unix directory separator string with trailing NUL (C
/// `DIR_SEPARATOR_S = "/"`). Used by [`get_save_game_dir`].
const DIR_SEPARATOR_S: &[u8] = b"/\0";

/// Global configuration directory (mirrors C `char *configdir`).
///
/// Initialised by [`set_config_dir`] from the command line or from
/// `default_config_dir_fallback`. Read by `load_defaults` and
/// [`get_save_game_dir`]. Exposed with C linkage because legacy C call sites
/// read it directly.
#[no_mangle]
pub static mut configdir: *mut c_char = ptr::null_mut();

/// Return a fresh malloc'd `"."` string as the fallback configuration
/// directory.
///
/// Used when [`set_config_dir`] is called with NULL. The C source has
/// elaborate per-platform discovery; this port keeps only the
/// "current directory" fallback. The returned pointer must be freed by
/// the caller (or, in practice, leaks for the lifetime of the program).
///
/// # Safety
///
/// Relies on libc `malloc` succeeding; the result is not null-checked,
/// matching the C source.
unsafe fn default_config_dir_fallback() -> *mut c_char {
    let result = malloc(2) as *mut c_char;
    *result = b'.' as c_char;
    *result.offset(1) = 0;
    result
}

/// Set the global [`configdir`] and create the directory if needed.
///
/// Passing NULL falls back to `default_config_dir_fallback`. A non-empty
/// path is echoed to the console. The path is created (recursive parents
/// are NOT created - matches C semantics) via [`M_MakeDirectory`]. Mirrors
/// C `M_SetConfigDir`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "M_SetConfigDir")]
#[export_name = "M_SetConfigDir"]
pub extern "C" fn set_config_dir(dir: *mut c_char) {
    unsafe {
        if !dir.is_null() {
            configdir = dir;
        } else {
            configdir = default_config_dir_fallback();
        }

        if strcmp(configdir, c"".as_ptr()) != 0 {
            c_printf1(
                c"Using %s for configuration and saves\n".as_ptr(),
                configdir,
            );
        }

        M_MakeDirectory(configdir);
    }
}

/// Compute and create the directory used to store save games.
///
/// Returns a heap-allocated path of the form
/// `"<configdir>/.savegame/"` on this port - the C source uses
/// `"<configdir>/savegame/<iwadname>/"` when `ORIGCODE` is defined. The
/// `_iwadname` parameter is therefore unused but kept for ABI
/// compatibility. Returns an empty `strdup("")` if `configdir` is empty
/// (Windows-style "no config dir" mode). The returned pointer is owned by
/// the caller and must be freed.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "M_GetSaveGameDir")]
#[export_name = "M_GetSaveGameDir"]
pub extern "C" fn get_save_game_dir(_iwadname: *mut c_char) -> *mut c_char {
    unsafe {
        if strcmp(configdir, c"".as_ptr()) == 0 {
            strdup(c"".as_ptr())
        } else {
            let strs: [*const c_char; 4] = [
                configdir,
                DIR_SEPARATOR_S.as_ptr() as *const c_char,
                c".savegame/".as_ptr(),
                std::ptr::null(),
            ];
            // SAFETY: null-terminated pointer array; ownership transferred to caller via return.
            let savegamedir = M_StringJoinA(strs.as_ptr());
            M_MakeDirectory(savegamedir);
            c_printf1(c"Using %s for savegames\n".as_ptr(), savegamedir);
            savegamedir
        }
    }
}
