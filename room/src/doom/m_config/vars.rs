//! The variable engine: lookup, parse, set, bind, and the programmatic
//! get/set family, plus the `.cfg` filename staging (`load_defaults` /
//! `save_defaults` - both no-op on the disk half, mirroring the upstream
//! `ORIGCODE` gating).

#![allow(non_upper_case_globals, static_mut_refs)]

use std::ffi::{c_char, c_float, c_int, c_void, CStr};
use std::ptr;

use super::paths::configdir;
use super::scankeys::SCANTOKEY;
use super::tables::{ConfigVarCollection, ConfigVarEntry, ConfigValueType};
use crate::doom::crt::{c_printf1, strdup};
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::m_misc::M_StringJoinA;
use crate::i_error;

extern "C" {
    /// libc `strcmp`: compare two C strings.
    pub(super) fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    /// libc `atof`: parse a `c_char*` as a double-precision number.
    pub(super) fn atof(s: *const c_char) -> f64;
    /// libc `malloc`: used by `paths.rs`'s config-dir fallback.
    pub(super) fn malloc(n: usize) -> *mut c_void;
}

/// Filename component for the main `.cfg` file (e.g. `"default.cfg"`).
/// Set by `set_config_filenames`.
static mut default_main_config: *mut c_char = ptr::null_mut();
/// Filename component for the extra (chocolate-doom) `.cfg` file. Set by
/// `set_config_filenames`.
static mut default_extra_config: *mut c_char = ptr::null_mut();

/// Linear search for the entry named `name` in a [`ConfigVarCollection`].
///
/// Returns a raw pointer to the matching entry or NULL if no match exists.
/// Mirrors C `SearchCollection`.
///
/// # Safety
///
/// `collection` must point to a valid, initialised [`ConfigVarCollection`].
/// `name` must be a valid NUL-terminated C string.
unsafe fn search_collection(
    collection: *const ConfigVarCollection,
    name: *const c_char,
) -> *mut ConfigVarEntry {
    let collection = &*collection;
    for i in 0..collection.numdefaults {
        let def = collection.defaults.add(i as usize);
        if strcmp(name, (*def).name) == 0 {
            return def;
        }
    }
    ptr::null_mut()
}

/// Find the named entry in either the main or extra collection.
///
/// Searches [`super::tables::doom_defaults`] first and falls back to
/// [`super::tables::extra_defaults`]; aborts via [`crate::i_error!`] if no
/// match is found (an unknown configuration variable is a fatal
/// programming error). Mirrors C `GetDefaultForName`.
///
/// # Safety
///
/// `name` must be a valid NUL-terminated C string.
unsafe fn get_default_for_name(name: *const c_char) -> *mut ConfigVarEntry {
    let mut result = search_collection(&super::tables::doom_defaults, name);
    if result.is_null() {
        result = search_collection(&super::tables::extra_defaults, name);
    }
    if result.is_null() {
        i_error!(
            "Unknown configuration variable: '{}'",
            std::ffi::CStr::from_ptr(name).to_string_lossy()
        );
    }
    result
}

/// Parse an integer parameter as written in a `.cfg` file.
///
/// Recognises `0x` / `0X` hexadecimal prefixes (case-insensitive) and
/// decimal otherwise. Returns `0` on any parse error. Used for the
/// `DEFAULT_INT*` / `DEFAULT_KEY` cases. Diverges from C
/// `ParseIntParameter` in two ways flagged inline below: this port
/// accepts the upper-case `0X` prefix and drops C's leading-zero octal
/// fallback.
fn parse_int_parameter(strparm: &CStr) -> c_int {
    let bytes = strparm.to_bytes();
    // FIXME: C `ParseIntParameter` only checks the lowercase `0x` prefix;
    // accepting `0X` is a Rust-port extension. Verify no `.cfg` file relied
    // on the C behaviour rejecting upper-case prefixes.
    if bytes.len() >= 2 && bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
        if let Ok(val) = i32::from_str_radix(std::str::from_utf8(&bytes[2..]).unwrap_or("0"), 16) {
            return val;
        }
    }
    // FIXME: C `ParseIntParameter` falls back to `sscanf("%i", ...)` which
    // also recognises leading-zero octal; Rust `parse::<i32>()` rejects it
    // and returns 0. Octal config values would silently become 0 here.
    if let Ok(val) = std::str::from_utf8(bytes).unwrap_or("0").parse::<c_int>() {
        return val;
    }
    0
}

