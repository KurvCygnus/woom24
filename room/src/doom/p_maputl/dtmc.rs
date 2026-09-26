//! Demo-synchronization surface extracted from `p_maputl`: the six
//! pure geometry computations behind the module's extern entries --
//! approximate distance, the two side-of-line predicates, divline
//! construction, the intercept fraction, and the box-vs-line
//! predicate -- whose exact integer results feed movement, aiming,
//! shooting, and collision decisions every simulation tic.

use std::ffi::c_int;

use crate::doom::c_ffi::{divline_t, line_t};
use crate::doom::m_fixed::{fixed_t, FixedDiv, FixedMul, FRACBITS};

/// Fast approximate Euclidean distance between two points.
///
/// Computes `|dx| + |dy| - min(|dx|, |dy|) / 2`, which over-estimates the
/// true distance by at most ~6 %. Used wherever an exact distance is not
/// required (enemy AI, sound attenuation). Matches `P_AproxDistance` in
/// `p_maputl.c`.
///
/// ## Technical Details
///
/// The wrapping arithmetic is load-bearing: `wrapping_abs` /
/// `wrapping_add` / `wrapping_sub` reproduce the C overflow behavior on
/// extreme fixed-point inputs (no panic, no saturation), and the `>> 1`
/// truncation of the halved smaller magnitude is the pinned vanilla
/// approximation error.
///
/// ## On Calling
///
/// Arguments and result are raw `fixed_t` (16.16) values; never
/// debug-assert on overflow -- truncation on overflow is the pinned
/// behavior demo playback depends on. Pure computation: no state, no
/// threading assumptions.
#[doc(alias = "P_AproxDistance")]
pub fn aprox_distance(dx: fixed_t, dy: fixed_t) -> fixed_t
{
    let dx = dx.wrapping_abs();
    let dy = dy.wrapping_abs();
    if dx < dy
    {
        dx.wrapping_add(dy).wrapping_sub(dx >> 1)
    }
    else
    {
        dx.wrapping_add(dy).wrapping_sub(dy >> 1)
    }
}

/// Return which side of a `line_t` a point lies on.
///
/// Returns 0 if the point is on the front (right-hand) side of the line, or
/// 1 if it is on the back side. Uses the precomputed `line.dx` / `line.dy`
/// and cross-product sign. Matches `P_PointOnLineSide` in `p_maputl.c`.
///
/// ## Technical Details
///
/// The axis-aligned fast paths branch on `line.dx == 0` / `line.dy == 0`
/// BEFORE any multiplication, and the generic arm shifts one factor of
/// each product right by `FRACBITS` (16) to keep the `FixedMul`
/// products inside 32 bits for real map coordinates -- both are pinned
/// (a reassociated cross product flips side decisions on long traces).
/// This predicate consumes the movement scratchpad values that the
/// spechit-overrun emulation can trample (`tmbbox`, catalog G1), so its
/// exact result on trampled inputs is demo-observable.
///
/// ## On Calling
///
/// `line` borrows a live `line_t` whose `v1` must point at a valid
/// `vertex_t`. Raw fixed-point in/out; never debug-assert on overflow.
/// Pure computation, single-thread-safe.
#[doc(alias = "P_PointOnLineSide")]
pub fn point_on_line_side(x: fixed_t, y: fixed_t, line: &line_t) -> c_int
{
    // SAFETY: reads through the borrowed line's `v1` raw pointer; the
    // caller guarantees a valid `vertex_t` (see On Calling).
    unsafe
    {
        if line.dx == 0
        {
            if x <= (*line.v1).x
            {
                return if line.dy > 0 { 1 } else { 0 };
            }
            return if line.dy < 0 { 1 } else { 0 };
        }
        if line.dy == 0
        {
            if y <= (*line.v1).y
            {
                return if line.dx < 0 { 1 } else { 0 };
            }
            return if line.dx > 0 { 1 } else { 0 };
        }
        let dx = x - (*line.v1).x;
        let dy = y - (*line.v1).y;
        let left = FixedMul(line.dy >> FRACBITS, dx);
        let right = FixedMul(dy, line.dx >> FRACBITS);
        if right < left
        {
            0
        }
        else
        {
            1
        }
    }
}

