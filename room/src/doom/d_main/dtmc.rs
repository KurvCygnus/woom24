//! The extracted demo-synchronization surface of `d_main`: the attract
//! loop's page-count core. The stateful sequencer stays in `sequencing`
//! (whole-body dtmc verdicts there); this pure core is pinned by
//! baseline vectors that were captured against the inline body BEFORE
//! the split (F10 §2.3) and re-pointed at this name after it -- same
//! vectors, same results.

use std::ffi::c_int;

use crate::doom::d_mode;

/// Count of attract-loop demo pages (`D_DoAdvanceDemo`'s `max_seq`): 7
/// for the `exe_ultimate`/`exe_final` executables, 6 for every other
/// gameversion.
///
/// ## Technical Details
///
/// Extracted from the inline `max_seq` block of `D_DoAdvanceDemo`
/// (pre-split `d_main.rs:1126-1132`, upstream `d_main.c`). The modulo
/// RHS sits on the per-tic attract path -- `D_PageTicker` fires
/// `D_AdvanceDemo` at exactly tic 0 of the page budget, so WHEN the
/// cycle wraps decides `gamestate`/`pagename`/music at every
/// demo-golden checkpoint (`tests/demo_playthrough.rs` boots into
/// `D_StartTitle`). Plain integer arithmetic; never panics for any
/// `c_int` input.
///
/// ## On Calling
///
/// Call with the live `gameversion` static's value; the caller applies
/// `(demosequence + 1) % result` itself. Baseline vectors over every
/// `GAME_VERSIONS` row live in this file's test module.
pub fn attract_sequence_count(gv: c_int) -> c_int {
    if gv == d_mode::exe_ultimate || gv == d_mode::exe_final { 7 } else { 6 }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_int;

    use crate::doom::d_main::{demosequence, D_DoAdvanceDemo};
    use crate::doom::d_mode;
    use crate::doom::doomstat::gameversion;

    use super::attract_sequence_count;

    /// Known vectors over every `GAME_VERSIONS` row plus the null
    /// sentinel: only the ultimate/final executables run the 7-page
    /// attract cycle. Captured against the inline body pre-move
    /// (commit `d1d1493`), re-pointed here post-move.
    #[test]
    fn attract_sequence_count_baseline_vectors() {
        let vectors: [(c_int, c_int); 10] = [
            (d_mode::exe_doom_1_666, 6),
            (d_mode::exe_doom_1_7, 6),
            (d_mode::exe_doom_1_8, 6),
            (d_mode::exe_doom_1_9, 6),
            (d_mode::exe_hacx, 6),
            (d_mode::exe_ultimate, 7),
            (d_mode::exe_final, 7),
            (d_mode::exe_final2, 6),
            (d_mode::exe_chex, 6),
            (0, 6), // the GAME_VERSIONS null-sentinel version
        ];
        for (version, expected) in vectors { assert_eq!(attract_sequence_count(version), expected); }
    }

    /// Drive the LIVE sequencer through the root shim
    /// `D_DoAdvanceDemo`: from `demosequence == 12` the wrap lands in a
    /// demo-play arm for both page counts (`13 % 6 == 1`,
    /// `13 % 7 == 6`) and never in the music arms (0/4), so the call is
    /// side-effect-safe in a unit test -- evidence the extracted core
    /// matches the body that actually runs.
    #[test]
    fn attract_sequence_count_live_do_advance_demo_matches_vectors() {
        unsafe {
            let saved_version = gameversion;
            for (version, max_seq) in
                [(d_mode::exe_ultimate, 7), (d_mode::exe_final, 7), (d_mode::exe_doom_1_9, 6), (d_mode::exe_chex, 6)]
            {
                gameversion = version;
                demosequence = 12;
                D_DoAdvanceDemo();
                assert_eq!(demosequence, 13 % max_seq);
                assert_eq!(attract_sequence_count(version), max_seq);
            }
            gameversion = saved_version;
            demosequence = 0;
        }
    }
}
