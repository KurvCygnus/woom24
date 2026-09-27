//! Linedef-triggered door events: `EV_DoDoor` (the per-type spawner
//! over every sector tagged to the line), `EV_DoLockedDoor` (the key
//! check in front of it), and `EV_VerticalDoor` (the use-action with
//! the vanilla plat/door `specialdata` aliasing quirk) -- bit-exact
//! with the event half of `vendor/doomgeneric/p_doors.c`.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use super::state::{
    it_blueskull, it_bluecard, it_redcard, it_redskull, it_yellowcard, it_yellowskull,
    locked_door_message, locked_object_message, vld_blazeClose, vld_blazeOpen, vld_blazeRaise,
    vld_close, vld_close30ThenOpen, vld_normal, vld_open, vldoor_t, VDOORSPEED, VDOORWAIT,
};
use super::thinker::T_VerticalDoor;
use crate::doom::c_ffi as cffi;
use crate::doom::c_ffi::mobj_t;
use crate::doom::d_player::PlayerT;
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_lights::{line_t, sector_t};
use crate::doom::p_plats::{plat_t, T_PlatRaise};
use crate::doom::p_setup::{sectors, sides};
use crate::doom::p_spec::{P_FindLowestCeilingSurrounding, P_FindSectorFromLineTag};
use crate::doom::p_tick::P_AddThinker;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

