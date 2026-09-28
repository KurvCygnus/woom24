//! The shared plane-movement primitive: `T_MovePlane`, the per-tic
//! height mover used by the floor, ceiling, platform, and door
//! subsystems -- bit-exact with the `T_MovePlane` half of
//! `vendor/doomgeneric/p_floor.c`. The overshoot/landing decision
//! routes through [`dtmc::step_toward`]; every `P_ChangeSector`
//! call/restore pair stays at this call site in the original order.

#![allow(non_snake_case)]

use std::ffi::c_int;

use super::dtmc;
use super::state::{result_crushed, result_ok, result_pastdest};
use crate::doom::c_ffi as cffi;
use crate::doom::m_fixed::fixed_t;
use crate::doom::p_lights::sector_t;
use crate::doom::p_map::P_ChangeSector;

/// Move a plane (floor or ceiling) one step toward `dest` and check for crushing.
///
/// Shared by the floor, ceiling, platform, and door subsystems.
///
/// `floorOrCeiling`: `0` = floor, `1` = ceiling.
/// `direction`: `1` = up, `-1` = down.
/// `crush`: non-zero enables crushing things caught by the moving plane.
///
/// Returns one of:
/// - `result_ok` — moved without incident.
/// - `result_crushed` — a thing was crushed; the plane may have been reversed.
/// - `result_pastdest` — the plane reached `dest` this tic.
#[no_mangle]
pub extern "C" fn T_MovePlane(
    sector: *mut sector_t,
    speed: fixed_t,
    dest: fixed_t,
    crush: c_int,
    floorOrCeiling: c_int,
    direction: c_int,
) -> c_int
{
    unsafe
    {
        let sec = &mut *sector;
        match floorOrCeiling
        {
            0 =>
            {
                // FLOOR
                match direction
                {
                    -1 =>
                    {
                        // DOWN
                        let (new_height, reached) = dtmc::step_toward(sec.floorheight, speed, dest, -1);
                        if reached
                        {
                            let lastpos = sec.floorheight;
                            sec.floorheight = new_height;
                            let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            if flag != 0
                            {
                                sec.floorheight = lastpos;
                                P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            }
                            return result_pastdest;
                        }
                        let lastpos = sec.floorheight;
                        sec.floorheight = new_height;
                        let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                        if flag != 0
                        {
                            sec.floorheight = lastpos;
                            P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            return result_crushed;
                        }
                    }
                    1 =>
                    {
                        // UP
                        let (new_height, reached) = dtmc::step_toward(sec.floorheight, speed, dest, 1);
                        if reached
                        {
                            let lastpos = sec.floorheight;
                            sec.floorheight = new_height;
                            let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            if flag != 0
                            {
                                sec.floorheight = lastpos;
                                P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            }
                            return result_pastdest;
                        }
                        let lastpos = sec.floorheight;
                        sec.floorheight = new_height;
                        let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                        if flag != 0
                        {
                            if crush != 0 { return result_crushed; }
                            sec.floorheight = lastpos;
                            P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            return result_crushed;
                        }
                    }
                    _ => {}
                }
            }
            1 =>
            {
                // CEILING
                match direction
                {
                    -1 =>
                    {
                        // DOWN
                        let (new_height, reached) = dtmc::step_toward(sec.ceilingheight, speed, dest, -1);
                        if reached
                        {
                            let lastpos = sec.ceilingheight;
                            sec.ceilingheight = new_height;
                            let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            if flag != 0
                            {
                                sec.ceilingheight = lastpos;
                                P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            }
                            return result_pastdest;
                        }
                        let lastpos = sec.ceilingheight;
                        sec.ceilingheight = new_height;
                        let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                        if flag != 0
                        {
                            if crush != 0 { return result_crushed; }
                            sec.ceilingheight = lastpos;
                            P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            return result_crushed;
                        }
                    }
                    1 =>
                    {
                        // UP
                        let (new_height, reached) = dtmc::step_toward(sec.ceilingheight, speed, dest, 1);
                        if reached
                        {
                            let lastpos = sec.ceilingheight;
                            sec.ceilingheight = new_height;
                            let flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            if flag != 0
                            {
                                sec.ceilingheight = lastpos;
                                P_ChangeSector(sector as *mut cffi::sector_t, crush);
                            }
                            return result_pastdest;
                        }
                        let _lastpos = sec.ceilingheight;
                        sec.ceilingheight = new_height;
                        let _flag = P_ChangeSector(sector as *mut cffi::sector_t, crush);
                        // The original C code has #if 0 here, so no crush check.
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        result_ok
    }
}
