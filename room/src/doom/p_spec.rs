//! Rust port of vendor/doomgeneric/p_spec.c.
//!
//! Special sector/line action dispatcher: texture animation, height and
//! lighting changes, line tag handling, sector triggers, and the donut
//! effect.  Also contains the utility functions (`getSide`, `getSector`,
//! `twoSided`, `P_Find*Surrounding`) used by the floor/ceiling/platform
//! code.
//!
//! Notable Rust-vs-C differences:
//! - The `animdefs` table is a Rust `const` slice of tuples instead of a
//!   C array of `animdef_t` structs; the `animdef_t` struct is kept for
//!   layout testing only.
//! - `DonutOverrun` uses `static mut` locals to replicate C `static` locals;
//!   first-call initialisation is guarded by a `first` flag exactly as in C.
//! - Button `where` enum (C `bwhere_e`) is stored as a raw `c_int` (`0/1/2`)
//!   because the Rust FFI button type uses an integer discriminant.
//! - `P_PlayerInSpecialSector` navigates via `subsector_t` to reach the
//!   sector; the C version accesses `player->mo->subsector->sector` directly.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use crate::doom::sounds::Sfx;
use std::ffi::{c_char, c_int, c_short, c_void};
use std::ptr;

use crate::doom::c_ffi::{line_t, mobj_t, sector_t, side_t, LinedefFlag, FLOORSPEED};
use crate::doom::d_player::{PlayerT, CF_GODMODE};
use crate::doom::i_timer::TICRATE;
use crate::doom::info::{MT_BFG, MT_BRUISERSHOT, MT_HEADSHOT, MT_PLASMA, MT_ROCKET, MT_TROOPSHOT};
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::m_misc::M_StrToInt;
use crate::doom::m_random::P_Random;
use crate::doom::p_floor::floormove_t;
use crate::doom::p_setup::{lines, numlines, numsectors, sectors, sides};
use crate::doom::p_switch::{buttonlist, MAXBUTTONS};
use crate::doom::p_tick::{leveltime, P_AddThinker};
use crate::doom::r_data::{
    flattranslation, numflats, texturetranslation, R_CheckTextureNumForName, R_FlatNumForName,
    R_TextureNumForName,
};
use crate::doom::s_sound::S_StartSound;
use crate::doom::violations::{self, VanillaViolation};
use crate::doom::z_zone::PU_LEVSPEC;
use crate::i_error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of animated texture/flat cycles that can be active at once.
/// Corresponds to `MAXANIMS` in p_spec.c.
const MAXANIMS: usize = 32;

/// Maximum number of scrolling-wall linedefs that may be registered per level.
/// Corresponds to `MAXLINEANIMS` in p_spec.c (Vanilla Doom limit is 64).
const MAXLINEANIMS: usize = 64;

/// Maximum number of adjoining sectors examined by `P_FindNextHighestFloor`.
/// Exceeding this limit emulates the vanilla stack-overflow behaviour.
/// Corresponds to `MAX_ADJOINING_SECTORS` in p_spec.c.
const MAX_ADJOINING_SECTORS: usize = 20;

// vldoor_e — door direction/behaviour constants used by `EV_DoDoor`.
/// Door opens and stays open. (`vld_open`)
const vld_normal: c_int = 0;
/// Door closes, waits 30 seconds, then opens again. (`vld_close30ThenOpen`)
const vld_close30ThenOpen: c_int = 1;
/// Door closes and stays closed. (`vld_close`)
const vld_close: c_int = 2;
/// Door opens normally (raise then wait then lower). (`vld_open` in C is 3 here mapped as `vld_normal=0`)
const vld_open: c_int = 3;
/// Blaze-speed raise-then-lower door. (`vld_blazeRaise`)
const vld_blazeRaise: c_int = 5;
/// Blaze-speed open door. (`vld_blazeOpen`)
const vld_blazeOpen: c_int = 6;
/// Blaze-speed close door. (`vld_blazeClose`)
const vld_blazeClose: c_int = 7;

// floor_e — floor movement type constants used by `EV_DoFloor`.
/// Lower floor to next lowest neighbouring floor. (`lowerFloor`)
const lowerFloor: c_int = 0;
/// Lower floor to the absolute lowest neighbouring floor. (`lowerFloorToLowest`)
const lowerFloorToLowest: c_int = 1;
/// Lower floor at turbo speed. (`turboLower`)
const turboLower: c_int = 2;
/// Raise floor to next highest neighbouring floor. (`raiseFloor`)
const raiseFloor: c_int = 3;
/// Raise floor to nearest neighbouring floor. (`raiseFloorToNearest`)
const raiseFloorToNearest: c_int = 4;
/// Raise floor to the height of the shortest lower texture on a bounding wall. (`raiseToTexture`)
const raiseToTexture: c_int = 5;
/// Lower floor and change its texture/special to match the destination sector. (`lowerAndChange`)
const lowerAndChange: c_int = 6;
/// Raise floor by exactly 24 map units (fixed-point 16.16). (`raiseFloor24`)
const raiseFloor24: c_int = 7;
/// Raise floor by 24 map units and change texture/special. (`raiseFloor24AndChange`)
const raiseFloor24AndChange: c_int = 8;
/// Raise floor while crushing anything caught between floor and ceiling. (`raiseFloorCrush`)
const raiseFloorCrush: c_int = 9;
/// Raise floor at turbo speed. (`raiseFloorTurbo`)
const raiseFloorTurbo: c_int = 10;
/// Raise floor to the height of the outer (ring) sector in a donut effect. (`donutRaise`)
const donutRaise: c_int = 11;

// ceiling_e — ceiling movement type constants used by `EV_DoCeiling`.
/// Raise ceiling to the highest neighbouring ceiling. (`raiseToHighest`)
const raiseToHighest: c_int = 1;
/// Lower ceiling while crushing. (`lowerAndCrush`)
const lowerAndCrush: c_int = 2;
/// Crush ceiling down then raise it repeatedly. (`crushAndRaise`)
const crushAndRaise: c_int = 3;
/// Fast crush-and-raise ceiling. (`fastCrushAndRaise`)
const fastCrushAndRaise: c_int = 4;
/// Silent crush-and-raise ceiling (no grinding sound). (`silentCrushAndRaise`)
const silentCrushAndRaise: c_int = 5;

// plattype_e — platform movement type constants used by `EV_DoPlat`.
/// Platform goes down, waits, then rises back up. (`downWaitUpStay`)
const downWaitUpStay: c_int = 1;
/// Platform raises to the nearest floor height and changes texture. (`raiseToNearestAndChange`)
const raiseToNearestAndChange: c_int = 3;
/// Blaze-speed down-wait-up-stay platform. (`blazeDWUS`)
const blazeDWUS: c_int = 4;
/// Platform oscillates perpetually up and down. (`perpetualRaise`)
const perpetualRaise: c_int = 0;

// stair_e — stair-building step size constants used by `EV_BuildStairs`.
/// Build stairs in 8-unit steps. (`build8`)
const build8: c_int = 0;
/// Build stairs in 16-unit turbo steps. (`turbo16`)
const turbo16: c_int = 1;

// powers — index into `player_t::powers[]`.
/// Index of the Radiation Suit power in the powers array; grants immunity to slime damage.
const pw_ironfeet: usize = 3;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Runtime state for a single animated texture or flat sequence.
///
/// Maps to the C `anim_t` typedef in p_spec.c (also used internally in
/// wi_stuff.c with different semantics, but this is the p_spec version).
/// Layout invariant: the struct is `#[repr(C)]` and its size is asserted to
/// be exactly 20 bytes on 64-bit targets (matching the C layout).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct anim_t {
    /// Non-zero if this animation cycle is for wall textures; zero for flats.
    pub istexture: c_int,
    /// WAD lump number of the last frame in the sequence.
    pub picnum: c_int,
    /// WAD lump number of the first frame in the sequence.
    pub basepic: c_int,
    /// Total number of frames in the cycle (`picnum - basepic + 1`).
    pub numpics: c_int,
    /// Tic-count duration of each frame; the sequence advances every `speed` tics.
    pub speed: c_int,
}

/// Static definition of one animation cycle as loaded from the ANIMDEFS table.
///
/// Maps to the C `animdef_t` typedef in p_spec.c.  The Rust code uses a
/// tuple-slice (`ANIMDEFS`) instead of an array of this struct for animation
/// initialisation; `animdef_t` is retained only for ABI size verification.
/// Layout invariant: 28 bytes on 64-bit (4-byte `istexture`, two 9-byte name
/// arrays padded to alignment, 4-byte `speed`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct animdef_t {
    /// Non-zero for wall textures, zero for flats; -1 marks the sentinel entry.
    pub istexture: c_int,
    /// Name of the last frame lump (NUL-padded to 9 bytes).
    pub endname: [c_char; 9],
    /// Name of the first frame lump (NUL-padded to 9 bytes).
    pub startname: [c_char; 9],
    /// Frame duration in tics.
    pub speed: c_int,
}

#[cfg(target_pointer_width = "64")]
mod layout_checks {
    use super::*;
    const _: () = assert!(std::mem::size_of::<anim_t>() == 20);
    const _: () = assert!(std::mem::size_of::<animdef_t>() == 28);
}

// ---------------------------------------------------------------------------
// Animation definitions
// ---------------------------------------------------------------------------

