//! Link anchor for the doors module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{
    EV_DoDoor, EV_DoLockedDoor, EV_VerticalDoor, P_SpawnDoorCloseIn30, P_SpawnDoorRaiseIn5Mins,
    T_VerticalDoor,
};

/// Anchor function ensuring all `#[no_mangle]` door functions survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_Doors_Link_Anchor()
{
    let _ = T_VerticalDoor as *const () as usize;
    let _ = EV_DoLockedDoor as *const () as usize;
    let _ = EV_DoDoor as *const () as usize;
    let _ = EV_VerticalDoor as *const () as usize;
    let _ = P_SpawnDoorCloseIn30 as *const () as usize;
    let _ = P_SpawnDoorRaiseIn5Mins as *const () as usize;
}
