//! The `g_game` game-state statics: ALL 57 `#[no_mangle] pub static mut`
//! engine-visible globals (game-flow slots, per-player tables, demo I/O
//! state, benchmark flags, movement tables, deferred new-game params, par
//! tables, the body queue) plus the two cross-file private save latches.
//!
//! One auditable home for the extern-by-symbol surface: every legacy
//! `extern "C"` declarer block in the tree links these BY SYMBOL --
//! `d_player/mod.rs:104-113` (`players`, `consoleplayer`),
//! `hu_stuff.rs:391-411` (`playeringame`, `consoleplayer`),
//! `d_net.rs` (`playeringame`, plus the extern fns `G_Ticker` /
//! `G_BuildTiccmd` re-pinned at their definitions), `p_saveg/mod.rs:274`
//! (`playeringame`) and `c_ffi.rs:507-539` (`forwardmove`, `sidemove`,
//! `angleturn`, `bodyqueslot`, `vanilla_savegame_limit`,
//! `vanilla_demo_limit`, `precache`, `testcontrols`, `levelstarttic`,
//! `totalkills`, `totalitems`, `totalsecret`). Dropping a `#[no_mangle]`
//! here is a link failure on the LP64 gate, and the `c_ffi` block also
//! feeds `c_tests/g_game_c.rs`, which asserts the movement tables' exact
//! values. Names and `#[no_mangle]` are kept verbatim per the statics
//! ruling (data-tier renaming comes with freeze-zone retirement); every
//! consumer path holds through the module-root re-exports in `mod.rs`.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::doom::d_player::{PlayerT, MAXPLAYERS};
use crate::doom::p_telept::mobj_t;
use crate::doom::wi_stuff::{wbplayerstruct_t, wbstartstruct_t};

use super::consts::{boolean, byte, fixed_t, ga_nothing, skill_t, GS_DEMOSCREEN};

// ---------------------------------------------------------------------------
// Public globals — #[no_mangle] so other Rust modules can access them via
// `extern "C"` declarations (the same pattern used throughout this codebase).
// ---------------------------------------------------------------------------

/// Value of `gamestate` from the previous `G_Ticker` invocation, used to
/// detect transitions (e.g. dismissing the intermission screen).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut oldgamestate: c_int = GS_DEMOSCREEN;

/// Deferred game action queue (`ga_*`); drained at the top of `G_Ticker`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut gameaction: c_int = ga_nothing;

/// Active gameplay state machine slot (`GS_LEVEL`/`GS_INTERMISSION`/...).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut gamestate: c_int = GS_DEMOSCREEN;

/// Currently selected skill level (`sk_baby` .. `sk_nightmare`).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut gameskill: c_int = 0;

/// Non-zero when monsters respawn (nightmare skill or `-respawn` parm).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut respawnmonsters: boolean = 0;

/// Currently loaded episode number (1-based; 1-3 for Doom, up to 4 with Ultimate).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut gameepisode: c_int = 0;

/// Currently loaded map number (1-based; 1-9 in Doom, 1-32 in Doom II).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut gamemap: c_int = 0;

/// If non-zero, exit the level after this number of minutes.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut timelimit: c_int = 0;

/// Non-zero while gameplay is paused (sound is paused, ticker suspended).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut paused: boolean = 0;

/// One-tic flag requesting a pause-toggle ticcmd from `G_BuildTiccmd`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut sendpause: boolean = 0;

/// One-tic flag requesting a savegame ticcmd from `G_BuildTiccmd`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut sendsave: boolean = 0;

/// Non-zero while a user-controlled game is in progress (vs demo / title).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut usergame: boolean = 0;

/// Set by `-timedemo`; on demo end prints fps stats via `I_Error`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut timingdemo: boolean = 0;

/// Set by `-nodraw`; disables rendering for benchmarking purposes.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut nodrawers: boolean = 0;

/// `I_GetTime()` value captured when a timed demo started.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut starttime: c_int = 0;

/// Non-zero while the 3D view is being rendered (false during intermission).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut viewactive: boolean = 0;

/// Deathmatch mode (`0`=co-op, `1`=DM, `2`=DM2 / altdeath).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut deathmatch: c_int = 0;