/// Linedef-triggered event: activate a key-locked door for `thing`.
///
/// Checks whether the player associated with `thing` holds any of the keys
/// required by `line`'s special number. Displays a message and plays `sfx_oof`
/// if the required key is absent, then returns `0`. If the key check passes,
/// delegates to `EV_DoDoor`.
///
/// Returns `1` if the door was activated, `0` otherwise.
///
/// # Safety
///
/// `line` and `thing` must be valid non-null pointers for the current map.
///
/// Lock-failure messages are routed through `DEH_String`, matching the C
/// original so Dehacked patches can override them.
#[no_mangle]
pub unsafe extern "C" fn EV_DoLockedDoor(
    line: *mut line_t,
    r#type: c_int,
    thing: *mut mobj_t,
) -> c_int
{
    let Some(p) = ((*thing).player as *mut PlayerT).as_mut()
    else
    {
        return 0;
    };

    match (*line).special as c_int
    {
        99 | 133 =>
        {
            if p.cards[it_bluecard] == 0 && p.cards[it_blueskull] == 0
            {
                p.message = locked_object_message((*line).special as c_int);
                S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
                return 0;
            }
        }
        134 | 135 =>
        {
            if p.cards[it_redcard] == 0 && p.cards[it_redskull] == 0
            {
                p.message = locked_object_message((*line).special as c_int);
                S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
                return 0;
            }
        }
        136 | 137 if p.cards[it_yellowcard] == 0 && p.cards[it_yellowskull] == 0 =>
        {
            p.message = locked_object_message((*line).special as c_int);
            S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
            return 0;
        }
        _ =>
        {}
    }

    EV_DoDoor(line, r#type)
}

/// Linedef-triggered event: activate doors on all sectors tagged to `line`.
///
/// For each tagged sector that does not already have an active special,
/// allocates a `vldoor_t` thinker and configures it according to `type`.
/// Blaze variants move at `VDOORSPEED * 4`.
///
/// Returns `1` if at least one door was activated, `0` otherwise.
///
/// # Safety
///
/// `line` must be a valid non-null pointer for the current map; the global
/// `sectors` array must be initialised.
#[no_mangle]
pub unsafe extern "C" fn EV_DoDoor(line: *mut line_t, r#type: c_int) -> c_int
{
    let mut secnum: c_int = -1;
    let mut rtn: c_int = 0;

    while
    {
        secnum = P_FindSectorFromLineTag(line as *mut cffi::line_t, secnum);
        secnum
    } >= 0
    {
        let sec = sectors.add(secnum as usize);
        if !(*sec).specialdata.is_null()
        {
            continue;
        }

        rtn = 1;
        let door = Z_Malloc(
            std::mem::size_of::<vldoor_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut vldoor_t;
        P_AddThinker(&mut (*door).thinker);
        (*sec).specialdata = door as *mut c_void;

        (*door).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut vldoor_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_VerticalDoor));
        (*door).sector = sec as *mut sector_t;
        (*door).r#type = r#type;
        (*door).topwait = VDOORWAIT;
        (*door).speed = VDOORSPEED;

        match r#type
        {
            x if x == vld_blazeClose =>
            {
                (*door).topheight = P_FindLowestCeilingSurrounding(sec);
                (*door).topheight -= 4 * FRACUNIT;
                (*door).direction = -1;
                (*door).speed = VDOORSPEED * 4;
                S_StartSound(
                    &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Bdcls as c_int,
                );
            }
            x if x == vld_close =>
            {
                (*door).topheight = P_FindLowestCeilingSurrounding(sec);
                (*door).topheight -= 4 * FRACUNIT;
                (*door).direction = -1;
                S_StartSound(
                    &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Dorcls as c_int,
                );
            }
            x if x == vld_close30ThenOpen =>
            {
                (*door).topheight = (*sec).ceilingheight;
                (*door).direction = -1;
                S_StartSound(
                    &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                    Sfx::Dorcls as c_int,
                );
            }
            x if x == vld_blazeRaise || x == vld_blazeOpen =>
            {
                (*door).direction = 1;
                (*door).topheight = P_FindLowestCeilingSurrounding(sec);
                (*door).topheight -= 4 * FRACUNIT;
                (*door).speed = VDOORSPEED * 4;
                if (*door).topheight != (*sec).ceilingheight
                {
                    S_StartSound(
                        &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                        Sfx::Bdopn as c_int,
                    );
                }
            }
            x if x == vld_normal || x == vld_open =>
            {
                (*door).direction = 1;
                (*door).topheight = P_FindLowestCeilingSurrounding(sec);
                (*door).topheight -= 4 * FRACUNIT;
                if (*door).topheight != (*sec).ceilingheight
                {
                    S_StartSound(
                        &(*(*door).sector).soundorg as *const [u8; 40] as *mut c_void,
                        Sfx::Doropn as c_int,
                    );
                }
            }
            _ =>
            {}
        }
    }
    rtn
}

/// Linedef use-action: manually open or toggle the door behind `line`.
///
/// Only the front side of a linedef can be used (`side = 0`). Checks key
/// locks first (specials 26-28, 32-34). If the sector behind the line
/// already has an active thinker, toggles its direction (open doors begin
/// closing, and vice versa). Otherwise spawns a new `vldoor_t` thinker.
///
/// # Safety
///
/// `line` and `thing` must be valid non-null pointers for the current map;
/// the global `sides` array must be initialised.
///
/// Key-locked door messages are routed through `DEH_String`, matching the C
/// original so Dehacked patches can override them.
#[no_mangle]
pub unsafe extern "C" fn EV_VerticalDoor(line: *mut line_t, thing: *mut mobj_t)
{
    let side = 0;

    // Check for locks
    let player = (*thing).player as *mut PlayerT;

    match (*line).special as c_int
    {
        26 | 32 =>
        {
            let Some(player) = player.as_mut()
            else
            {
                return;
            };
            if player.cards[it_bluecard] == 0 && player.cards[it_blueskull] == 0
            {
                player.message = locked_door_message((*line).special as c_int);
                S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
                return;
            }
        }
        27 | 34 =>
        {
            let Some(player) = player.as_mut()
            else
            {
                return;
            };
            if player.cards[it_yellowcard] == 0 && player.cards[it_yellowskull] == 0
            {
                player.message = locked_door_message((*line).special as c_int);
                S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
                return;
            }
        }
        28 | 33 =>
        {
            let Some(player) = player.as_mut()
            else
            {
                return;
            };
            if player.cards[it_redcard] == 0 && player.cards[it_redskull] == 0
            {
                player.message = locked_door_message((*line).special as c_int);
                S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
                return;
            }
        }
        _ =>
        {}
    }

    let sec = (*sides.offset((*line).sidenum[(side ^ 1) as usize] as isize)).sector;

    if !(*sec).specialdata.is_null()
    {
        let door = (*sec).specialdata as *mut vldoor_t;
        match (*line).special as c_int
        {
            1 | 26 | 27 | 28 | 117 =>
            {
                if (*door).direction == -1
                {
                    (*door).direction = 1;
                }
                else
                {
                    if (*thing).player.is_null()
                    {
                        return;
                    }
                    let t_vdoor = Some(core::mem::transmute::<
                        unsafe extern "C" fn(*mut vldoor_t),
                        unsafe extern "C" fn(*mut c_void),
                    >(T_VerticalDoor));
                    let t_plat = Some(core::mem::transmute::<
                        unsafe extern "C" fn(*mut plat_t),
                        unsafe extern "C" fn(*mut c_void),
                    >(T_PlatRaise));
                    if (*door).thinker.function.acp1 == t_vdoor
                    {
                        (*door).direction = -1;
                    }
                    else if (*door).thinker.function.acp1 == t_plat
                    {
                        let plat = door as *mut plat_t;
                        (*plat).wait = -1;
                    }
                    else
                    {
                        eprintln!("EV_VerticalDoor: Tried to close something that wasn't a door.");
                        (*door).direction = -1;
                    }
                }
                return;
            }
            _ =>
            {}
        }
    }

    // for proper sound
    match (*line).special as c_int
    {
        117 | 118 =>
        {
            S_StartSound(
                &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                Sfx::Bdopn as c_int,
            );
        }
        1 | 31 =>
        {
            S_StartSound(
                &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                Sfx::Doropn as c_int,
            );
        }
        _ =>
        {
            S_StartSound(
                &(*sec).soundorg as *const [u8; 40] as *mut c_void,
                Sfx::Doropn as c_int,
            );
        }
    }

    // new door thinker
    let door = Z_Malloc(
        std::mem::size_of::<vldoor_t>() as c_int,
        PU_LEVSPEC,
        std::ptr::null_mut(),
    ) as *mut vldoor_t;
    P_AddThinker(&mut (*door).thinker);
    (*sec).specialdata = door as *mut c_void;
    (*door).thinker.function.acp1 = Some(core::mem::transmute::<
        unsafe extern "C" fn(*mut vldoor_t),
        unsafe extern "C" fn(*mut c_void),
    >(T_VerticalDoor));
    (*door).sector = sec as *mut sector_t;
    (*door).direction = 1;
    (*door).speed = VDOORSPEED;
    (*door).topwait = VDOORWAIT;

    match (*line).special as c_int
    {
        1 | 26 | 27 | 28 =>
        {
            (*door).r#type = vld_normal;
        }
        31..=34 =>
        {
            (*door).r#type = vld_open;
            (*line).special = 0;
        }
        117 =>
        {
            (*door).r#type = vld_blazeRaise;
            (*door).speed = VDOORSPEED * 4;
        }
        118 =>
        {
            (*door).r#type = vld_blazeOpen;
            (*line).special = 0;
            (*door).speed = VDOORSPEED * 4;
        }
        _ =>
        {}
    }

    // find the top and bottom of the movement range
    (*door).topheight = P_FindLowestCeilingSurrounding(sec);
    (*door).topheight -= 4 * FRACUNIT;
}
