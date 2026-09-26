//! Sight-traversal state: the seven `#[no_mangle]` statics of the LOS
//! check -- bit-exact with the globals of `vendor/doomgeneric/p_sight.c`.

#![allow(non_upper_case_globals)]

use std::os::raw::c_int;

use super::types::divline_t;

/// Z-coordinate of the looker's eyes (fixed-point map units).
///
/// Set by `P_CheckSight` to `t1->z + t1->height - (t1->height >> 2)` before
/// BSP traversal. Read by `P_CrossSubsector` for slope comparisons.
#[no_mangle]
pub static mut sightzstart: c_int = 0;

/// Upper slope bound for the sight corridor (fixed-point).
///
/// Initialised to the slope from `sightzstart` to the top of the target.
/// Narrowed downward as occluding portals are encountered.
#[no_mangle]
pub static mut topslope: c_int = 0;

/// Lower slope bound for the sight corridor (fixed-point).
///
/// Initialised to the slope from `sightzstart` to the bottom of the target.
/// Narrowed upward as occluding portals are encountered.
#[no_mangle]
pub static mut bottomslope: c_int = 0;

/// The sight-trace ray from `t1` to `t2`, stored as a `divline_t`.
///
/// Set by `P_CheckSight` and used throughout the BSP traversal in
/// `P_CrossBSPNode` and `P_CrossSubsector`.
#[no_mangle]
pub static mut strace: divline_t = divline_t {
    x: 0,
    y: 0,
    dx: 0,
    dy: 0,
};

/// X coordinate of the sight target (`t2->x`).
///
/// Stored separately from `strace` so that `P_CrossBSPNode` can test which
/// side of each partition the target falls on without a full ray struct.
#[no_mangle]
pub static mut t2x: c_int = 0;

/// Y coordinate of the sight target (`t2->y`).
///
/// See `t2x`.
#[no_mangle]
pub static mut t2y: c_int = 0;

/// Diagnostic counters: `sightcounts[0]` is REJECT table hits (early-out),
/// `sightcounts[1]` is full BSP traversals attempted.
#[no_mangle]
pub static mut sightcounts: [c_int; 2] = [0, 0];