/// Compile-time animation definition table, equivalent to C `animdefs[]`.
///
/// Each tuple is `(istexture, endname, startname, speed)`.  A sentinel entry
/// with `istexture == -1` terminates the list.  Pointers are created from
/// C string literals and are valid for the program lifetime. Sourced from
/// p_spec.c `animdefs[]`.
const ANIMDEFS: &[(c_int, *mut c_char, *mut c_char, c_int)] = &[
    (0, c"NUKAGE3".as_ptr().cast_mut(), c"NUKAGE1".as_ptr().cast_mut(), 8),
    (0, c"FWATER4".as_ptr().cast_mut(), c"FWATER1".as_ptr().cast_mut(), 8),
    (0, c"SWATER4".as_ptr().cast_mut(), c"SWATER1".as_ptr().cast_mut(), 8),
    (0, c"LAVA4".as_ptr().cast_mut(), c"LAVA1".as_ptr().cast_mut(), 8),
    (0, c"BLOOD3".as_ptr().cast_mut(), c"BLOOD1".as_ptr().cast_mut(), 8),
    (0, c"RROCK08".as_ptr().cast_mut(), c"RROCK05".as_ptr().cast_mut(), 8),
    (0, c"SLIME04".as_ptr().cast_mut(), c"SLIME01".as_ptr().cast_mut(), 8),
    (0, c"SLIME08".as_ptr().cast_mut(), c"SLIME05".as_ptr().cast_mut(), 8),
    (0, c"SLIME12".as_ptr().cast_mut(), c"SLIME09".as_ptr().cast_mut(), 8),
    (1, c"BLODGR4".as_ptr().cast_mut(), c"BLODGR1".as_ptr().cast_mut(), 8),
    (1, c"SLADRIP3".as_ptr().cast_mut(), c"SLADRIP1".as_ptr().cast_mut(), 8),
    (1, c"BLODRIP4".as_ptr().cast_mut(), c"BLODRIP1".as_ptr().cast_mut(), 8),
    (1, c"FIREWALL".as_ptr().cast_mut(), c"FIREWALA".as_ptr().cast_mut(), 8),
    (1, c"GSTFONT3".as_ptr().cast_mut(), c"GSTFONT1".as_ptr().cast_mut(), 8),
    (1, c"FIRELAVA".as_ptr().cast_mut(), c"FIRELAV3".as_ptr().cast_mut(), 8),
    (1, c"FIREMAG3".as_ptr().cast_mut(), c"FIREMAG1".as_ptr().cast_mut(), 8),
    (1, c"FIREBLU2".as_ptr().cast_mut(), c"FIREBLU1".as_ptr().cast_mut(), 8),
    (1, c"ROCKRED3".as_ptr().cast_mut(), c"ROCKRED1".as_ptr().cast_mut(), 8),
    (1, c"BFALL4".as_ptr().cast_mut(), c"BFALL1".as_ptr().cast_mut(), 8),
    (1, c"SFALL4".as_ptr().cast_mut(), c"SFALL1".as_ptr().cast_mut(), 8),
    (1, c"WFALL4".as_ptr().cast_mut(), c"WFALL1".as_ptr().cast_mut(), 8),
    (1, c"DBRAIN4".as_ptr().cast_mut(), c"DBRAIN1".as_ptr().cast_mut(), 8),
    (-1, c"".as_ptr().cast_mut(), c"".as_ptr().cast_mut(), 0),
];

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

/// Array of active animation state records, one per registered animation cycle.
///
/// Populated by `P_InitPicAnims`; only entries in `anims[0..lastanim)` are
/// valid.  Exported with C linkage (`#[no_mangle]`) for access from p_spec.c
/// and the renderer (r_data.c reads `texturetranslation`/`flattranslation`
/// which are updated in `P_UpdateSpecials` based on this array).
#[no_mangle]
pub static mut anims: [anim_t; MAXANIMS] = [anim_t {
    istexture: 0,
    picnum: 0,
    basepic: 0,
    numpics: 0,
    speed: 0,
}; MAXANIMS];

/// Pointer one-past the last valid entry in `anims[]`.
///
/// Acts as an end-iterator: the range `anims..lastanim` contains every active
/// animation cycle.  Null on startup, set by `P_InitPicAnims`.  Exported with
/// C linkage for symmetry with the C declaration `extern anim_t* lastanim`.
#[no_mangle]
pub static mut lastanim: *mut anim_t = ptr::null_mut();

/// Number of scrolling-wall linedefs registered in `linespeciallist`.
///
/// Counts lines with special 48 (first-column texture scroll).  Reset to zero
/// by `P_SpawnSpecials` at map start.  Exported with C linkage; referenced by
/// `P_UpdateSpecials` and p_spec.c.
#[no_mangle]
pub static mut numlinespecials: c_short = 0;

/// List of pointers to linedefs that carry the scrolling-wall special (48).
///
/// Populated by `P_SpawnSpecials`; iterated by `P_UpdateSpecials` each tic to
/// advance `textureoffset` by one `FRACUNIT`.  Capped at `MAXLINEANIMS` (64)
/// entries.  Exported with C linkage.
#[no_mangle]
pub static mut linespeciallist: [*mut line_t; MAXLINEANIMS] = [ptr::null_mut(); MAXLINEANIMS];

/// Non-zero when a deathmatch level timer is active.
///
/// Set to 1 by `P_SpawnSpecials` when `timelimit > 0` and `deathmatch != 0`.
/// Checked each tic by `P_UpdateSpecials`.  Stored as `c_int` boolean to
/// match the C declaration `boolean levelTimer`.  Exported with C linkage.
#[no_mangle]
pub static mut levelTimer: c_int = 0; // boolean

/// Remaining tics before the deathmatch time limit expires.
///
/// Initialised by `P_SpawnSpecials` to `timelimit * 60 * TICRATE`.
/// Decremented each tic by `P_UpdateSpecials`; reaching zero calls
/// `G_ExitLevel`.  Exported with C linkage.
#[no_mangle]
pub static mut levelTimeCount: c_int = 0;

// ---------------------------------------------------------------------------
// Externs from other modules / remaining C code
// ---------------------------------------------------------------------------

use crate::doom::g_game::{deathmatch, timelimit, totalsecret, G_ExitLevel, G_SecretExitLevel};
use crate::doom::p_ceilng::{EV_CeilingCrushStop, EV_DoCeiling};
use crate::doom::p_doors::{EV_DoDoor, P_SpawnDoorCloseIn30, P_SpawnDoorRaiseIn5Mins};
use crate::doom::p_floor::{EV_BuildStairs, EV_DoFloor, T_MoveFloor};
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_lights::{
    EV_LightTurnOn, EV_StartLightStrobing, EV_TurnTagLightsOff, P_SpawnFireFlicker,
    P_SpawnGlowingLight, P_SpawnLightFlash, P_SpawnStrobeFlash,
};
use crate::doom::p_plats::{EV_DoPlat, EV_StopPlat};
use crate::doom::p_switch::P_ChangeSwitchTexture;
use crate::doom::p_telept::EV_Teleport;
use crate::doom::w_wad::W_CheckNumForName;
use crate::doom::z_zone::Z_Malloc;

/// `line_t` as seen by `p_lights` - `#[repr(C)]` layout identical to `c_ffi::line_t`.
type LightsLine = crate::doom::p_lights::line_t;
/// `sector_t` as seen by `p_lights` - `#[repr(C)]` layout identical to `c_ffi::sector_t`.
type LightsSector = crate::doom::p_lights::sector_t;
/// `line_t` as seen by `p_telept` - `#[repr(C)]` layout identical to `c_ffi::line_t`.
type TeleptLine = crate::doom::p_telept::line_t;
/// `mobj_t` as seen by `p_telept` - `#[repr(C)]` layout identical to `c_ffi::mobj_t`.
type TeleptMobj = crate::doom::p_telept::mobj_t;

// ---------------------------------------------------------------------------
// DEH_String shim — identity when dehacked is disabled.
// ---------------------------------------------------------------------------

/// Returns `s` unchanged; a no-op shim for the Dehacked string-replacement
/// hook that exists in the full Chocolate Doom build.
///
/// In the C source, `DEH_String` may redirect a hard-coded string to a
/// Dehacked patch string.  Because this port does not support Dehacked, the
/// shim simply returns its argument so that all call sites compile without
/// conditional compilation guards.
///
/// # Safety
///
/// `s` must be a valid, non-null pointer to a NUL-terminated C string for the
/// duration of the call (the pointer is returned unchanged and must remain valid
/// for however long the caller uses it).
#[inline(always)]
unsafe fn DEH_String(s: *mut c_char) -> *mut c_char {
    s
}

// ---------------------------------------------------------------------------
// P_InitPicAnims
// ---------------------------------------------------------------------------

