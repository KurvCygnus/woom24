//! Link anchor for the module: references every exported function by
//! address so the linker's dead-code elimination cannot strip the
//! `#[no_mangle]`/`#[export_name]` symbols that are called only from C.
//! The anchor itself exists upstream of this split (unlike `r_sky`,
//! where no anchor exists) -- name + `#[no_mangle]` carried, body
//! re-pointed at the renamed functions through the root shims.

use super::{
    R_CheckTextureNumForName, R_FlatNumForName, R_GenerateComposite, R_GenerateLookup, R_GetColumn,
    R_InitColormaps, R_InitData, R_InitFlats, R_InitSpriteLumps, R_InitTextures, R_PrecacheLevel,
    R_TextureNumForName,
};

/// Force the linker to retain all exported symbols in this module.
///
/// Rust's dead-code elimination would otherwise strip `#[no_mangle]` functions
/// that are called only from C.  This anchor function references every exported
/// function by address so the linker keeps them in the final binary.
///
/// # Safety
///
/// Must only be called from the C side during startup. Taking function
/// addresses is safe; no pointers are dereferenced.
#[no_mangle]
pub unsafe extern "C" fn R_Data_Link_Anchor() {
    let _ = R_GenerateComposite as *const () as usize;
    let _ = R_GenerateLookup as *const () as usize;
    let _ = R_GetColumn as *const () as usize;
    let _ = R_InitTextures as *const () as usize;
    let _ = R_InitFlats as *const () as usize;
    let _ = R_InitSpriteLumps as *const () as usize;
    let _ = R_InitColormaps as *const () as usize;
    let _ = R_InitData as *const () as usize;
    let _ = R_FlatNumForName as *const () as usize;
    let _ = R_CheckTextureNumForName as *const () as usize;
    let _ = R_TextureNumForName as *const () as usize;
    let _ = R_PrecacheLevel as *const () as usize;
}
