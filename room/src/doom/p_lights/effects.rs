//! The four sector-lighting effects: their `repr(C)` thinker states
//! (`fireflicker_t`, `lightflash_t`, `strobe_t`, `glow_t`), the
//! per-tic `T_*` updaters the thinker dispatcher calls, the
//! `P_Spawn*` spawners, and the effect timing constants -- bit-exact
//! with the corresponding halves of `vendor/doomgeneric/p_lights.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use super::dtmc;
use super::types::sector_t;
use crate::doom::c_ffi as cffi;
use crate::doom::m_random::P_Random;
use crate::doom::p_spec::P_FindMinSurroundingLight;
use crate::doom::p_tick::{thinker_t, P_AddThinker};
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

/// Light change applied each step of the glow oscillation (light units per tic).
pub(super) const GLOWSPEED: c_int = 8;

/// Duration of the bright phase for strobe lights, in tics.
pub(super) const STROBEBRIGHT: c_int = 5;

/// Dark-phase duration for fast strobe lights, in tics (`FASTDARK` in C).
pub(super) const FASTDARK: c_int = 15;

/// Dark-phase duration for slow strobe lights, in tics (`SLOWDARK` in C).
pub(super) const SLOWDARK: c_int = 35;

/// Thinker state for the fire-flicker lighting effect.
///
/// The sector's light level randomly drops by a multiple of 16 every 4 tics,
/// simulating a flickering fire (sector special 17 in Doom).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fireflicker_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// The sector whose light level is being animated.
    pub sector: *mut sector_t,
    /// Tics remaining until the next light change.
    pub count: c_int,
    /// Sector's original (maximum) light level.
    pub maxlight: c_int,
    /// Minimum light level the flicker will not go below.
    pub minlight: c_int,
    _pad: [u8; 4],
}

/// Thinker state for the random light-flash effect.
///
/// The sector alternates between `maxlight` and `minlight` at random intervals
/// bounded by `maxtime` and `mintime` (sector special 1 in Doom).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct lightflash_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// The sector whose light level is being animated.
    pub sector: *mut sector_t,
    /// Tics remaining until the next light change.
    pub count: c_int,
    /// Bright phase light level.
    pub maxlight: c_int,
    /// Dark phase light level.
    pub minlight: c_int,
    /// Bitmask used to generate the random bright-phase duration (`count & maxtime`).
    pub maxtime: c_int,
    /// Bitmask used to generate the random dark-phase duration (`count & mintime`).
    pub mintime: c_int,
}

/// Thinker state for the strobe-flash lighting effect.
///
/// The sector alternates between `maxlight` (bright) and `minlight` (dark)
/// with fixed durations of `brighttime` and `darktime` tics respectively
/// (sector specials 3 and 13 in Doom, among others).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct strobe_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// The sector whose light level is being animated.
    pub sector: *mut sector_t,
    /// Tics remaining until the next phase change.
    pub count: c_int,
    /// Dark-phase light level.
    pub minlight: c_int,
    /// Bright-phase light level.
    pub maxlight: c_int,
    /// Duration of the dark phase in tics.
    pub darktime: c_int,
    /// Duration of the bright phase in tics (always `STROBEBRIGHT` = 5).
    pub brighttime: c_int,
}

/// Thinker state for the smoothly oscillating glow effect.
///
/// The sector's light level rises and falls continuously between `minlight`
/// and `maxlight` by `GLOWSPEED` units per tic (sector special 8 in Doom).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct glow_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// The sector whose light level is being animated.
    pub sector: *mut sector_t,
    /// Lower bound of the oscillation (minimum light level).
    pub minlight: c_int,
    /// Upper bound of the oscillation (maximum / original light level).
    pub maxlight: c_int,
    /// Direction of travel: `1` = increasing, `-1` = decreasing.
    pub direction: c_int,
    _pad: [u8; 4],
}

