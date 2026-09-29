//! Link anchor for the module: calls every exported function so the
//! linker's dead-code elimination cannot strip the `#[no_mangle]`/
//! `#[export_name]` symbols that are called only from C. The anchor itself
//! exists upstream of this split -- name + `#[no_mangle]` carried, body
//! re-pointed at the renamed functions through the root shims. Not wired
//! from `doomgeneric.rs` (pre-move absence preserved).

use std::ptr;

use super::patch::dummy_clip;
use super::{
    V_CopyRect, V_DrawAltTLPatch, V_DrawBlock, V_DrawBox, V_DrawFilledBox, V_DrawHorizLine,
    V_DrawMouseSpeedBox, V_DrawPatch, V_DrawPatchDirect, V_DrawPatchFlipped, V_DrawRawScreen,
    V_DrawShadowedPatch, V_DrawTLPatch, V_DrawVertLine, V_DrawXlaPatch, V_Init,
    V_LoadTintTable, V_LoadXlaTable, V_MarkRect, V_RestoreBuffer, V_ScreenShot,
    V_SetPatchClipCallback, V_UseBuffer, WritePCXfile,
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
pub unsafe extern "C" fn V_Video_Link_Anchor() {
    V_MarkRect(0, 0, 0, 0);
    V_CopyRect(0, 0, ptr::null_mut(), 0, 0, 0, 0);
    V_SetPatchClipCallback(Some(dummy_clip));
    V_DrawPatch(0, 0, ptr::null_mut());
    V_DrawPatchFlipped(0, 0, ptr::null_mut());
    V_DrawPatchDirect(0, 0, ptr::null_mut());
    V_DrawTLPatch(0, 0, ptr::null_mut());
    V_DrawXlaPatch(0, 0, ptr::null_mut());
    V_DrawAltTLPatch(0, 0, ptr::null_mut());
    V_DrawShadowedPatch(0, 0, ptr::null_mut());
    V_LoadTintTable();
    V_LoadXlaTable();
    V_DrawBlock(0, 0, 0, 0, ptr::null_mut());
    V_DrawFilledBox(0, 0, 0, 0, 0);
    V_DrawHorizLine(0, 0, 0, 0);
    V_DrawVertLine(0, 0, 0, 0);
    V_DrawBox(0, 0, 0, 0, 0);
    V_DrawRawScreen(ptr::null_mut());
    V_Init();
    V_UseBuffer(ptr::null_mut());
    V_RestoreBuffer();
    WritePCXfile(ptr::null_mut(), ptr::null_mut(), 0, 0, ptr::null_mut());
    V_ScreenShot(ptr::null_mut());
    V_DrawMouseSpeedBox(0);
}
