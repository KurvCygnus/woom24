//! Link anchor for the teleporter module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::EV_Teleport;

/// Anchor function referenced from `doomgeneric_Create` to ensure
/// `EV_Teleport` survives link-time dead-code elimination.
#[no_mangle]
pub extern "C" fn P_Telept_Link_Anchor()
{
    let _ = EV_Teleport as *const () as usize;
}