/// Apply `value` (raw C string) to the bound location of `def`.
///
/// Mirrors C `SetVariable`. For `String` entries, the value is heap-
/// duplicated via `strdup` and the previous pointer is overwritten without
/// being freed (matching the C behaviour - the old buffer leaks). For
/// `Key` entries, the raw scancode is recorded in `untranslated`, then
/// looked up in [`SCANTOKEY`] (out-of-range scancodes resolve to `0`) and
/// the translated value is stored both in `original_translated` and at the
/// bound location.
///
/// # Safety
///
/// `def` must be a valid pointer to a `ConfigVarEntry` whose `location`
/// field has been bound by [`bind_variable`] to a memory cell of the
/// matching type. `value` must be a valid NUL-terminated C string.
unsafe fn set_variable(def: *mut ConfigVarEntry, value: *const c_char) {
    let def = &mut *def;
    match def.ty {
        ConfigValueType::String => {
            *(def.location as *mut *mut c_char) = strdup(value);
        }
        ConfigValueType::Int | ConfigValueType::IntHex => {
            let strparm = CStr::from_ptr(value);
            *(def.location as *mut c_int) = parse_int_parameter(strparm);
        }
        ConfigValueType::Key => {
            let strparm = CStr::from_ptr(value);
            let intparm = parse_int_parameter(strparm);
            def.untranslated = intparm;
            let translated = if (0..128).contains(&intparm) {
                SCANTOKEY[intparm as usize]
            } else {
                0
            };
            def.original_translated = translated;
            *(def.location as *mut c_int) = translated;
        }
        ConfigValueType::Float => {
            *(def.location as *mut c_float) = atof(value) as c_float;
        }
    }
}

/// Record the default `.cfg` filenames used by [`load_defaults`].
///
/// Mirrors C `M_SetConfigFilenames`. Stores the pointers without copying;
/// the caller owns the underlying buffers and must keep them alive for the
/// remainder of the program.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "M_SetConfigFilenames")]
#[export_name = "M_SetConfigFilenames"]
pub extern "C" fn set_config_filenames(main_config: *mut c_char, extra_config: *mut c_char) {
    unsafe {
        default_main_config = main_config;
        default_extra_config = extra_config;
    }
}

/// Write configuration to disk - no-op on this port.
///
/// In the C source the body of `SaveDefaultCollection` is gated behind
/// `#if ORIGCODE`, which is undefined in the chocolate-doom build used
/// here, so the original is also a no-op at runtime.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "M_SaveDefaults")]
#[export_name = "M_SaveDefaults"]
pub extern "C" fn save_defaults() {
    // No-op: ORIGCODE is undefined, so the file-I/O body is never compiled
    // in the original C either.
}

/// Temporarily swap the active filenames and invoke [`save_defaults`].
///
/// Mirrors C `M_SaveDefaultsAlternate`. Because [`save_defaults`] is a
/// no-op in this build, the function only manipulates filename pointers;
/// no actual writes occur.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_SaveDefaultsAlternate")]
#[export_name = "M_SaveDefaultsAlternate"]
pub extern "C" fn save_defaults_alternate(main: *mut c_char, extra: *mut c_char) {
    unsafe {
        let orig_main = super::tables::doom_defaults.filename;
        let orig_extra = super::tables::extra_defaults.filename;
        super::tables::doom_defaults.filename = main;
        super::tables::extra_defaults.filename = extra;
        save_defaults();
        super::tables::doom_defaults.filename = orig_main;
        super::tables::extra_defaults.filename = orig_extra;
    }
}

/// Compute filenames for the main/extra config files and announce them.
///
/// Mirrors C `M_LoadDefaults` up to (but not including) the actual file
/// parsing step (`LoadDefaultCollection`), which is itself gated by
/// `ORIGCODE` and so is a no-op upstream. `-config` and `-extraconfig`
/// command-line flags override the default filename construction. The
/// resulting filenames are stored in the `doom_defaults` /
/// `extra_defaults` collections for any future call to [`save_defaults`].
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "M_LoadDefaults")]
#[export_name = "M_LoadDefaults"]
pub extern "C" fn load_defaults() {
    unsafe {
        let i = M_CheckParmWithArgs(c"-config".as_ptr().cast_mut(), 1);
        if i != 0 {
            super::tables::doom_defaults.filename = *myargv.offset((i + 1) as isize);
            c_printf1(
                c"\tdefault file: %s\n".as_ptr(),
                super::tables::doom_defaults.filename,
            );
        } else {
            let strs: [*const c_char; 3] =
                [configdir, default_main_config, std::ptr::null()];
            // SAFETY: null-terminated pointer array; result lives for the duration of the program.
            super::tables::doom_defaults.filename = M_StringJoinA(strs.as_ptr());
        }

        c_printf1(
            c"saving config in %s\n".as_ptr(),
            super::tables::doom_defaults.filename,
        );

        let i = M_CheckParmWithArgs(c"-extraconfig".as_ptr().cast_mut(), 1);
        if i != 0 {
            super::tables::extra_defaults.filename = *myargv.offset((i + 1) as isize);
            c_printf1(
                c"        extra configuration file: %s\n".as_ptr(),
                super::tables::extra_defaults.filename,
            );
        } else {
            let strs: [*const c_char; 3] =
                [configdir, default_extra_config, std::ptr::null()];
            // SAFETY: null-terminated pointer array; result lives for the duration of the program.
            super::tables::extra_defaults.filename = M_StringJoinA(strs.as_ptr());
        }

        // No-ops because ORIGCODE is undefined
        let _ = &super::tables::doom_defaults;
        let _ = &super::tables::extra_defaults;
    }
}

