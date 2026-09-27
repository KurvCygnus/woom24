//! Demo-synchronization surface extracted from `p_spec`: the animation
//! phase formula that decides which frame of every animated texture/flat
//! is visible on each tic. The stateful half (the translation-table
//! writes, the walk over the active-`anims` array) stays at the call
//! site in `ticker::update_specials`.

use std::ffi::c_int;

/// Compute the pic lump index for frame `i` of an animation cycle on the
/// current tic -- the pure half of the per-frame assignment in
/// `P_UpdateSpecials` (`base + ((leveltime / (*anim).speed + i) % numpics)`,
/// upstream `p_spec.c` `P_UpdateSpecials` animation stage).
///
/// ## Technical Details
///
/// The formula is a round-robin with per-frame phase: the whole cycle
/// advances one step every `speed` tics and each frame slot maps to a
/// different phase of `(leveltime / speed + i) % numpics`, so the same
/// formula (not a per-frame counter) drives every animated texture/flat.
/// Integer division and modulo order is load-bearing: `leveltime / speed`
/// is the elapsed-period counter (truncating), and the `% numpics` wrap is
/// what makes the cycle loop. Any reassociation (e.g. `(leveltime % (speed
/// * numpics)) / speed`) can change results near wraps and desync demos.
/// There is no RNG and no state; the function is a pure map from
/// `(basepic, numpics, speed, leveltime, i)` to the pic index.
///
/// ## On Calling
///
/// Pass the cycle's `basepic`/`numpics`/`speed` exactly as read from the
/// `anim_t` record, the current `leveltime`, and the frame index `i` from
/// `base..base + numpics`. `numpics` of 0 would divide by zero (cannot
/// occur: `init_pic_anims` rejects cycles with fewer than two frames).
/// Never debug-assert on the arithmetic; the wrap is the behavior.
pub fn anim_frame_pic(basepic: c_int, numpics: c_int, speed: c_int, leveltime: c_int, i: c_int) -> c_int
{
    basepic + ((leveltime / speed + i) % numpics)
}

#[cfg(test)]
mod tests
{
    use super::anim_frame_pic;

    /// Baseline contract (F10 wave B3b): the phase vectors were written
    /// and run against the pre-move in-file expression (test-local
    /// transcription, commit `3841018`) BEFORE the extraction, then
    /// retargeted here -- same vectors, same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): wholly qualifying -- the extracted pic index is what
    /// every animated texture/flat displays on the tic, and the modulo
    /// wrap is part of that.
    #[test]
    fn baseline_anim_frame_pic_phase_wrap()
    {
        let (base, numpics, speed) = (10, 3, 8);

        // Tic 0: frames in identity order.
        assert_eq!(anim_frame_pic(base, numpics, speed, 0, 0), 10);
        assert_eq!(anim_frame_pic(base, numpics, speed, 0, 1), 11);
        assert_eq!(anim_frame_pic(base, numpics, speed, 0, 2), 12);
        // Before the first advance (leveltime < speed) the phase is 0.
        assert_eq!(anim_frame_pic(base, numpics, speed, 7, 0), 10);
        // One speed period: every frame shifts forward by one and
        // frame 2 wraps to the base pic.
        assert_eq!(anim_frame_pic(base, numpics, speed, 8, 0), 11);
        assert_eq!(anim_frame_pic(base, numpics, speed, 8, 1), 12);
        assert_eq!(anim_frame_pic(base, numpics, speed, 8, 2), 10);
        // A full cycle of the phase (3 periods = numpics): back to the
        // identity order -- the modulo wrap.
        assert_eq!(anim_frame_pic(base, numpics, speed, 24, 0), 10);
        assert_eq!(anim_frame_pic(base, numpics, speed, 24, 1), 11);
        assert_eq!(anim_frame_pic(base, numpics, speed, 24, 2), 12);
        // Single-frame cycles are pinned to the base pic.
        assert_eq!(anim_frame_pic(base, 1, 8, 8, 0), 10);
        assert_eq!(anim_frame_pic(base, 1, 8, 23, 0), 10);
    }
}
