//! Linedef-use dispatch: `P_UseSpecialLine`, the gate that decides which
//! switch, button, door, floor, ceiling, plat, stair, donut, and light
//! specials fire when a thing presses Use against a linedef -- bit-exact
//! with the `P_UseSpecialLine` half of `vendor/doomgeneric/p_switch.c`.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};

use super::state::{
    ceiling_crushAndRaise, ceiling_lowerToFloor, floor_lowerFloor, floor_lowerFloorToLowest,
    floor_raiseFloor, floor_raiseFloorCrush, floor_raiseFloorToNearest, floor_raiseFloorTurbo,
    floor_raiseFloor512, floor_turboLower, plat_blazeDWUS, plat_downWaitUpStay, plat_raiseAndChange,
    plat_raiseToNearestAndChange, stair_build8, stair_turbo16, vld_blazeClose, vld_blazeOpen,
    vld_blazeRaise, vld_close, vld_normal, vld_open,
};
use super::switches::P_ChangeSwitchTexture;
use crate::doom::c_ffi::LinedefFlag;
use crate::doom::g_game::{G_ExitLevel, G_SecretExitLevel};
use crate::doom::p_ceilng::EV_DoCeiling;
use crate::doom::p_doors::{EV_DoDoor, EV_DoLockedDoor, EV_VerticalDoor};
use crate::doom::p_floor::{EV_BuildStairs, EV_DoFloor};
use crate::doom::p_lights::{line_t, EV_LightTurnOn};
use crate::doom::p_plats::EV_DoPlat;
use crate::doom::p_spec::EV_DoDonut;

// Type aliases for cross-module pointer casts (all #[repr(C)] identical layouts).
type CffiLine = crate::doom::c_ffi::line_t;
type CffiMobj = crate::doom::c_ffi::mobj_t;

