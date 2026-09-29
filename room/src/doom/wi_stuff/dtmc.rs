//! Extracted demo-synchronization surface: the pure animated-background
//! delay behind `anim::init_animated_back`/`anim::update_animated_back`
//! -- `draw % modulus` -- with the known-vector baseline.
//!
//! The draws advance `rndindex` (state-hash word 2) once per
//! ANIM_ALWAYS/ANIM_RANDOM anim at intermission init and once per
//! ANIM_RANDOM cycle retrigger, so the extraction deliberately takes
//! ONLY the pure delay: the `M_Random` call sites stay whole and in
//! order in `anim` at their exact call shapes (`bcnt + 1 + delay` at
//! init, `bcnt + data2 + delay` at retrigger). The baseline vectors
//! were written against the pre-move inline bodies (commit feb6318)
//! and re-pointed here unchanged.

use std::ffi::c_int;

/// The animated-background delay: consume one `M_Random` byte into the
/// intermission's frame-advance scheduling (`WI_initAnimatedBack`
/// upstream `:912`/`:914`, `WI_updateAnimatedBack` upstream `:959`).
///
/// ## Technical Details
///
/// `draw % modulus` where the modulus is `cfg.period` for ANIM_ALWAYS
/// and `cfg.data1` for ANIM_RANDOM. The modulo truncation at each
/// period boundary is the observable: it staggers the animation frame
/// advances across `nexttic` beats, visible in every episode-map
/// intermission's frame content. `M_Random` bytes are 0..=255 so the
/// remainder is non-negative; the plain `%` (not a wrapping variant)
/// is itself load-bearing for any future negative-input use.
///
/// ## On Calling
///
/// Raw `c_int` in/out; the caller passes the raw `M_Random` byte and
/// the config modulus. Pure computation -- no state, no threading
/// assumptions, no panics (modulus 0 would trap, but no shipped config
/// has a zero period/data1; never debug-assert here -- the vanilla
/// code would divide by zero silently at the hardware level).
#[doc(alias = "WI_initAnimatedBack")]
#[doc(alias = "WI_updateAnimatedBack")]
pub fn anim_delay(draw: c_int, modulus: c_int) -> c_int {
    draw % modulus
}

#[cfg(test)]
mod tests {
    use super::anim_delay;
    use std::ffi::c_int;

    /// Re-compositions of the three call shapes `anim` uses, so the
    /// vectors below pin not just the delay but the exact arithmetic
    /// around it. The expected values were hand-computed against the
    /// pre-move inline bodies (commit feb6318).
    fn init_always_nexttic(beat: c_int, draw: c_int, period: c_int) -> c_int {
        beat + 1 + anim_delay(draw, period)
    }

    fn init_random_nexttic(beat: c_int, draw: c_int, data2: c_int, data1: c_int) -> c_int {
        beat + 1 + data2 + anim_delay(draw, data1)
    }

    fn update_random_nexttic(beat: c_int, draw: c_int, data2: c_int, data1: c_int) -> c_int {
        beat + data2 + anim_delay(draw, data1)
    }

    /// Known vectors for the pure delay `draw % modulus` over both
    /// shipped period families (TICRATE/3 = 11 for the episode 0/1/2
    /// body anims, TICRATE/4 = 8 for episode 2's tail) and the
    /// ANIM_RANDOM `data1` family, at the byte extremes and exact
    /// period multiples.
    #[test]
    fn anim_delay_baseline_vectors() {
        let vectors: [(c_int, c_int, c_int); 9] = [
            (0, 11, 0),    // TICRATE/3: zero draw contributes nothing
            (10, 11, 10),  // largest in-range remainder below the period
            (11, 11, 0),   // exactly one period wraps to zero
            (255, 11, 2),  // full byte draw: 255 = 23*11 + 2
            (0, 8, 0),     // TICRATE/4 (episode 2's tail anim)
            (255, 8, 7),   // full byte draw against the quarter-rate period
            (1, 30, 1),    // ANIM_RANDOM data1 family
            (255, 30, 15), // 255 = 8*30 + 15
            (0, 1, 0),     // degenerate modulus 1: always zero
        ];
        for (draw, modulus, expected) in vectors {
            assert_eq!(
                anim_delay(draw, modulus),
                expected,
                "anim delay drifted at draw={draw}, modulus={modulus}"
            );
        }
    }

    /// Hand-computed nexttic compositions for both init sites: the
    /// ANIM_ALWAYS composition over both shipped periods, and the
    /// ANIM_RANDOM init composition with its `+ 1` lead-in.
    #[test]
    fn init_nexttic_compositions_baseline() {
        // ANIM_ALWAYS, episode 0/1 period TICRATE/3 = 11.
        assert_eq!(init_always_nexttic(0, 0, 11), 1);
        assert_eq!(init_always_nexttic(5, 10, 11), 16);
        assert_eq!(init_always_nexttic(100, 255, 11), 103);
        // ANIM_ALWAYS, episode 2 tail period TICRATE/4 = 8.
        assert_eq!(init_always_nexttic(0, 255, 8), 8);
        // ANIM_RANDOM init keeps the + 1 lead-in before data2.
        assert_eq!(init_random_nexttic(7, 255, 3, 30), 26);
        assert_eq!(init_random_nexttic(0, 0, 0, 1), 1);
    }

    /// The update-retrigger composition differs from the init
    /// composition ONLY by the missing `+ 1` lead-in -- pinned by
    /// paired literals and by an exact-difference assertion.
    #[test]
    fn update_random_nexttic_baseline() {
        assert_eq!(update_random_nexttic(7, 255, 3, 30), 25);
        assert_eq!(update_random_nexttic(0, 0, 0, 1), 0);
        assert_eq!(
            init_random_nexttic(9, 200, 5, 17) - update_random_nexttic(9, 200, 5, 17),
            1
        );
    }
}