/// Non-zero in a networked game (changes consistency-check behaviour).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut netgame: boolean = 0;

/// Per-slot presence flag for the four player slots (`MAXPLAYERS = 4`).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut playeringame: [boolean; MAXPLAYERS] = [0; MAXPLAYERS];

/// Player state slots (inventory, position, view angle, etc.).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut players: [PlayerT; MAXPLAYERS] = unsafe { std::mem::zeroed() };

/// Per-player "turbo" detection latch consumed by `G_Ticker`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut turbodetected: [boolean; MAXPLAYERS] = [0; MAXPLAYERS];

/// Index of the local player receiving input events.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut consoleplayer: c_int = 0;

/// Index of the player whose first-person view is being drawn (spy mode).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut displayplayer: c_int = 0;

/// `gametic` value captured when the current level was loaded.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut levelstarttic: c_int = 0;

/// Sum of monster kills across all players for the current level.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut totalkills: c_int = 0;

/// Sum of item pickups across all players for the current level.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut totalitems: c_int = 0;

/// Sum of secret-sector finds across all players for the current level.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut totalsecret: c_int = 0;

/// File name of the demo currently being recorded (Z_Malloc'd).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demoname: *mut c_char = ptr::null_mut();

/// Non-zero while a `.lmp` demo is being written from input.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demorecording: boolean = 0;

/// cph's Doom 1.91 longtics hack - encodes angleturn in 2 bytes per tic
/// instead of 1, enabling smooth high-res turning in demos.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut longtics: boolean = 0;

/// Round per-tic angleturn to the nearest 256 BAM when recording vanilla
/// (non-longtics) demos so the 1-byte demo angleturn replays accurately.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut lowres_turn: boolean = 0;

/// Non-zero while a `.lmp` demo is being played back.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demoplayback: boolean = 0;

/// Non-zero when the active demo was recorded in a networked session.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut netdemo: boolean = 0;

/// Base pointer to the current demo I/O buffer (Z_Malloc'd).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demobuffer: *mut byte = ptr::null_mut();

/// Read/write cursor within `demobuffer`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demo_p: *mut byte = ptr::null_mut();

/// One-past-the-end pointer for `demobuffer`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut demoend: *mut byte = ptr::null_mut();

/// Non-zero when launched with `-playdemo`; quits after the demo ends.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut singledemo: boolean = 0;

/// Non-zero (default) to precache all level graphics during map load.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut precache: boolean = 1; // true by default

/// Non-zero while invoked from the setup utility's "test controls" mode.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut testcontrols: boolean = 0;

/// Low-pass-filtered mouse speed displayed by the test-controls thermometer.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut testcontrols_mousespeed: c_int = 0;

/// Parameters for the world-map / intermission screen, populated by
/// `G_DoCompleted` before transitioning to `GS_INTERMISSION`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut wminfo: wbstartstruct_t = wbstartstruct_t {
    epsd: 0,
    didsecret: 0,
    last: 0,
    next: 0,
    maxkills: 0,
    maxitems: 0,
    maxsecret: 0,
    maxfrags: 0,
    partime: 0,
    pnum: 0,
    plyr: [wbplayerstruct_t {
        in_: 0,
        skills: 0,
        sitems: 0,
        ssecret: 0,
        stime: 0,
        frags: [0; 4],
        score: 0,
    }; MAXPLAYERS],
};

/// Net consistency check ring: `consistancy[player][gametic/ticdup % BACKUPTICS]`.
/// A mismatch on a remote command triggers `I_Error("consistency failure ...")`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut consistancy: [[byte; 128]; MAXPLAYERS] = [[0; 128]; MAXPLAYERS];

/// Index into `bodyque` for the next corpse to add (modulo 32).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut bodyqueslot: c_int = 0;

/// Non-zero enforces the vanilla `SAVEGAMESIZE` cap (`I_Error` on overrun).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut vanilla_savegame_limit: c_int = 1;

/// Non-zero enforces the vanilla demo buffer cap; zero auto-grows it.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut vanilla_demo_limit: c_int = 1;

/// Forward-movement speed table: `[slow=0x19, fast=0x32]` (`fixed_t` per tic).
/// Indexed by the `speed` flag computed in `G_BuildTiccmd`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut forwardmove: [fixed_t; 2] = [0x19, 0x32];

