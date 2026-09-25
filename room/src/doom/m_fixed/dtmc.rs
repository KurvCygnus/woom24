//! Demo-synchronization surface extracted from `m_fixed`: the 16.16
//! fixed-point multiply/divide primitives whose bit-exact results feed
//! movement, angle, and damage math every simulation tic.

use super::{fixed_t, FRACBITS};

/// 16.16 fixed-point multiplication, bit-exact with upstream `FixedMul`
/// (`vendor/doomgeneric/m_fixed.c`), whose result feeds
/// movement/angle/damage math every simulation tic.
///
/// ## Technical Details
///
/// The product is computed in a full 64-bit intermediate, shifted right
/// by `FRACBITS` (16) with an arithmetic (floor) shift, then truncated
/// back to `fixed_t` -- the exact C sequence. The 64-bit intermediate is
/// load-bearing: a 32-bit product would lose the low bits before the
/// shift (see the baseline vectors in the test module:
/// `0x10001 * 0x10001 >> 16` must be `0x10002`, not `2`). Overflow is
/// silently truncated, exactly like the C original -- no panic, no
/// saturation, and the floor shift makes negative products round toward
/// minus infinity.
///
/// ## On Calling
///
/// Arguments and result are raw `fixed_t` (16.16) values; callers pass
/// and interpret fixed-point, never floats. Do not debug-assert on
/// overflow: truncation on overflow is the pinned behavior demo
/// playback depends on. Pure computation -- no state, no threading
/// assumptions.
#[doc(alias = "FixedMul")]
pub extern "C" fn fixed_mul(a: fixed_t, b: fixed_t) -> fixed_t
{
    ((a as i64 * b as i64) >> FRACBITS) as fixed_t
}

/// 16.16 fixed-point division with the upstream saturation guard,
/// bit-exact with upstream `FixedDiv`
/// (`vendor/doomgeneric/m_fixed.c`), whose result feeds
/// movement/angle/damage math every simulation tic.
///
/// ## Technical Details
///
/// Upstream shape: when `(abs(a) >> 14) >= abs(b)` the result saturates
/// by the sign of `a ^ b` (`i32::MAX` positive, `i32::MIN` negative);
/// otherwise `((a as i64) << 16) / b` is computed in 64 bits and
/// truncated back to `fixed_t`. Two C-undefined-behavior corners are
/// reproduced without UB: `i32::MIN.wrapping_abs()` stays `i32::MIN`,
/// whose arithmetic `>> 14` is negative, so for `a == i32::MIN` the
/// guard does not fire and the 64-bit division runs (yielding `0` for
/// `b == 1` after truncation); and the 64-bit division itself can never
/// overflow `i64` because the shifted dividend is bounded by `2^47`.
/// Unlike vanilla's asm `FixedDiv2`, this Chocolate-style C version has
/// no `I_Error` path -- overflow saturates instead of aborting.
///
/// ## On Calling
///
/// Arguments and result are raw `fixed_t` values; never debug-assert on
/// the saturation result -- it is pinned, demo-observable behavior.
/// `b == 0` saturates by the sign of `a` for every `a` except
/// `i32::MIN`: there the guard is bypassed and the 64-bit division by
/// zero panics, so `(i32::MIN, 0)` is the one forbidden input pair (the
/// C original hit undefined behavior at the same corner). No other
/// input panics; the function is pure with no threading assumptions.
#[doc(alias = "FixedDiv")]
pub extern "C" fn fixed_div(a: fixed_t, b: fixed_t) -> fixed_t
{
    if (a.wrapping_abs() >> 14) >= b.wrapping_abs() {
        if (a ^ b) < 0 {
            i32::MIN
        }
        else {
            i32::MAX
        }
    }
    else {
        (((a as i64) << 16) / b as i64) as fixed_t
    }
}

#[cfg(test)]
mod tests {
    use super::{fixed_div, fixed_mul};
    use crate::doom::m_fixed::FRACUNIT;

