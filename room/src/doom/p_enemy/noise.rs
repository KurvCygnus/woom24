//! Sound propagation: the sector flood that wakes monsters in earshot
//! of a noise -- bit-exact with the `P_RecursiveSound` / `P_NoiseAlert`
//! pair of `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::c_ffi::{line_t, sector_t, LinedefFlag};
use crate::doom::p_maputl::{openrange, P_LineOpening};
use crate::doom::p_setup::sides;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::validcount;

/// The monster or player whose noise triggered the current `P_RecursiveSound` traversal;
/// written by `P_NoiseAlert`, read by `P_RecursiveSound` to stamp each sector.
/// Has C linkage (`#[no_mangle]`); referenced directly from `p_enemy.c`.
#[no_mangle]
pub static mut soundtarget: *mut mobj_t = std::ptr::null_mut::<mobj_t>();

/// Recursively floods sound through adjacent sectors, stamping each with `soundtarget`.
///
/// Traversal stops at sectors already visited this frame (`validcount`) and at
/// two-sided linedefs with the `ML_SOUNDBLOCK` flag - the block flag allows one
/// crossing (incrementing `soundblocks` from 0 to 1) but never two, so sound
/// cannot pass through two consecutive blocking walls.
///
/// Called by `P_NoiseAlert` and recursively by itself.
///
/// # Safety
///
/// `sec` must point to a valid, live `sector_t`. All sector/sidedef/linedef
/// pointers reachable from `sec` must also be valid. `soundtarget` must be null
/// or point to a valid `mobj_t`. This function is called from C.
#[doc(alias = "P_RecursiveSound")]
#[export_name = "P_RecursiveSound"]
pub unsafe extern "C" fn recursive_sound(sec: *mut sector_t, soundblocks: c_int)
{
    let mut i: c_int;

    let mut check: *mut line_t;

    let mut other: *mut sector_t;

    if(*sec).validcount == validcount && (*sec).soundtraversed <= soundblocks + 1 as c_int { return; }
    (*sec).validcount = validcount;
    (*sec).soundtraversed = soundblocks + 1 as c_int;
    (*sec).soundtarget = soundtarget as *mut c_void;
    i = 0 as c_int;
    while i < (*sec).linecount
    {
        check = *(*sec).lines.offset(i as isize) as *mut line_t;
        if(*check).flags as c_int & LinedefFlag::TWOSIDED as c_int != 0
        {
            P_LineOpening(check);
            if openrange > 0 as c_int
            {
                if std::ptr::eq(
                    (*sides.offset((*check).sidenum[0 as c_int as usize] as isize)).sector,
                    sec,
                ) { other = (*sides.offset((*check).sidenum[1 as c_int as usize] as isize)).sector; }
                else { other = (*sides.offset((*check).sidenum[0 as c_int as usize] as isize)).sector; }
                if(*check).flags as c_int & LinedefFlag::SOUNDBLOCK as c_int != 0 { if soundblocks == 0 { recursive_sound(other, 1 as c_int); } }
                else { recursive_sound(other, soundblocks); }
            }
        }
        i += 1;
    }
}

/// Alerts monsters in earshot that a target (typically the player) has made noise.
///
/// Sets the global `soundtarget`, increments `validcount` to mark a new traversal
/// frame, then calls `P_RecursiveSound` starting from the sector containing `emmiter`.
/// Called by `p_inter.rs` and `p_map.rs` whenever a shot, explosion, or door fires.
///
/// # Safety
///
/// `target` and `emmiter` must both be non-null, valid `mobj_t` pointers. `emmiter`
/// must have a valid `subsector` with a valid `sector`. This function is called from C.
#[doc(alias = "P_NoiseAlert")]
#[export_name = "P_NoiseAlert"]
pub unsafe extern "C" fn noise_alert(target: *mut mobj_t, emmiter: *mut mobj_t)
{
    soundtarget = target;
    validcount += 1;
    recursive_sound((*(*emmiter).subsector).sector as *mut sector_t, 0 as c_int);
}
