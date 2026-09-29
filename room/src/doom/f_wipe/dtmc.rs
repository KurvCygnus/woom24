//! Extracted demo-synchronization surface: the pure melt column-seed
//! math behind `melt::init_melt` -- the first-column start rule and
//! the per-column recurrence -- with the known-vector baseline.
//!
//! The seeding draws advance `rndindex` (state-hash word 2) once per
//! column per wipe start, so the extraction deliberately takes ONLY
//! the pure math: the `M_Random` call sites stay whole and in order in
//! `melt::init_melt`. The baseline vectors were written against the
//! pre-move inline bodies (commit f8c7e47) and re-pointed here
//! unchanged.

use std::os::raw::c_int;

/// Seed the first melt column: the negative start offset drawn from
/// the wipe's opening `M_Random` draw (`wipe_initMelt` upstream,
/// f_wipe.c `:209`).
///
/// ## Technical Details
///
/// `-(draw % 16)` keeps every column's start offset in `-15..=0`; the
/// modulo-16 draw spread is what staggers the columns' fall start.
/// The negative encoding means "not yet falling" to `melt::do_melt`.
///
/// ## On Calling
///
/// Raw `c_int` in/out; the caller passes the raw `M_Random` byte. Pure
/// computation -- no state, no threading assumptions, no panics.
#[doc(alias = "wipe_initMelt")]
pub fn melt_column_start(draw: c_int) -> c_int { -(draw % 16) }

/// Seed a following melt column from its left neighbor and the next
/// `M_Random` draw (`wipe_initMelt` upstream, f_wipe.c `:211-217`).
///
/// The recurrence is `y = previous + (draw % 3) - 1` -- a random walk
/// with steps in `{-1, 0, +1}` -- followed by two exactness-bearing
/// corrections: a walk step above zero clamps to `0` (a column may
/// never start below the top), and a walk landing exactly on `-16`
/// re-biases to `-15` (the vanilla quirk that keeps the offset inside
/// the `-(draw % 16)` start range).
///
/// ## Technical Details
///
/// The clamp and the re-bias are the observable: the `== -16 -> -15`
/// quirk biases the walk's floor so consecutive columns stay within
/// the seeded spread, which is visible in the melt's column stagger in
/// every state transition's frame. The correction ORDER (clamp above
/// zero first, re-bias second) is pinned by the baseline vectors.
///
/// ## On Calling
///
/// Raw `c_int` in/out; `draw` is the raw `M_Random` byte, `previous`
/// the left neighbor's seed. Pure computation -- no state, no
/// threading assumptions, no panics (any `i32` input is representable;
/// overflow is not reachable for in-range `previous` values and is not
/// debug-asserted regardless).
#[doc(alias = "wipe_initMelt")]
pub fn melt_column_seed(previous: c_int, draw: c_int) -> c_int {
    let y = previous + (draw % 3) - 1;
    if y > 0 {
        0
    } else if y == -16 {
        -15
    } else {
        y
    }
}

#[cfg(test)]
mod tests {
    use super::{melt_column_seed, melt_column_start};
    use std::os::raw::c_int;

    /// Re-composition of the full seeding loop in the exact call
    /// shape `melt::init_melt` uses: the start draw, then one
    /// recurrence draw per remaining column, left to right. The
    /// expected arrays were hand-computed against the pre-move inline
    /// bodies (commit f8c7e47).
    fn melt_seed_loop(draws: &[c_int]) -> Vec<c_int> {
        let mut y = Vec::with_capacity(draws.len());
        y.push(melt_column_start(draws[0]));
        for i in 1..draws.len() {
            y.push(melt_column_seed(y[i - 1], draws[i]));
        }
        y
    }

    /// Known vectors over every branch of the recurrence: the `> 0`
    /// clamp, the pass-through interior, the `== -16 -> -15` quirk on
    /// both entry columns, and the sub-`-16` floor (which the quirk
    /// deliberately does NOT catch). `draw % 3` selects the step
    /// `-1/0/+1`.
    #[test]
    fn melt_column_seed_baseline_vectors() {
        let vectors: [(c_int, c_int, c_int); 12] = [
            (0, 3, -1),    // step -1 from the top: plain pass-through
            (0, 4, 0),     // step 0 lands exactly on 0: not > 0, kept
            (0, 5, 0),     // step +1 would rise above 0: clamped to 0
            (1, 4, 0),     // 1 + 0 = 1: clamped to 0
            (1, 3, 0),     // 1 - 1 = 0: kept
            (-14, 4, -14), // interior pass-through
            (-15, 3, -15), // -15 - 1 = -16: the quirk re-biases to -15
            (-15, 4, -15), // -15 + 0: kept
            (-15, 5, -14), // -15 + 1: kept
            (-16, 3, -17), // -16 - 1 = -17: below the quirk's == -16 check
            (-16, 4, -15), // -16 + 0: the quirk fires on entry value -16
            (-16, 5, -15), // -16 + 1: kept
        ];
        for (previous, draw, expected) in vectors {
            assert_eq!(
                melt_column_seed(previous, draw),
                expected,
                "melt seed recurrence drifted at previous={previous}, draw={draw}"
            );
        }
    }

    /// Known vectors for the first-column start rule: the negative
    /// remainder (draw 0..=15) and the wrap points at multiples of 16.
    #[test]
    fn melt_column_start_baseline_vectors() {
        let vectors: [(c_int, c_int); 7] =
            [(0, 0), (1, -1), (15, -15), (16, 0), (17, -1), (32, 0), (255, -15)];
        for (draw, expected) in vectors {
            assert_eq!(melt_column_start(draw), expected);
        }
    }

    /// Two full seeding sequences with hand-computed expectations: the
    /// first walks the interior down column by column, the second
    /// parks on -15 and shows the quirk firing on every step-0 draw
    /// (the transient -16 is re-biased before it can escape) plus one
    /// +1 draw climbing to -14. This pins draw-order-sensitivity: the
    /// Nth element must consume the Nth draw.
    #[test]
    fn melt_seed_full_sequences_baseline() {
        // Interior walk: start -(7 % 16) = -7, then one step per draw.
        let walk = [7, 200, 3, 48, 255, 1, 130, 77];
        assert_eq!(melt_seed_loop(&walk), vec![-7, -6, -7, -8, -9, -9, -9, -8]);

        // Quirk soak: start -15, every step-0 draw re-biases the
        // transient -16 back to -15 so the walk never escapes below
        // -15, and the single +1 draw climbs to -14.
        let quirk = [15, 3, 3, 200, 3, 3, 3, 3];
        assert_eq!(melt_seed_loop(&quirk), vec![-15, -15, -15, -14, -15, -15, -15, -15]);
    }
}
