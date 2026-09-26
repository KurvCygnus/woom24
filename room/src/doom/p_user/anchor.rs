//! Link anchor for the user module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{onground, P_CalcHeight, P_DeathThink, P_MovePlayer, P_PlayerThink, P_Thrust};

/// Anchor function ensuring all `#[no_mangle]` player exports survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_User_Link_Anchor()
{
    unsafe
    {
        let _ = onground as usize;
    }
    let _ = P_Thrust as *const () as usize;
    let _ = P_CalcHeight as *const () as usize;
    let _ = P_MovePlayer as *const () as usize;
    let _ = P_DeathThink as *const () as usize;
    let _ = P_PlayerThink as *const () as usize;
}
