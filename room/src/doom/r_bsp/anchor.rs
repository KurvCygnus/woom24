//! Link anchor for the module: calls every exported function so the
//! linker's dead-code elimination cannot strip the `#[no_mangle]`/
//! `#[export_name]` symbols that are called only from C. The anchor
//! itself exists upstream of this split -- name + `#[no_mangle]`
//! carried, body re-pointed at the renamed functions through the root
//! shims.

use std::ptr;

use super::{
    R_CheckBBox, R_ClipPassWallSegment, R_ClipSolidWallSegment, R_ClearClipSegs, R_ClearDrawSegs,
    R_RenderBSPNode, R_Subsector,
};

/// Linker anchor: references every public `#[no_mangle]` function in this
/// module so the linker does not dead-strip them when building as a library.
/// Not intended to be called at runtime.
///
/// # Safety
/// Calls all exported functions with zero/null arguments purely to create
/// symbol references. Behavior is undefined if called during normal
/// execution; this function exists only to prevent linker GC.
#[no_mangle]
pub unsafe extern "C" fn R_Bsp_Link_Anchor() {
    R_ClearDrawSegs();
    R_ClearClipSegs();
    R_RenderBSPNode(0);
    R_Subsector(0);
    R_ClipSolidWallSegment(0, 0);
    R_ClipPassWallSegment(0, 0);
    R_CheckBBox(ptr::null_mut());
}
