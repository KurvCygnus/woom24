//! Line-crossing and line-shooting special activation: the big
//! `line->special` dispatch (one-shot vs retriggerable, the
//! special-clear writes) and the impact-trigger trio -- bit-exact with
//! the `P_CrossSpecialLine` / `P_ShootSpecialLine` half of
//! `vendor/doomgeneric/p_spec.c`.

use std::ffi::c_int;

use crate::doom::c_ffi::{line_t, mobj_t};
use crate::doom::g_game::{G_ExitLevel, G_SecretExitLevel};
use crate::doom::info::{MT_BFG, MT_BRUISERSHOT, MT_HEADSHOT, MT_PLASMA, MT_ROCKET, MT_TROOPSHOT};
use crate::doom::p_ceilng::{EV_CeilingCrushStop, EV_DoCeiling};
use crate::doom::p_doors::EV_DoDoor;
use crate::doom::p_floor::{EV_BuildStairs, EV_DoFloor};
use crate::doom::p_lights::{EV_LightTurnOn, EV_StartLightStrobing, EV_TurnTagLightsOff};
use crate::doom::p_plats::{EV_DoPlat, EV_StopPlat};
use crate::doom::p_setup::lines;
use crate::doom::p_switch::P_ChangeSwitchTexture;
use crate::doom::p_telept::EV_Teleport;

use super::consts::{
    blazeDWUS, build8, crushAndRaise, downWaitUpStay, fastCrushAndRaise, lowerAndChange,
    lowerAndCrush, lowerFloor, lowerFloorToLowest, perpetualRaise, raiseFloor, raiseFloor24,
    raiseFloor24AndChange, raiseFloorCrush, raiseFloorToNearest, raiseFloorTurbo, raiseToHighest,
    raiseToNearestAndChange, raiseToTexture, silentCrushAndRaise, turbo16, turboLower,
    vld_blazeClose, vld_blazeOpen, vld_blazeRaise, vld_close, vld_close30ThenOpen, vld_normal,
    vld_open,
};

/// `line_t` as seen by `p_lights` - `#[repr(C)]` layout identical to `c_ffi::line_t`.
type LightsLine = crate::doom::p_lights::line_t;
/// `line_t` as seen by `p_telept` - `#[repr(C)]` layout identical to `c_ffi::line_t`.
type TeleptLine = crate::doom::p_telept::line_t;
/// `mobj_t` as seen by `p_telept` - `#[repr(C)]` layout identical to `c_ffi::mobj_t`.
type TeleptMobj = crate::doom::p_telept::mobj_t;

