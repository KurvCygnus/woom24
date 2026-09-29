//! The abstract main game-loop layer. Rust port of
//! `vendor/doomgeneric/d_loop.c` (+ `d_loop.h`): the tic pump that
//! collects player input, builds `TiccmdT` command records, runs game
//! tics via the registered callback table, and maintains timing state.
//! The loop advances in fixed increments of 1/35 second (one tic = one
//! invocation of the `RunTic` callback).
//!
//! In the original Chocolate Doom, this file also contains the networking
//! layer, but the doomgeneric fork compiles with `FEATURE_MULTIPLAYER`
//! undefined so all network-specific paths are stripped out. The Rust port
//! faithfully reflects this simplified build: `init_net_game`,
//! `start_net_game`, and `quit_net_game` are stubs, and `receive_tic`
//! only handles the null net client branch (the multiplayer branch is
//! compiled out).
//!
//! ## Submodule Responsibility
//!
//! - `tic_pump.rs` -- the tic pump proper: pacing and input collection
//!   (`get_adjusted_time`, `build_new_tic`, `net_update`), availability
//!   and skip bookkeeping (`get_low_tic`, `old_net_sync`,
//!   `players_in_game`), the per-set squashes (`ticdup_squash`,
//!   `single_player_clear`), and the loop driver `try_run_tics`
//! - `net_stub.rs` -- the single-player net stubs and loop plumbing:
//!   `handle_disconnected`, `receive_tic`, `start_game_loop`,
//!   `start_net_game`, `init_net_game`, `quit_net_game`,
//!   `register_loop_callbacks`, plus the chocolate-parity test
//!
//! The module root additionally carries the LOOP STATE: every static and
//! constant, carried verbatim with names and attributes unchanged (F10
//! wave C2 maintainer ruling), plus the verbatim extern block. The
//! first-class engine surfaces keep their `room::doom::d_loop::<name>`
//! module-root paths by construction -- `gametic` / `singletics` /
//! `ticdup` / `offsetms` (re-exported through `c_ffi`, symbol-linked by
//! the F9 golden harness), `pump_tic_cap` (path-only render policy), and
//! `BACKUPTICS` (`c_ffi` re-export feeding the `consistancy` ring).
//!
//! # Extern-by-symbol contract (carried VERBATIM)
//!
//! The `extern "C"` block below is carried verbatim from pre-split
//! `d_loop.rs:183-207`. These are extern DECLARATIONS, not definitions:
//! they link BY SYMBOL to the freeze-zone definitions named in the item
//! comments (`dummy/stubs.rs` for the `drone` / `net_client_connected`
//! statics, `i_timer` / `i_video` / `i_system` for the `I_*`
//! calls, `d_main.rs` / `g_game` / `m_menu.rs` for the per-tic
//! helpers). Never re-point this block to Rust paths (`crate::doom::...`)
//! while the freeze zone exists -- that would silently breach the
//! extern-by-symbol contract and lose the declarer record.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! functions only -- plain-English internal names with
//! `#[doc(alias = "OriginalName")]`, upstream-name shims at this root,
//! and `#[export_name]` re-pins exactly where the pre-move surface had
//! `#[no_mangle]` (all eight exported functions; the C-symbol set stays
//! byte-identical). Statics and types keep their names and attributes
//! outright: `gametic` / `singletics` / `ticdup` / `offsetms` keep
//! `#[no_mangle]`; `pump_tic_cap` deliberately keeps having NONE (F1 M1
//! policy, no upstream counterpart: it is path-consumed only, and adding
//! an export would grow the wasm export table).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `GetAdjustedTime` (d_loop.c:119) | `tic_pump::get_adjusted_time` | glue | alias added; wall-clock read + new_sync nudge (dead: `NEW_SYNC` hardcoded 0); pacing input, never sim state |
//! | `BuildNewTic` (d_loop.c:136) | `tic_pump::build_new_tic` | glue | alias added; input-collection pump; ticcmd content is g_game's surface (`G_BuildTiccmd` callback); buffering limits affect when, not what |
//! | `NetUpdate` (d_loop.c:203) | `tic_pump::net_update` | glue | shim + `#[export_name = "NetUpdate"]` pin; tic-build pacing (elapsed-tic accounting) |
//! | `D_Disconnected` (d_loop.c:252) | `net_stub::handle_disconnected` | glue | alias only (private, no pin); `d_` prefix stutter dropped; drone abort; dead path single-player |
//! | `D_ReceiveTic` (d_loop.c:271) | `net_stub::receive_tic` | glue | shim + `#[export_name = "D_ReceiveTic"]` pin; multiplayer-only entry, zero in-tree callers; exported for the C surface |
//! | `D_StartGameLoop` (d_loop.c:305) | `net_stub::start_game_loop` | glue | shim + pin; one-shot timer latch |
//! | `D_StartNetGame` (d_loop.c:340) | `net_stub::start_net_game` | glue | shim + pin; forces ticdup=1 -- load-bearing invariant for every `/ticdup` (pinned by the parity test and the G2 guard) |
//! | `D_InitNetGame` (d_loop.c:452) | `net_stub::init_net_game` | glue | shim + pin; at-exit registration + player_class read; returns 0 |
//! | `D_QuitNetGame` (d_loop.c:560) | `net_stub::quit_net_game` | glue | shim + pin; no-op stub |
//! | `GetLowTic` (d_loop.c:568) | `tic_pump::get_low_tic` | glue | alias added; net branch dead (`net_client_connected` always 0) |
//! | `OldNetSync` (d_loop.c:591) | `tic_pump::old_net_sync` | glue | alias added; net branch dead |
//! | `PlayersInGame` (d_loop.c:642) | `tic_pump::players_in_game` | glue | alias added; always true single-player |
//! | `TicdupSquash` (d_loop.c:672) | `tic_pump::ticdup_squash` | glue | alias added; only mutates cmd content when `ticdup > 1` -- unreachable in this build |
//! | `SinglePlayerClear` (d_loop.c:689) | `tic_pump::single_player_clear` | glue | alias added; single-player slot masking |
//! | `TryRunTics` (d_loop.c:706) | `tic_pump::try_run_tics` | glue | shim + `#[export_name = "TryRunTics"]` pin; the tic pump -- the counts decision is pacing (constraint 3), the demo-observable contract stays whole-body |
//! | `D_RegisterLoopCallbacks` (d_loop.c:823) | `net_stub::register_loop_callbacks` | glue | shim + pin; pointer latch |
//! | (data) `gametic` / `singletics` / `ticdup` / `offsetms` (statics) | module root (this file) | data | names + `#[no_mangle]` kept verbatim (statics ruling); F9 contract: `demo_playthrough.rs:84-85` links `gametic`/`singletics` by symbol; `c_ffi.rs:690` re-exports all four; web probe exports read `gametic`/`singletics`/`ticdup` by path |
//! | (data) `pump_tic_cap` (static) | module root (this file) | data | name kept; NO `#[no_mangle]` before or after; path-only consumers (`frame_split_common`, `d_main.rs:428`, web shell) |
//! | (data) `BACKUPTICS`, `NET_MAXPLAYERS`, `MAX_NETGAME_STALL_TICS`, `TICDATA`..`OLDNETTICS`, `TiccmdSetT` | module root (this file) | data | names kept verbatim; root state per the wave ruling; `TiccmdSetT` is `#[repr(C)]` (C `ticcmd_set_t`) |
//! | (data) extern `drone` / `net_client_connected` statics + `I_*` / per-tic fn declarations | module root (extern block) | data | carried VERBATIM; definitions live in freeze-zone `dummy/stubs.rs:18/:27` (`c_uint` there vs `c_int` here -- ABI-compatible width, carried oddity), `i_timer`, `i_video`, `i_system`, `d_main`, `g_game`, `m_menu.rs` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc submodule by adjudication (F10 wave C2; per-function verdicts
//! in `c2-net-loop-report.md` SS2.4 -- every function is glue): this
//! module's observable behavior belongs to the tic *sequencing*, and
//! extraction would split bodies whose correctness is exactly their
//! ordering. Nothing pure can be pinned against known vectors (the
//! counts formula is pacing over `realtics`/`availabletics`, constraint
//! 3: render rate must never influence simulation state), and wholesale
//! moves into dtmc are forbidden by the F10 criterion. The load-bearing
//! ORDERING constraints the verbatim move preserves (any future rewrite
//! must re-prove them):
//!
//! 1. In d_net's `run_tic` (per-tic callback), the `advancedemo` check
//!    precedes `G_Ticker` -- the attract-mode advance lands before the
//!    tic, never after.
//! 2. Inside the `try_run_tics` ticdup loop, `r_interp::begin_tic()`
//!    runs BEFORE the `RunTic` callback and
//!    `r_interp::end_tic_and_capture()` right AFTER it (render-side
//!    interpolation latch; single call site, `r_interp.rs:331`).
//! 3. `ticdup_squash` runs after each sub-tic (clears the one-shot
//!    chatchar / `BT_SPECIAL` fields for the next duplicate).
//! 4. `single_player_clear` runs before the tic loop (slot masking
//!    ahead of any command fetch).
//!
//! The ticcmd that enters the demo stream flows through this module's
//! seam with d_net: `build_new_tic` latches the built command into
//! `TICDATA`, d_net's `run_tic` copies it into `netcmds`, and `G_Ticker`
//! (`g_game/ticker.rs`) fetches it per tic.
//!
//! - `pump_tic_cap` is a RENDERING policy (`AGENTS.md` constraint 3):
//!   it caps how many tics a frame may pump; it must never gate what a
//!   tic produces. Default 0 = vanilla unbounded (the legacy
//!   `doomgeneric_Tick` path and the demo tests rely on it).
//! - `MAX_NETGAME_STALL_TICS = 5` is chocolate parity and a recorded
//!   deliberate deviation from vanilla's one-boundary early return
//!   (`docs/vanilla-workarounds.md`, "TryRunTics stall cap" row):
//!   demo-exact, pacing-only.
//! - `singletics` (timedemo mode) drives the F9 golden harness: exactly
//!   one `build_new_tic` plus at least one run tic per `try_run_tics`
//!   call.

