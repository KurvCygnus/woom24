//! F1 M1 determinism red line (jittered-cadence half): drive the engine
//! through `doomgeneric_frame` with jittered frame times, withheld frames and
//! a simulated tab-suspend, and assert the final simulation state is
//! identical to the exact-cadence run (shared EXPECTED constant). Any timing
//! leak into the simulation — through the pump, the cap, or the interpolation
//! fraction — shows up here first.

#![allow(non_snake_case, non_upper_case_globals)]

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: room::dhat::Alloc = room::dhat::Alloc;

mod frame_split_common;

use frame_split_common::{assert_matches_expected, boot, run};

#[test]
fn frame_split_jittered_cadence_lands_on_expected_state()
{
    boot();
    let got = run(true);
    assert_matches_expected(&got);
}