/// Return which side of a `divline_t` a point lies on.
///
/// Returns 0 (front) or 1 (back). Uses a sign-bit fast path before falling
/// back to a fixed-point cross product. This variant operates on a `divline_t`
/// (ray) rather than a full `line_t`. Matches `P_PointOnDivlineSide` in
/// `p_maputl.c`.
///
/// ## Technical Details
///
/// The sign-bit fast path tests `(dy ^ dx ^ dx_p ^ dy_p) & 0x80000000`
/// and, when the product's sign cannot be trusted to the shifted
/// multiply, decides from `(dy ^ dx_p)` alone -- reproducing the C
/// sign-trick exactly, including the case where it disagrees with the
/// full cross product. The fallback shifts BOTH factors right by 8
/// (not `FRACBITS`) before `FixedMul` -- that pre-shift width is
/// load-bearing for long-trace intercepts.
///
/// ## On Calling
///
/// `line` borrows a live `divline_t`; raw fixed-point in/out; never
/// debug-assert on overflow. Pure computation, single-thread-safe.
#[doc(alias = "P_PointOnDivlineSide")]
pub fn point_on_divline_side(x: fixed_t, y: fixed_t, line: &divline_t) -> c_int
{
    if line.dx == 0
    {
        if x <= line.x
        {
            return if line.dy > 0 { 1 } else { 0 };
        }
        return if line.dy < 0 { 1 } else { 0 };
    }
    if line.dy == 0
    {
        if y <= line.y
        {
            return if line.dx < 0 { 1 } else { 0 };
        }
        return if line.dx > 0 { 1 } else { 0 };
    }
    let dx = x - line.x;
    let dy = y - line.y;
    if ((line.dy ^ line.dx ^ dx ^ dy) & (0x80000000u32 as i32)) != 0
    {
        if ((line.dy ^ dx) & (0x80000000u32 as i32)) != 0
        {
            return 1;
        }
        return 0;
    }
    let left = FixedMul(line.dy >> 8, dx >> 8);
    let right = FixedMul(dy >> 8, line.dx >> 8);
    if right < left
    {
        0
    }
    else
    {
        1
    }
}

/// Populate a `divline_t` from the geometry of a `line_t`.
///
/// Sets `dl.x` / `dl.y` to `line.v1`, and `dl.dx` / `dl.dy` to
/// `line.dx` / `line.dy`. Used to convert a map linedef into a ray for
/// intercept testing. Matches `P_MakeDivline` in `p_maputl.c`.
///
/// ## Technical Details
///
/// Pure field marshalling with no arithmetic: the behavior is the exact
/// field correspondence, which is why the core returns the struct by
/// value and the extern wrapper performs the single pointer store.
///
/// ## On Calling
///
/// `li` borrows a live `line_t` whose `v1` must point at a valid
/// `vertex_t`; the caller must not alias the destination with the
/// source (no in-tree caller does). Single-thread-safe.
#[doc(alias = "P_MakeDivline")]
pub fn make_divline(li: &line_t) -> divline_t
{
    // SAFETY: reads through the borrowed line's `v1` raw pointer; the
    // caller guarantees a valid `vertex_t` (see On Calling).
    unsafe
    {
        divline_t
        {
            x: (*li.v1).x,
            y: (*li.v1).y,
            dx: li.dx,
            dy: li.dy,
        }
    }
}

