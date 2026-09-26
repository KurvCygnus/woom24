//! Link anchor for the lighting module: a single never-called C
//! function referencing every public function of the module so the
//! linker keeps their symbols alive.

use super::{
    EV_LightTurnOn, EV_StartLightStrobing, EV_TurnTagLightsOff, P_SpawnFireFlicker,
    P_SpawnGlowingLight, P_SpawnLightFlash, P_SpawnStrobeFlash, T_FireFlicker, T_Glow,
    T_LightFlash, T_StrobeFlash,
};

/// Anchor function referenced from `doomgeneric_Create` to ensure all
/// `#[no_mangle]` light functions survive link-time dead-code elimination.
///
/// # Safety
///
/// Must only be called during engine initialisation before any thinker
/// dispatch occurs; taking the address of each function is always safe.
#[no_mangle]
pub unsafe extern "C" fn P_Lights_Link_Anchor()
{
    // Force the linker to include every exported symbol from this module.
    let _ = T_FireFlicker as *const () as usize;
    let _ = T_LightFlash as *const () as usize;
    let _ = T_StrobeFlash as *const () as usize;
    let _ = T_Glow as *const () as usize;
    let _ = P_SpawnFireFlicker as *const () as usize;
    let _ = P_SpawnLightFlash as *const () as usize;
    let _ = P_SpawnStrobeFlash as *const () as usize;
    let _ = EV_StartLightStrobing as *const () as usize;
    let _ = EV_TurnTagLightsOff as *const () as usize;
    let _ = EV_LightTurnOn as *const () as usize;
    let _ = P_SpawnGlowingLight as *const () as usize;
}
