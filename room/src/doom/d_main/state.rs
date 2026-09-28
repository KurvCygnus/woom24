//! The `d_main` engine-visible statics: ALL 22 `#[no_mangle] pub static
//! mut` globals (startup flags, WAD paths, demo-sequencing slots) plus
//! the test-only `title` buffer.
//!
//! One auditable home for the extern-by-symbol surface: every legacy
//! `extern "C"` declarer block in the tree links these BY SYMBOL --
//! `d_net/mod.rs:196-235` (8 startup statics, forwarded to
//! `net_glue.rs`), `d_net/loop_table.rs:90-96` (`advancedemo`, polled
//! every tic), `p_saveg/mod.rs:262` (`savegamedir`; "never re-point to
//! Rust paths while the freeze zone exists") and `c_ffi.rs:745-748`
//! (14 statics feeding `c_tests/d_main_c.rs`). The freeze-zone path
//! consumers (`g_game/actions.rs`, `g_game/demo.rs`, `f_finale.rs`,
//! `m_menu.rs`, `p_enemy/chase.rs`, `p_mobj/mapthings.rs`) resolve
//! through the module-root re-exports in `mod.rs`. Names and
//! `#[no_mangle]` are kept verbatim per the statics ruling (data-tier
//! renaming comes with freeze-zone retirement).
//!
//! `wadfile` / `mapdir` are dead-but-exported (zero callers in the
//! tree): kept for symbol-set byte-identity, retires with the freeze
//! zone. `title` is test-only, same retirement.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int};
use std::ptr;

use super::consts::GS_DEMOSCREEN;

// ---------------------------------------------------------------------------
// Globals — these are #[no_mangle] so remaining C code (g_game.c etc.)
// can resolve them at link time.
// ---------------------------------------------------------------------------

/// Savegame directory.
#[no_mangle]
pub static mut savegamedir: *mut c_char = ptr::null_mut();

/// Path to the currently loaded IWAD file.
#[no_mangle]
pub static mut iwadfile: *mut c_char = ptr::null_mut();

/// Started game with -devparm.
#[no_mangle]
pub static mut devparm: c_int = 0;

/// Checkparm of -nomonsters.
#[no_mangle]
pub static mut nomonsters: c_int = 0;

/// Checkparm of -respawn.
#[no_mangle]
pub static mut respawnparm: c_int = 0;

/// Checkparm of -fast.
#[no_mangle]
pub static mut fastparm: c_int = 0;

/// Skill level to start at (set by -skill).
#[no_mangle]
pub static mut startskill: c_int = 0;

/// Episode to start at (set by -episode).
#[no_mangle]
pub static mut startepisode: c_int = 0;

/// Map to start at (set by -warp / -episode).
#[no_mangle]
pub static mut startmap: c_int = 0;

/// True when -warp / -episode has provided an explicit start.
#[no_mangle]
pub static mut autostart: c_int = 0;

/// Load game slot (-1 = not loading).
#[no_mangle]
pub static mut startloadgame: c_int = 0;

/// True while playing back a built-in demo sequence.
#[no_mangle]
pub static mut advancedemo: c_int = 0;

/// Store demo, do not accept any inputs.
#[no_mangle]
pub static mut storedemo: c_int = 0;

/// True when the BFG edition of the IWAD is detected.
#[no_mangle]
pub static mut bfgedition: c_int = 0;

/// True once the main event loop has started.
#[no_mangle]
pub static mut main_loop_started: c_int = 0;

/// Primary WAD file path buffer.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut wadfile: [c_char; 1024] = [0; 1024];

/// Directory of development maps.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut mapdir: [c_char; 1024] = [0; 1024];

/// When non-zero, display the ENDOOM text on exit.
#[no_mangle]
pub static mut show_endoom: c_int = 1;

/// Demo loop sequence counter.
#[no_mangle]
pub static mut demosequence: c_int = 0;

/// Tics remaining on the current demo page.
#[no_mangle]
pub static mut pagetic: c_int = 0;

/// Name of the current demo page lump.
#[no_mangle]
pub static mut pagename: *mut c_char = ptr::null_mut();

/// Wipe gamestate for transition effects. -1 forces a wipe on next draw.
#[no_mangle]
pub static mut wipegamestate: c_int = GS_DEMOSCREEN;

/// Title string printed at startup.
///
/// Kept for the buffer-size test only (zero live callers); retires with
/// the freeze zone.
static mut title: [c_char; 128] = [0; 128];

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all startup flag globals are zero-initialized (except `show_endoom`).
    #[test]
    fn startup_flags_defaults() {
        unsafe {
            assert_eq!(devparm, 0);
            assert_eq!(nomonsters, 0);
            assert_eq!(respawnparm, 0);
            assert_eq!(fastparm, 0);
            assert_eq!(autostart, 0);
            assert_eq!(advancedemo, 0);
            assert_eq!(storedemo, 0);
            assert_eq!(bfgedition, 0);
            assert_eq!(main_loop_started, 0);
            assert_eq!(show_endoom, 1);
            assert_eq!(startepisode, 0);
            assert_eq!(startmap, 0);
        }
    }

    /// Verify `wipegamestate` starts at `GS_DEMOSCREEN` so the first frame always wipes.
    #[test]
    fn wipegamestate_default() {
        unsafe {
            assert_eq!(wipegamestate, GS_DEMOSCREEN);
        }
    }

    /// Verify the sizes of the fixed-length character buffers match their C counterparts.
    #[test]
    fn global_buffer_sizes() {
        unsafe {
            assert_eq!(std::mem::size_of_val(&wadfile), 1024);
            assert_eq!(std::mem::size_of_val(&mapdir), 1024);
            assert_eq!(std::mem::size_of_val(&title), 128);
        }
    }
}
