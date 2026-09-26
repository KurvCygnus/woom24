//! Link anchor for the floor module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{EV_BuildStairs, EV_DoFloor, T_MoveFloor, T_MovePlane};

/// Anchor function ensuring all `#[no_mangle]` floor functions survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_Floor_Link_Anchor()
{
    let _ = T_MovePlane as *const () as usize;
    let _ = T_MoveFloor as *const () as usize;
    let _ = EV_DoFloor as *const () as usize;
    let _ = EV_BuildStairs as *const () as usize;
}
