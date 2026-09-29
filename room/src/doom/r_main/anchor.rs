//! Link anchor for the module: force-references every exported function so
//! the linker's dead-code elimination cannot strip the `#[no_mangle]`/
//! `#[export_name]` symbols that are called only from C. The anchor itself
//! exists upstream of this split -- name + `#[no_mangle]` carried, body
//! re-pointed at the renamed functions through the root shims (the ONE
//! live renderer anchor: `doomgeneric.rs` imports and calls it).

use super::{
    R_AddPointToBox, R_ExecuteSetViewSize, R_Init, R_InitLightTables, R_InitPointToAngle,
    R_InitTables, R_InitTextureMapping, R_PointInSubsector, R_PointOnSegSide, R_PointOnSide,
    R_PointToAngle, R_PointToAngle2, R_PointToDist, R_RenderPlayerView, R_ScaleFromGlobalAngle,
    R_SetViewSize, R_SetupFrame,
};

/// Linker anchor: references every public `#[no_mangle]` function in this
/// module so the linker does not dead-strip them when building as a library.
/// Not intended to be called at runtime.
///
/// # Safety
/// This function only takes addresses of functions; no actual calls are made.
/// Safe to call at any time.
#[no_mangle]
pub unsafe extern "C" fn R_Main_Link_Anchor() {
    let _ = R_AddPointToBox as *const () as usize;
    let _ = R_PointOnSide as *const () as usize;
    let _ = R_PointOnSegSide as *const () as usize;
    let _ = R_PointToAngle as *const () as usize;
    let _ = R_PointToAngle2 as *const () as usize;
    let _ = R_PointToDist as *const () as usize;
    let _ = R_InitPointToAngle as *const () as usize;
    let _ = R_ScaleFromGlobalAngle as *const () as usize;
    let _ = R_InitTables as *const () as usize;
    let _ = R_InitTextureMapping as *const () as usize;
    let _ = R_InitLightTables as *const () as usize;
    let _ = R_SetViewSize as *const () as usize;
    let _ = R_ExecuteSetViewSize as *const () as usize;
    let _ = R_Init as *const () as usize;
    let _ = R_PointInSubsector as *const () as usize;
    let _ = R_SetupFrame as *const () as usize;
    let _ = R_RenderPlayerView as *const () as usize;
}