/// Dispatch a player or monster Use action on a special linedef.
///
/// Called by `P_UseLines` when `thing` presses Use against `line`.  `side`
/// is `0` for the front face (the normal case) or `1` for the back face.
/// Using the back side is only allowed for special 124 (unused sliding
/// door).
///
/// Non-player actors can only activate lines that are not secret and have
/// one of the four manual-door specials (1, 32, 33, 34).
///
/// The main dispatch covers:
/// - Manual doors (specials 1, 26-28, 31-34, 117-118): calls
///   `EV_VerticalDoor` directly with no texture change.
/// - One-shot switches (specials 7-140): perform the action, then call
///   `P_ChangeSwitchTexture(line, 0)` to flip the texture permanently.
/// - Repeatable buttons (specials 42-139): perform the action, then call
///   `P_ChangeSwitchTexture(line, 1)` to flip and queue a reset timer.
///
/// Returns `1` (true) after handling any special; unrecognised specials
/// fall through silently and also return `1`.
///
/// # Safety
///
/// `thing` must be a valid, non-null pointer to a `mobj_t`.  `line` must
/// be a valid, non-null pointer to a live `line_t`.  All global game-state
/// statics must only be accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_UseSpecialLine(
    thing: *mut c_void,
    line: *mut line_t,
    side: c_int,
) -> c_int
{
    // Err...
    // Use the back sides of VERY SPECIAL lines...
    if side != 0
    {
        match (*line).special
        {
            124 =>
            {
                // Sliding door open&close
                // UNUSED?
            }
            _ => return 0,
        }
    }

    // Switches that other things can activate.
    let mobj = thing as *mut crate::doom::c_ffi::mobj_t;
    if (*mobj).player.is_null()
    {
        // never open secret doors
        if(*line).flags & LinedefFlag::SECRET as i16 != 0 { return 0; }
        match (*line).special
        {
            1 | 32 | 33 | 34 => {}
            _ => return 0,
        }
    }

    // do something
    match (*line).special
    {
        // MANUALS
        1 | 26 | 27 | 28 | 31 | 32 | 33 | 34 | 117 | 118 =>
        {
            EV_VerticalDoor(line, thing as *mut CffiMobj);
        }

        // SWITCHES
        7 if EV_BuildStairs(line, stair_build8) != 0 => { P_ChangeSwitchTexture(line, 0); }
        9 if EV_DoDonut(line as *mut CffiLine) != 0 => { P_ChangeSwitchTexture(line, 0); }
        11 =>
        {
            P_ChangeSwitchTexture(line, 0);
            G_ExitLevel();
        }
        14 if EV_DoPlat(line, plat_raiseAndChange, 32) != 0 => { P_ChangeSwitchTexture(line, 0); }
        15 if EV_DoPlat(line, plat_raiseAndChange, 24) != 0 => { P_ChangeSwitchTexture(line, 0); }
        18 if EV_DoFloor(line, floor_raiseFloorToNearest) != 0 => { P_ChangeSwitchTexture(line, 0); }
        20 if EV_DoPlat(line, plat_raiseToNearestAndChange, 0) != 0 => { P_ChangeSwitchTexture(line, 0); }
        21 if EV_DoPlat(line, plat_downWaitUpStay, 0) != 0 => { P_ChangeSwitchTexture(line, 0); }
        23 if EV_DoFloor(line, floor_lowerFloorToLowest) != 0 => { P_ChangeSwitchTexture(line, 0); }
        29 if EV_DoDoor(line, vld_normal) != 0 => { P_ChangeSwitchTexture(line, 0); }
        41 if EV_DoCeiling(line, ceiling_lowerToFloor) != 0 => { P_ChangeSwitchTexture(line, 0); }
        71 if EV_DoFloor(line, floor_turboLower) != 0 => { P_ChangeSwitchTexture(line, 0); }
        49 if EV_DoCeiling(line, ceiling_crushAndRaise) != 0 => { P_ChangeSwitchTexture(line, 0); }
        50 if EV_DoDoor(line, vld_close) != 0 => { P_ChangeSwitchTexture(line, 0); }
        51 =>
        {
            P_ChangeSwitchTexture(line, 0);
            G_SecretExitLevel();
        }
        55 if EV_DoFloor(line, floor_raiseFloorCrush) != 0 => { P_ChangeSwitchTexture(line, 0); }
        101 if EV_DoFloor(line, floor_raiseFloor) != 0 => { P_ChangeSwitchTexture(line, 0); }
        102 if EV_DoFloor(line, floor_lowerFloor) != 0 => { P_ChangeSwitchTexture(line, 0); }
        103 if EV_DoDoor(line, vld_open) != 0 => { P_ChangeSwitchTexture(line, 0); }
        111 if EV_DoDoor(line, vld_blazeRaise) != 0 => { P_ChangeSwitchTexture(line, 0); }
        112 if EV_DoDoor(line, vld_blazeOpen) != 0 => { P_ChangeSwitchTexture(line, 0); }
        113 if EV_DoDoor(line, vld_blazeClose) != 0 => { P_ChangeSwitchTexture(line, 0); }
        122 if EV_DoPlat(line, plat_blazeDWUS, 0) != 0 => { P_ChangeSwitchTexture(line, 0); }
        127 if EV_BuildStairs(line, stair_turbo16) != 0 => { P_ChangeSwitchTexture(line, 0); }
        131 if EV_DoFloor(line, floor_raiseFloorTurbo) != 0 => { P_ChangeSwitchTexture(line, 0); }
        133 | 135 | 137 if EV_DoLockedDoor(line, vld_blazeOpen, thing as *mut CffiMobj) != 0 => { P_ChangeSwitchTexture(line, 0); }
        140 if EV_DoFloor(line, floor_raiseFloor512) != 0 => { P_ChangeSwitchTexture(line, 0); }

        // BUTTONS
        42 if EV_DoDoor(line, vld_close) != 0 => { P_ChangeSwitchTexture(line, 1); }
        43 if EV_DoCeiling(line, ceiling_lowerToFloor) != 0 => { P_ChangeSwitchTexture(line, 1); }
        45 if EV_DoFloor(line, floor_lowerFloor) != 0 => { P_ChangeSwitchTexture(line, 1); }
        60 if EV_DoFloor(line, floor_lowerFloorToLowest) != 0 => { P_ChangeSwitchTexture(line, 1); }
        61 if EV_DoDoor(line, vld_open) != 0 => { P_ChangeSwitchTexture(line, 1); }
        62 if EV_DoPlat(line, plat_downWaitUpStay, 1) != 0 => { P_ChangeSwitchTexture(line, 1); }
        63 if EV_DoDoor(line, vld_normal) != 0 => { P_ChangeSwitchTexture(line, 1); }
        64 if EV_DoFloor(line, floor_raiseFloor) != 0 => { P_ChangeSwitchTexture(line, 1); }
        66 if EV_DoPlat(line, plat_raiseAndChange, 24) != 0 => { P_ChangeSwitchTexture(line, 1); }
        67 if EV_DoPlat(line, plat_raiseAndChange, 32) != 0 => { P_ChangeSwitchTexture(line, 1); }
        65 if EV_DoFloor(line, floor_raiseFloorCrush) != 0 => { P_ChangeSwitchTexture(line, 1); }
        68 if EV_DoPlat(line, plat_raiseToNearestAndChange, 0) != 0 => { P_ChangeSwitchTexture(line, 1); }
        69 if EV_DoFloor(line, floor_raiseFloorToNearest) != 0 => { P_ChangeSwitchTexture(line, 1); }
        70 if EV_DoFloor(line, floor_turboLower) != 0 => { P_ChangeSwitchTexture(line, 1); }
        114 if EV_DoDoor(line, vld_blazeRaise) != 0 => { P_ChangeSwitchTexture(line, 1); }
        115 if EV_DoDoor(line, vld_blazeOpen) != 0 => { P_ChangeSwitchTexture(line, 1); }
        116 if EV_DoDoor(line, vld_blazeClose) != 0 => { P_ChangeSwitchTexture(line, 1); }
        123 if EV_DoPlat(line, plat_blazeDWUS, 0) != 0 => { P_ChangeSwitchTexture(line, 1); }
        132 if EV_DoFloor(line, floor_raiseFloorTurbo) != 0 => { P_ChangeSwitchTexture(line, 1); }
        99 | 134 | 136 if EV_DoLockedDoor(line, vld_blazeOpen, thing as *mut CffiMobj) != 0 => { P_ChangeSwitchTexture(line, 1); }
        138 =>
        {
            EV_LightTurnOn(line, 255);
            P_ChangeSwitchTexture(line, 1);
        }
        139 =>
        {
            EV_LightTurnOn(line, 35);
            P_ChangeSwitchTexture(line, 1);
        }

        _ => {}
    }

    1 // true
}
