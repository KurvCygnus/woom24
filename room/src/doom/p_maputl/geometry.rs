//! The `#[no_mangle]` extern geometry entries: raw-pointer marshalling
//! over the pure cores in [`super::dtmc`] -- bit-exact with the
//! geometry half of `vendor/doomgeneric/p_maputl.c`.

#![allow(non_snake_case)]

use std::ffi::c_int;

use crate::doom::c_ffi::{divline_t, line_t};
use crate::doom::m_fixed::fixed_t;

use super::dtmc::{
    aprox_distance, box_on_line_side, intercept_vector, make_divline, point_on_divline_side,
    point_on_line_side,
};

/// Fast approximate Euclidean distance between two points.
///
/// Computes `|dx| + |dy| - min(|dx|, |dy|) / 2`, which over-estimates the
/// true distance by at most ~6 %. Used wherever an exact distance is not
/// required (enemy AI, sound attenuation).
///
/// Matches `P_AproxDistance` in `p_maputl.c`.
#[no_mangle]
pub extern "C" fn P_AproxDistance(dx: fixed_t, dy: fixed_t) -> fixed_t
{
    aprox_distance(dx, dy)
}

/// Return which side of a `line_t` a point lies on.
///
/// Returns 0 if the point is on the front (right-hand) side of the line, or
/// 1 if it is on the back side. Uses the precomputed `line.dx` / `line.dy`
/// and cross-product sign.
///
/// Matches `P_PointOnLineSide` in `p_maputl.c`.
///
/// # Safety
/// `line` must be a valid, non-null pointer to an initialised `line_t` whose
/// `v1` field points to a valid `vertex_t`.
#[no_mangle]
pub extern "C" fn P_PointOnLineSide(x: fixed_t, y: fixed_t, line: *mut line_t) -> c_int
{
    unsafe { point_on_line_side(x, y, &*line) }
}

/// Return which side of a `divline_t` a point lies on.
///
/// Returns 0 (front) or 1 (back). Uses a sign-bit fast path before falling
/// back to a fixed-point cross product. This variant operates on a `divline_t`
/// (ray) rather than a full `line_t`.
///
/// Matches `P_PointOnDivlineSide` in `p_maputl.c`.
///
/// # Safety
/// `line` must be a valid, non-null pointer to an initialised `divline_t`.
#[no_mangle]
pub extern "C" fn P_PointOnDivlineSide(x: fixed_t, y: fixed_t, line: *mut divline_t) -> c_int
{
    unsafe { point_on_divline_side(x, y, &*line) }
}

/// Populate a `divline_t` from the geometry of a `line_t`.
///
/// Sets `dl.x` / `dl.y` to `line.v1`, and `dl.dx` / `dl.dy` to
/// `line.dx` / `line.dy`. Used to convert a map linedef into a ray for
/// intercept testing.
///
/// Matches `P_MakeDivline` in `p_maputl.c`.
///
/// # Safety
/// `li` must be a valid, non-null pointer to an initialised `line_t` whose
/// `v1` field points to a valid `vertex_t`. `dl` must be a valid, non-null,
/// writable pointer to a `divline_t`.
#[no_mangle]
pub extern "C" fn P_MakeDivline(li: *mut line_t, dl: *mut divline_t)
{
    unsafe
    {
        let li = &*li;
        let dl = &mut *dl;
        *dl = make_divline(li);
    }
}

/// Compute the fractional intercept `t` along `v2` where it crosses `v1`.
///
/// Returns the fixed-point parameter `t` such that `v2.origin + t * v2.dir`
/// is the intersection point. Returns 0 when the lines are parallel
/// (`den == 0`). Uses an 8-bit pre-shift to avoid overflow with large
/// fixed-point coordinates.
///
/// `v2` is the ray being measured; `v1` is the crossing line.
///
/// Matches `P_InterceptVector` in `p_maputl.c`.
///
/// # Safety
/// Both `v1` and `v2` must be valid, non-null pointers to initialised
/// `divline_t` values.
#[no_mangle]
pub extern "C" fn P_InterceptVector(v2: *mut divline_t, v1: *mut divline_t) -> fixed_t
{
    unsafe { intercept_vector(&*v2, &*v1) }
}

/// Determine which side(s) of a linedef an axis-aligned bounding box spans.
///
/// `tmbox` is a 4-element array `[top, bottom, left, right]` in fixed-point
/// map units. Returns 0 if the entire box is on the front side, 1 if entirely
/// on the back side, or -1 if the box straddles the line. Treats the line as
/// infinite.
///
/// Uses `line.slopetype` to select the fastest test variant (`ST_HORIZONTAL`,
/// `ST_VERTICAL`, `ST_POSITIVE`, or `ST_NEGATIVE`).
///
/// Matches `P_BoxOnLineSide` in `p_maputl.c`.
///
/// # Safety
/// `tmbox` must point to at least 4 consecutive `c_int` values. `ld` must be
/// a valid, non-null pointer to an initialised `line_t` with valid `v1`.
#[no_mangle]
pub extern "C" fn P_BoxOnLineSide(tmbox: *mut c_int, ld: *mut line_t) -> c_int
{
    unsafe
    {
        let tmbox = std::slice::from_raw_parts_mut(tmbox, 4);
        let ld = &*ld;
        box_on_line_side(tmbox, ld)
    }
}