/// Processes the special action for a linedef that a map object has just crossed.
///
/// Mirrors `P_CrossSpecialLine` in `p_spec.c`; called from the `try_move`
/// drain in p_map (whose `spechit drain -> P_CrossSpecialLine` ordering is
/// pinned by `p_map/mod.rs`).
///
/// ## Technical Details
///
/// The dispatch order and the special-clear writes are the demo surface.
/// Non-player things are filtered first: projectiles (rockets, plasma, BFG
/// balls, trooper/head/bruiser shots) are always ignored; other monsters may
/// only activate specials 4, 10, 39, 88, 97, 125, and 126. One-shot
/// specials (2-71, 130-141) run their `EV_*` handler and then write
/// `line->special = 0` so the line cannot fire again; retriggerable
/// specials (72-129) leave it armed. Special 52/124 exit the level without
/// clearing. Specials 125/126 carry the monster-only teleport guard
/// (`thing` must have no player) as a match guard, so the arm itself
/// re-checks -- moving that check would change which specials a monster
/// consumes.
///
/// ## On Calling
///
/// `linenum` indexes the global `lines` array, `side` is the approach side
/// (0 = front), `thing` the crossing mobj. The handler order inside each
/// arm (EV_* call, then the clear) is verbatim upstream.
///
/// # Safety
///
/// `linenum` must index a loaded linedef and `thing` must be a valid,
/// non-null mobj pointer.
#[doc(alias = "P_CrossSpecialLine")]
#[export_name = "P_CrossSpecialLine"]
pub unsafe extern "C" fn cross_special_line(linenum: c_int, side: c_int, thing: *mut mobj_t)
{
    let line = lines.offset(linenum as isize);

    if (*thing).player.is_null()
    {
        match (*thing).type_
        {
            MT_ROCKET | MT_PLASMA | MT_BFG | MT_TROOPSHOT | MT_HEADSHOT | MT_BRUISERSHOT => { return; }
            _ => {}
        }

        let mut ok = 0;
        match (*line).special as c_int
        {
            39 | 97 | 125 | 126 | 4 | 10 | 88 => ok = 1,
            _ => {}
        }
        if ok == 0 { return; }
    }

    match (*line).special as c_int
    {
        2 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_open);
            (*line).special = 0;
        }
        3 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_close);
            (*line).special = 0;
        }
        4 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_normal);
            (*line).special = 0;
        }
        5 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloor);
            (*line).special = 0;
        }
        6 =>
        {
            EV_DoCeiling(line as *mut LightsLine, fastCrushAndRaise);
            (*line).special = 0;
        }
        8 =>
        {
            EV_BuildStairs(line as *mut LightsLine, build8);
            (*line).special = 0;
        }
        10 =>
        {
            EV_DoPlat(line as *mut LightsLine, downWaitUpStay, 0);
            (*line).special = 0;
        }
        12 =>
        {
            EV_LightTurnOn(line as *mut LightsLine, 0);
            (*line).special = 0;
        }
        13 =>
        {
            EV_LightTurnOn(line as *mut LightsLine, 255);
            (*line).special = 0;
        }
        16 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_close30ThenOpen);
            (*line).special = 0;
        }
        17 =>
        {
            EV_StartLightStrobing(line as *mut LightsLine);
            (*line).special = 0;
        }
        19 =>
        {
            EV_DoFloor(line as *mut LightsLine, lowerFloor);
            (*line).special = 0;
        }
        22 =>
        {
            EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0);
            (*line).special = 0;
        }
        25 =>
        {
            EV_DoCeiling(line as *mut LightsLine, crushAndRaise);
            (*line).special = 0;
        }
        30 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseToTexture);
            (*line).special = 0;
        }
        35 =>
        {
            EV_LightTurnOn(line as *mut LightsLine, 35);
            (*line).special = 0;
        }
        36 =>
        {
            EV_DoFloor(line as *mut LightsLine, turboLower);
            (*line).special = 0;
        }
        37 =>
        {
            EV_DoFloor(line as *mut LightsLine, lowerAndChange);
            (*line).special = 0;
        }
        38 =>
        {
            EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest);
            (*line).special = 0;
        }
        39 =>
        {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
            (*line).special = 0;
        }
        40 =>
        {
            EV_DoCeiling(line as *mut LightsLine, raiseToHighest);
            EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest);
            (*line).special = 0;
        }
        44 =>
        {
            EV_DoCeiling(line as *mut LightsLine, lowerAndCrush);
            (*line).special = 0;
        }
        52 => { G_ExitLevel(); }
        53 =>
        {
            EV_DoPlat(line as *mut LightsLine, perpetualRaise, 0);
            (*line).special = 0;
        }
        54 =>
        {
            EV_StopPlat(line as *mut LightsLine);
            (*line).special = 0;
        }
        56 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloorCrush);
            (*line).special = 0;
        }
        57 =>
        {
            EV_CeilingCrushStop(line as *mut LightsLine);
            (*line).special = 0;
        }
        58 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24);
            (*line).special = 0;
        }
        59 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24AndChange);
            (*line).special = 0;
        }
        104 =>
        {
            EV_TurnTagLightsOff(line as *mut LightsLine);
            (*line).special = 0;
        }
        108 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_blazeRaise);
            (*line).special = 0;
        }
        109 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_blazeOpen);
            (*line).special = 0;
        }
        100 =>
        {
            EV_BuildStairs(line as *mut LightsLine, turbo16);
            (*line).special = 0;
        }
        110 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_blazeClose);
            (*line).special = 0;
        }
        119 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloorToNearest);
            (*line).special = 0;
        }
        121 =>
        {
            EV_DoPlat(line as *mut LightsLine, blazeDWUS, 0);
            (*line).special = 0;
        }
        124 => { G_SecretExitLevel(); }
        125 if (*thing).player.is_null() =>
        {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
            (*line).special = 0;
        }
        130 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloorTurbo);
            (*line).special = 0;
        }
        141 =>
        {
            EV_DoCeiling(line as *mut LightsLine, silentCrushAndRaise);
            (*line).special = 0;
        }
        // RETRIGGERS
        72 => { EV_DoCeiling(line as *mut LightsLine, lowerAndCrush); }
        73 => { EV_DoCeiling(line as *mut LightsLine, crushAndRaise); }
        74 => { EV_CeilingCrushStop(line as *mut LightsLine); }
        75 => { EV_DoDoor(line as *mut LightsLine, vld_close); }
        76 => { EV_DoDoor(line as *mut LightsLine, vld_close30ThenOpen); }
        77 => { EV_DoCeiling(line as *mut LightsLine, fastCrushAndRaise); }
        79 => { EV_LightTurnOn(line as *mut LightsLine, 35); }
        80 => { EV_LightTurnOn(line as *mut LightsLine, 0); }
        81 => { EV_LightTurnOn(line as *mut LightsLine, 255); }
        82 => { EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest); }
        83 => { EV_DoFloor(line as *mut LightsLine, lowerFloor); }
        84 => { EV_DoFloor(line as *mut LightsLine, lowerAndChange); }
        86 => { EV_DoDoor(line as *mut LightsLine, vld_open); }
        87 => { EV_DoPlat(line as *mut LightsLine, perpetualRaise, 0); }
        88 => { EV_DoPlat(line as *mut LightsLine, downWaitUpStay, 0); }
        89 => { EV_StopPlat(line as *mut LightsLine); }
        90 => { EV_DoDoor(line as *mut LightsLine, vld_normal); }
        91 => { EV_DoFloor(line as *mut LightsLine, raiseFloor); }
        92 => { EV_DoFloor(line as *mut LightsLine, raiseFloor24); }
        93 => { EV_DoFloor(line as *mut LightsLine, raiseFloor24AndChange); }
        94 => { EV_DoFloor(line as *mut LightsLine, raiseFloorCrush); }
        95 => { EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0); }
        96 => { EV_DoFloor(line as *mut LightsLine, raiseToTexture); }
        97 => { EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj); }
        98 => { EV_DoFloor(line as *mut LightsLine, turboLower); }
        105 => { EV_DoDoor(line as *mut LightsLine, vld_blazeRaise); }
        106 => { EV_DoDoor(line as *mut LightsLine, vld_blazeOpen); }
        107 => { EV_DoDoor(line as *mut LightsLine, vld_blazeClose); }
        120 => { EV_DoPlat(line as *mut LightsLine, blazeDWUS, 0); }
        126 if (*thing).player.is_null() => { EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj); }
        128 => { EV_DoFloor(line as *mut LightsLine, raiseFloorToNearest); }
        129 => { EV_DoFloor(line as *mut LightsLine, raiseFloorTurbo); }
        _ => {}
    }
}

