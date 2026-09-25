//! F9 M3: the host-side scenario twin (cross-target determinism proof).
//!
//! One test binary, two roles:
//! - [`scenario_twin`] is the parent driver: it enumerates the committed
//!   manifests under `shells/web/scripts/scenarios/` and re-invokes this
//!   binary once per scenario with `SCENARIO_TWIN_SCENARIO` set. One fresh
//!   process per scenario gives the twin the same property the wasm runner
//!   gets for free - a pristine engine per run (the engine cannot re-boot in
//!   one process).
//! - [`scenario_twin_child`] is the per-scenario executor. Without the
//!   selection env (a plain `cargo test --test scenario_harness` pass) it is
//!   a deliberate no-op so the harness pass stays green.
//!
//! Each child boots the engine through the manifest's boot profile, executes
//! the manifest steps exactly like `run-scenarios.mjs` does, writes
//! `scenarios/out/host-<name>.jsonl` and compares its ledger against the
//! SAME wasm-blessed golden the wasm runner uses. Equal state/frame hashes
//! across host and wasm are the deliverable; a divergence is a finding, not
//! something to mask (never regenerate goldens host-side).
//!
//! Commands that consume the artifacts (Task 7 wires these into CI):
//!   cargo test -p room --test scenario_harness
//!   node shells/web/scripts/diff-ledgers.mjs \
//!     shells/web/scripts/scenarios/out/wasm-title_attract.jsonl \
//!     shells/web/scripts/scenarios/out/host-title_attract.jsonl

#![allow(non_snake_case, non_upper_case_globals)]

// The shared module keeps the frame_split_common directory layout
// (`scenario_harness/mod.rs`). The #[path] attribute is required: a plain
// `mod scenario_harness;` inside scenario_harness.rs would collide with the
// binary's own directory form (rustc E0761), so the module travels under a
// distinct name.
#[path = "scenario_harness/mod.rs"]
mod twin;

use std::process::Command;

use twin::{allow_fail_set, list_manifest_names, run_selected_scenario, scenarios_dir};

#[test]
fn scenario_twin()
{
    let names = list_manifest_names();
    assert!(
        !names.is_empty(),
        "no scenario manifests found in {} - nothing to prove parity on",
        scenarios_dir().display()
    );
    let exe = std::env::current_exe().expect("current_exe of this test binary");
    for name in &names
    {
        eprintln!("[twin] === {name}: booting a fresh engine process ===");
        let out = Command::new(&exe).
            arg("scenario_twin_child").
            arg("--exact").
            // The child's WARN frame-divergence / PASS lines must surface in
            // this run's output even when the child test passes (libtest
            // would otherwise swallow a passing test's captured output).
            arg("--nocapture").
            env("SCENARIO_TWIN_SCENARIO", name).
            output().
            unwrap_or_else(|e| panic!("[scenario-fail] {name}: spawn twin child failed: {e}"));
        // Forward the child's engine log + ledger evidence into this run's
        // output, so a failing cargo test shows the full bisect material.
        print!("{}", String::from_utf8_lossy(&out.stdout));
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        if out.status.success()
        {
            continue;
        }
        if String::from_utf8_lossy(&out.stderr).contains("[twin-setup-error]")
        {
            // Setup errors (bad manifest, missing golden) are never
            // allow-failed - the wasm runner's contract.
            panic!("[scenario-fail] {name}: setup error (exit {})", out.status);
        }
        if allow_fail_set().iter().any(|a| a == name)
        {
            // A process-death failure (engine I_Error) cannot be converted
            // to exit 0 inside the child; the parent applies the same
            // allow-list here and records the expected fail.
            eprintln!("[expected-fail] {name}: twin child failed (exit {}) while allow-listed", out.status);
            continue;
        }
        panic!("[scenario-fail] {name}: twin child failed (exit {})", out.status);
    }
    println!("[twin] {} scenario(s) compared against the wasm goldens: {}", names.len(), names.join(", "));
}

#[test]
fn scenario_twin_child()
{
    let Ok(name) = std::env::var("SCENARIO_TWIN_SCENARIO")
    else
    {
        // Parent-driven child only: nothing to do in the plain pass.
        return;
    };
    run_selected_scenario(&name);
}