/// Initialises the animated texture and flat cycle table from `ANIMDEFS`.
///
/// Iterates `ANIMDEFS` until the sentinel entry (`istexture == -1`).  For
/// each entry it resolves the start and end lump numbers via
/// `R_TextureNumForName` / `R_FlatNumForName` (skipping entries whose start
/// lump does not exist in the WAD).  The resolved `anim_t` record is written
/// into the `anims` array and `lastanim` is advanced.
///
/// Calls `I_Error` (via `i_error!`) if an animation cycle contains fewer than
/// two frames.
///
/// Called once at map load time from C (p_spec.c `P_SpawnSpecials` via the
/// game initialisation path).
#[no_mangle]
pub unsafe extern "C" fn P_InitPicAnims() {
    lastanim = std::ptr::addr_of_mut!(anims[0]);
    for &(istexture, endname, startname, speed) in ANIMDEFS {
        if istexture == -1 {
            break;
        }
        let startname = DEH_String(startname);
        let endname = DEH_String(endname);

        if istexture != 0 {
            if R_CheckTextureNumForName(startname) == -1 {
                continue;
            }
            (*lastanim).picnum = R_TextureNumForName(endname);
            (*lastanim).basepic = R_TextureNumForName(startname);
        } else {
            if W_CheckNumForName(startname) == -1 {
                continue;
            }
            (*lastanim).picnum = R_FlatNumForName(endname);
            (*lastanim).basepic = R_FlatNumForName(startname);
        }

        (*lastanim).istexture = istexture;
        (*lastanim).numpics = (*lastanim).picnum - (*lastanim).basepic + 1;

        if (*lastanim).numpics < 2 {
            i_error!(
                "P_InitPicAnims: bad cycle from {} to {}",
                std::ffi::CStr::from_ptr(startname).to_string_lossy(),
                std::ffi::CStr::from_ptr(endname).to_string_lossy()
            );
        }

        (*lastanim).speed = speed;
        lastanim = lastanim.offset(1);
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Returns a pointer to the `side_t` on a given side of a line bounding a sector.
///
/// - `currentSector`: index into the global `sectors` array.
/// - `line`: index into `sector.lines[]` (the sector's own line list, not the
///   global `lines` array).
/// - `side`: 0 for front, 1 for back.
///
/// Returns a pointer into the global `sides` array.  The pointer is valid as
/// long as the map data is loaded.  Called from p_spec.c and the floor/ceiling
/// modules.
#[no_mangle]
pub unsafe extern "C" fn getSide(currentSector: c_int, line: c_int, side: c_int) -> *mut side_t {
    let line_ptr = *(*sectors.offset(currentSector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    let side_idx = (*line_ptr).sidenum[side as usize];
    sides.offset(side_idx as isize)
}

/// Returns a pointer to the `sector_t` on a given side of a line bounding a sector.
///
/// - `currentSector`: index into the global `sectors` array.
/// - `line`: index into `sector.lines[]`.
/// - `side`: 0 for front sector, 1 for back sector.
///
/// Returns a pointer to the sector.  Callers must guard against the back sector
/// being null (one-sided line) before dereferencing the result.  Called from
/// p_spec.c and various map-action modules.
#[no_mangle]
pub unsafe extern "C" fn getSector(
    currentSector: c_int,
    line: c_int,
    side: c_int,
) -> *mut sector_t {
    let line_ptr = *(*sectors.offset(currentSector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    let side_idx = (*line_ptr).sidenum[side as usize];
    (*sides.offset(side_idx as isize)).sector
}

/// Returns non-zero if the given line on a sector's boundary is two-sided.
///
/// - `sector`: index into the global `sectors` array.
/// - `line`: index into `sector.lines[]`.
///
/// Returns the `ML_TWOSIDED` flag value (non-zero) if the line has a back
/// sector, or zero for one-sided lines.  Called from p_spec.c and the
/// floor/ceiling/platform modules.
#[no_mangle]
pub unsafe extern "C" fn twoSided(sector: c_int, line: c_int) -> c_int {
    let line_ptr = *(*sectors.offset(sector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    ((*line_ptr).flags as c_int) & (LinedefFlag::TWOSIDED as c_int)
}

/// Returns the sector on the opposite side of `line` from `sec`, or null.
///
/// If `line` is one-sided (no `ML_TWOSIDED` flag), returns null.  Otherwise
/// returns whichever of the front/back sectors is not `sec`.  Called
/// extensively by the `P_Find*Surrounding` family of functions.
#[no_mangle]
pub unsafe extern "C" fn getNextSector(line: *mut line_t, sec: *mut sector_t) -> *mut sector_t {
    if ((*line).flags as c_int) & (LinedefFlag::TWOSIDED as c_int) == 0 {
        return ptr::null_mut();
    }
    if (*line).frontsector == sec as *mut c_void {
        return (*line).backsector as *mut sector_t;
    }
    (*line).frontsector as *mut sector_t
}

/// Returns the lowest floor height among all sectors neighbouring `sec`.
///
/// Walks every line bounding `sec`, finds the sector on the other side via
/// `getNextSector`, and returns the minimum floor height seen.  If no
/// two-sided lines are found the current sector's own floor height is returned.
///
/// Heights are in fixed-point 16.16 (`fixed_t` / `c_int`).  Called from
/// p_floor.c and p_spec.c (also called from C).
#[no_mangle]
pub unsafe extern "C" fn P_FindLowestFloorSurrounding(sec: *mut sector_t) -> c_int {
    let mut floor = (*sec).floorheight;
    for i in 0..(*sec).linecount {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = getNextSector(check, sec);
        if other.is_null() {
            continue;
        }
        if (*other).floorheight < floor {
            floor = (*other).floorheight;
        }
    }
    floor
}

/// Returns the highest floor height among all sectors neighbouring `sec`.
///
/// Walks every bounding line of `sec` and returns the maximum neighbouring
/// floor height.  If no two-sided neighbours are found returns -500 *
/// `FRACUNIT` (the C sentinel initial value), which is effectively negative
/// infinity for practical map heights.
///
/// Heights are fixed-point 16.16.  Called from p_floor.c and p_spec.c.
#[no_mangle]
pub unsafe extern "C" fn P_FindHighestFloorSurrounding(sec: *mut sector_t) -> c_int {
    let mut floor = -500 * FRACUNIT;
    for i in 0..(*sec).linecount {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = getNextSector(check, sec);
        if other.is_null() {
            continue;
        }
        if (*other).floorheight > floor {
            floor = (*other).floorheight;
        }
    }
    floor
}

/// Returns the next floor height above `currentheight` among neighbouring sectors.
///
/// Collects all neighbouring floor heights that exceed `currentheight` into a
/// fixed-size array (`MAX_ADJOINING_SECTORS + 2` entries) and returns the
/// minimum of those heights, which is the lowest floor that is still above the
/// current one.
///
/// Emulates Vanilla Doom's buffer-overrun behaviour for sectors with more than
/// 20 adjoining sectors:
/// - At exactly 21 neighbours (`h == MAX_ADJOINING_SECTORS + 1`) the loop
///   overwrites the `height` variable on the (virtual) stack, updating the
///   working height.
/// - At 22 neighbours (`h == MAX_ADJOINING_SECTORS + 2`) the game would crash
///   in Vanilla; here `i_error!` is called instead.
///
/// Returns `currentheight` if no higher neighbour exists.  Heights are
/// fixed-point 16.16.  Called from p_floor.c.
#[no_mangle]
pub unsafe extern "C" fn P_FindNextHighestFloor(sec: *mut sector_t, currentheight: c_int) -> c_int {
    let mut height = currentheight;
    let mut heightlist: [c_int; MAX_ADJOINING_SECTORS + 2] = [0; MAX_ADJOINING_SECTORS + 2];
    let mut h = 0;

    for i in 0..(*sec).linecount {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = getNextSector(check, sec);
        if other.is_null() {
            continue;
        }
        if (*other).floorheight > height {
            if h == MAX_ADJOINING_SECTORS + 1 {
                height = (*other).floorheight;
            } else if h == MAX_ADJOINING_SECTORS + 2 {
                i_error!("Sector with more than 22 adjoining sectors. Vanilla will crash here");
            }
            heightlist[h] = (*other).floorheight;
            h += 1;
        }
    }

    if h == 0 {
        return currentheight;
    }

    let mut min = heightlist[0];
    for i in 1..h {
        if heightlist[i] < min {
            min = heightlist[i];
        }
    }
    min
}

/// Returns the lowest ceiling height among all sectors neighbouring `sec`.
///
/// Starts from `c_int::MAX` (matching C `INT_MAX`) and walks every bounding
/// line, returning the minimum neighbouring ceiling height.  If no two-sided
/// neighbours exist returns `INT_MAX`.
///
/// Heights are fixed-point 16.16.  Called from p_ceilng.c and p_spec.c.
#[no_mangle]
pub unsafe extern "C" fn P_FindLowestCeilingSurrounding(sec: *mut sector_t) -> c_int {
    let mut height = c_int::MAX;
    for i in 0..(*sec).linecount {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = getNextSector(check, sec);
        if other.is_null() {
            continue;
        }
        if (*other).ceilingheight < height {
            height = (*other).ceilingheight;
        }
    }
    height
}

/// Returns the highest ceiling height among all sectors neighbouring `sec`.
///
/// Starts from 0 and walks every bounding line, returning the maximum
/// neighbouring ceiling height.  If no two-sided neighbours exist returns 0.
///
/// Heights are fixed-point 16.16.  Called from p_ceilng.c and p_spec.c.
#[no_mangle]
pub unsafe extern "C" fn P_FindHighestCeilingSurrounding(sec: *mut sector_t) -> c_int {
    let mut height = 0;
    for i in 0..(*sec).linecount {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = getNextSector(check, sec);
        if other.is_null() {
            continue;
        }
        if (*other).ceilingheight > height {
            height = (*other).ceilingheight;
        }
    }
    height
}

/// Finds the next sector whose tag matches `line->tag`, searching from `start + 1`.
///
/// Iterates the global `sectors` array from index `start + 1` onwards and
/// returns the index of the first sector whose `tag` equals `line->tag`.
/// Returns -1 if no matching sector is found.
///
/// Used as the iterator in all tag-based action loops (donut, door, floor,
/// ceiling, etc.).  Called from many action functions including `EV_DoDonut`.
#[no_mangle]
pub unsafe extern "C" fn P_FindSectorFromLineTag(line: *mut line_t, start: c_int) -> c_int {
    for i in (start + 1)..numsectors {
        if (*sectors.offset(i as isize)).tag == (*line).tag {
            return i;
        }
    }
    -1
}

/// Returns the minimum light level among all sectors neighbouring `sector`,
/// clamped to `max` from above.
///
/// Walks every line bounding `sector`, checks the light level of each
/// two-sided neighbour, and returns the smallest value found.  If all
/// neighbours have light >= `max`, returns `max`.
///
/// Light levels are raw `i16` values cast to `c_int`; the valid Doom range
/// is 0-255.  Called from p_lights.c to calculate strobe and flash targets.
#[no_mangle]
pub unsafe extern "C" fn P_FindMinSurroundingLight(sector: *mut sector_t, max: c_int) -> c_int {
    let mut min = max;
    for i in 0..(*sector).linecount {
        let line = *(*sector).lines.offset(i as isize) as *mut line_t;
        let check = getNextSector(line, sector);
        if check.is_null() {
            continue;
        }
        if ((*check).lightlevel as c_int) < min {
            min = (*check).lightlevel as c_int;
        }
    }
    min
}

// ---------------------------------------------------------------------------
// P_CrossSpecialLine
// ---------------------------------------------------------------------------

/// Processes the special action for a linedef that a map object has just crossed.
///
/// Called every tic when a thing's origin crosses a line whose `special` field
/// is non-zero.  Non-player things are filtered: projectiles (rockets, plasma,
/// BFG balls, trooper/head/bruiser shots) are always ignored; other monsters
/// may only activate specials 4, 10, 39, 88, 97, 125, and 126.
///
/// The function dispatches on `line->special`:
/// - Specials 2-71 and 130-141 are one-shot: the handler clears `line->special`
///   to 0 after firing so the line cannot trigger again.
/// - Specials 72-129 are re-triggerable: `line->special` is left intact.
/// - Special 52 exits the level; special 124 exits to the secret level (both
///   are in the re-triggerable range and do not clear `line->special`).
/// - Special 125 is a one-shot monster-only teleport (activates only when
///   `thing` has no player, then clears `line->special`).
/// - Special 126 is a re-triggerable monster-only teleport (same player guard,
///   but does not clear `line->special`).
///
/// - `linenum`: index into the global `lines` array.
/// - `side`: side of the line the thing is approaching from (0 = front).
/// - `thing`: the map object that crossed the line.
///
/// Called from `p_map.c` via C FFI.
#[no_mangle]
pub unsafe extern "C" fn P_CrossSpecialLine(linenum: c_int, side: c_int, thing: *mut mobj_t) {
    let line = lines.offset(linenum as isize);

    if (*thing).player.is_null() {
        match (*thing).type_ {
            MT_ROCKET | MT_PLASMA | MT_BFG | MT_TROOPSHOT | MT_HEADSHOT | MT_BRUISERSHOT => {
                return;
            }
            _ => {}
        }

        let mut ok = 0;
        match (*line).special as c_int {
            39 | 97 | 125 | 126 | 4 | 10 | 88 => ok = 1,
            _ => {}
        }
        if ok == 0 {
            return;
        }
    }

    match (*line).special as c_int {
        2 => {
            EV_DoDoor(line as *mut LightsLine, vld_open);
            (*line).special = 0;
        }
        3 => {
            EV_DoDoor(line as *mut LightsLine, vld_close);
            (*line).special = 0;
        }
        4 => {
            EV_DoDoor(line as *mut LightsLine, vld_normal);
            (*line).special = 0;
        }
        5 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor);
            (*line).special = 0;
        }
        6 => {
            EV_DoCeiling(line as *mut LightsLine, fastCrushAndRaise);
            (*line).special = 0;
        }
        8 => {
            EV_BuildStairs(line as *mut LightsLine, build8);
            (*line).special = 0;
        }
        10 => {
            EV_DoPlat(line as *mut LightsLine, downWaitUpStay, 0);
            (*line).special = 0;
        }
        12 => {
            EV_LightTurnOn(line as *mut LightsLine, 0);
            (*line).special = 0;
        }
        13 => {
            EV_LightTurnOn(line as *mut LightsLine, 255);
            (*line).special = 0;
        }
        16 => {
            EV_DoDoor(line as *mut LightsLine, vld_close30ThenOpen);
            (*line).special = 0;
        }
        17 => {
            EV_StartLightStrobing(line as *mut LightsLine);
            (*line).special = 0;
        }
        19 => {
            EV_DoFloor(line as *mut LightsLine, lowerFloor);
            (*line).special = 0;
        }
        22 => {
            EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0);
            (*line).special = 0;
        }
        25 => {
            EV_DoCeiling(line as *mut LightsLine, crushAndRaise);
            (*line).special = 0;
        }
        30 => {
            EV_DoFloor(line as *mut LightsLine, raiseToTexture);
            (*line).special = 0;
        }
        35 => {
            EV_LightTurnOn(line as *mut LightsLine, 35);
            (*line).special = 0;
        }
        36 => {
            EV_DoFloor(line as *mut LightsLine, turboLower);
            (*line).special = 0;
        }
        37 => {
            EV_DoFloor(line as *mut LightsLine, lowerAndChange);
            (*line).special = 0;
        }
        38 => {
            EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest);
            (*line).special = 0;
        }
        39 => {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
            (*line).special = 0;
        }
        40 => {
            EV_DoCeiling(line as *mut LightsLine, raiseToHighest);
            EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest);
            (*line).special = 0;
        }
        44 => {
            EV_DoCeiling(line as *mut LightsLine, lowerAndCrush);
            (*line).special = 0;
        }
        52 => {
            G_ExitLevel();
        }
        53 => {
            EV_DoPlat(line as *mut LightsLine, perpetualRaise, 0);
            (*line).special = 0;
        }
        54 => {
            EV_StopPlat(line as *mut LightsLine);
            (*line).special = 0;
        }
        56 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorCrush);
            (*line).special = 0;
        }
        57 => {
            EV_CeilingCrushStop(line as *mut LightsLine);
            (*line).special = 0;
        }
        58 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24);
            (*line).special = 0;
        }
        59 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24AndChange);
            (*line).special = 0;
        }
        104 => {
            EV_TurnTagLightsOff(line as *mut LightsLine);
            (*line).special = 0;
        }
        108 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeRaise);
            (*line).special = 0;
        }
        109 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeOpen);
            (*line).special = 0;
        }
        100 => {
            EV_BuildStairs(line as *mut LightsLine, turbo16);
            (*line).special = 0;
        }
        110 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeClose);
            (*line).special = 0;
        }
        119 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorToNearest);
            (*line).special = 0;
        }
        121 => {
            EV_DoPlat(line as *mut LightsLine, blazeDWUS, 0);
            (*line).special = 0;
        }
        124 => {
            G_SecretExitLevel();
        }
        125 if (*thing).player.is_null() => {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
            (*line).special = 0;
        }
        130 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorTurbo);
            (*line).special = 0;
        }
        141 => {
            EV_DoCeiling(line as *mut LightsLine, silentCrushAndRaise);
            (*line).special = 0;
        }
        // RETRIGGERS
        72 => {
            EV_DoCeiling(line as *mut LightsLine, lowerAndCrush);
        }
        73 => {
            EV_DoCeiling(line as *mut LightsLine, crushAndRaise);
        }
        74 => {
            EV_CeilingCrushStop(line as *mut LightsLine);
        }
        75 => {
            EV_DoDoor(line as *mut LightsLine, vld_close);
        }
        76 => {
            EV_DoDoor(line as *mut LightsLine, vld_close30ThenOpen);
        }
        77 => {
            EV_DoCeiling(line as *mut LightsLine, fastCrushAndRaise);
        }
        79 => {
            EV_LightTurnOn(line as *mut LightsLine, 35);
        }
        80 => {
            EV_LightTurnOn(line as *mut LightsLine, 0);
        }
        81 => {
            EV_LightTurnOn(line as *mut LightsLine, 255);
        }
        82 => {
            EV_DoFloor(line as *mut LightsLine, lowerFloorToLowest);
        }
        83 => {
            EV_DoFloor(line as *mut LightsLine, lowerFloor);
        }
        84 => {
            EV_DoFloor(line as *mut LightsLine, lowerAndChange);
        }
        86 => {
            EV_DoDoor(line as *mut LightsLine, vld_open);
        }
        87 => {
            EV_DoPlat(line as *mut LightsLine, perpetualRaise, 0);
        }
        88 => {
            EV_DoPlat(line as *mut LightsLine, downWaitUpStay, 0);
        }
        89 => {
            EV_StopPlat(line as *mut LightsLine);
        }
        90 => {
            EV_DoDoor(line as *mut LightsLine, vld_normal);
        }
        91 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor);
        }
        92 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24);
        }
        93 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor24AndChange);
        }
        94 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorCrush);
        }
        95 => {
            EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0);
        }
        96 => {
            EV_DoFloor(line as *mut LightsLine, raiseToTexture);
        }
        97 => {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
        }
        98 => {
            EV_DoFloor(line as *mut LightsLine, turboLower);
        }
        105 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeRaise);
        }
        106 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeOpen);
        }
        107 => {
            EV_DoDoor(line as *mut LightsLine, vld_blazeClose);
        }
        120 => {
            EV_DoPlat(line as *mut LightsLine, blazeDWUS, 0);
        }
        126 if (*thing).player.is_null() => {
            EV_Teleport(line as *mut TeleptLine, side, thing as *mut TeleptMobj);
        }
        128 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorToNearest);
        }
        129 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloorTurbo);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// P_ShootSpecialLine