/// Processes a linedef special triggered by a projectile or hitscan impact.
///
/// Mirrors `P_ShootSpecialLine` in `p_spec.c`; called from
/// `ptr_shoot_traverse` (p_map).
///
/// ## Technical Details
///
/// Handles the three impact (gun-activated) line specials: 24 (raise floor,
/// one-shot switch), 46 (open door, re-triggerable), 47 (raise platform to
/// nearest floor, one-shot). Non-player things can only activate special
/// 46 -- the gate reads before the dispatch, and each arm runs its `EV_*`
/// handler then `P_ChangeSwitchTexture` in that order (the switch-state
/// write order is demo-visible through later switch reads).
///
/// ## On Calling
///
/// `thing` is the attacking mobj, `line` the hit linedef.
///
/// # Safety
///
/// Both pointers must be valid, non-null, and reference loaded map data.
#[doc(alias = "P_ShootSpecialLine")]
#[export_name = "P_ShootSpecialLine"]
pub unsafe extern "C" fn shoot_special_line(thing: *mut mobj_t, line: *mut line_t)
{
    if (*thing).player.is_null()
    {
        let mut ok = 0;
        if(*line).special as c_int == 46 { ok = 1 }
        if ok == 0 { return; }
    }

    match (*line).special as c_int
    {
        24 =>
        {
            EV_DoFloor(line as *mut LightsLine, raiseFloor);
            P_ChangeSwitchTexture(line as *mut LightsLine, 0);
        }
        46 =>
        {
            EV_DoDoor(line as *mut LightsLine, vld_open);
            P_ChangeSwitchTexture(line as *mut LightsLine, 1);
        }
        47 =>
        {
            EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0);
            P_ChangeSwitchTexture(line as *mut LightsLine, 0);
        }
        _ => {}
    }
}
