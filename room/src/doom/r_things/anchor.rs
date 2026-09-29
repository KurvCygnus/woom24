//! Link anchor for the module: calls every exported function so the
//! linker's dead-code elimination cannot strip the `#[no_mangle]`/
//! `#[export_name]` symbols that are called only from C. The anchor
//! itself exists upstream of this split -- name + `#[no_mangle]`
//! carried, body re-pointed at the renamed functions through the root
//! shims. Not wired from `doomgeneric.rs` (pre-move absence preserved).

use super::{
    R_AddSprites, R_ClearSprites, R_DrawMasked, R_DrawMaskedColumn, R_DrawPSprite,
    R_DrawPlayerSprites, R_DrawSprite, R_DrawVisSprite, R_InitSprites, R_NewVisSprite,
    R_ProjectSprite, R_SortVisSprites,
};

/// Linker anchor: references every public `#[no_mangle]` function in this
/// module so the linker does not dead-strip them when building as a library.
/// Not intended to be called at runtime.
///
/// # Safety
/// This function has no preconditions and performs no operations; it only
/// takes function addresses.
#[no_mangle]
pub unsafe extern "C" fn R_Things_Link_Anchor() {
    let _ = R_InitSprites as *const () as usize;
    let _ = R_ClearSprites as *const () as usize;
    let _ = R_NewVisSprite as *const () as usize;
    let _ = R_DrawMaskedColumn as *const () as usize;
    let _ = R_DrawVisSprite as *const () as usize;
    let _ = R_ProjectSprite as *const () as usize;
    let _ = R_AddSprites as *const () as usize;
    let _ = R_DrawPSprite as *const () as usize;
    let _ = R_DrawPlayerSprites as *const () as usize;
    let _ = R_SortVisSprites as *const () as usize;
    let _ = R_DrawSprite as *const () as usize;
    let _ = R_DrawMasked as *const () as usize;
}