// ---------------------------------------------------------------------------

/// Processes a linedef special triggered by a projectile or hitscan impact.
///
/// Handles the three impact (gun-activated) line specials:
/// - Special 24: raise floor (one-shot, texture changes to switch).
/// - Special 46: open door (re-triggerable; only special 46 can be activated
///   by non-player things).
/// - Special 47: raise platform to nearest floor and change texture (one-shot).
///
/// Non-player things can only activate special 46; all other specials require
/// a player-controlled attack.
///
/// - `thing`: the map object whose attack hit the line.
/// - `line`: pointer to the linedef that was hit.
///
/// Called from p_map.c (`PTR_ShootTraverse`) when a bullet or projectile
/// hits a special linedef.
#[no_mangle]
pub unsafe extern "C" fn P_ShootSpecialLine(thing: *mut mobj_t, line: *mut line_t) {
    if (*thing).player.is_null() {
        let mut ok = 0;
        if (*line).special as c_int == 46 {
            ok = 1
        }
        if ok == 0 {
            return;
        }
    }

    match (*line).special as c_int {
        24 => {
            EV_DoFloor(line as *mut LightsLine, raiseFloor);
            P_ChangeSwitchTexture(line as *mut LightsLine, 0);
        }
        46 => {
            EV_DoDoor(line as *mut LightsLine, vld_open);
            P_ChangeSwitchTexture(line as *mut LightsLine, 1);
        }
        47 => {
            EV_DoPlat(line as *mut LightsLine, raiseToNearestAndChange, 0);
            P_ChangeSwitchTexture(line as *mut LightsLine, 0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// P_PlayerInSpecialSector
// ---------------------------------------------------------------------------

/// Applies the environmental effect of a special sector to the player each tic.
///
/// Called every tic for each player whose map-object origin is at floor level
/// in a special sector (`mo->z == sector->floorheight`).  Players who are
/// airborne (falling, jumping via cheats) are not affected.
///
/// Handled sector specials:
/// - 5: Hellslime - 10 hp damage every 32 tics unless Radiation Suit active.
/// - 7: Nukage - 5 hp damage every 32 tics unless Radiation Suit active.
/// - 4 / 16: Strobe Hurt / Super Hellslime - 20 hp damage every 32 tics;
///   Radiation Suit grants only partial protection (bypassed if `P_Random() < 5`).
/// - 9: Secret - increments `player->secretcount` and clears `sector->special`
///   so it fires only once.
/// - 11: End-level damage (E1M8 finale style) - strips God Mode, deals 20 hp
///   damage every 32 tics, and exits the level when health drops to 10 or below.
///
/// Calls `i_error!` for any unhandled special number, matching C `I_Error`.
///
/// - `player`: pointer to the player structure being updated.
///
/// Called from g_game.c / p_tick.c each game tic for every active player.
#[no_mangle]
pub unsafe extern "C" fn P_PlayerInSpecialSector(player: *mut PlayerT) {
    let mo = (*player).mo as *mut mobj_t;
    let sub = (*mo).subsector as *mut crate::doom::c_ffi::subsector_t;
    let sector = (*sub).sector as *mut sector_t;

    if (*mo).z != (*sector).floorheight {
        return;
    }

    match (*sector).special as c_int {
        5 => {
            if (*player).powers[pw_ironfeet] == 0 && leveltime & 0x1f == 0 {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    10,
                );
            }
        }
        7 => {
            if (*player).powers[pw_ironfeet] == 0 && leveltime & 0x1f == 0 {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    5,
                );
            }
        }
        16 | 4 => {
            if ((*player).powers[pw_ironfeet] == 0 || P_Random() < 5) && leveltime & 0x1f == 0 {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    20,
                );
            }
        }
        9 => {
            (*player).secretcount += 1;
            (*sector).special = 0;
        }
        11 => {
            (*player).cheats &= !CF_GODMODE;
            if leveltime & 0x1f == 0 {
                P_DamageMobj(
                    (*player).mo as *mut TeleptMobj,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    20,
                );
            }
            if (*player).health <= 10 {
                G_ExitLevel();
            }
        }
        _ => {
            i_error!(
                "P_PlayerInSpecialSector: unknown special {}",
                (*sector).special
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P_UpdateSpecials
// ---------------------------------------------------------------------------

/// Advances all time-based special effects by one tic.
///
/// Called once per game tic from the main thinker loop.  Performs three tasks:
///
/// 1. **Level timer**: if `levelTimer` is set (deathmatch time limit), decrements
///    `levelTimeCount` and calls `G_ExitLevel` when it reaches zero.
///
/// 2. **Texture/flat animation**: iterates the active `anims` table and updates
///    `texturetranslation` or `flattranslation` for each frame in every cycle.
///    The frame index is `(leveltime / speed + i) % numpics`, giving a smooth
///    round-robin across all frames simultaneously (each frame slot maps to a
///    different phase).
///
/// 3. **Scrolling walls** (special 48): increments `textureoffset` by one
///    `FRACUNIT` on the first side of each registered line, producing a
///    continuous left-to-right texture scroll.
///
/// 4. **Timed buttons**: decrements each active button timer; when it expires,
///    restores the original switch texture (top/mid/bottom depending on
///    `buttonlist[i].where_`) and plays the switch sound.
///
/// Called from p_tick.c `P_Ticker`.
#[no_mangle]
pub unsafe extern "C" fn P_UpdateSpecials() {
    if levelTimer != 0 {
        levelTimeCount -= 1;
        if levelTimeCount == 0 {
            G_ExitLevel();
        }
    }

    let mut anim = std::ptr::addr_of_mut!(anims[0]);
    while anim < lastanim {
        let base = (*anim).basepic;
        let numpics = (*anim).numpics;
        for i in base..base + numpics {
            let pic = base + ((leveltime / (*anim).speed + i) % numpics);
            if (*anim).istexture != 0 {
                *texturetranslation.offset(i as isize) = pic;
            } else {
                *flattranslation.offset(i as isize) = pic;
            }
        }
        anim = anim.offset(1);
    }

    for i in 0..numlinespecials as usize {
        let line = linespeciallist[i];
        if (*line).special as c_int == 48 {
            let sidenum = (*line).sidenum[0] as isize;
            (*sides.offset(sidenum)).textureoffset += FRACUNIT;
        }
    }

    for i in 0..MAXBUTTONS {
        if buttonlist[i].btimer != 0 {
            buttonlist[i].btimer -= 1;
            if buttonlist[i].btimer == 0 {
                let sidenum = (*buttonlist[i].line).sidenum[0] as isize;
                match buttonlist[i].where_ {
                    0 => {
                        (*sides.offset(sidenum)).toptexture = buttonlist[i].btexture as i16;
                    }
                    1 => {
                        (*sides.offset(sidenum)).midtexture = buttonlist[i].btexture as i16;
                    }
                    2 => {
                        (*sides.offset(sidenum)).bottomtexture = buttonlist[i].btexture as i16;
                    }
                    _ => {}
                }
                S_StartSound(
                    &mut buttonlist[i].soundorg as *mut _ as *mut c_void,
                    Sfx::Swtchn as c_int,
                );
                buttonlist[i] = std::mem::zeroed();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Donut overrun emulation
// ---------------------------------------------------------------------------

/// Fills `*s3_floorheight` and `*s3_floorpic` with the values that Vanilla Doom
/// would have read from address `0000:0000` when the donut s3 sector is null.
///
/// This replicates a real memory-access bug: in Vanilla Doom on DOS, reading
/// `s3->floorheight` with a null `s3` reads whatever happened to be at the
/// start of the DOS data segment.  The Chocolate Doom approach (which this port
/// follows) is to use configurable default values (`0` and `0x16`) that match
/// the Windows 98 memory layout, overridable with `-donut <height> <pic>`.
///
/// The function initialises its state only on the first call (guarded by the
/// `first` static flag), parsing `-donut` command-line arguments at that point.
/// Subsequent calls return the same cached values.
///
/// - `s3_floorheight`: output - the substitute floor height (fixed-point 16.16).
/// - `s3_floorpic`: output - the substitute floor picture lump index.
/// - `_line`: the triggering linedef (used only for diagnostic output in C;
///   unused in this Rust port).
/// - `_pillar_sector`: the inner donut sector (used only for diagnostics in C;
///   unused in this Rust port).
///
/// # Safety
///
/// `s3_floorheight` and `s3_floorpic` must be valid, non-null, writable
/// pointers. They are written unconditionally on every call.
unsafe fn DonutOverrun(
    s3_floorheight: *mut c_int,
    s3_floorpic: *mut i16,
    _line: *mut line_t,
    _pillar_sector: *mut sector_t,
) {
    static mut first: c_int = 1;
    static mut tmp_s3_floorheight: c_int = 0;
    static mut tmp_s3_floorpic: c_int = 0;

    if first != 0 {
        first = 0;
        tmp_s3_floorheight = 0;
        tmp_s3_floorpic = 0x16;

        let p = M_CheckParmWithArgs(c"-donut".as_ptr(), 2);
        if p > 0 {
            M_StrToInt(
                *myargv.offset((p + 1) as isize),
                &raw mut tmp_s3_floorheight,
            );
            M_StrToInt(*myargv.offset((p + 2) as isize), &raw mut tmp_s3_floorpic);
            if tmp_s3_floorpic >= numflats {
                eprintln!(
                    "DonutOverrun: The second parameter for \"-donut\" switch should be greater than 0 and less than number of flats ({}). Using default value ({}) instead. ",
                    numflats as c_int, 0x16
                );
                tmp_s3_floorpic = 0x16;
            }
        }
    }

    *s3_floorheight = tmp_s3_floorheight;
    *s3_floorpic = tmp_s3_floorpic as i16;
}

// ---------------------------------------------------------------------------
// EV_DoDonut
// ---------------------------------------------------------------------------

/// Executes the "donut" special effect for all sectors tagged to `line`.
///
/// The donut effect involves three concentric sectors:
/// - **s1** (inner, pillar): the sector directly tagged by the line.  Its floor
///   lowers to match s3's floor height (`lowerFloor` thinker).
/// - **s2** (ring): the sector adjacent to s1 via s1's first line.  Its floor
///   rises to s3's height and adopts s3's floor texture (`donutRaise` thinker).
/// - **s3** (outer): the sector adjacent to s2 that is not s1; provides the
///   target height and texture.
///
/// Edge cases (matching Chocolate Doom behaviour):
/// - If s1 already has a `specialdata` thinker running, the sector is skipped.
/// - If s2 is null (s1's first line is one-sided), a warning is printed and the
///   loop breaks early without spawning thinkers.
/// - If s3 is null (s2's bounding line has no back sector), `DonutOverrun` is
///   called to supply substitute height and texture values, emulating the
///   vanilla memory overrun.
///
/// Both thinkers use `T_MoveFloor` and are allocated with `Z_Malloc(PU_LEVSPEC)`.
/// The transmute of `T_MoveFloor` is required because the thinker function
/// pointer is typed as `unsafe extern "C" fn(*mut c_void)` at the FFI boundary.
///
/// Returns 1 if at least one sector was acted upon, 0 otherwise.
///
/// Called from p_spec.c via C when a donut-tagged line is activated.
#[no_mangle]
pub unsafe extern "C" fn EV_DoDonut(line: *mut line_t) -> c_int {
    let mut secnum = -1;
    let mut rtn = 0;

    while {
        secnum = P_FindSectorFromLineTag(line, secnum);
        secnum
    } >= 0
    {
        let s1 = sectors.offset(secnum as isize);
        if !(*s1).specialdata.is_null() {
            continue;
        }

        rtn = 1;
        let s2 = getNextSector(*(*s1).lines.offset(0) as *mut line_t, s1);

        if s2.is_null() {
            eprintln!("EV_DoDonut: linedef had no second sidedef! Unexpected behavior may occur in Vanilla Doom. ");
            break;
        }

        for i in 0..(*s2).linecount {
            let s3 =
                (*((*(*s2).lines.offset(i as isize)) as *mut line_t)).backsector as *mut sector_t;

            if s3 == s1 {
                continue;
            }

            let (s3_floorheight, s3_floorpic) = if s3.is_null() {
                eprintln!("EV_DoDonut: WARNING: emulating buffer overrun due to NULL back sector. Unexpected behavior may occur in Vanilla Doom.");
                violations::record(VanillaViolation::DonutOverrun);
                let mut fh = 0;
                let mut fp = 0i16;
                DonutOverrun(&mut fh, &mut fp, line, s1);
                (fh, fp)
            } else {
                ((*s3).floorheight, (*s3).floorpic)
            };

            // Spawn rising slime
            let floor = Z_Malloc(
                std::mem::size_of::<floormove_t>() as c_int,
                PU_LEVSPEC,
                ptr::null_mut(),
            ) as *mut floormove_t;
            P_AddThinker(&mut (*floor).thinker);
            (*s2).specialdata = floor as *mut c_void;
            (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
                unsafe extern "C" fn(*mut floormove_t),
                unsafe extern "C" fn(*mut c_void),
            >(T_MoveFloor));
            (*floor).r#type = donutRaise;
            (*floor).crush = 0;
            (*floor).direction = 1;
            (*floor).sector = s2 as *mut _;
            (*floor).speed = FLOORSPEED / 2;
            (*floor).texture = s3_floorpic;
            (*floor).newspecial = 0;
            (*floor).floordestheight = s3_floorheight;

            // Spawn lowering donut-hole
            let floor = Z_Malloc(
                std::mem::size_of::<floormove_t>() as c_int,
                PU_LEVSPEC,
                ptr::null_mut(),
            ) as *mut floormove_t;
            P_AddThinker(&mut (*floor).thinker);
            (*s1).specialdata = floor as *mut c_void;
            (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
                unsafe extern "C" fn(*mut floormove_t),
                unsafe extern "C" fn(*mut c_void),
            >(T_MoveFloor));
            (*floor).r#type = lowerFloor;
            (*floor).crush = 0;
            (*floor).direction = -1;
            (*floor).sector = s1 as *mut _;
            (*floor).speed = FLOORSPEED / 2;
            (*floor).floordestheight = s3_floorheight;

            break;
        }
    }

    rtn
}

// ---------------------------------------------------------------------------
// P_SpawnSpecials
// ---------------------------------------------------------------------------

/// Scans all sectors and linedefs at map load time and spawns thinkers for specials.
///
/// Called once after the map is loaded (from p_spec.c / `G_DoLoadLevel`).
/// Performs the following initialisation:
///
/// 1. **Deathmatch timer**: if `timelimit > 0` and `deathmatch != 0`, sets
///    `levelTimer` and `levelTimeCount` (in tics = `timelimit * 60 * TICRATE`).
///
/// 2. **Sector specials**: iterates all sectors and, for non-zero `sector->special`,
///    spawns the appropriate thinker:
///    - 1: random light flash (`P_SpawnLightFlash`)
///    - 2: fast strobe (`P_SpawnStrobeFlash(FASTDARK, 0)`)
///    - 3: slow strobe (`P_SpawnStrobeFlash(SLOWDARK, 0)`)
///    - 4: fast strobe + death slime (special reset to 4 after spawning strobe)
///    - 8: glowing light (`P_SpawnGlowingLight`)
///    - 9: secret sector (increments `totalsecret`)
///    - 10: door closes in 30 s (`P_SpawnDoorCloseIn30`)
///    - 12: slow sync strobe (`P_SpawnStrobeFlash(SLOWDARK, 1)`)
///    - 13: fast sync strobe (`P_SpawnStrobeFlash(FASTDARK, 1)`)
///    - 14: door raises in 5 min (`P_SpawnDoorRaiseIn5Mins`)
///    - 17: fire flicker (`P_SpawnFireFlicker`)
///
/// 3. **Line specials**: scans all linedefs; lines with special 48 (first-column
///    scroll) are registered in `linespeciallist`.  Aborts with `i_error!` if
///    more than `MAXLINEANIMS` (64) scrolling walls are found.
///
/// 4. **Misc cleanup**: clears `activeceilings`, `activeplats`, and `buttonlist`.
#[no_mangle]
pub unsafe extern "C" fn P_SpawnSpecials() {
    if timelimit > 0 && deathmatch != 0 {
        levelTimer = 1;
        levelTimeCount = timelimit * 60 * TICRATE;
    } else {
        levelTimer = 0;
    }

    for i in 0..numsectors {
        let sector = sectors.offset(i as isize);
        if (*sector).special == 0 {
            continue;
        }
        match (*sector).special as c_int {
            1 => P_SpawnLightFlash(sector as *mut LightsSector),
            2 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 0),
            3 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::SLOWDARK, 0),
            4 => {
                P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 0);
                (*sector).special = 4;
            }
            8 => P_SpawnGlowingLight(sector as *mut LightsSector),
            9 => {
                totalsecret += 1;
            }
            10 => P_SpawnDoorCloseIn30(sector as *mut LightsSector),
            12 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::SLOWDARK, 1),
            13 => P_SpawnStrobeFlash(sector as *mut LightsSector, crate::doom::c_ffi::FASTDARK, 1),
            14 => P_SpawnDoorRaiseIn5Mins(sector as *mut LightsSector, i),
            17 => P_SpawnFireFlicker(sector as *mut LightsSector),
            _ => {}
        }
    }

    numlinespecials = 0;
    for i in 0..numlines {
        if (*lines.offset(i as isize)).special as c_int == 48 {
            if numlinespecials as c_int >= MAXLINEANIMS as c_int {
                i_error!("Too many scrolling wall linedefs! (Vanilla limit is 64)");
            }
            linespeciallist[numlinespecials as usize] = lines.offset(i as isize);
            numlinespecials += 1;
        }
    }

    for i in 0..crate::doom::c_ffi::MAXCEILINGS as usize {
        crate::doom::p_ceilng::activeceilings[i] = ptr::null_mut();
    }
    for i in 0..crate::doom::c_ffi::MAXPLATS as usize {
        crate::doom::p_plats::activeplats[i] = ptr::null_mut();
    }
    for i in 0..MAXBUTTONS {
        buttonlist[i] = std::mem::zeroed();
    }
}

// ---------------------------------------------------------------------------
// Link anchor
// ---------------------------------------------------------------------------

/// Forces all public symbols in this module to be included in the final binary.
///
/// The linker may discard `pub unsafe extern "C"` functions that are not
/// referenced from Rust code.  This anchor function takes the address of every
/// exported symbol to prevent dead-code elimination.  It is itself exported
/// with C linkage and called from the C-side link anchor in p_spec.c (or the
/// equivalent build glue).
#[no_mangle]
pub unsafe extern "C" fn P_Spec_Link_Anchor() {
    let _ = P_InitPicAnims as *const () as usize;
    let _ = P_SpawnSpecials as *const () as usize;
    let _ = P_UpdateSpecials as *const () as usize;
    let _ = P_CrossSpecialLine as *const () as usize;
    let _ = P_ShootSpecialLine as *const () as usize;
    let _ = P_PlayerInSpecialSector as *const () as usize;
    let _ = EV_DoDonut as *const () as usize;
    let _ = getSide as *const () as usize;
    let _ = getSector as *const () as usize;
    let _ = twoSided as *const () as usize;
    let _ = getNextSector as *const () as usize;
    let _ = P_FindLowestFloorSurrounding as *const () as usize;
    let _ = P_FindHighestFloorSurrounding as *const () as usize;
    let _ = P_FindNextHighestFloor as *const () as usize;
    let _ = P_FindLowestCeilingSurrounding as *const () as usize;
    let _ = P_FindHighestCeilingSurrounding as *const () as usize;
    let _ = P_FindSectorFromLineTag as *const () as usize;
    let _ = P_FindMinSurroundingLight as *const () as usize;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const ANIM_T_SIZEOF: usize = 20;
    const ANIMDEF_T_SIZEOF: usize = 28;

    #[test]
    fn anim_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<anim_t>(), ANIM_T_SIZEOF);
    }

    #[test]
    fn animdef_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<animdef_t>(), ANIMDEF_T_SIZEOF);
    }

    #[test]
    fn globals_are_zero_initialized() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(levelTimer, 0);
            assert_eq!(levelTimeCount, 0);
            assert_eq!(numlinespecials, 0);
            assert!(lastanim.is_null());
            for (i, &v) in linespeciallist.iter().enumerate() {
                assert!(v.is_null(), "linespeciallist[{i}] should be null");
            }
            for (i, a) in anims.iter().enumerate() {
                assert_eq!(a.istexture, 0, "anims[{i}].istexture should be 0");
                assert_eq!(a.picnum, 0, "anims[{i}].picnum should be 0");
            }
        }
    }

    #[test]
    fn constants_match_c() {
        assert_eq!(MAXANIMS, 32);
        assert_eq!(MAXLINEANIMS, 64);
        assert_eq!(MAX_ADJOINING_SECTORS, 20);
        assert_eq!(PU_LEVSPEC, 6);
        assert_eq!(Sfx::Swtchn as c_int, 23);
        assert_eq!(CF_GODMODE, 2);
        assert_eq!(pw_ironfeet, 3);
    }

    #[test]
    fn animation_defs_terminated() {
        let last = ANIMDEFS[ANIMDEFS.len() - 1];
        assert_eq!(last.0, -1);
    }

    // --- F10 wave B3b dtmc baseline vectors (pre-move; F10 spec §2.3) ---

    //* The B3b graduation extracts the eleven-function pure geometry
    //* family (`getSide`/`getSector`/`twoSided`/`getNextSector` + the
    //* seven `P_Find*`) whole into the module's dtmc surface, plus the
    //* `anim_frame_pic` phase formula that is inline inside
    //* `P_UpdateSpecials` today (`p_spec.rs:1150`). Per F10 §2.3 the
    //* baseline vectors land BEFORE the move: the geometry drives below
    //* call the current in-file functions directly; the phase formula
    //* is pinned through this test-local transcription (its doc
    //* comment names the body line it mirrors verbatim). The graduation
    //* commit retargets the SAME test bodies onto the moved/extracted
    //* functions -- same vectors, same results.

    /// Baseline transcription of the animation phase formula, verbatim
    /// from `P_UpdateSpecials` (`p_spec.rs:1150`):
    /// `base + ((leveltime / (*anim).speed + i) % numpics)`. The tic
    /// parameter is named `tic` because `leveltime` is a module static
    /// this test module imports.
    fn anim_frame_pic_transcription(
        basepic: c_int,
        numpics: c_int,
        speed: c_int,
        tic: c_int,
        i: c_int,
    ) -> c_int
    {
        basepic + ((tic / speed + i) % numpics)
    }

    /// Synthetic sector graph for the geometry drives: a center sector
    /// (index 0) ringed by one line per neighbor, plus full
    /// snapshot/restore of the `p_setup` map globals the family reads
    /// (`sectors`, `sides`, `numsectors`). Line `i` of the center is
    /// two-sided, `front` = center, `back` = neighbor `i`, with
    /// `sidenum = [2i, 2i + 1]`. `one_sided` adds that many flag-less
    /// lines after the ring lines (their `getNextSector` arm is the
    /// null return the report names). Caller sets heights/light/tags on
    /// the returned vectors before installing. (Field names avoid the
    /// module statics this test module imports.)
    struct GeometryFixture
    {
        sec_storage: Vec<sector_t>,
        side_storage: Vec<side_t>,
        line_storage: Vec<line_t>,
        center_line_ptrs: Vec<*mut c_void>,
    }

    impl GeometryFixture
    {
        fn build(ring_lines: usize, one_sided: usize) -> Self
        {
            let total_lines = ring_lines + one_sided;
            let mut sec_list = vec![unsafe { std::mem::zeroed::<sector_t>() }; ring_lines + 1];
            let mut side_list = vec![unsafe { std::mem::zeroed::<side_t>() }; total_lines * 2];
            let mut line_list = vec![unsafe { std::mem::zeroed::<line_t>() }; total_lines];
            let mut center_line_ptrs: Vec<*mut c_void> = Vec::with_capacity(total_lines);

            let center = &mut sec_list[0] as *mut sector_t;
            for i in 0..total_lines
            {
                let line = &mut line_list[i] as *mut line_t;
                if i < ring_lines
                {
                    let back = &mut sec_list[i + 1] as *mut sector_t;
                    unsafe
                    {
                        (*line).flags = LinedefFlag::TWOSIDED as c_short;
                        (*line).sidenum = [(i * 2) as c_short, (i * 2 + 1) as c_short];
                        (*line).frontsector = center as *mut c_void;
                        (*line).backsector = back as *mut c_void;
                        side_list[i * 2].sector = center;
                        side_list[i * 2 + 1].sector = back;
                    }
                }
                else
                {
                    // One-sided: no TWOSIDED flag, no back sector, the
                    // front sidedef still resolves through `sides`.
                    unsafe
                    {
                        (*line).sidenum = [(i * 2) as c_short, -1];
                        (*line).frontsector = center as *mut c_void;
                        side_list[i * 2].sector = center;
                    }
                }
                center_line_ptrs.push(line as *mut c_void);
            }

            unsafe
            {
                (*center).linecount = total_lines as c_int;
                (*center).lines = center_line_ptrs.as_mut_ptr();
            }

            GeometryFixture
            {
                sec_storage: sec_list,
                side_storage: side_list,
                line_storage: line_list,
                center_line_ptrs,
            }
        }

        /// Center sector pointer (index 0).
        fn center(&mut self) -> *mut sector_t
        {
            &mut self.sec_storage[0] as *mut sector_t
        }

        /// Neighbor sector pointer (ring index `i`, sector `i + 1`).
        fn neighbor(&mut self, i: usize) -> *mut sector_t
        {
            &mut self.sec_storage[i + 1] as *mut sector_t
        }

        /// Line pointer (center line `i`).
        fn line(&mut self, i: usize) -> *mut line_t
        {
            &mut self.line_storage[i] as *mut line_t
        }

        //* Installs the fixture into the p_setup globals the geometry
        //* family reads and returns the prior values for
        //* [`GeometryFixture::restore`]. The writes are plain
        //* static-mut assignments (no references); the reads use
        //* addr_of! so no shared-reference lint fires.
        unsafe fn install(&mut self) -> (*mut sector_t, *mut side_t, c_int)
        {
            let sectors_before = std::ptr::addr_of!(sectors).read();
            let sides_before = std::ptr::addr_of!(sides).read();
            let numsectors_before = std::ptr::addr_of!(numsectors).read();
            sectors = self.sec_storage.as_mut_ptr();
            sides = self.side_storage.as_mut_ptr();
            numsectors = self.sec_storage.len() as c_int;
            (sectors_before, sides_before, numsectors_before)
        }

        unsafe fn restore(before: (*mut sector_t, *mut side_t, c_int))
        {
            sectors = before.0;
            sides = before.1;
            numsectors = before.2;
        }
    }

    /// Baseline vectors (F10 §2.3, wave B3b pre-move) for the pure map
    /// queries: `twoSided` (`p_spec.rs:446`), `getSide` (`:407`),
    /// `getSector` (`:425`), `getNextSector` (`:459`, including the
    /// null arm for one-sided lines), and the sentinel-initialised
    /// surrounding sweeps `P_FindLowestFloorSurrounding` (`:478`),
    /// `P_FindHighestFloorSurrounding` (`:502`, the `-500 * FRACUNIT`
    /// no-neighbor sentinel), `P_FindLowestCeilingSurrounding`
    /// (`:578`, the `INT_MAX` sentinel), `P_FindHighestCeilingSurrounding`
    /// (`:600`, the 0 sentinel), and `P_FindMinSurroundingLight`
    /// (`:643`, the `max` clamp).
    #[test]
    fn baseline_geometry_query_vectors()
    {
        let _engine = crate::doom::violations::ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap();

        unsafe
        {
            let mut fx = GeometryFixture::build(1, 1);

            // Neighbor + center geometry.
            fx.sec_storage[0].floorheight = 16 * FRACUNIT;
            fx.sec_storage[0].ceilingheight = 160 * FRACUNIT;
            fx.sec_storage[1].floorheight = -8 * FRACUNIT;
            fx.sec_storage[1].ceilingheight = 128 * FRACUNIT;
            fx.sec_storage[1].lightlevel = 96;

            let before = fx.install();

            // twoSided: the two-sided ring line carries the flag bit,
            // the one-sided line (index 1) carries none.
            assert_eq!(twoSided(0, 0), LinedefFlag::TWOSIDED as c_int);
            assert_eq!(twoSided(0, 1), 0);

            // getSide resolves the line's sidenum through `sides`:
            // line 0 has sidenum [0, 1], so side 0/1 land on the first
            // two side_t entries.
            assert_eq!(getSide(0, 0, 0), fx.side_storage.as_mut_ptr());
            assert_eq!(getSide(0, 0, 1), fx.side_storage.as_mut_ptr().offset(1));

            // getSector resolves the side's sector: front = center,
            // back = the ring neighbor.
            assert_eq!(getSector(0, 0, 0), fx.center());
            assert_eq!(getSector(0, 0, 1), fx.neighbor(0));

            // getNextSector: opposite side of the line from `sec`;
            // one-sided (flag-less) lines return the null arm.
            assert_eq!(getNextSector(fx.line(0), fx.center()), fx.neighbor(0));
            assert_eq!(getNextSector(fx.line(0), fx.neighbor(0)), fx.center());
            assert!(getNextSector(fx.line(1), fx.center()).is_null());

            // Surrounding sweeps: the two-sided neighbor participates,
            // the one-sided line is skipped, and a center with only the
            // one-sided line sees its sentinel initial value.
            assert_eq!(P_FindLowestFloorSurrounding(fx.center()), -8 * FRACUNIT);
            assert_eq!(P_FindHighestFloorSurrounding(fx.center()), -8 * FRACUNIT);
            assert_eq!(P_FindLowestCeilingSurrounding(fx.center()), 128 * FRACUNIT);
            assert_eq!(P_FindHighestCeilingSurrounding(fx.center()), 128 * FRACUNIT);
            assert_eq!(P_FindMinSurroundingLight(fx.center(), 160), 96);
            // The clamp: no neighbor below `max` returns `max` itself.
            assert_eq!(P_FindMinSurroundingLight(fx.center(), 96), 96);

            GeometryFixture::restore(before);

            // Sentinel drives: an isolated center whose only line is
            // one-sided never sees a neighbor, so each sweep returns
            // its initial value (own floor, -500 * FRACUNIT, INT_MAX,
            // 0).
            let mut iso = GeometryFixture::build(0, 1);
            iso.sec_storage[0].floorheight = 42 * FRACUNIT;
            let iso_before = iso.install();
            assert_eq!(P_FindLowestFloorSurrounding(iso.center()), 42 * FRACUNIT);
            assert_eq!(P_FindHighestFloorSurrounding(iso.center()), -500 * FRACUNIT);
            assert_eq!(P_FindLowestCeilingSurrounding(iso.center()), c_int::MAX);
            assert_eq!(P_FindHighestCeilingSurrounding(iso.center()), 0);
            GeometryFixture::restore(iso_before);
        }
    }

    /// Baseline vectors for `P_FindNextHighestFloor`
    /// (`p_spec.rs:535`): the vanilla adjoining-sector overrun boundary
    /// (catalog-relevant; see docs/vanilla-workarounds.md). Drives with
    /// 20, 21 and 22 qualifying neighbors all return the true minimum.
    /// The 23-line drive is the one that makes the
    /// `h == MAX_ADJOINING_SECTORS + 1` stack-shadow arm OBSERVABLE
    /// (`p_spec.rs:547-548`, the vanilla write that lands on `height`
    /// instead of the array): the 22nd qualifying neighbor raises the
    /// filter threshold, so the 23rd (between `currentheight` and the
    /// 22nd's height) never qualifies -- with the arm, the sweep
    /// returns the min of 22; without it (an "array-bounds fix") the
    /// 23rd would qualify, reach `h == MAX_ADJOINING_SECTORS + 2` and
    /// hit the `i_error!` at `:549-551` (the vanilla crash; never
    /// driven here, it aborts). Also pins the no-higher-neighbor
    /// return of `currentheight` and the min-selection sweep.
    #[test]
    fn baseline_next_highest_floor_20_21_22_overrun_boundary()
    {
        let _engine = crate::doom::violations::ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap();

        unsafe
        {
            let current = 50 * FRACUNIT;

            // 20/21/22 qualifying neighbors: all stored (the 22nd
            // enters the shadow arm, harmless as the last sweep
            // entry), minimum lands at a different index per drive so
            // the min-selection sweep is load-bearing every time.
            for (count, expect_min_at) in [(20usize, 7usize), (21, 0), (22, 21)]
            {
                let mut fx = GeometryFixture::build(count, 0);
                for i in 0..count
                {
                    fx.sec_storage[i + 1].floorheight =
                        current + ((i as c_int * 7 + 13) % (count as c_int * 7) + 6) * FRACUNIT;
                }
                fx.sec_storage[expect_min_at + 1].floorheight = current + FRACUNIT;

                let before = fx.install();
                assert_eq!(
                    P_FindNextHighestFloor(fx.center(), current),
                    current + FRACUNIT,
                    "drive with {count} qualifying neighbors"
                );
                GeometryFixture::restore(before);
            }

            // 23 lines, 23rd shadow-guarded: neighbors 0..=20 sit at
            // +10..=30, the 22nd at +100 (enters the shadow arm and
            // raises the filter threshold), the 23rd at +50 (above
            // `currentheight`, below the raised threshold -> never
            // qualifies). Result: min of the 22-entry list. A port
            // that "fixed" the overrun into a plain bounds-checked
            // store would qualify the 23rd, reach h == 22 and abort.
            let mut fx = GeometryFixture::build(23, 0);
            for i in 0..21
            {
                fx.sec_storage[i + 1].floorheight = current + (10 + i as c_int) * FRACUNIT;
            }
            fx.sec_storage[22].floorheight = current + 100 * FRACUNIT;
            fx.sec_storage[23].floorheight = current + 50 * FRACUNIT;

            let before = fx.install();
            assert_eq!(P_FindNextHighestFloor(fx.center(), current), current + 10 * FRACUNIT);
            GeometryFixture::restore(before);

            // No neighbor above `currentheight`: the qualifying set is
            // empty and the function returns `currentheight` itself.
            let mut fx = GeometryFixture::build(2, 0);
            fx.sec_storage[1].floorheight = current;
            fx.sec_storage[2].floorheight = current - FRACUNIT;
            let before = fx.install();
            assert_eq!(P_FindNextHighestFloor(fx.center(), current), current);
            GeometryFixture::restore(before);
        }
    }

    /// Baseline vectors for `P_FindSectorFromLineTag`
    /// (`p_spec.rs:624`): the tag scan starts at `start + 1` (so
    /// `start = -1` sees sector 0), resumes after the previous hit
    /// (the donut/door action loops' iterator protocol), and returns
    /// -1 once the scan runs off the end.
    #[test]
    fn baseline_sector_from_line_tag_scan_vectors()
    {
        let _engine = crate::doom::violations::ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap();

        unsafe
        {
            let mut fx = GeometryFixture::build(2, 0);
            fx.sec_storage[0].tag = 7;
            fx.sec_storage[1].tag = 9;
            fx.sec_storage[2].tag = 7;
            let mut tag_line: Box<line_t> = Box::new(std::mem::zeroed());
            tag_line.tag = 7;
            let tag_ptr = &mut *tag_line as *mut line_t;

            let before = fx.install();
            assert_eq!(P_FindSectorFromLineTag(tag_ptr, -1), 0);
            assert_eq!(P_FindSectorFromLineTag(tag_ptr, 0), 2);
            assert_eq!(P_FindSectorFromLineTag(tag_ptr, 2), -1);
            (*tag_ptr).tag = 42;
            assert_eq!(P_FindSectorFromLineTag(tag_ptr, -1), -1);
            GeometryFixture::restore(before);
        }
    }

    /// Baseline vectors for the animation phase formula the B3b
    /// graduation extracts as `anim_frame_pic` (transcription above,
    /// mirrored verbatim from `P_UpdateSpecials`,
    /// `p_spec.rs:1150`): frame `i` shows pic
    /// `base + ((leveltime / speed + i) % numpics)`, so the whole
    /// cycle advances one phase per `speed` tics and wraps modulo
    /// `numpics`. Integer division/modulo order is load-bearing for
    /// every animated texture/flat in demos.
    #[test]
    fn baseline_anim_frame_pic_phase_wrap()
    {
        let (base, numpics, speed) = (10, 3, 8);

        // Tic 0: frames in identity order.
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 0, 0), 10);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 0, 1), 11);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 0, 2), 12);
        // Before the first advance (leveltime < speed) the phase is 0.
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 7, 0), 10);
        // One speed period: every frame shifts forward by one and
        // frame 2 wraps to the base pic.
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 8, 0), 11);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 8, 1), 12);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 8, 2), 10);
        // A full cycle of the phase (3 periods = numpics): back to the
        // identity order -- the modulo wrap.
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 24, 0), 10);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 24, 1), 11);
        assert_eq!(anim_frame_pic_transcription(base, numpics, speed, 24, 2), 12);
        // Single-frame cycles are pinned to the base pic.
        assert_eq!(anim_frame_pic_transcription(base, 1, 8, 8, 0), 10);
        assert_eq!(anim_frame_pic_transcription(base, 1, 8, 23, 0), 10);
    }
}