#![allow(non_upper_case_globals, non_snake_case)]

use std::os::raw::c_int;

use crate::doom::d_net::LoopInterfaceT;
use crate::doom::d_player::TiccmdT;
use crate::types::Boolean;

pub mod net_stub;
pub mod tic_pump;

// ---------------------------------------------------------------------------
// Root state -- carried VERBATIM from pre-split d_loop.rs (F10 wave C2
// maintainer ruling: ALL d_loop statics and constants keep their names and
// attributes at the module root; only functions rename). Submodules reach
// these via `super::`.
// ---------------------------------------------------------------------------

/// Maximum number of players in a multiplayer session.
///
/// Mirrors `NET_MAXPLAYERS` from `net_defs.h`. Even in single-player mode,
/// arrays are sized by this constant to keep the data layout compatible with
/// the C original.
const NET_MAXPLAYERS: usize = 8;

/// Number of tic slots kept in the circular `TICDATA` ring buffer.
///
/// Exported as `pub` because other modules (e.g. `d_loop_c.rs`) read it.
/// Corresponds to `BACKUPTICS` in `d_loop.h`.
pub const BACKUPTICS: usize = 128;

/// A complete set of player commands and participation flags for one tic.
///
/// One `TiccmdSetT` occupies a slot in `TICDATA`. The `cmds` array holds
/// one input record per potential player; `ingame` tracks which player slots
/// are actually active for this tic.
///
/// Corresponds to `ticcmd_set_t` in `d_loop.c`.
#[repr(C)]
struct TiccmdSetT
{
    /// Per-player input commands for this tic.
    cmds: [TiccmdT; NET_MAXPLAYERS],
    /// Non-zero for each player slot that is active this tic (`boolean` in C).
    ingame: [c_int; NET_MAXPLAYERS],
}

