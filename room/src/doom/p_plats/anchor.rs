//! Link anchor for the plats module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{
    EV_DoPlat, EV_StopPlat, P_ActivateInStasis, P_AddActivePlat, P_RemoveActivePlat, T_PlatRaise,
};

/// Anchor function ensuring all `#[no_mangle]` plat functions survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_Plats_Link_Anchor()
{
    let _ = T_PlatRaise as *const () as usize;
    let _ = EV_DoPlat as *const () as usize;
    let _ = P_ActivateInStasis as *const () as usize;
    let _ = EV_StopPlat as *const () as usize;
    let _ = P_AddActivePlat as *const () as usize;
    let _ = P_RemoveActivePlat as *const () as usize;
}
