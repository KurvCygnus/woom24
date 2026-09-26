//! Link anchor for the ceiling module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{
    EV_CeilingCrushStop, EV_DoCeiling, P_ActivateInStasisCeiling, P_AddActiveCeiling,
    P_RemoveActiveCeiling, T_MoveCeiling,
};

/// Anchor function ensuring all `#[no_mangle]` ceiling functions survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_Ceilng_Link_Anchor()
{
    let _ = T_MoveCeiling as *const () as usize;
    let _ = EV_DoCeiling as *const () as usize;
    let _ = P_AddActiveCeiling as *const () as usize;
    let _ = P_RemoveActiveCeiling as *const () as usize;
    let _ = P_ActivateInStasisCeiling as *const () as usize;
    let _ = EV_CeilingCrushStop as *const () as usize;
}
