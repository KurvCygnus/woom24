//! Demo-synchronization surface extracted from `p_sight`: the two pure
//! fixed-point sight computations -- the side-of-divline classifier
//! (with its load-bearing vanilla quirk) and the sight-local intercept
//! fraction -- whose exact integer results steer the BSP/seg crossing
//! decisions and the LOS slope narrowing of `P_CheckSight`.

use std::os::raw::c_int;

use crate::doom::m_fixed::{FixedDiv, FixedMul, FRACBITS};

use super::types::divline_t;

/// Determine which half-space of a `divline_t` a point lies in.
///
/// Returns 0 (front side), 1 (back side), or 2 (on the line). The exact
/// integer result decides which BSP subtrees and which segs the sight
/// ray is considered to cross, so it directly steers the LOS outcome of
/// `P_CheckSight`. Matches `P_DivlineSide` in `p_sight.c`.
///
/// ## Technical Details
///
/// The `dy == 0` fast path contains a deliberate quirk inherited from
/// the original Doom source: it tests `x == node.y` rather than
/// `y == node.y`. This is intentional for demo-compatible
/// determinism; do not correct it. The generic arm shifts both
/// cross-product factors right by `FRACBITS` (16) BEFORE multiplying --
/// a 32-bit truncated product of unshifted fixed-point values would
/// overflow and flip side decisions on real coordinates. On-line
/// returns 2 in every arm; the BSP caller coerces 2 -> 0 at its own
/// call sites, which is part of the pinned traversal behavior.
///
/// ## On Calling
///
/// Arguments and struct fields are raw `c_int` fixed-point map units;
/// no overflow guard exists and none may be added (the shifted
/// arithmetic is the pinned C-equivalent behavior). Pure computation:
/// no state, no threading assumptions, no allocation.
#[doc(alias = "P_DivlineSide")]
pub fn divline_side(x: c_int, y: c_int, node: &divline_t) -> c_int
{
    if node.dx == 0
    {
        if x == node.x
        {
            return 2;
        }
        return if x <= node.x
        {
            (node.dy > 0) as c_int
        }
        else
        {
            (node.dy < 0) as c_int
        };
    }

    if node.dy == 0
    {
        // NOTE: The original C code reads `if (x==node->y)` here -- NOT
        // `y==node->y`. This is a long-standing quirk in the Doom source
        // that demos rely on for deterministic playback. DO NOT "fix" it.
        if x == node.y
        {
            return 2;
        }
        return if y <= node.y
        {
            (node.dx < 0) as c_int
        }
        else
        {
            (node.dx > 0) as c_int
        };
    }

    let dx = x - node.x;
    let dy = y - node.y;

    let left = (node.dy >> FRACBITS) * (dx >> FRACBITS);
    let right = (dy >> FRACBITS) * (node.dx >> FRACBITS);

    if right < left
    {
        return 0; // front side
    }
    if left == right
    {
        return 2;
    }
    1 // back side
}

/// Returns the fractional intercept point along the first divline.
///
/// Computes the `t`-parameter where ray `v2` intersects line `v1` using
/// fixed-point arithmetic (8-bit pre-shift to avoid overflow). Returns 0
/// when the lines are parallel (`den == 0`). The result narrows the
/// `topslope` / `bottomslope` corridor in `P_CrossSubsector`, so it feeds
/// the LOS decision of `P_CheckSight`. Matches `P_InterceptVector2` in
/// `p_sight.c` (the sight-local variant; the publicly exported sibling
/// lives in `p_maputl` as `P_InterceptVector`).
///
/// ## Technical Details
///
/// The `den == 0 -> 0` pin is load-bearing: a parallel portal reports
/// intercept fraction 0, which the slope comparison then treats as the
/// portal start -- the pinned vanilla value, not a mathematical
/// degenerate-case choice. The 8-bit pre-shift of one factor pair keeps
/// the `FixedMul` products inside 32 bits for real map coordinates;
/// removing it changes every fraction and desyncs demos.
///
/// ## On Calling
///
/// Both divlines are borrowed reads; the function is pure, allocation-
/// free, and single-thread-safe. `FixedDiv` saturates by sign outside
/// the representable range -- that saturation is pinned upstream
/// behavior, never debug-assert on it.
#[doc(alias = "P_InterceptVector2")]
pub fn intercept_vector2(v2: &divline_t, v1: &divline_t) -> c_int
{
    let den = FixedMul(v1.dy >> 8, v2.dx) - FixedMul(v1.dx >> 8, v2.dy);

    if den == 0
    {
        return 0;
    }

    let num = FixedMul((v1.x - v2.x) >> 8, v1.dy) + FixedMul((v2.y - v1.y) >> 8, v1.dx);

    FixedDiv(num, den)
}

