//! Link anchor for the psprite module: a single never-called C
//! function referencing every public function and static of the
//! module so the linker keeps their symbols alive.

use super::{
    bulletslope, swingx, swingy, A_BFGSpray, A_BFGsound, A_CheckReload, A_FireBFG, A_FireCGun,
    A_FireMissile, A_FirePistol, A_FirePlasma, A_FireShotgun, A_FireShotgun2, A_GunFlash,
    A_Light0, A_Light1, A_Light2, A_Lower, A_Punch, A_Raise, A_ReFire, A_Saw, A_WeaponReady,
    P_BringUpWeapon, P_BulletSlope, P_CalcSwing, P_CheckAmmo, P_DropWeapon, P_FireWeapon,
    P_GunShot, P_MovePsprites, P_SetPsprite, P_SetupPsprites,
};

/// Anchor function ensuring all `#[no_mangle]` psprite symbols survive
/// link-time dead-code elimination.
///
/// Referenced from `doomgeneric_Create` during engine initialisation.
#[no_mangle]
pub extern "C" fn P_Pspr_Link_Anchor()
{
    unsafe
    {
        let _ = swingx as usize;
        let _ = swingy as usize;
        let _ = bulletslope as usize;
    }
    let _ = P_SetPsprite as *const () as usize;
    let _ = P_CalcSwing as *const () as usize;
    let _ = P_BringUpWeapon as *const () as usize;
    let _ = P_CheckAmmo as *const () as usize;
    let _ = P_FireWeapon as *const () as usize;
    let _ = P_DropWeapon as *const () as usize;
    let _ = A_WeaponReady as *const () as usize;
    let _ = A_ReFire as *const () as usize;
    let _ = A_CheckReload as *const () as usize;
    let _ = A_Lower as *const () as usize;
    let _ = A_Raise as *const () as usize;
    let _ = A_GunFlash as *const () as usize;
    let _ = A_Punch as *const () as usize;
    let _ = A_Saw as *const () as usize;
    let _ = A_FireMissile as *const () as usize;
    let _ = A_FireBFG as *const () as usize;
    let _ = A_FirePlasma as *const () as usize;
    let _ = P_BulletSlope as *const () as usize;
    let _ = P_GunShot as *const () as usize;
    let _ = A_FirePistol as *const () as usize;
    let _ = A_FireShotgun as *const () as usize;
    let _ = A_FireShotgun2 as *const () as usize;
    let _ = A_FireCGun as *const () as usize;
    let _ = A_Light0 as *const () as usize;
    let _ = A_Light1 as *const () as usize;
    let _ = A_Light2 as *const () as usize;
    let _ = A_BFGSpray as *const () as usize;
    let _ = A_BFGsound as *const () as usize;
    let _ = P_SetupPsprites as *const () as usize;
    let _ = P_MovePsprites as *const () as usize;
}
