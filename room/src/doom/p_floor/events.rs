//! Linedef-triggered floor events: `EV_DoFloor` (the per-type floor
//! spawner) and `EV_BuildStairs` (the staircase builder), whose
//! tag-scan and sector-walk orders are directly observable in the
//! simulation -- bit-exact with the event half of
//! `vendor/doomgeneric/p_floor.c`, including the two PRE-EXISTING
//! port divergences documented on `EV_DoFloor` (never normalize them
//! here).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};

use super::state::{
    floormove_t, FLOORSPEED, floor_lowerAndChange, floor_lowerFloor, floor_lowerFloorToLowest,
    floor_raiseFloor, floor_raiseFloor24, floor_raiseFloor24AndChange, floor_raiseFloor512,
    floor_raiseFloorCrush, floor_raiseFloorToNearest, floor_raiseFloorTurbo, floor_raiseToTexture,
    floor_turboLower, stair_build8, stair_turbo16,
};
use super::thinker::T_MoveFloor;
use crate::doom::c_ffi as cffi;
use crate::doom::c_ffi::LinedefFlag;
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_lights::{line_t, sector_t};
use crate::doom::p_setup::sectors;
use crate::doom::p_spec::{
    getSector, getSide, twoSided, P_FindHighestFloorSurrounding, P_FindLowestCeilingSurrounding,
    P_FindLowestFloorSurrounding, P_FindNextHighestFloor, P_FindSectorFromLineTag,
};
use crate::doom::p_tick::P_AddThinker;
use crate::doom::r_data::textureheight;
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

/// Convenience alias for `c_int::MAX`, used as an initial "infinity" in texture height searches.
const INT_MAX: c_int = c_int::MAX;

