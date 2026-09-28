//! Super Shotgun shell actions: the open/load/close breech sounds,
//! with the close action chaining into the psprite refire check --
//! bit-exact with the SSG trio of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::d_player::{PlayerT, PspdefT};

//* The A_ReFire cycle crosses into p_pspr: import through p_pspr's
//* module-root re-export (p_pspr/mod.rs pins this ordering contract) --
//* never a direct submodule path.
use crate::doom::p_pspr::A_ReFire;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

/// Super Shotgun open action: plays the barrel-break sound (`sfx_dbopn`) for the player.
///
/// # Safety
///
/// `player` must be non-null with a valid `mo`. Called from C.
#[doc(alias = "A_OpenShotgun2")]
#[export_name = "A_OpenShotgun2"]
pub unsafe extern "C" fn action_open_shotgun2(player: *mut PlayerT, _psp: *mut PspdefT) { S_StartSound((*player).mo as *mut c_void, Sfx::Dbopn as c_int); }

/// Super Shotgun load action: plays the shell-load sound (`sfx_dbload`) for the player.
///
/// # Safety
///
/// `player` must be non-null with a valid `mo`. Called from C.
#[doc(alias = "A_LoadShotgun2")]
#[export_name = "A_LoadShotgun2"]
pub unsafe extern "C" fn action_load_shotgun2(player: *mut PlayerT, _psp: *mut PspdefT) { S_StartSound((*player).mo as *mut c_void, Sfx::Dbload as c_int); }

/// Super Shotgun close action: plays the close sound (`sfx_dbcls`) then checks for refire.
///
/// # Safety
///
/// `player` must be non-null with a valid `mo`; `psp` must be non-null. Called from C.
#[doc(alias = "A_CloseShotgun2")]
#[export_name = "A_CloseShotgun2"]
pub unsafe extern "C" fn action_close_shotgun2(player: *mut PlayerT, psp: *mut PspdefT)
{
    S_StartSound((*player).mo as *mut c_void, Sfx::Dbcls as c_int);
    A_ReFire(player, psp);
}