#[cfg(test)]
mod tests
{
    use super::*;

    /// Baseline contract (F10 wave B2a): these vectors were written and
    /// run GREEN against the pre-extraction `P_DivlineSide` body BEFORE
    /// the graduation move, then retargeted to `divline_side` -- same
    /// vectors, same results (F10 §2.3).
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): pure classifier whose exact integer result steers
    /// BSP/seg crossing; the `dy == 0` arm's `x == node.y` quirk is
    /// load-bearing demo behavior.
    #[test]
    fn baseline_divline_side_axis_arms()
    {
        // dx == 0 (vertical partition), dy > 0: the on-line test is
        // x == node.x and the half-space sign follows dy.
        let v = divline_t
        {
            x: 10,
            y: -20,
            dx: 0,
            dy: 1 << 16,
        };
        assert_eq!(divline_side(10, 999, &v), 2); // x == node.x -> on line
        assert_eq!(divline_side(4, 0, &v), 1); // x < node.x, dy > 0 -> back
        assert_eq!(divline_side(16, 0, &v), 0); // x > node.x, dy > 0 -> front
    }

    /// Quirk pin: on the `dy == 0` arm the on-line test reads
    /// `x == node->y` (NOT `y == node->y`). Demos rely on it; the
    /// first vector asserts the wrong-coordinate equality returning 2.
    #[test]
    fn baseline_divline_side_horizontal_quirk()
    {
        let h = divline_t
        {
            x: 0,
            y: 10,
            dx: 1 << 16,
            dy: 0,
        };
        assert_eq!(divline_side(10, 999_999, &h), 2); // quirk: x == node.y
        assert_eq!(divline_side(0, 10, &h), 0); // y == node.y but x != node.y: NOT on line; y <= node.y, dx > 0 -> front
        assert_eq!(divline_side(0, 4, &h), 0); // y < node.y -> front
        assert_eq!(divline_side(0, 16, &h), 1); // y > node.y -> back
    }

    /// Generic cross-product arm (both dx and dy non-zero) and the
    /// on-line return of 2.
    #[test]
    fn baseline_divline_side_generic_and_online()
    {
        let g = divline_t
        {
            x: 0,
            y: 0,
            dx: 1 << 16,
            dy: 1 << 16,
        };
        assert_eq!(divline_side(1 << 16, 0, &g), 0); // right < left -> front
        assert_eq!(divline_side(0, 1 << 16, &g), 1); // right > left -> back
        assert_eq!(divline_side(1 << 16, 1 << 16, &g), 2); // on line -> 2
    }

    /// Baseline contract (F10 wave B2a): written and run GREEN against
    /// the pre-extraction `P_InterceptVector2` body BEFORE the move,
    /// retargeted afterwards -- same vectors, same results (F10 §2.3).
    ///
    /// Adjudication: pure fixed-point intercept fraction narrowing the
    /// LOS slopes; `den == 0 -> 0` and the 8-bit pre-shift are
    /// exactness-sensitive.
    #[test]
    fn baseline_intercept_vector2()
    {
        // Parallel divlines: den == 0 -> 0.
        let a = divline_t
        {
            x: 0,
            y: 0,
            dx: 1 << 16,
            dy: 0,
        };
        let b = divline_t
        {
            x: 0,
            y: 0,
            dx: 1 << 16,
            dy: 0,
        };
        assert_eq!(intercept_vector2(&a, &b), 0);

        // v1 vertical at x = 1.0, v2 the ray from the origin along +x
        // at 2.0 per unit t: crossing fraction t = 0.5.
        let v1 = divline_t
        {
            x: 1 << 16,
            y: 0,
            dx: 0,
            dy: 1 << 16,
        };
        let v2 = divline_t
        {
            x: 0,
            y: 0,
            dx: 2 << 16,
            dy: 0,
        };
        assert_eq!(intercept_vector2(&v2, &v1), 1 << 15);

        // Same geometry with v1 at x = 4.0: t = 2.0.
        let v1_far = divline_t
        {
            x: 4 << 16,
            y: 0,
            dx: 0,
            dy: 1 << 16,
        };
        assert_eq!(intercept_vector2(&v2, &v1_far), 2 << 16);
    }
}
