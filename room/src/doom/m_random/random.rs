//! The random-generator wrappers: C-linkage marshalling of the
//! `state.rs` cursors through the `dtmc::random_advance` sequence
//! primitive, plus the cursor/reset test module.

#![allow(non_snake_case)]

use std::ffi::c_int;

use super::dtmc;
use super::state::{prndindex, rndindex};

/// `int P_Random(void)` -- advance and read the play-simulation
/// random cursor. Returns the next byte from `RNDTABLE` masked to
/// `0..=255`. This is the deterministic generator used by game logic;
/// demos and net-play depend on its exact sequence.
#[no_mangle]
pub extern "C" fn P_Random() -> c_int
{
    let (idx, val) = dtmc::random_advance(unsafe { prndindex });
    unsafe { prndindex = idx; }
    val
}

/// `int M_Random(void)` -- advance and read the game-thinker random
/// cursor. Returns the next byte from `RNDTABLE` masked to `0..=255`.
/// Used for non-simulation effects (e.g. menu animation) so its use
/// does not perturb demo determinism.
#[no_mangle]
pub extern "C" fn M_Random() -> c_int
{
    let (idx, val) = dtmc::random_advance(unsafe { rndindex });
    unsafe { rndindex = idx; }
    val
}

/// `void M_ClearRandom(void)` -- reset both `rndindex` and
/// `prndindex` to 0 atomically. Called at the start of a new
/// game/demo so the random sequence reproduces.
#[no_mangle]
pub extern "C" fn M_ClearRandom()
{
    unsafe
    {
        rndindex = 0;
        prndindex = 0;
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::m_random::{M_ClearRandom, M_Random, P_Random, RNDTABLE, prndindex, rndindex};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;
    use std::ffi::c_int;

    /// Three consecutive `M_Random` calls should return entries 1, 2, 3
    /// of `RNDTABLE` in order (the pre-increment makes index 0 unused).
    #[test]
    fn m_random_walks_the_table()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_ClearRandom();
        assert_eq!(M_Random(), RNDTABLE[1] as c_int);
        assert_eq!(M_Random(), RNDTABLE[2] as c_int);
        assert_eq!(M_Random(), RNDTABLE[3] as c_int);
    }

    /// `M_Random` and `P_Random` each advance their own cursor; the
    /// first call to either after `M_ClearRandom` returns `RNDTABLE[1]`.
    #[test]
    fn p_and_m_independent()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_ClearRandom();
        let a = M_Random();
        let b = P_Random();
        assert_eq!(a, b);
        unsafe
        {
            assert_eq!(rndindex, 1);
            assert_eq!(prndindex, 1);
        }
    }

    /// `M_ClearRandom` must zero `rndindex` even if both cursors have
    /// been advanced by prior calls.
    #[test]
    fn clear_resets_both()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_Random();
        M_Random();
        P_Random();
        M_ClearRandom();
        unsafe { assert_eq!(rndindex, 0); }
    }

    /// Two `P_Random` calls should leave `prndindex` at 2 and return
    /// `RNDTABLE[1]` then `RNDTABLE[2]`.
    #[test]
    fn p_random_increments_prndindex()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_ClearRandom();
        assert_eq!(P_Random(), RNDTABLE[1] as c_int);
        assert_eq!(P_Random(), RNDTABLE[2] as c_int);
        unsafe { assert_eq!(prndindex, 2); }
    }

    /// `M_ClearRandom` must zero `prndindex` (in addition to `rndindex`,
    /// covered by `clear_resets_both`).
    #[test]
    fn clear_also_resets_prndindex()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        P_Random();
        P_Random();
        M_ClearRandom();
        unsafe { assert_eq!(prndindex, 0); }
    }

    /// After exactly 256 calls to M_Random the index wraps back to 0,
    /// so the 257th call returns RNDTABLE[1] -- identical to the first
    /// call.
    #[test]
    fn m_random_wraps_at_256()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_ClearRandom();
        let first = M_Random();
        for _ in 1..256 { M_Random(); }
        // 256 calls done; rndindex = (0 + 256) & 0xFF = 0
        unsafe { assert_eq!(rndindex, 0); }
        // 257th call should match the first (RNDTABLE[1])
        assert_eq!(M_Random(), first);
    }

    /// P_Random and M_Random share the same RNDTABLE but use
    /// independent cursors.
    #[test]
    fn p_and_m_cursors_are_independent()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        M_ClearRandom();
        // advance M five steps
        for _ in 0..5 { M_Random(); }
        // P cursor is still at 0; first P_Random returns RNDTABLE[1]
        assert_eq!(P_Random(), RNDTABLE[1] as c_int);
    }
}
