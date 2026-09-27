//! The IWAD suggestion and enumeration family: `D_FindAllIWADs`,
//! `D_SaveGameIWADName`, `D_SuggestIWADName`, `D_SuggestGameName`, and
//! the `D_CheckCorrectIWAD` no-op, verbatim ports of the corresponding
//! `d_iwad.c` functions. Returned pointers reference static literals
//! (or CRT allocations, for `find_all_iwads`) exactly as upstream --
//! never rewrite the ownership.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::table::{iwad_t, IWADS};
use crate::doom::d_mode::indetermined;

use super::malloc;

/// Finds all IWAD files on disk whose mission bits are set in `mask`.
///
/// Allocates and returns a null-terminated array of pointers into `IWADS`.
/// Each element points to a static `iwad_t` whose backing WAD file was found
/// on disk. The array itself is heap-allocated and must be freed by the caller
/// (the pointed-to `iwad_t` entries must not be freed). Corresponds to
/// `D_FindAllIWADs` in `d_iwad.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below.
//* Dead-but-exported (zero callers in the tree): kept for symbol-set
//* byte-identity, retires with the freeze zone (p_spec precedent).
///
/// # Safety
/// The returned pointer must be freed with `free` when no longer needed.
#[doc(alias = "D_FindAllIWADs")]
#[export_name = "D_FindAllIWADs"]
pub unsafe extern "C" fn find_all_iwads(mask: c_int) -> *mut *const iwad_t
{
    let result =
        malloc(std::mem::size_of::<*const iwad_t>() * (IWADS.len() + 1)) as *mut *const iwad_t;
    let mut result_len: usize = 0;

    for i in 0..IWADS.len()
    {
        if ((1 << IWADS[i].mission) & mask) == 0
        {
            continue;
        }

        let filename = super::find::find_wad_by_name(IWADS[i].name);

        if !filename.is_null()
        {
            *result.add(result_len) = &IWADS[i];
            result_len += 1;
        }
    }

    *result.add(result_len) = ptr::null();

    result
}

/// Returns the canonical IWAD filename for the given `gamemission`.
///
/// Walks `IWADS` and returns the `name` field of the first entry whose
/// mission matches `gamemission`. Falls back to `"unknown.wad"` if no match
/// is found. The returned pointer points into a static string and must not be
/// freed. This name is used as the savegame subdirectory so that `doom.wad`
/// and `doom1.wad` saves share the same location.
///
/// Called from `d_main.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below;
//* the freeze-zone caller `d_main.rs` imports the upstream name
//* through the root shim.
#[doc(alias = "D_SaveGameIWADName")]
#[export_name = "D_SaveGameIWADName"]
pub unsafe extern "C" fn save_game_iwad_name(gamemission: c_int) -> *mut c_char
{
    for i in 0..IWADS.len()
    {
        if gamemission == IWADS[i].mission
        {
            return IWADS[i].name;
        }
    }
    c"unknown.wad".as_ptr().cast_mut()
}

/// Returns the canonical IWAD filename that best matches `mission` and `mode`.
///
/// Walks `IWADS` and returns the `name` field of the first entry where both
/// `mission` and `mode` match. Falls back to `"unknown.wad"` if no match is
/// found. The returned pointer points into a static string and must not be
/// freed. Corresponds to `D_SuggestIWADName` in `d_iwad.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below.
//* Dead-but-exported (zero callers in the tree): kept for symbol-set
//* byte-identity, retires with the freeze zone (p_spec precedent).
#[doc(alias = "D_SuggestIWADName")]
#[export_name = "D_SuggestIWADName"]
pub unsafe extern "C" fn suggest_iwad_name(mission: c_int, mode: c_int) -> *mut c_char
{
    for i in 0..IWADS.len()
    {
        if IWADS[i].mission == mission && IWADS[i].mode == mode
        {
            return IWADS[i].name;
        }
    }
    c"unknown.wad".as_ptr().cast_mut()
}

/// Returns a human-readable game name for the given `mission` and `mode`.
///
/// Walks `IWADS` and returns the `description` field of the first entry
/// where the mission matches and either the mode matches or `mode` is
/// `indetermined`. Falls back to `"Unknown game?"` if no match is
/// found. The returned pointer points into a static string and must not be
/// freed. Called from `w_wad.c`.
///
//* The pre-move export symbol is kept with `#[export_name]` below;
//* the graduated consumer `w_wad/iwad.rs` imports the upstream name
//* through the root shim.
#[doc(alias = "D_SuggestGameName")]
#[export_name = "D_SuggestGameName"]
pub unsafe extern "C" fn suggest_game_name(mission: c_int, mode: c_int) -> *mut c_char
{
    for i in 0..IWADS.len()
    {
        if IWADS[i].mission == mission && (mode == indetermined || IWADS[i].mode == mode)
        {
            return IWADS[i].description;
        }
    }
    c"Unknown game?".as_ptr().cast_mut()
}

/// Validates that the loaded IWAD matches the expected `_mission`.
///
/// This function is intentionally a no-op. The C original also provides an
/// empty implementation in the doomgeneric fork. No validation is performed.
///
//* The pre-move export symbol is kept with `#[export_name]` below;
//* the pre-move signature is SAFE `pub extern "C"` and stays safe
//* (signature-parity rule). Dead-but-exported: retires with the
//* freeze zone. See the module-root archaeology note.
#[doc(alias = "D_CheckCorrectIWAD")]
#[export_name = "D_CheckCorrectIWAD"]
pub extern "C" fn check_correct_iwad(_mission: c_int)
{
    // Not implemented in original C codebase.
}