    // Adjudication note (F10 dtmc criterion: "does this function's
    // observable behavior belong to the demo synchronization surface?"):
    //   * fixed_mul -> dtmc: pure computation consumed by movement
    //     (p_map/p_user/p_mobj), aiming (p_enemy/p_pspr), damage and
    //     pickup scaling (p_inter), sight and render math (p_sight, r_*)
    //     inside the 35 tics/s simulation -- wholly qualifying, extracted
    //     in full.
    //   * fixed_div -> dtmc: same call graph; the saturation guard is
    //     demo-observable -- wholly qualifying, extracted in full. No
    //     marshalling or static-mut glue exists to leave behind.
    //   * FixedDiv2 -> absent from this port (vendor/doomgeneric/
    //     m_fixed.c is the Chocolate-style C version with only FixedMul
    //     and FixedDiv; vanilla's asm FixedDiv2 was never ported).
    //     Nothing to extract; recorded in the module mapping table.
    //
    // Baseline vectors: written and run against the original FixedMul /
    // FixedDiv paths BEFORE the extraction moved the bodies here
    // (F10 pilot Sec.2.3), then retargeted to the new names -- same vectors,
    // same results. Formulas transcribed from the extracted bodies:
    //
    //   fixed_mul: ((a as i64 * b as i64) >> FRACBITS) as fixed_t
    //
    //   fixed_div: if (a.wrapping_abs() >> 14) >= b.wrapping_abs() {
    //                  if (a ^ b) < 0 { i32::MIN } else { i32::MAX }
    //              } else {
    //                  (((a as i64) << 16) / b as i64) as fixed_t
    //              }
    //
    // There is no div-by-zero abort path in these tests: b == 0 makes
    // the guard (|a| >> 14 >= |b| == 0) true for every a except
    // i32::MIN, where the division itself would panic -- that one pair
    // stays untested by design (see fixed_div's "On Calling").
    #[test]
    fn baseline_fixed_mul()
    {
        assert_eq!(fixed_mul(FRACUNIT, FRACUNIT), FRACUNIT);
        assert_eq!(fixed_mul(2 * FRACUNIT, 3 * FRACUNIT), 6 * FRACUNIT);
        assert_eq!(fixed_mul(-2 * FRACUNIT, 3 * FRACUNIT), -6 * FRACUNIT);
        // 64-bit intermediate: LSB-preserving where a 32-bit product
        // would lose the low bits. 0x10001 * 0x10001 = 0x100020001;
        // >> 16 -> 0x10002 (a truncated 32-bit product would give
        // 0x00020001 >> 16 == 2).
        assert_eq!(fixed_mul(0x0001_0001, 0x0001_0001), 0x0001_0002);
        // Arithmetic (floor) right shift on a negative product:
        // -(0x100020001) >> 16 == floor(-65538.0000153) == -65539.
        assert_eq!(fixed_mul(-0x0001_0001, 0x0001_0001), -0x0001_0003);
    }

    #[test]
    fn baseline_fixed_div()
    {
        // Exact division: the i64 quotient is exact when b divides
        // (a << 16), sign handled by the i64 division itself.
        assert_eq!(fixed_div(3 * FRACUNIT, FRACUNIT), 3 * FRACUNIT);
        assert_eq!(fixed_div(-4 * FRACUNIT, 2 * FRACUNIT), -2 * FRACUNIT);
        assert_eq!(fixed_div(FRACUNIT, 2 * FRACUNIT), FRACUNIT / 2);
        // Saturation guard: |a| >> 14 >= |b| saturates by the sign of
        // a ^ b (positive -> INT_MAX, negative -> INT_MIN).
        assert_eq!(fixed_div(i32::MAX, 1), i32::MAX);
        assert_eq!(fixed_div(i32::MAX, -1), i32::MIN);
        // i32::MIN.wrapping_abs() == i32::MIN, whose arithmetic >> 14 is
        // negative, so the guard does not fire and the i64 division runs:
        // (i32::MIN << 16) / 1 truncates back to 0 through `as fixed_t`.
        assert_eq!(fixed_div(i32::MIN, 1), 0);
        // b == 0: the guard is always true (|a| >> 14 >= 0 == |b|), so
        // the result saturates by sign; no abort exists on this path.
        assert_eq!(fixed_div(1, 0), i32::MAX);
        assert_eq!(fixed_div(-1, 0), i32::MIN);
    }

    /// `1.0 * 1.0 == 1.0` in 16.16.
    #[test]
    fn mul_identity()
    {
        assert_eq!(fixed_mul(1 << 16, 1 << 16), 1 << 16);
    }

    /// `0.5 * 2.0 == 1.0` in 16.16.
    #[test]
    fn mul_half_times_two()
    {
        assert_eq!(fixed_mul(1 << 15, 2 << 16), 1 << 16);
    }

    /// `-1.0 * 1.0 == -1.0` in 16.16 (sign preserved through `>>`).
    #[test]
    fn mul_negative()
    {
        assert_eq!(fixed_mul(-(1 << 16), 1 << 16), -(1 << 16));
    }

    /// `3.0 / 1.0 == 3.0` in 16.16.
    #[test]
    fn div_one()
    {
        assert_eq!(fixed_div(3 << 16, 1 << 16), 3 << 16);
    }

