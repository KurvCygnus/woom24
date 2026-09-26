//! Link anchor for the interactions module: a single never-called C
//! function referencing the module's public functions and ammo tables
//! so the linker keeps their symbols alive.

use super::{
    clipammo, maxammo, P_DamageMobj, P_GiveAmmo, P_GiveArmor, P_GiveBody, P_GiveCard,
    P_GivePower, P_GiveWeapon, P_KillMobj, P_TouchSpecialThing,
};

/// Anchor function ensuring all `#[no_mangle]` interaction symbols
/// survive link-time dead-code elimination.
///
/// Carried verbatim from the pre-graduation flat file. Not called from
/// `doomgeneric_Create` (the anchor call list covers
/// Ceilng/Doors/Floor/Lights/Plats/Pspr/Sight/Switch/Telept/User/
/// I_Input/R_Main but not Inter) -- a pre-existing gap kept as-is for
/// anchor parity; every symbol here is Rust-reachable through the
/// p_map / p_enemy / p_spec / st_stuff / g_game callers, so retention
/// is not at risk.
#[no_mangle]
pub extern "C" fn P_Inter_Link_Anchor()
{
    unsafe
    {
        let _ = std::ptr::addr_of_mut!(maxammo[0]) as usize;
        let _ = std::ptr::addr_of_mut!(clipammo[0]) as usize;
    }
    let _ = P_GiveAmmo as *const () as usize;
    let _ = P_GiveWeapon as *const () as usize;
    let _ = P_GiveBody as *const () as usize;
    let _ = P_GiveArmor as *const () as usize;
    let _ = P_GiveCard as *const () as usize;
    let _ = P_GivePower as *const () as usize;
    let _ = P_TouchSpecialThing as *const () as usize;
    let _ = P_KillMobj as *const () as usize;
    let _ = P_DamageMobj as *const () as usize;
}
