//! The movement collision scratchpad: process-global state written by
//! `move_.rs`'s `check_position` / `teleport_move` and read by the
//! blockmap callbacks (`pit_check_line`, `pit_check_thing`), carried
//! verbatim from the monolithic `p_map.rs` with upstream names retained
//! (data tier; the F10 wave B2b renaming scope is functions only). The
//! `TeleptMobj` alias is the shared vocabulary for the cross-module
//! `p_inter` / `p_mobj` calls that expect `p_telept`'s local `mobj_t`.

use std::ffi::c_int;
use std::ptr;

use crate::doom::c_ffi::{line_t, mobj_t};
use crate::doom::m_fixed::fixed_t;

use super::consts::MAXSPECIALCROSS;

/// Type alias used when calling functions in `p_telept` that expect its own
/// local `mobj_t` definition (all `#[repr(C)]` layouts are identical).
pub(super) type TeleptMobj = crate::doom::p_telept::mobj_t;

/// Axis-aligned bounding box of the thing currently being position-checked.
///
/// Set by `check_position` / `teleport_move` before iterating over
/// blockmap cells. Indices follow `BBox` (TOP, BOTTOM, LEFT, RIGHT).
#[no_mangle]
pub static mut tmbbox: [fixed_t; 4] = [0; 4];

/// Pointer to the map object currently being tested by `check_position`.
///
/// Used by the blockmap iterator callbacks (`pit_check_thing`,
/// `pit_check_line`) to access the moving object without passing it through
/// the C-style function-pointer interface.
#[no_mangle]
pub static mut tmthing: *mut mobj_t = ptr::null_mut();

/// Cached copy of `tmthing->flags` for the current position check.
///
/// Avoids repeated pointer dereferences inside tight blockmap loops.
#[no_mangle]
pub static mut tmflags: c_int = 0;

/// Destination x-coordinate being tested in the current position check.
#[no_mangle]
pub static mut tmx: fixed_t = 0;

/// Destination y-coordinate being tested in the current position check.
#[no_mangle]
pub static mut tmy: fixed_t = 0;

/// Set to non-zero by `try_move` when the gap between floor and ceiling
/// is large enough for the thing to fit, even if other constraints still
/// block the move.
///
/// Callers (e.g. floating monsters) read this to decide whether to keep
/// trying to ascend or descend rather than giving up entirely.
#[no_mangle]
pub static mut floatok: c_int = 0; // boolean

/// Highest floor height touched during the current position check.
///
/// Updated by `pit_check_line` as two-sided linedefs are crossed.
/// After `check_position` returns, this is the floor the thing would
/// stand on at the tested position.
#[no_mangle]
pub static mut tmfloorz: fixed_t = 0;

/// Lowest ceiling height encountered during the current position check.
///
/// Updated by `pit_check_line`. After `check_position` returns, this
/// is the ceiling height at the tested position.
#[no_mangle]
pub static mut tmceilingz: fixed_t = 0;

/// Lowest floor height seen across all contacted sectors during the check.
///
/// Monsters will not move to a position where `tmfloorz - tmdropoffz`
/// exceeds 24 map units unless they have `MF_DROPOFF` or `MF_FLOAT`.
#[no_mangle]
pub static mut tmdropoffz: fixed_t = 0;

/// The linedef that produced the current value of `tmceilingz`.
///
/// Missiles use this to avoid exploding against "sky hack" walls: if the
/// ceiling line's front sector has the sky flat, the missile silently
/// disappears instead of spawning a puff.
#[no_mangle]
pub static mut ceilingline: *mut line_t = ptr::null_mut();

/// Array of special linedefs crossed during the current move attempt.
///
/// Filled by `pit_check_line`; processed by `try_move` once the move
/// is confirmed valid. Kept separate so specials are not triggered for
/// moves that ultimately fail.
#[no_mangle]
pub static mut spechit: [*mut line_t; MAXSPECIALCROSS] = [ptr::null_mut(); MAXSPECIALCROSS];

/// Number of valid entries currently in `spechit`.
#[no_mangle]
pub static mut numspechit: c_int = 0;
