//! Link anchor for the WAD directory: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use std::ptr;

use super::{
    W_AddFile, W_CacheLumpName, W_CacheLumpNum, W_CheckCorrectIWAD, W_CheckNumForName,
    W_GenerateHashTable, W_GetNumForName, W_LumpLength, W_LumpNameHash, W_NumLumps, W_ReadLump,
    W_ReleaseLumpName, W_ReleaseLumpNum,
};

/// Link anchor referencing every public function of this module --
/// the renamed Rust functions behind the upstream-name shims -- so
/// the linker keeps their symbols alive. Not part of the original
/// Doom API.
///
/// # Safety
///
/// Passes null pointers everywhere and would crash if called.
/// Treat as link-only.
#[no_mangle]
pub unsafe extern "C" fn W_Wad_Link_Anchor()
{
    W_LumpNameHash(ptr::null());
    W_AddFile(ptr::null_mut());
    W_NumLumps();
    W_CheckNumForName(ptr::null());
    W_GetNumForName(ptr::null());
    W_LumpLength(0);
    W_ReadLump(0, ptr::null_mut());
    W_CacheLumpNum(0, 0);
    W_CacheLumpName(ptr::null(), 0);
    W_ReleaseLumpNum(0);
    W_ReleaseLumpName(ptr::null());
    W_GenerateHashTable();
    W_CheckCorrectIWAD(0);
}