/// Circular ring buffer of tic command sets, indexed by `tic % BACKUPTICS`.
///
/// `MAKETIC` selects where new local commands are written; `RECVTIC` tracks
/// how many complete tic sets have been received from the network (single-
/// player: always equals `MAKETIC`). Corresponds to `ticdata[]` in `d_loop.c`.
static mut TICDATA: [TiccmdSetT; BACKUPTICS] = unsafe { std::mem::zeroed() };

/// Index of the next tic for which a local `TiccmdT` has not yet been built.
///
/// Incremented by `build_new_tic`. Corresponds to `maketic` in `d_loop.c`.
static mut MAKETIC: c_int = 0;

/// Number of complete tic command sets received from the server.
///
/// In single-player mode this advances in lock-step with `MAKETIC` via
/// `receive_tic`. Corresponds to `recvtic` in `d_loop.c`.
static mut RECVTIC: c_int = 0;

/// Index of the tic currently being run (or about to be run).
///
/// Exported as `#[no_mangle]` so C code can read it directly.
/// Corresponds to `gametic` in `d_loop.c`.
#[no_mangle]
pub static mut gametic: c_int = 0;

/// When non-zero, exactly one tic is built and run per `try_run_tics` call.
/// Set by `-timedemo` mode. Exported as `#[no_mangle]` for C access.
/// Corresponds to `singletics` in `d_loop.c` (type `boolean`).
#[no_mangle]
pub static mut singletics: c_int = 0; // boolean

