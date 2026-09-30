//! Interpolation math: the integer lerp primitives, the tic-fraction
//! derivation from the engine clock, and the fraction accessors.
//!
//! No float ever reaches a sample: every function here is integer-exact
//! 16.16 fixed-point math (the Woof! reference formulas, see the module
//! root's provenance doc).

use crate::doom::m_fixed::{fixed_t, FRACUNIT};

/// Fractional part of the current tic, as set by
/// [`crate::doom::r_interp::board::begin_frame`].
pub fn fraction() -> fixed_t { unsafe { super::board::FRACTION } }

/// Test hook: set the fraction directly (production derives it from the
/// engine clock via [`crate::doom::r_interp::board::begin_frame`]).
pub fn set_fraction(frac: fixed_t) { unsafe { super::board::FRACTION = frac; } }

/// Pure fraction math, split out for unit tests: `rel_ms` is milliseconds
/// since the engine clock's BASETIME (the same domain `I_GetTime` uses), so
/// tic boundaries sit at multiples of `1000 / TICRATE` and the in-tic phase
/// is `(rel_ms * TICRATE) % 1000` (Woof! `i_timer.c:113-116`).
pub(super) fn fraction_from_rel_ms(rel_ms: u32) -> fixed_t
{
    let ms35 = (rel_ms as u64) * (crate::doom::i_timer::TICRATE as u64);
    ((ms35 % 1000) * (FRACUNIT as u64) / 1000) as fixed_t
}

/// Wipe-path fraction re-sample (F1 L2): `D_Display`'s wipe loop presents
/// many frames per simulated tic, and the spec requires the fraction be
/// refreshed once per wipe iteration exactly as Woof! re-samples per wipe
/// pass (`d_main.c:396-401`). Reads the engine's own clock (`I_GetTimeMS`,
/// the same `DG_GetTicksMs` heartbeat the frame path derives from) — no new
/// time source. M1's wipe iterations render no 3-D view, so today this closes
/// the contract and keeps the fraction fresh for any wipe-path consumer.
pub fn refresh_fraction() { unsafe { super::board::FRACTION = fraction_from_rel_ms(crate::doom::i_timer::I_GetTimeMS() as u32); } }

/// `LerpFixed` from `woof/src/r_main.h:133-157`: `old + FixedMul(new - old,
/// frac)`, entirely in 16.16 fixed point.
pub fn lerp_fixed(old: fixed_t, new: fixed_t, frac: fixed_t) -> fixed_t { old.wrapping_add(crate::doom::m_fixed::FixedMul(new.wrapping_sub(old), frac)) }

/// Short-arc angle lerp over BAM angles (`angle_t` wraps at 2^32). The
/// threshold is ANG180: the pair (o, n) always travels the arc shorter than
/// half a turn. (Woof!'s ANG270 threshold sends 90..180 degree turns the long
/// way round; the F1 contract text specifies the short arc.)
pub fn lerp_angle(old: u32, new: u32, frac: fixed_t) -> u32
{
    if new == old { return new; }
    let forward = new.wrapping_sub(old);
    if forward < crate::doom::tables::ANG180
    {
        // Short arc runs forward from old to new.
        old.wrapping_add((((forward as u64) * (frac as u64)) >> 16) as u32)
    }
    else
    {
        // Short arc runs backward from old to new (through the 0/2^32 wrap).
        let backward = 0u32.wrapping_sub(forward);
        old.wrapping_sub((((backward as u64) * (frac as u64)) >> 16) as u32)
    }
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_int;

    use crate::doom::m_fixed::{fixed_t, FRACUNIT};

    use super::{lerp_angle, lerp_fixed};

    /// Half tic — the midpoint fraction.
    const HALF: fixed_t = FRACUNIT / 2;

    #[test]
    fn lerp_fixed_midpoint()
    {
        let old = 10 * FRACUNIT as c_int;
        let new = 20 * FRACUNIT as c_int;
        assert_eq!(lerp_fixed(old, new, HALF), 15 * FRACUNIT as c_int);
    }

    #[test]
    fn lerp_fixed_ends()
    {
        let old = -(3 * FRACUNIT as c_int);
        let new = 7 * FRACUNIT as c_int;
        assert_eq!(lerp_fixed(old, new, 0), old);
        assert_eq!(lerp_fixed(old, new, FRACUNIT), new);
    }

    #[test]
    fn lerp_fixed_negative_delta()
    {
        let old = 5 * FRACUNIT as c_int;
        let new = 1 * FRACUNIT as c_int;
        assert_eq!(lerp_fixed(old, new, HALF), 3 * FRACUNIT as c_int);
    }

    #[test]
    fn lerp_angle_short_arc_forward()
    {
        // degrees -> BAM: deg * 2^32 / 360.
        let d = |deg: u32| (((deg as u64) << 32) / 360) as u32;
        let got = lerp_angle(d(10), d(40), HALF);
        let want = d(25);
        assert!(
            (got as i64 - want as i64).abs() <= 4096,
            "expected ~25deg, got {got}"
        );
    }

    #[test]
    fn lerp_angle_short_arc_wraps_through_zero_both_ways()
    {
        let d = |deg: u64| ((deg << 32) / 360) as u32;
        // 350 -> 10: short arc runs backward through 0; midpoint 0 degrees.
        let got = lerp_angle(d(350), d(10), HALF);
        assert!(got <= 4096 || got >= u32::MAX - 4096, "expected ~0deg, got {got}");
        // 10 -> 350: same short arc, other direction; midpoint 0 degrees.
        let got = lerp_angle(d(10), d(350), HALF);
        assert!(got <= 4096 || got >= u32::MAX - 4096, "expected ~0deg, got {got}");
    }

    #[test]
    fn lerp_angle_equal_is_identity()
    {
        let a = 12345u32;
        assert_eq!(lerp_angle(a, a, HALF), a);
    }

    #[test]
    fn fraction_math_matches_reference_formula()
    {
        // (rel_ms * 35 % 1000) * FRACUNIT / 1000, monotone in [0, FRACUNIT).
        assert_eq!(super::fraction_from_rel_ms(0), 0);
        assert_eq!(
            super::fraction_from_rel_ms(28),
            (28 * 35 % 1000) * FRACUNIT as u64 as i32 / 1000
        );
        assert_eq!(
            super::fraction_from_rel_ms(56),
            (56 * 35 % 1000) * FRACUNIT as u64 as i32 / 1000
        );
        let big = super::fraction_from_rel_ms(u32::MAX / 2);
        assert!((0..FRACUNIT).contains(&big), "fraction out of range: {big}");
    }
}
