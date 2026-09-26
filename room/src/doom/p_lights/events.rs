//! Linedef-triggered lighting events: the three `EV_*` entry points
//! the special-line dispatchers call, whose tag-scan and
//! neighbor-scan orders are directly observable in the sector
//! `lightlevel` state -- bit-exact with the event half of
//! `vendor/doomgeneric/p_lights.c`.

#![allow(non_snake_case)]

use std::os::raw::c_int;

use super::effects::{SLOWDARK, P_SpawnStrobeFlash};
use super::types::{line_t, sector_t};
use crate::doom::c_ffi as cffi;
use crate::doom::p_setup::{numsectors, sectors};
use crate::doom::p_spec::{getNextSector, P_FindSectorFromLineTag};

/// Linedef-triggered event: start slow-strobe lighting on all tagged sectors.
///
/// Iterates over sectors whose tag matches `line`'s tag. Sectors that already
/// have an active special effect are skipped.
#[no_mangle]
pub extern "C" fn EV_StartLightStrobing(line: *mut line_t)
{
    unsafe
    {
        let mut secnum: c_int = -1;
        loop
        {
            secnum = P_FindSectorFromLineTag(line as *mut cffi::line_t, secnum);
            if secnum < 0
            {
                break;
            }
            let sec = sectors.add(secnum as usize);
            if !(*sec).specialdata.is_null()
            {
                continue;
            }
            P_SpawnStrobeFlash(sec as *mut sector_t, SLOWDARK, 0);
        }
    }
}

/// Linedef-triggered event: set all tagged sectors to the darkest neighboring level.
///
/// For each sector whose tag matches `line`'s tag, scans adjacent sectors and
/// sets the light level to the minimum found among all neighbors.
#[no_mangle]
pub extern "C" fn EV_TurnTagLightsOff(line: *mut line_t)
{
    unsafe
    {
        for j in 0..numsectors as usize
        {
            let sec = sectors.add(j);
            if (*sec).tag != (*line).tag
            {
                continue;
            }

            let mut min = (*sec).lightlevel as c_int;
            for i in 0..(*sec).linecount as usize
            {
                let templine = *(*sec).lines.add(i);
                let tsec = getNextSector(templine as *mut cffi::line_t, sec);
                if tsec.is_null()
                {
                    continue;
                }
                let tl = (*tsec).lightlevel as c_int;
                if tl < min
                {
                    min = tl;
                }
            }
            (*sec).lightlevel = min as i16;
        }
    }
}

/// Linedef-triggered event: set all tagged sectors to `bright` light level.
///
/// If `bright == 0`, the function searches adjacent sectors and uses the
/// highest light level found among them instead.
#[no_mangle]
pub extern "C" fn EV_LightTurnOn(line: *mut line_t, bright: c_int)
{
    unsafe
    {
        let mut bright = bright;
        for i in 0..numsectors as usize
        {
            let sec = sectors.add(i);
            if (*sec).tag != (*line).tag
            {
                continue;
            }

            if bright == 0
            {
                for j in 0..(*sec).linecount as usize
                {
                    let templine = *(*sec).lines.add(j);
                    let temp = getNextSector(templine as *mut cffi::line_t, sec);
                    if temp.is_null()
                    {
                        continue;
                    }
                    let tl = (*temp).lightlevel as c_int;
                    if tl > bright
                    {
                        bright = tl;
                    }
                }
            }
            (*sec).lightlevel = bright as i16;
        }
    }
}
