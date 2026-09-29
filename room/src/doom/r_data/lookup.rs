//! The name-lookup entry points the freeze-zone callers invoke:
//! `R_FlatNumForName` / `R_TextureNumForName` (fatal on miss) and
//! `R_CheckTextureNumForName` (miss = `-1`). The texture wrapper's
//! hash-chain decision core is extracted into `dtmc`; the wrappers
//! themselves are kept whole (the `I_Error` marshalling and the flat
//! `i - firstflat` arithmetic ride the demo goldens end-to-end).

use std::ffi::{c_char, c_int, c_uint, CStr};

use crate::i_error;
use crate::doom::w_wad::{W_CheckNumForName, W_LumpNameHash};

use super::column_cache::textures_hashtable;
use super::dtmc::texture_index_for_name;
use super::globals::{firstflat, numtextures};

/// Look up a flat by name and return its flat index (lump number minus `firstflat`).
///
/// Calls `W_CheckNumForName`; if the lump does not exist, calls `I_Error`.
/// The returned value is suitable for use as an index into `flattranslation`.
///
/// Called by `g_game.c` to resolve `SKYFLATNAME` into `skyflatnum`, and by
/// various map-object and sector-setup code paths.
///
/// # Safety
///
/// - `name` must be a valid, null-terminated C string of at most 8 characters.
/// - `R_InitFlats` must have been called first.
#[doc(alias = "R_FlatNumForName")]
#[export_name = "R_FlatNumForName"]
pub unsafe extern "C" fn flat_num_for_name(name: *mut c_char) -> c_int {
    let i = W_CheckNumForName(name);
    if i == -1 {
        let mut namet: [c_char; 9] = [0; 9];
        std::ptr::copy_nonoverlapping(name, namet.as_mut_ptr(), 8);
        i_error!(
            "R_FlatNumForName: {} not found",
            CStr::from_ptr(namet.as_ptr()).to_string_lossy()
        );
    }
    i - firstflat
}

/// Look up a texture by name and return its index, or `-1` if not found.
///
/// A name starting with `'-'` is the "no texture" marker and returns `0`
/// immediately without a hash lookup.  Otherwise the hash table built by
/// `generate_texture_hash_table` is used for O(1) average-case lookup via
/// the `dtmc::texture_index_for_name` decision core.
///
/// Called by `p_setup.c` (and others) when loading map geometry; the return
/// value of `-1` signals that the texture slot is intentionally empty.
///
/// # Safety
///
/// - `name` must be a valid, null-terminated C string of at most 8 characters.
/// - `R_InitTextures` must have been called first.
#[doc(alias = "R_CheckTextureNumForName")]
#[export_name = "R_CheckTextureNumForName"]
pub unsafe extern "C" fn check_texture_num_for_name(name: *mut c_char) -> c_int {
    let key = (W_LumpNameHash(name) % numtextures as c_uint) as usize;
    let head = *textures_hashtable.add(key);
    texture_index_for_name(name, head)
}

/// Look up a texture by name and return its index; calls `I_Error` if not found.
///
/// Wraps `check_texture_num_for_name` and aborts if the result is `-1`.
/// Used wherever a missing texture is a fatal error (e.g., side-def loading
/// in `p_setup.c`, switch definitions in `p_switch.c`).
///
/// # Safety
///
/// - `name` must be a valid, null-terminated C string of at most 8 characters.
/// - `R_InitTextures` must have been called first.
#[doc(alias = "R_TextureNumForName")]
#[export_name = "R_TextureNumForName"]
pub unsafe extern "C" fn texture_num_for_name(name: *mut c_char) -> c_int {
    let i = check_texture_num_for_name(name);
    if i == -1 {
        i_error!(
            "R_TextureNumForName: {} not found",
            CStr::from_ptr(name).to_string_lossy()
        );
    }
    i
}