/// Per-tic update for the fire-flicker effect.
///
/// Decrements the countdown; when it reaches zero, randomises the sector's
/// light level and resets the 4-tic counter. The amount + clamp computation
/// routes through [`dtmc::fire_flicker_level`]; the `P_Random` draw stays at
/// this call site.
///
/// # Safety
///
/// `flick` must be a valid, aligned, non-null pointer to a `fireflicker_t`
/// whose embedded `sector` pointer is also valid for the current map.
/// Called exclusively by the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_FireFlicker(flick: *mut fireflicker_t)
{
    (*flick).count -= 1;
    if (*flick).count != 0
    {
        return;
    }

    let sec = &mut *(*flick).sector;
    sec.lightlevel = dtmc::fire_flicker_level(
        sec.lightlevel as c_int,
        (*flick).maxlight,
        (*flick).minlight,
        P_Random(),
    ) as i16;

    (*flick).count = 4;
}

/// Allocate and initialise a fire-flicker thinker for `sector`.
///
/// Clears the sector special, finds the minimum surrounding light level
/// (used as the lower flicker bound + 16), and registers the thinker.
#[no_mangle]
pub extern "C" fn P_SpawnFireFlicker(sector: *mut sector_t)
{
    unsafe
    {
        (*sector).special = 0;

        let flick = Z_Malloc(
            std::mem::size_of::<fireflicker_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut fireflicker_t;

        P_AddThinker(&mut (*flick).thinker);

        (*flick).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut fireflicker_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_FireFlicker));
        (*flick).sector = sector;
        (*flick).maxlight = (*sector).lightlevel as c_int;
        (*flick).minlight =
            P_FindMinSurroundingLight(sector as *mut cffi::sector_t, (*sector).lightlevel as c_int)
                + 16;
        (*flick).count = 4;
    }
}

/// Per-tic update for the random light-flash effect.
///
/// Decrements the countdown; when it reaches zero, toggles the sector's
/// light between `maxlight` and `minlight`, picking a new random duration
/// through [`dtmc::flash_duration`] (the `P_Random` draw stays here).
///
/// # Safety
///
/// `flash` must be a valid, aligned, non-null pointer to a `lightflash_t`
/// whose embedded `sector` pointer is also valid for the current map.
/// Called exclusively by the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_LightFlash(flash: *mut lightflash_t)
{
    (*flash).count -= 1;
    if (*flash).count != 0
    {
        return;
    }

    let sec = &mut *(*flash).sector;

    if sec.lightlevel as c_int == (*flash).maxlight
    {
        sec.lightlevel = (*flash).minlight as i16;
        (*flash).count = dtmc::flash_duration(P_Random(), (*flash).mintime);
    }
    else
    {
        sec.lightlevel = (*flash).maxlight as i16;
        (*flash).count = dtmc::flash_duration(P_Random(), (*flash).maxtime);
    }
}

/// Allocate and initialise a random light-flash thinker for `sector`.
///
/// Sets `maxtime = 64` and `mintime = 7`, giving the flash a long bright phase
/// and a short dark phase. The initial countdown is randomised through
/// [`dtmc::flash_duration`] (the `P_Random` draw stays here).
#[no_mangle]
pub extern "C" fn P_SpawnLightFlash(sector: *mut sector_t)
{
    unsafe
    {
        (*sector).special = 0;

        let flash = Z_Malloc(
            std::mem::size_of::<lightflash_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut lightflash_t;

        P_AddThinker(&mut (*flash).thinker);

        (*flash).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut lightflash_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_LightFlash));
        (*flash).sector = sector;
        (*flash).maxlight = (*sector).lightlevel as c_int;
        (*flash).minlight =
            P_FindMinSurroundingLight(sector as *mut cffi::sector_t, (*sector).lightlevel as c_int);
        (*flash).maxtime = 64;
        (*flash).mintime = 7;
        (*flash).count = dtmc::flash_duration(P_Random(), (*flash).maxtime);
    }
}