/// Lateral strafe speed table: `[slow=0x18, fast=0x28]` (`fixed_t` per tic).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut sidemove: [fixed_t; 2] = [0x18, 0x28];

/// Turn-speed table: `[normal=640, fast=1280, slow=320]` (BAM units per tic).
/// The "slow" entry is selected for the first `SLOWTURNTICS` (6) of held input.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut angleturn: [fixed_t; 3] = [640, 1280, 320];

/// Non-zero when the next level transition should route through the secret
/// exit (set by `G_SecretExitLevel`).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut secretexit: boolean = 0;

/// File name of the demo deferred for playback (set by `G_DeferedPlayDemo`).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut defdemoname: *mut c_char = ptr::null_mut();

/// File name buffer for the savegame currently being loaded.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut savename: [c_char; 256] = [0; 256];

/// Deferred `G_DeferedInitNew` parameter: skill level.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut d_skill: skill_t = 0;
/// Deferred `G_DeferedInitNew` parameter: episode.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut d_episode: c_int = 0;
/// Deferred `G_DeferedInitNew` parameter: map number.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut d_map: c_int = 0;

/// Doom episode par times (episodes 1-3, maps 1-9), in seconds.
/// Index `[0]` and `[*][0]` are dummies to keep the table 1-based.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut pars: [[c_int; 10]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 30, 75, 120, 90, 165, 180, 180, 30, 165],
    [0, 90, 90, 90, 120, 90, 360, 240, 30, 170],
    [0, 90, 45, 90, 150, 90, 90, 165, 30, 135],
];

/// Doom II par times (maps 1-32), in seconds. Index `[0]` is map 1.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut cpars: [c_int; 32] = [
    30, 90, 120, 120, 90, 150, 120, 120, 270, 90, 210, 150, 150, 150, 210, 150, 420, 150, 210, 150,
    240, 150, 180, 150, 150, 300, 330, 420, 300, 180, 120, 30,
];

/// Circular queue of `BODYQUESIZE=32` recent player corpses; the oldest is
/// removed when the queue wraps (see `G_CheckSpot`).
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut bodyque: [*mut mobj_t; 32] = [ptr::null_mut(); 32];

// ---------------------------------------------------------------------------
// Cross-file private statics
// ---------------------------------------------------------------------------

/// Selected savegame slot for the next save (set by `G_SaveGame`,
/// encoded into the save ticcmd by `G_BuildTiccmd`).
pub(super) static mut SAVEGAMESLOT: c_int = 0;
/// Description text for the next savegame (set by `G_SaveGame`, read by
/// `G_Ticker`'s `BTS_SAVEGAME` decode and written into the save header).
pub(super) static mut SAVEDESCRIPTION: [c_char; 32] = [0; 32];

#[cfg(test)]
mod tests
{
    use crate::doom::g_game::{angleturn, forwardmove, sidemove};

    /// `forwardmove[0]` (slow) baseline of 0x19 - guards against accidental edits.
    #[test]
    fn forwardmove_slow_is_0x19()
    {
        unsafe
        {
            assert_eq!(forwardmove[0], 0x19);
        }
    }

    /// `forwardmove[1]` (fast) baseline of 0x32 - also the turbo threshold.
    #[test]
    fn forwardmove_fast_is_0x32()
    {
        unsafe
        {
            assert_eq!(forwardmove[1], 0x32);
        }
    }

    /// `sidemove[0]` (slow) baseline of 0x18.
    #[test]
    fn sidemove_slow_is_0x18()
    {
        unsafe
        {
            assert_eq!(sidemove[0], 0x18);
        }
    }

    /// `sidemove[1]` (fast) baseline of 0x28.
    #[test]
    fn sidemove_fast_is_0x28()
    {
        unsafe
        {
            assert_eq!(sidemove[1], 0x28);
        }
    }

    /// `angleturn[]` baseline values (normal / fast / slow).
    #[test]
    fn angleturn_values()
    {
        unsafe
        {
            assert_eq!(angleturn[0], 640);
            assert_eq!(angleturn[1], 1280);
            assert_eq!(angleturn[2], 320);
        }
    }
}
