//! The doomgeneric frame entries -- `tick_entry` (upstream
//! `doomgeneric_Tick`) and `frame_entry` (upstream `doomgeneric_frame`,
//! the F1 M1 frame/pump split) -- plus the `MAX_TICS_PER_FRAME`
//! catch-up cap.
//!
//! This is the wasm-symbol-critical surface: the web shell's rAF loop
//! (`shells/web/src/lib.rs:80`/`:93`), the native shell's event loop
//! (`shells/native/src/main.rs:251`) and every F9 golden harness drive
//! the engine through the pinned C symbols defined here.

use std::ffi::c_int;

use crate::doom::d_loop::{gametic, pump_tic_cap, singletics, TryRunTics};
use crate::doom::d_player::{consoleplayer, players, MAXPLAYERS};
use crate::doom::i_timer::I_GetTime;
use crate::doom::i_video::{screenvisible, I_StartFrame};
use crate::doom::r_interp;
use crate::doom::s_sound::S_UpdateSounds;

use super::display::display as D_Display;

/// Execute one rendered frame: run game tics, update sounds, and draw the display.
///
/// Called by the doomgeneric platform layer once per video frame.
///
/// # Engine-created latch (browser BUG A fix)
///
/// A host may drive this entry before `doomgeneric_Create` (the web
/// loader starts its rAF loop with the launcher UI, whose Start click
/// performs the creation much later). Every engine global is still
/// zero-initialised then -- `TryRunTics` divides by `ticdup == 0` -- so
/// the call must no-op entirely, without pumping, presenting, or
/// touching any engine state.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `doomgeneric-sys` and the wasm shell import the upstream name
/// through the root shim.
#[doc(alias = "doomgeneric_Tick")]
#[export_name = "doomgeneric_Tick"]
pub extern "C" fn tick_entry() {
    if !crate::doom::doomgeneric::dg_created() { return; }
    unsafe {
        // Frame synchronous IO operations
        I_StartFrame();

        TryRunTics(); // will run at least one tic

        // Update positional sounds
        let console = consoleplayer;
        let mo = if console >= 0 && (console as usize) < MAXPLAYERS {
            players[console as usize].mo
        } else {
            std::ptr::null_mut()
        };
        S_UpdateSounds(mo as *mut crate::doom::s_sound::MobjStub);

        // Update display
        if screenvisible != 0 { D_Display(); }
    }
}

/// Maximum tics one [`frame_entry`] call may pump. A browser tab that
/// was suspended returns with seconds of accumulated clock debt; pumping all
/// of it (what the references do, `woof/src/d_loop.c:777-785`) would freeze
/// the tab for the whole debt. Four tics (~114 ms of simulation) bound the
/// catch-up per frame — a woom24 browser-shell POLICY, never a simulation
/// policy; the human pass may tune the constant (config constant, not a
/// setting).
pub const MAX_TICS_PER_FRAME: c_int = 4;

/// Frame/pump split (F1 M1): advance simulation by 0..`MAX_TICS_PER_FRAME`
/// tics, then present exactly one interpolated frame.
///
/// `now_ms` must come from the same clock the shell feeds to
/// `DG_GetTicksMs` — the fraction interpolating between the last two tic
/// boundaries is derived from it via the engine's own timer baseline, so no
/// new time source exists (Woof! computes the same quantity per `D_Display`,
/// `d_main.c:255-261`). When no full tic period has elapsed the call pumps
/// zero tics and still presents — that is what uncaps the render rate.
///
/// The legacy [`tick_entry`] stays for tests and compat: it always runs
/// at least one tic and never activates the interpolation board, so its
/// output stays bit-identical to the pre-F1 engine.
///
/// # Engine-created latch (browser BUG A fix, same rationale as [`tick_entry`])
///
/// A pre-`doomgeneric_Create` call would pump `TryRunTics` into a
/// zero-initialised engine (`ticdup == 0` division) and present an
/// unrendered screen buffer -- no-op until creation has finished.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// wasm and native shells import the upstream name through the root
/// shim.
#[doc(alias = "doomgeneric_frame")]
#[export_name = "doomgeneric_frame"]
pub extern "C" fn frame_entry(now_ms: u32) {
    if !crate::doom::doomgeneric::dg_created() { return; }
    unsafe {
        // Frame synchronous IO operations
        I_StartFrame();

        // One fraction sample per present (render-loop state only).
        r_interp::begin_frame(now_ms);

        // Pump 0..N tics through the existing ticker: only call TryRunTics
        // when the engine clock has a due tic, so the call never blocks; the
        // cap keeps the catch-up burst bounded. The cap is shell
        // CONFIGURATION (set once, sticky) — the deterministic landing tests
        // dial it to 1 for their tic-exact landing crawl.
        if pump_tic_cap == 0 { pump_tic_cap = MAX_TICS_PER_FRAME; }
        // Per-frame budget: the sticky cap (a test dial may lower it).
        let budget = if pump_tic_cap < MAX_TICS_PER_FRAME { pump_tic_cap } else { MAX_TICS_PER_FRAME };
        let start_gametic = gametic;
        while gametic - start_gametic < budget {
            if singletics == 0 && I_GetTime() <= gametic { break; } // next tic period not reached; present instead of waiting
            TryRunTics();
        }

        // Update positional sounds
        let console = consoleplayer;
        let mo = if console >= 0 && (console as usize) < MAXPLAYERS {
            players[console as usize].mo
        } else {
            std::ptr::null_mut()
        };
        S_UpdateSounds(mo as *mut crate::doom::s_sound::MobjStub);

        // Update display
        if screenvisible != 0 { D_Display(); }
    }
}