/// Index of the local player in the `NET_MAXPLAYERS` arrays.
///
/// Always `0` in single-player mode. Corresponds to `localplayer` in
/// `d_loop.c`.
static mut LOCALPLAYER: c_int = 0;

/// Number of tics to skip building during the next `net_update` call.
///
/// Used by the old net-sync algorithm to slow input generation when a follower
/// player is too far ahead of the key player. Corresponds to `skiptics` in
/// `d_loop.c`.
static mut SKIPTICS: c_int = 0;

/// Maximum tics `try_run_tics` waits in its stall loop for new input before
/// giving up and letting the caller render anyway. Mirrors
/// `MAX_NETGAME_STALL_TICS` from chocolate `d_loop.c` (vanilla used 20; the
/// smaller value keeps the menu responsive while stalled).
const MAX_NETGAME_STALL_TICS: c_int = 5;

/// Tic duplication factor: every `ticdup`-th input sample is sent over the
/// network, reducing bandwidth at the cost of input resolution.
///
/// Always `1` in single-player mode. Exported as `#[no_mangle]` for C access.
/// Corresponds to `ticdup` in `d_loop.c`.
#[no_mangle]
pub static mut ticdup: c_int = 0;

/// Millisecond offset applied to the timer when `NEW_SYNC` is active.
///
/// Allows the network client to nudge the local clock to stay in sync with the
/// server. Stored as a `fixed_t` (i.e. scaled by `FRACUNIT`). Exported as
/// `#[no_mangle]` for C access. Corresponds to `offsetms` in `d_loop.c`.
#[no_mangle]
pub static mut offsetms: c_int = 0; // fixed_t

/// woom24 frame/pump policy (F1 M1): when non-zero, `try_run_tics` never runs
/// more than this many tics per call. The browser shell's `doomgeneric_frame`
/// sets this to the catch-up cap so a suspended tab cannot trigger a
/// multi-second tic burst; the cap is a RENDERING policy, never a simulation
/// policy. `0` (the default) keeps vanilla unbounded behavior, which the
/// legacy `doomgeneric_Tick` path and the demo tests rely on.
///
/// Not from any reference: Woof! simply runs all available tics
/// (`woof/src/d_loop.c:777-785`), which is safe for a native event loop but
/// unbounded after a suspend — the cap is our own browser-shell policy.
pub static mut pump_tic_cap: c_int = 0;

/// Whether to use the new client synchronisation algorithm.
///
/// When non-zero, `try_run_tics` runs exactly `availabletics` tics per frame
/// and `build_new_tic` applies tighter buffering limits.
/// Always `0` in this single-player build (hardcoded in `start_net_game`).
/// Corresponds to `new_sync` in `d_loop.c` (type `boolean`).
static mut NEW_SYNC: c_int = 0; // boolean