/// Bind the named configuration variable to a memory `location`.
///
/// After binding, [`load_defaults`] (and the chocolate-doom config UI)
/// can read and write the value at the supplied pointer. Aborts via
/// `I_Error` if the name is unknown. Mirrors C `M_BindVariable`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `i_sound/config.rs` and `d_main/bind.rs` import the upstream name through the
/// root shim (`i_joystick` / `m_controls` converted their extern
/// declarations to this path call in this same commit).
#[doc(alias = "M_BindVariable")]
#[export_name = "M_BindVariable"]
pub extern "C" fn bind_variable(name: *mut c_char, location: *mut c_void) {
    unsafe {
        let variable = get_default_for_name(name as *const c_char);
        (*variable).location = location;
        (*variable).bound = true;
    }
}

/// Programmatic setter: parse `value` and store it at the bound location
/// for the named variable. Returns `false` if the variable is unknown or
/// unbound. Mirrors C `M_SetVariable`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_SetVariable")]
#[export_name = "M_SetVariable"]
pub extern "C" fn set_config_variable(name: *mut c_char, value: *mut c_char) -> bool {
    unsafe {
        let variable = get_default_for_name(name as *const c_char);
        if variable.is_null() || !(*variable).bound {
            return false;
        }
        set_variable(variable, value as *const c_char);
        true
    }
}

/// Read the value of a bound `Int` or `IntHex` variable by name.
///
/// Returns `0` if the variable is unknown, unbound, or has a non-integer
/// type. Mirrors C `M_GetIntVariable`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_GetIntVariable")]
#[export_name = "M_GetIntVariable"]
pub extern "C" fn get_int_variable(name: *mut c_char) -> c_int {
    unsafe {
        let variable = get_default_for_name(name as *const c_char);
        if variable.is_null()
            || !(*variable).bound
            || ((*variable).ty != ConfigValueType::Int
                && (*variable).ty != ConfigValueType::IntHex)
        {
            return 0;
        }
        *((*variable).location as *mut c_int)
    }
}

/// Read the value of a bound `String` variable by name.
///
/// Returns NULL if the variable is unknown, unbound, or has a non-string
/// type. The returned pointer aliases the storage held by the caller of
/// [`bind_variable`]. Mirrors C `M_GetStrVariable`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_GetStrVariable")]
#[export_name = "M_GetStrVariable"]
pub extern "C" fn get_str_variable(name: *mut c_char) -> *const c_char {
    unsafe {
        let variable = get_default_for_name(name as *const c_char);
        if variable.is_null() || !(*variable).bound || (*variable).ty != ConfigValueType::String {
            return ptr::null();
        }
        *((*variable).location as *mut *const c_char)
    }
}

/// Read the value of a bound `Float` variable by name.
///
/// Returns `0.0` if the variable is unknown, unbound, or has a non-float
/// type. Mirrors C `M_GetFloatVariable`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_GetFloatVariable")]
#[export_name = "M_GetFloatVariable"]
pub extern "C" fn get_float_variable(name: *mut c_char) -> c_float {
    unsafe {
        let variable = get_default_for_name(name as *const c_char);
        if variable.is_null() || !(*variable).bound || (*variable).ty != ConfigValueType::Float {
            return 0.0;
        }
        *((*variable).location as *mut c_float)
    }
}

/// Baseline vectors written before the graduation move (F10 wave F2-b):
/// the parse behaviour the `.cfg` round-trip depends on must be identical
/// before and after the split.
#[cfg(test)]
mod tests {
    use super::parse_int_parameter;
    use std::ffi::CStr;

    /// `parse_int_parameter` known vectors: `0x`/`0X` hex (the upper-case
    /// prefix is the documented Rust-port extension), decimal, and the
    /// documented octal divergence (`"010"` parses as decimal 10, not
    /// octal 8); non-numeric input yields 0.
    #[test]
    fn parse_int_parameter_known_vectors() {
        let p = |s: &[u8]| parse_int_parameter(CStr::from_bytes_with_nul(s).unwrap());
        assert_eq!(p(b"0x10\0"), 16);
        assert_eq!(p(b"0X10\0"), 16);
        assert_eq!(p(b"010\0"), 10);
        assert_eq!(p(b"ff\0"), 0);
    }
}
