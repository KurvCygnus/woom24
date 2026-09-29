//! Extracted demo-synchronization surface: the pure face-row formula
//! behind `face::calc_pain_offset`, with the known-vector baseline and
//! the ST_Ticker RNG-ledger vector.
//!
//! The baseline vectors were written against the pre-move inline body
//! (commit f8c7e47) and re-pointed here unchanged. The `lastcalc`/
//! `oldhealth` cache and the `> 100 -> 100` clamp stay at the
//! `face::calc_pain_offset` call site (the clamp is the cache-key
//! decision); only the exact integer sequence moved.

use std::ffi::c_int;

use super::consts::{ST_FACESTRIDE, ST_NUMPAINFACES};

/// The status-bar face-row formula: map a (already > 100-clamped at
/// the call site) health value to the base face-array index inside the
/// flat `faces` table (`ST_calcPainOffset` upstream, st_stuff.c).
///
/// ## Technical Details
///
/// `ST_FACESTRIDE * (((100 - health) * ST_NUMPAINFACES) / 101)` --
/// the integer division happens BEFORE the stride multiply, so the
/// quotient truncation at each 101-boundary is the observable: the
/// five pain rows sit at health fenceposts 20/40/60/80 (quotients
/// 4/3/2/1/0), pinned by the baseline vectors below. There is no
/// lower clamp: negative health walks the index past `ST_GODFACE`
/// (upstream-faithful; the dead-face priority branch usually hides
/// it, but the arithmetic must not change).
///
/// ## On Calling
///
/// Raw `c_int` in/out; the caller passes the clamped local from
/// `face::calc_pain_offset`. Pure computation -- no state, no
/// threading assumptions, no panics (Rust truncating division; the
/// `health == 101..` corner truncates toward zero and is pinned by
/// the vector table even though the call site clamps it away).
#[doc(alias = "ST_calcPainOffset")]
pub fn pain_offset(health: c_int) -> c_int {
    ST_FACESTRIDE * (((100 - health) * ST_NUMPAINFACES) / 101)
}

#[cfg(test)]
mod tests {
    use super::pain_offset;
    use crate::doom::g_game::players;
    use crate::doom::m_random::{M_ClearRandom, RNDTABLE, rndindex};
    use crate::doom::st_stuff::{face::calc_pain_offset, plyr, ticker::ticker};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;
    use std::ffi::c_int;

    /// Known vectors at every quotient boundary of `((100 - h) * 5) /
    /// 101` plus out-of-clamp inputs: the five pain rows at health
    /// 0/20/40/60/80 fenceposts (stride 8, rows 32/24/16/8/0), the
    /// row-interior fenceposts 19/39/59/79/99, and the negative-health
    /// overflow past `ST_GODFACE` (= 40) that the missing lower clamp
    /// produces.
    #[test]
    fn pain_offset_formula_baseline_vectors() {
        let vectors: [(c_int, c_int); 15] = [
            (0, 32),    // row 0 (critical): 500/101 = 4
            (19, 32),   // last row-0 health: 405/101 = 4
            (20, 24),   // row 1: 400/101 = 3
            (39, 24),   // last row-1 health: 305/101 = 3
            (40, 16),   // row 2: 300/101 = 2
            (59, 16),   // last row-2 health: 205/101 = 2
            (60, 8),    // row 3: 200/101 = 1
            (79, 8),    // last row-3 health: 105/101 = 1
            (80, 0),    // row 4 (healthy): 100/101 = 0
            (99, 0),    // 5/101 = 0
            (100, 0),   // 0/101 = 0
            (-1, 40),   // out of clamp: 505/101 = 5 -> past ST_GODFACE
            (-10, 40),  // 550/101 = 5
            (101, 0),   // (-5)/101 truncates toward zero: 0
            (200, -32), // (-500)/101 = -4: negative rows, never reached via the clamped call site
        ];
        for (health, expected) in vectors {
            assert_eq!(pain_offset(health), expected, "pain offset drifted at health={health}");
        }
    }

    /// Drive the LIVE `calc_pain_offset` body through distinct health
    /// values (the `oldhealth` cache recomputes exactly on change) and
    /// assert it against the extracted formula. `plyr` is redirected
    /// at `players[0]` for the duration and restored; the cache
    /// statics are function-local and only ever seen by this test.
    #[test]
    fn st_calc_pain_offset_live_matches_formula() {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let saved_plyr = plyr;
            plyr = std::ptr::addr_of_mut!(players[0]);
            for health in [100, 50, 20, 0, -1] {
                (*plyr).health = health;
                assert_eq!(calc_pain_offset(), pain_offset(health), "health={health}");
            }
            plyr = saved_plyr;
        }
    }

    /// The ST_Ticker RNG-ledger vector: with the cursors cleared, one
    /// `ticker` call consumes EXACTLY one `M_Random` draw (the
    /// `st_randomnumber` sample), leaving `rndindex` at 1 and the
    /// sample equal to `RNDTABLE[1]`; a second call walks to
    /// `RNDTABLE[2]` / `rndindex` 2. Any extraction-era reordering
    /// that adds, drops, or moves a draw between `st_clock` and
    /// `update_widgets` shifts this count and breaks every frozen
    /// state digest.
    #[test]
    fn st_ticker_draws_exactly_one_m_random_per_tic() {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            use crate::doom::st_stuff::{
                st_chat, st_clock, st_facecount, st_faceindex, st_msgcounter, st_oldhealth,
                st_randomnumber,
            };
            let saved_plyr = plyr;
            let saved_clock = st_clock;
            let saved_randomnumber = st_randomnumber;
            let saved_oldhealth = st_oldhealth;
            let saved_facecount = st_facecount;
            let saved_faceindex = st_faceindex;
            let saved_msgcounter = st_msgcounter;
            let saved_chat = st_chat;
            plyr = std::ptr::addr_of_mut!(players[0]);

            M_ClearRandom();
            ticker();
            assert_eq!(st_randomnumber, RNDTABLE[1] as c_int);
            assert_eq!(rndindex, 1);

            ticker();
            assert_eq!(st_randomnumber, RNDTABLE[2] as c_int);
            assert_eq!(rndindex, 2);

            plyr = saved_plyr;
            st_clock = saved_clock;
            st_randomnumber = saved_randomnumber;
            st_oldhealth = saved_oldhealth;
            st_facecount = saved_facecount;
            st_faceindex = saved_faceindex;
            st_msgcounter = saved_msgcounter;
            st_chat = saved_chat;
        }
    }
}