/// Pointer to the registered loop callback table.
///
/// Set by `register_loop_callbacks` before the game loop starts.
/// Corresponds to `loop_interface` in `d_loop.c`.
static mut LOOP_INTERFACE: *mut LoopInterfaceT = std::ptr::null_mut();

/// Per-player activity flags as last seen from the network layer.
///
/// Distinct from the `playeringame[]` array used by the game logic, which may
/// be modified during demo playback. Corresponds to `local_playeringame[]` in
/// `d_loop.c`.
static mut LOCAL_PLAYERINGAME: [c_int; NET_MAXPLAYERS] = [0; NET_MAXPLAYERS];

/// Player class sent to the server at connection time.
///
/// Stored here and copied into `NetGameSettingsT` by `start_net_game`.
/// Corresponds to `player_class` in `d_loop.c`.
static mut PLAYER_CLASS: c_int = 0;

/// Adjusted time (in tics) at the start of the previous `net_update` call.
///
/// Used to compute `newtics = nowtime - LASTTIME`. Corresponds to `lasttime`
/// in `d_loop.c`.
static mut LASTTIME: c_int = 0;

/// Frame counter used by the old net-sync algorithm.
///
/// Corresponds to `frameon` in `d_loop.c`.
static mut FRAMEON: c_int = 0;

/// Rolling window of four booleans used to detect consistent lag in the old
/// net-sync algorithm. When all four entries are non-zero, `SKIPTICS` is set.
///
/// Corresponds to `frameskip[4]` in `d_loop.c`.
static mut FRAMESKIP: [c_int; 4] = [0; 4];

/// `MAKETIC` value recorded at the previous old-net-sync frame.
///
/// Compared against `RECVTIC` to determine whether the local player is ahead
/// of the network. Corresponds to `oldnettics` in `d_loop.c`.
static mut OLDNETTICS: c_int = 0;

// ---------------------------------------------------------------------------
// External C declarations -- carried VERBATIM from pre-split d_loop.rs:183-207
// (see the extern-by-symbol contract above).
// ---------------------------------------------------------------------------

extern "C"
{
    /// Non-zero when running as a network drone (spectator only, no input).
    static mut drone: c_int; // boolean
    /// Non-zero when a network client connection is active.
    static mut net_client_connected: c_int; // boolean

    /// Returns the current time in milliseconds since startup.
    fn I_GetTimeMS() -> c_int;
    /// Returns the current time in tics (1/35 second units) since startup.
    fn I_GetTime() -> c_int;
    /// Polls the OS for new input events and queues them.
    fn I_StartTic();
    /// Sleeps for approximately `ms` milliseconds.
    fn I_Sleep(ms: c_int);
    /// Registers `func` to be called at clean exit; `run_on_error` controls
    /// whether it also runs when `I_Error` is invoked.
    fn I_AtExit(func: extern "C" fn(), run_on_error: Boolean);

    /// Drains the event queue and dispatches events to handlers.
    fn D_ProcessEvents();
    /// Fills `cmd` with input state for the local player at tic `maketic`.
    fn G_BuildTiccmd(cmd: *mut TiccmdT, maketic: c_int);
    /// Advances the menu state by one tic.
    fn M_Ticker();
}

//* path-stability re-exports: the freeze-zone callers keep the upstream
//* names -- `d_main.rs:422-429` (`NetUpdate`, `TryRunTics`,
//* `D_StartGameLoop`, plus the root statics), `r_main.rs:324`
//* (`NetUpdate`). Plain `pub use` of function items; the C symbols are
//* re-pinned at the definitions with `#[export_name = "<C name>"]`, so
//* the wasm/extern symbol name set stays byte-identical to the pre-split
//* module.
pub use net_stub::{
    init_net_game as D_InitNetGame, quit_net_game as D_QuitNetGame,
    receive_tic as D_ReceiveTic, register_loop_callbacks as D_RegisterLoopCallbacks,
    start_game_loop as D_StartGameLoop, start_net_game as D_StartNetGame,
};
pub use tic_pump::{net_update as NetUpdate, try_run_tics as TryRunTics};
