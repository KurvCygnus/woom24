//! F1 M1 determinism red line (exact-cadence half): drive the engine through
//! `doomgeneric_frame` at an exact 35 Hz frame cadence and assert the final
//! simulation state matches the shared EXPECTED constant. The jittered twin
//! (`frame_split_jittered.rs`) must land on the identical state — that is the
//! proof that render timing never leaks into the simulation.

#![allow(non_snake_case, non_upper_case_globals)]

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: room::dhat::Alloc = room::dhat::Alloc;

mod frame_split_common;

use frame_split_common::{assert_matches_expected, boot, run};

#[test]
fn frame_split_exact_cadence_lands_on_expected_state()
{
    boot();
    let got = run(false);
    if std::env::var("BLESS").is_ok()
    {
        got.print_bless();
        panic!("BLESS mode: paste the output into frame_split_common::EXPECTED");
    }
    assert_matches_expected(&got);
}
