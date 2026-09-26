//! Setup-time door spawners: `P_SpawnDoorCloseIn30` and
//! `P_SpawnDoorRaiseIn5Mins`, which join timed door thinkers into the
//! tick order for the sector specials 10 and 14 -- bit-exact with the
//! spawner half of `vendor/doomgeneric/p_doors.c`.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::raw::c_int;

use super::state::{vld_normal, vld_raiseIn5Mins, vldoor_t, VDOORSPEED, VDOORWAIT};
use super::thinker::T_VerticalDoor;
use crate::doom::c_ffi as cffi;
use crate::doom::i_timer::TICRATE;
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_lights::sector_t;
use crate::doom::p_spec::P_FindLowestCeilingSurrounding;
use crate::doom::p_tick::P_AddThinker;
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

/// Spawn a door thinker that closes `sec` after 30 seconds.
///
/// The door starts in direction `0` (waiting) with `topcountdown = 30 * TICRATE`.
/// After the countdown it starts closing as a `vld_normal` door.
///
/// # Safety
///
/// `sec` must be a valid non-null pointer to a sector for the current map.
#[no_mangle]
pub unsafe extern "C" fn P_SpawnDoorCloseIn30(sec: *mut sector_t)
{
    let door = Z_Malloc(
        std::mem::size_of::<vldoor_t>() as c_int,
        PU_LEVSPEC,
        std::ptr::null_mut(),
    ) as *mut vldoor_t;

    P_AddThinker(&mut (*door).thinker);

    (*sec).specialdata = door as *mut c_void;
    (*sec).special = 0;

    (*door).thinker.function.acp1 = Some(core::mem::transmute::<
        unsafe extern "C" fn(*mut vldoor_t),
        unsafe extern "C" fn(*mut c_void),
    >(T_VerticalDoor));
    (*door).sector = sec;
    (*door).direction = 0;
    (*door).r#type = vld_normal;
    (*door).speed = VDOORSPEED;
    (*door).topcountdown = 30 * TICRATE;
}

/// Spawn a door thinker that opens `sec` after 5 minutes (`vld_raiseIn5Mins`).
///
/// The door starts in direction `2` (initial wait) with
/// `topcountdown = 5 * 60 * TICRATE`. `_secnum` is accepted for C ABI
/// compatibility but is not used.
///
/// # Safety
///
/// `sec` must be a valid non-null pointer to a sector for the current map;
/// the global `sectors` array must be initialised.
#[no_mangle]
pub unsafe extern "C" fn P_SpawnDoorRaiseIn5Mins(sec: *mut sector_t, _secnum: c_int)
{
    let door = Z_Malloc(
        std::mem::size_of::<vldoor_t>() as c_int,
        PU_LEVSPEC,
        std::ptr::null_mut(),
    ) as *mut vldoor_t;

    P_AddThinker(&mut (*door).thinker);

    (*sec).specialdata = door as *mut c_void;
    (*sec).special = 0;

    (*door).thinker.function.acp1 = Some(core::mem::transmute::<
        unsafe extern "C" fn(*mut vldoor_t),
        unsafe extern "C" fn(*mut c_void),
    >(T_VerticalDoor));
    (*door).sector = sec;
    (*door).direction = 2;
    (*door).r#type = vld_raiseIn5Mins;
    (*door).speed = VDOORSPEED;
    (*door).topheight = P_FindLowestCeilingSurrounding(sec as *mut cffi::sector_t);
    (*door).topheight -= 4 * FRACUNIT;
    (*door).topwait = VDOORWAIT;
    (*door).topcountdown = 5 * 60 * TICRATE;
}
