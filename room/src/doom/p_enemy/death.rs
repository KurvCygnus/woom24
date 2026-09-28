//! Death, pain, and footstep state actions: the variant-randomised
//! death screams, the gib scream, pain/fall reactions, the boss
//! footsteps, and the player death scream -- bit-exact with the
//! death half of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::doomstat::gamemode;
use crate::doom::info::{MF_SOLID, MobjInfo, MT_CYBORG, MT_SPIDER};
use crate::doom::m_random::P_Random;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::chase::action_chase;
use super::consts::commercial;

/// Plays the monster's death scream; randomises for multi-variant sound monsters.
///
/// Zombie Pig (Podth variants) picks randomly from three sounds; Demon (Bgdth variants)
/// from two. Cyberdemon and Spider Mastermind play at full (global) volume; all others
/// play positioned at the actor.
///
/// # Safety
///
/// `actor` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_Scream")]
#[export_name = "A_Scream"]
pub unsafe extern "C" fn action_scream(actor: *mut mobj_t)
{
    let sound = match (*((*actor).info as *mut MobjInfo)).deathsound
    {
        Sfx::None => return,
        Sfx::Podth1 | Sfx::Podth2 | Sfx::Podth3 => Sfx::Podth1 as c_int + P_Random() % 3 as c_int,
        Sfx::Bgdth1 | Sfx::Bgdth2 => Sfx::Bgdth1 as c_int + P_Random() % 2 as c_int,
        s => s as c_int,
    };
    if(*actor).mobjtype as c_uint == MT_SPIDER as c_int as c_uint ||
        (*actor).mobjtype as c_uint == MT_CYBORG as c_int as c_uint { S_StartSound(std::ptr::null_mut::<c_void>(), sound); }
    else { S_StartSound(actor as *mut c_void, sound); };
}

/// Plays the player/monster gibbing scream (`sfx_slop`) positioned at the actor.
///
/// # Safety
///
/// `actor` must be non-null. Called from C.
#[doc(alias = "A_XScream")]
#[export_name = "A_XScream"]
pub unsafe extern "C" fn action_x_scream(actor: *mut mobj_t) { S_StartSound(actor as *mut c_void, Sfx::Slop as c_int); }

/// Plays the monster's pain sound if it has one.
///
/// # Safety
///
/// `actor` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_Pain")]
#[export_name = "A_Pain"]
pub unsafe extern "C" fn action_pain(actor: *mut mobj_t)
{
    if(*((*actor).info as *mut MobjInfo)).painsound != Sfx::None
    {
        S_StartSound(
            actor as *mut c_void,
            (*((*actor).info as *mut MobjInfo)).painsound as c_int,
        );
    }
}

/// Clears `MF_SOLID` so the falling corpse can be walked over.
///
/// # Safety
///
/// `actor` must be non-null. Called from C.
#[doc(alias = "A_Fall")]
#[export_name = "A_Fall"]
pub unsafe extern "C" fn action_fall(actor: *mut mobj_t) { (*actor).flags &= !(MF_SOLID as c_int); }

/// Plays the player death scream; uses the gibbing scream in Doom II when health is below -50.
///
/// Default sound is `sfx_pldeth`. In commercial mode, if `mo->health < -50`, plays
/// `sfx_pdiehi` (the "squish" gib scream) instead.
///
/// # Safety
///
/// `mo` must be non-null. Called from C via state-machine action pointer.
#[doc(alias = "A_PlayerScream")]
#[export_name = "A_PlayerScream"]
pub unsafe extern "C" fn action_player_scream(mo: *mut mobj_t)
{
    let mut sound: c_int = Sfx::Pldeth as c_int;
    if gamemode as c_uint == commercial as c_int as c_uint && (*mo).health < -50 as c_int { sound = Sfx::Pdiehi as c_int; }
    S_StartSound(mo as *mut c_void, sound);
}

/// Cyberdemon footstep: plays `sfx_hoof` then advances via `A_Chase`.
///
/// # Safety
///
/// `mo` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_Hoof")]
#[export_name = "A_Hoof"]
pub unsafe extern "C" fn action_hoof(mo: *mut mobj_t)
{
    S_StartSound(mo as *mut c_void, Sfx::Hoof as c_int);
    action_chase(mo);
}

/// Spider Mastermind footstep: plays `sfx_metal` then advances via `A_Chase`.
///
/// # Safety
///
/// `mo` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_Metal")]
#[export_name = "A_Metal"]
pub unsafe extern "C" fn action_metal(mo: *mut mobj_t)
{
    S_StartSound(mo as *mut c_void, Sfx::Metal as c_int);
    action_chase(mo);
}

/// Arachnotron footstep: plays `sfx_bspwlk` then advances via `A_Chase`.
///
/// # Safety
///
/// `mo` must be non-null with a valid `info`. Called from C.
#[doc(alias = "A_BabyMetal")]
#[export_name = "A_BabyMetal"]
pub unsafe extern "C" fn action_baby_metal(mo: *mut mobj_t)
{
    S_StartSound(mo as *mut c_void, Sfx::Bspwlk as c_int);
    action_chase(mo);
}