/// Per-tic update for the strobe-flash effect.
///
/// Decrements the countdown; when it reaches zero, switches the sector's
/// light between `minlight` (dark phase, `darktime` tics) and `maxlight`
/// (bright phase, `brighttime` tics).
///
/// # Safety
///
/// `flash` must be a valid, aligned, non-null pointer to a `strobe_t`
/// whose embedded `sector` pointer is also valid for the current map.
/// Called exclusively by the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_StrobeFlash(flash: *mut strobe_t)
{
    (*flash).count -= 1;
    if (*flash).count != 0
    {
        return;
    }

    let sec = &mut *(*flash).sector;

    if sec.lightlevel as c_int == (*flash).minlight
    {
        sec.lightlevel = (*flash).maxlight as i16;
        (*flash).count = (*flash).brighttime;
    }
    else
    {
        sec.lightlevel = (*flash).minlight as i16;
        (*flash).count = (*flash).darktime;
    }
}

/// Allocate and initialise a strobe-flash thinker for `sector`.
///
/// `fastOrSlow` sets `darktime` (use `FASTDARK` or `SLOWDARK`).
/// `inSync = 1` starts the flash immediately; `inSync = 0` gives a random
/// offset through [`dtmc::flash_duration`] (the `P_Random` draw stays here).
/// If `minlight == maxlight`, `minlight` is forced to 0.
#[no_mangle]
pub extern "C" fn P_SpawnStrobeFlash(sector: *mut sector_t, fastOrSlow: c_int, inSync: c_int)
{
    unsafe
    {
        let flash = Z_Malloc(
            std::mem::size_of::<strobe_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut strobe_t;

        P_AddThinker(&mut (*flash).thinker);

        (*flash).sector = sector;
        (*flash).darktime = fastOrSlow;
        (*flash).brighttime = STROBEBRIGHT;
        (*flash).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut strobe_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_StrobeFlash));
        (*flash).maxlight = (*sector).lightlevel as c_int;
        (*flash).minlight =
            P_FindMinSurroundingLight(sector as *mut cffi::sector_t, (*sector).lightlevel as c_int);

        if (*flash).minlight == (*flash).maxlight
        {
            (*flash).minlight = 0;
        }

        (*sector).special = 0;

        if inSync == 0
        {
            (*flash).count = dtmc::flash_duration(P_Random(), 7);
        }
        else
        {
            (*flash).count = 1;
        }
    }
}

/// Per-tic update for the smooth glow oscillation effect.
///
/// Adjusts `lightlevel` by `GLOWSPEED` each tic, bouncing at `minlight` and
/// `maxlight` by reversing `direction`.
///
/// # Safety
///
/// `g` must be a valid, aligned, non-null pointer to a `glow_t`
/// whose embedded `sector` pointer is also valid for the current map.
/// Called exclusively by the thinker dispatcher from `P_RunThinkers`.
#[no_mangle]
pub unsafe extern "C" fn T_Glow(g: *mut glow_t)
{
    let sec = &mut *(*g).sector;
    match (*g).direction
    {
        -1 =>
        {
            sec.lightlevel -= GLOWSPEED as i16;
            if (sec.lightlevel as c_int) <= (*g).minlight
            {
                sec.lightlevel += GLOWSPEED as i16;
                (*g).direction = 1;
            }
        }
        1 =>
        {
            sec.lightlevel += GLOWSPEED as i16;
            if (sec.lightlevel as c_int) >= (*g).maxlight
            {
                sec.lightlevel -= GLOWSPEED as i16;
                (*g).direction = -1;
            }
        }
        _ =>
        {}
    }
}

/// Allocate and initialise a glow thinker for `sector`.
///
/// Sets `maxlight` to the sector's current light level and `minlight` to the
/// lowest light level in adjacent sectors. The glow starts moving downward
/// (`direction = -1`).
#[no_mangle]
pub extern "C" fn P_SpawnGlowingLight(sector: *mut sector_t)
{
    unsafe
    {
        let g = Z_Malloc(
            std::mem::size_of::<glow_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut glow_t;

        P_AddThinker(&mut (*g).thinker);

        (*g).sector = sector;
        (*g).minlight =
            P_FindMinSurroundingLight(sector as *mut cffi::sector_t, (*sector).lightlevel as c_int);
        (*g).maxlight = (*sector).lightlevel as c_int;
        (*g).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut glow_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_Glow));
        (*g).direction = -1;

        (*sector).special = 0;
    }
}