/// Compute the fractional intercept `t` along `v2` where it crosses `v1`.
///
/// Returns the fixed-point parameter `t` such that `v2.origin + t * v2.dir`
/// is the intersection point. Returns 0 when the lines are parallel
/// (`den == 0`). Uses an 8-bit pre-shift to avoid overflow with large
/// fixed-point coordinates. `v2` is the ray being measured; `v1` is the
/// crossing line. Matches `P_InterceptVector` in `p_maputl.c`.
///
/// ## Technical Details
///
/// The `den == 0 -> 0` pin is load-bearing: a parallel hit reports
/// intercept fraction 0, which the traverser then treats as the nearest
/// hit -- the pinned vanilla value, not a degenerate-case choice. The
/// 8-bit pre-shift of one factor pair keeps the `FixedMul` products
/// inside 32 bits; removing it changes every fraction and desyncs
/// demos. Same formula family as `p_sight::dtmc::intercept_vector2`.
///
/// ## On Calling
///
/// Both divlines are borrowed reads; the function is pure,
/// allocation-free, and single-thread-safe. `FixedDiv` saturates by
/// sign outside the representable range -- that saturation is pinned
/// upstream behavior, never debug-assert on it.
#[doc(alias = "P_InterceptVector")]
pub fn intercept_vector(v2: &divline_t, v1: &divline_t) -> fixed_t
{
    let den = FixedMul(v1.dy >> 8, v2.dx) - FixedMul(v1.dx >> 8, v2.dy);
    if den == 0
    {
        return 0;
    }
    let num = FixedMul((v1.x - v2.x) >> 8, v1.dy) + FixedMul((v2.y - v1.y) >> 8, v1.dx);
    FixedDiv(num, den)
}

/// Determine which side(s) of a linedef an axis-aligned bounding box spans.
///
/// `tmbox` is a 4-element array `[top, bottom, left, right]` in fixed-point
/// map units. Returns 0 if the entire box is on the front side, 1 if entirely
/// on the back side, or -1 if the box straddles the line. Treats the line as
/// infinite. Uses `line.slopetype` to select the fastest test variant
/// (`ST_HORIZONTAL`, `ST_VERTICAL`, `ST_POSITIVE`, or `ST_NEGATIVE`).
///
/// Matches `P_BoxOnLineSide` in `p_maputl.c`.
///
/// ## Technical Details
///
/// The four-arm `slopetype` dispatch is the pinned behavior: the H and V
/// arms compare the box edges against `v1` and flip on the line
/// direction sign, while the diagonal arms reuse `point_on_line_side`
/// on opposite box corners. The `-1` result is what makes
/// `PIT_CheckLine` record a special (the box straddles the line), so
/// any change is demo-observable through collision behavior.
///
/// ## On Calling
///
/// `tmbox` must hold at least the 4 compared slots; `ld` borrows a live
/// `line_t` with a valid `v1`. Raw fixed-point in/out, no overflow
/// guards. Pure computation, single-thread-safe.
#[doc(alias = "P_BoxOnLineSide")]
pub fn box_on_line_side(tmbox: &[c_int], ld: &line_t) -> c_int
{
    // SAFETY: reads through the borrowed line's `v1` raw pointer; the
    // caller guarantees a valid `vertex_t` (see On Calling).
    unsafe
    {
        let mut p1: c_int;
        let mut p2: c_int;
        match ld.slopetype
        {
            0 =>
            {
                // ST_HORIZONTAL
                p1 = (tmbox[0] > (*ld.v1).y) as c_int;
                p2 = (tmbox[1] > (*ld.v1).y) as c_int;
                if ld.dx < 0
                {
                    p1 ^= 1;
                    p2 ^= 1;
                }
            }
            1 =>
            {
                // ST_VERTICAL
                p1 = (tmbox[3] < (*ld.v1).x) as c_int;
                p2 = (tmbox[2] < (*ld.v1).x) as c_int;
                if ld.dy < 0
                {
                    p1 ^= 1;
                    p2 ^= 1;
                }
            }
            2 =>
            {
                // ST_POSITIVE
                p1 = point_on_line_side(tmbox[2], tmbox[0], ld);
                p2 = point_on_line_side(tmbox[3], tmbox[1], ld);
            }
            _ =>
            {
                // ST_NEGATIVE
                p1 = point_on_line_side(tmbox[3], tmbox[0], ld);
                p2 = point_on_line_side(tmbox[2], tmbox[1], ld);
            }
        }
        if p1 == p2
        {
            p1
        }
        else
        {
            -1
        }
    }
}
