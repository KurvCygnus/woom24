//! Central census for vanilla-defect emulations.
//!
//! The emulation code itself stays at its natural site (upstream room keeps
//! the chocolate-shaped emulations in place; relocating them is Fork
//! Discipline merge friction). This module only counts trigger events, so
//! tests and audits can prove which workarounds a given input exercises.
//! Counters are diagnostics: nothing in the simulation may ever read them
//! (determinism red line - demo state hashes must not observe this).

use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(test)]
use std::sync::Mutex;

/// Every cataloged vanilla-defect emulation (docs/vanilla-workarounds.md).
/// `TmbBoxOverrun` and `DemoWindow` are reserved for the G1/G2 audit outcomes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VanillaViolation
{
    SpechitOverrun,
    InterceptsOverrun,
    DonutOverrun,
    RejectPadOverrun,
    MissedBackSideOverrun,
    TeleportFogAngleOverrun,
    ParTimeOverrun,
    PlayeringameOverrun,
    TmbBoxOverrun,
    DemoWindow,
}

const VARIANT_COUNT: usize = 10;
const _: () = assert!(VARIANT_COUNT == VanillaViolation::DemoWindow as usize + 1);

static HITS: [AtomicU32; VARIANT_COUNT] = [const { AtomicU32::new(0) }; VARIANT_COUNT];

fn slot(v: VanillaViolation) -> usize { v as usize }

pub fn record(v: VanillaViolation)
{
    HITS[slot(v)].fetch_add(1, Ordering::Relaxed);
}

pub fn hits(v: VanillaViolation) -> u32
{
    HITS[slot(v)].load(Ordering::Relaxed)
}

/// Test/audit helper: zero every counter. Never call from simulation code.
pub fn reset_all()
{
    for c in &HITS
    {
        c.store(0, Ordering::Relaxed);
    }
}

//* Test-only serialization for census-asserting tests. The counters are
//* process-global statics and the asserting tests live in several modules
//* (here, p_map.rs, p_mobj.rs, g_game); without this one shared lock,
//* any test's reset_all() can land inside another's record/assert window
//* under cargo's default parallel test execution. Census tests take this
//* lock alongside their module-local state locks.
#[cfg(test)]
pub(crate) static CENSUS_TEST_LOCK: Mutex<()> = Mutex::new(());

//* Shared lock for tests that write or exact-value-assert the engine's
//* global game statics (gametic, gamestate, rndindex, prndindex, players):
//* m_random's cursor tests, c_tests/d_loop_c.rs::gametic_default_zero (unix)
//* and harness_hash's mutation/layout tests. Same rationale as
//* CENSUS_TEST_LOCK: under cargo's default parallel test execution, a
//* write-then-assert window on process-global state must not interleave
//* with another test's read of the same static. Promoted from m_random.rs's
//* former module-private cursor lock so harness_hash's tests (which also
//* write the cursors) serialize against the same critical sections.
#[cfg(test)]
pub(crate) static ENGINE_STATICS_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn record_counts_per_violation_and_resets()
    {
        let _g = CENSUS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all();
        record(VanillaViolation::SpechitOverrun);
        record(VanillaViolation::SpechitOverrun);
        record(VanillaViolation::InterceptsOverrun);
        assert_eq!(hits(VanillaViolation::SpechitOverrun), 2);
        assert_eq!(hits(VanillaViolation::InterceptsOverrun), 1);
        assert_eq!(hits(VanillaViolation::DonutOverrun), 0);
        reset_all();
        assert_eq!(hits(VanillaViolation::SpechitOverrun), 0);
    }

    #[test]
    fn record_counts_without_panic()
    {
        let _g = CENSUS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Pins only that record/hits round-trip without panicking; the u32
        // wrap path itself is covered by `fetch_add`'s wrapping semantics,
        // so the diagnostic path stays a non-crash path at the ceiling.
        for _ in 0..3 { record(VanillaViolation::DemoWindow); }
        assert_eq!(hits(VanillaViolation::DemoWindow), 3);
    }
}