/// Linedef-triggered event: activate floor movement on all tagged sectors.
///
/// For each sector whose tag matches `line`'s tag and that has no active
/// special, allocates a `floormove_t` thinker and configures it according to
/// `floortype`. Returns `1` if at least one floor was started, `0` otherwise.
///
/// # Safety
///
/// `line` must be a valid non-null pointer for the current map; the global
/// `sectors` array must be initialised.
///
/// # FIXME
/// The `floor_raiseFloorCrush` arm only sets `crush = 1` and leaves direction,
/// sector, speed, and `floordestheight` uninitialised. In C, the switch falls
/// through into `raiseFloor`, which sets those fields. The Rust port handles
/// `raiseFloor` and `raiseFloorCrush` as separate arms; only the `raiseFloor`
/// arm sets direction/speed/dest, so `raiseFloorCrush` sectors never actually
/// move. This is a behavioural divergence from the C original.
///
/// # FIXME
/// In the `floor_raiseFloor24` arm, the line
/// `(*floor).floordestheight = (*floor).sector.offset_from(sec as *mut sector_t) as fixed_t`
/// is a leftover dead assignment that is immediately overwritten. It is harmless
/// (the correct value is set on the next line) but should be removed.
#[no_mangle]
pub unsafe extern "C" fn EV_DoFloor(line: *mut line_t, floortype: c_int) -> c_int
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

        if !(*sec).specialdata.is_null() { continue; }

        rtn = 1;
        let floor = Z_Malloc(
            std::mem::size_of::<floormove_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut floormove_t;
        P_AddThinker(&mut (*floor).thinker);
        (*sec).specialdata = floor as *mut c_void;
        (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut floormove_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_MoveFloor));
        (*floor).r#type = floortype;
        (*floor).crush = 0;

        match floortype
        {
            floor_lowerFloor =>
            {
                (*floor).direction = -1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = P_FindHighestFloorSurrounding(sec);
            }
            floor_lowerFloorToLowest =>
            {
                (*floor).direction = -1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = P_FindLowestFloorSurrounding(sec);
            }
            floor_turboLower =>
            {
                (*floor).direction = -1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED * 4;
                (*floor).floordestheight = P_FindHighestFloorSurrounding(sec);
                if(*floor).floordestheight != (*sec).floorheight { (*floor).floordestheight += 8 * FRACUNIT; }
            }
            floor_raiseFloorCrush =>
            {
                // FIXME: In C, `raiseFloorCrush` falls through into `raiseFloor`,
                // so it sets crush = true AND then configures direction/speed/dest.
                // Here the arms are separate: only crush is set; the floor never
                // actually moves. This diverges from the C original.
                (*floor).crush = 1;
            }
            floor_raiseFloor =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = P_FindLowestCeilingSurrounding(sec);
                if(*floor).floordestheight > (*sec).ceilingheight { (*floor).floordestheight = (*sec).ceilingheight; }
                (*floor).floordestheight -= (8 * FRACUNIT) * ((floortype == floor_raiseFloorCrush) as c_int);
            }
            floor_raiseFloorTurbo =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED * 4;
                (*floor).floordestheight = P_FindNextHighestFloor(sec, (*sec).floorheight);
            }
            floor_raiseFloorToNearest =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = P_FindNextHighestFloor(sec, (*sec).floorheight);
            }
            floor_raiseFloor24 =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                // FIXME: the next line is a dead assignment (offset_from produces the wrong type
                // and the value is immediately overwritten); it should be removed.
                (*floor).floordestheight = (*floor).sector.offset_from(sec as *mut sector_t) as fixed_t;
                (*floor).floordestheight = (*sec).floorheight + 24 * FRACUNIT;
            }
            floor_raiseFloor512 =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = (*sec).floorheight + 512 * FRACUNIT;
            }
            floor_raiseFloor24AndChange =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = (*sec).floorheight + 24 * FRACUNIT;
                (*sec).floorpic = (*(*line).frontsector).floorpic;
                (*sec).special = (*(*line).frontsector).special;
            }
            floor_raiseToTexture =>
            {
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                let mut minsize: c_int = INT_MAX;
                for i in 0..(*sec).linecount as usize
                {
                    if twoSided(secnum, i as c_int) != 0
                    {
                        let side = getSide(secnum, i as c_int, 0);
                        if (*side).bottomtexture >= 0
                        {
                            let th = *textureheight.offset((*side).bottomtexture as isize);
                            if th < minsize { minsize = th; }
                        }
                        let side = getSide(secnum, i as c_int, 1);
                        if (*side).bottomtexture >= 0
                        {
                            let th = *textureheight.offset((*side).bottomtexture as isize);
                            if th < minsize { minsize = th; }
                        }
                    }
                }
                (*floor).floordestheight = (*sec).floorheight + minsize;
            }
            floor_lowerAndChange =>
            {
                (*floor).direction = -1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = FLOORSPEED;
                (*floor).floordestheight = P_FindLowestFloorSurrounding(sec);
                (*floor).texture = (*sec).floorpic;

                for i in 0..(*sec).linecount as c_int
                {
                    if twoSided(secnum, i) != 0
                    {
                        let side0 = getSide(secnum, i, 0);
                        if (*side0).sector.offset_from(sectors) as c_int == secnum
                        {
                            let check_sec = getSector(secnum, i, 1);
                            if (*check_sec).floorheight == (*floor).floordestheight
                            {
                                (*floor).texture = (*check_sec).floorpic;
                                (*floor).newspecial = (*check_sec).special as c_int;
                                break;
                            }
                        }
                        else
                        {
                            let check_sec = getSector(secnum, i, 0);
                            if (*check_sec).floorheight == (*floor).floordestheight
                            {
                                (*floor).texture = (*check_sec).floorpic;
                                (*floor).newspecial = (*check_sec).special as c_int;
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    rtn
}

/// Linedef-triggered event: build a rising staircase starting from tagged sectors.
///
/// For each tagged sector, raises its floor by `stairsize` (8 or 16 units
/// depending on `stype`), then walks adjacent sectors that share the same floor
/// texture, raising each step by another `stairsize`. Sectors already moving are
/// skipped but the height counter still increments so the step sequence is
/// preserved.
///
/// Returns `1` if at least one stair was started, `0` otherwise.
///
/// # Safety
///
/// `line` must be a valid non-null pointer for the current map; the global
/// `sectors` array must be initialised.
#[no_mangle]
pub unsafe extern "C" fn EV_BuildStairs(line: *mut line_t, stype: c_int) -> c_int
{
    let mut secnum: c_int = -1;
    let mut rtn: c_int = 0;

    while
    {
        secnum = P_FindSectorFromLineTag(line as *mut cffi::line_t, secnum);
        secnum
    } >= 0
    {
        let mut sec = sectors.add(secnum as usize);

        if !(*sec).specialdata.is_null() { continue; }

        rtn = 1;
        let mut floor = Z_Malloc(
            std::mem::size_of::<floormove_t>() as c_int,
            PU_LEVSPEC,
            std::ptr::null_mut(),
        ) as *mut floormove_t;
        P_AddThinker(&mut (*floor).thinker);
        (*sec).specialdata = floor as *mut c_void;
        (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
            unsafe extern "C" fn(*mut floormove_t),
            unsafe extern "C" fn(*mut c_void),
        >(T_MoveFloor));
        (*floor).direction = 1;
        (*floor).sector = sec as *mut sector_t;

        let (speed, stairsize): (fixed_t, fixed_t) = match stype
        {
            stair_build8 => (FLOORSPEED / 4, 8 * FRACUNIT),
            stair_turbo16 => (FLOORSPEED * 4, 16 * FRACUNIT),
            _ => (FLOORSPEED, 8 * FRACUNIT),
        };

        (*floor).speed = speed;
        let mut height: fixed_t = (*sec).floorheight + stairsize;
        (*floor).floordestheight = height;

        let texture = (*sec).floorpic;

        // Find next sector to raise
        loop
        {
            let mut ok: c_int = 0;
            for i in 0..(*sec).linecount as c_int
            {
                let l = *(*sec).lines.offset(i as isize) as *mut line_t;
                if(*l).flags & LinedefFlag::TWOSIDED as i16 == 0 { continue; }

                let mut tsec = (*l).frontsector;
                let newsecnum = tsec.offset_from(sectors as *mut sector_t) as c_int;

                if secnum != newsecnum { continue; }

                tsec = (*l).backsector;
                let newsecnum = tsec.offset_from(sectors as *mut sector_t) as c_int;

                if(*tsec).floorpic != texture { continue; }

                height += stairsize;

                if !(*tsec).specialdata.is_null() { continue; }

                sec = tsec as *mut cffi::sector_t;
                let _secnum_new = newsecnum;
                floor = Z_Malloc(
                    std::mem::size_of::<floormove_t>() as c_int,
                    PU_LEVSPEC,
                    std::ptr::null_mut(),
                ) as *mut floormove_t;

                P_AddThinker(&mut (*floor).thinker);

                (*sec).specialdata = floor as *mut c_void;
                (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
                    unsafe extern "C" fn(*mut floormove_t),
                    unsafe extern "C" fn(*mut c_void),
                >(T_MoveFloor));
                (*floor).direction = 1;
                (*floor).sector = sec as *mut sector_t;
                (*floor).speed = speed;
                (*floor).floordestheight = height;
                ok = 1;
                break;
            }
            if ok == 0 { break; }
        }
    }
    rtn
}