    /// Positive overflow saturates to `INT_MAX`.
    #[test]
    fn div_saturates_pos()
    {
        assert_eq!(fixed_div(i32::MAX, 1), i32::MAX);
    }

    /// Negative overflow (positive / negative) saturates to `INT_MIN`.
    #[test]
    fn div_saturates_neg()
    {
        assert_eq!(fixed_div(i32::MAX, -1), i32::MIN);
    }

    /// Regression guard: `fixed_div(INT_MIN, x)` must not panic on the
    /// `abs(INT_MIN)` step; Rust's `wrapping_abs` keeps the call defined.
    #[test]
    fn div_min_no_panic()
    {
        let _ = fixed_div(i32::MIN, 1 << 16);
    }

    /// Any operand of 0 yields 0.
    #[test]
    fn mul_zero()
    {
        assert_eq!(fixed_mul(0, 1 << 16), 0);
        assert_eq!(fixed_mul(1 << 16, 0), 0);
        assert_eq!(fixed_mul(0, 0), 0);
    }

    /// `(-2.0) * 3.0 == -6.0` in 16.16.
    #[test]
    fn mul_neg_pos()
    {
        // (-2.0) x 3.0 = -6.0 in 16.16 fixed-point
        assert_eq!(fixed_mul(-(2 << 16), 3 << 16), -(6 << 16));
    }

    /// `(-2.0) * (-3.0) == 6.0` in 16.16.
    #[test]
    fn mul_neg_neg()
    {
        // (-2.0) x (-3.0) = 6.0 in 16.16 fixed-point
        assert_eq!(fixed_mul(-(2 << 16), -(3 << 16)), 6 << 16);
    }

    /// `1.5 * 2.0 == 3.0` in 16.16.
    #[test]
    fn mul_fraction()
    {
        // 1.5 x 2.0 = 3.0; 1.5 = (3 << 15)
        assert_eq!(fixed_mul(3 << 15, 2 << 16), 3 << 16);
    }

    /// Multiplying extreme values must not invoke i64 UB or panic. The
    /// `as` cast at the end truncates silently, matching the C original.
    #[test]
    fn mul_large_does_not_panic()
    {
        // The intermediate i64 must not overflow to UB; Rust guarantees this
        let _ = fixed_mul(i32::MAX, i32::MAX);
        let _ = fixed_mul(i32::MIN, i32::MIN);
        let _ = fixed_mul(i32::MAX, i32::MIN);
    }

    /// `1.0 / 2.0 == 0.5` in 16.16.
    #[test]
    fn div_half()
    {
        // 1.0 / 2.0 = 0.5 in 16.16 fixed-point
        assert_eq!(fixed_div(1 << 16, 2 << 16), 1 << 15);
    }

    /// `(-4.0) / 2.0 == -2.0` in 16.16.
    #[test]
    fn div_neg_by_pos()
    {
        // (-4.0) / 2.0 = -2.0 in 16.16 fixed-point
        assert_eq!(fixed_div(-(4 << 16), 2 << 16), -(2 << 16));
    }

    /// FixedDiv(a, 0): abs(a)>>14 >= abs(0)=0 is always true.
    /// (a ^ 0) = a; sign of a decides INT_MAX vs INT_MIN.
    #[test]
    fn div_positive_by_zero_saturates_max()
    {
        assert_eq!(fixed_div(1, 0), i32::MAX);
        assert_eq!(fixed_div(i32::MAX, 0), i32::MAX);
    }

    /// For negative `a` with |a| small enough that wrapping_abs doesn't
    /// overflow, dividing by zero saturates to INT_MIN.
    /// Note: fixed_div(i32::MIN, 0) is *not* tested here -- i32::MIN.wrapping_abs()
    /// returns i32::MIN whose arithmetic right-shift is negative, so the
    /// saturation guard does not fire (same UB/trap as the C original for
    /// abs(INT_MIN) / 0).
    #[test]
    fn div_negative_by_zero_saturates_min()
    {
        assert_eq!(fixed_div(-1, 0), i32::MIN);
        assert_eq!(fixed_div(-65536, 0), i32::MIN); // -1.0 in 16.16
    }

    /// Saturation when |a|/2^14 >= |b| and a, b have opposite signs -> INT_MIN.
    #[test]
    fn div_saturates_neg_opposite_signs()
    {
        // abs(i32::MAX) >> 14 = 131071 >= abs(-1) = 1 -> saturate,
        // (MAX ^ -1) has bit 31 set -> negative -> INT_MIN
        assert_eq!(fixed_div(i32::MAX, -1), i32::MIN);
    }
}
