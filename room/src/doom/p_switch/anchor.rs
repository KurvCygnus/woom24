//! Link anchor for the switch module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{P_ChangeSwitchTexture, P_InitSwitchList, P_StartButton, P_UseSpecialLine};

/// Ensures all public symbols in this module are retained by the linker.
///
/// Not intended for direct use in game logic.
///
/// # Safety
///
/// Accesses function pointers as raw integers purely to prevent dead-code
/// elimination; no actual function calls are made.
#[no_mangle]
pub unsafe extern "C" fn P_Switch_Link_Anchor()
{
    let _ = P_InitSwitchList as *const () as usize;
    let _ = P_StartButton as *const () as usize;
    let _ = P_ChangeSwitchTexture as *const () as usize;
    let _ = P_UseSpecialLine as *const () as usize;
}
