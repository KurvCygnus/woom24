//! Rust port of vendor/doomgeneric/d_net.c.
//!
//! Network game communication and protocol layer, OS-independent parts.
//!
//! In the original Doom, `d_net.c` implements tic-synchronisation over a
//! network connection. In this single-player port, `FEATURE_MULTIPLAYER` is
//! never defined, so the networking stack is replaced by stub implementations
//! provided by the loop interface defined in `d_loop.h`. The primary responsibilities that remain are:
//!
//! * Connecting and starting a "net game" via the loop back-end
//!   (`connect_net_game`, `check_net_game`).
//! * Registering the per-tic callback table (`DOOM_LOOP_INTERFACE`) so that the
//!   loop layer can call `D_ProcessEvents`, `G_BuildTiccmd`, `run_tic`, and
//!   `M_Ticker` at the right moments.
//! * Propagating game settings between the global variables (`deathmatch`,
//!   `startepisode`, etc.) and the `NetGameSettingsT` structure that the loop
//!   layer reads/writes.
//! * Handling player-quit events during multi-player (skipped when `demoplayback`
//!   is active, mirroring the original guard).
//!
//! Notable Rust-vs-C differences:
//! * `handle_player_quit` takes a player index (`usize`) instead of a
//!   `player_t *` pointer; pointer arithmetic is replaced by array indexing.
//! * `deh_string` is a no-op inline stub (DeHackEd support is not implemented);
//!   the C original calls into `deh_main.c`.
//! * The `DEH_printf` diagnostic prints present in `D_CheckNetGame` in C are
//!   omitted in this port.
//! * `deh_sha1sum` in `init_connect_data` is left zeroed (the C original called
//!   `DEH_Checksum`), guarded by `#if ORIGCODE` in the vendor source.
//!
//! ## Submodule Responsibility
//!
//! - `protocol_types.rs` -- the `#[repr(C)]` ABI mirrors
//!   `NetConnectDataT` / `NetGameSettingsT` / `LoopInterfaceT`, their
//!   size constants, and the compile-time size guards
//! - `net_glue.rs` -- boot-time settings exchange:
//!   `init_connect_data`, `save_game_settings`, `load_game_settings`,
//!   the `connect_net_game` / `check_net_game` entry points, the
//!   `deh_string` stub, and the `EXIT_MSG` template
//! - `loop_table.rs` -- the per-tic callback side: `run_tic`,
//!   `handle_player_quit`, and the `DOOM_LOOP_INTERFACE` table
//!
//! The module root is documentation + wiring only beyond the two
//! verbatim surfaces: the `netcmds` static (name + `#[no_mangle]`
//! kept -- statics ruling) and the extern block (see the
//! extern-by-symbol contract below).
//!
//! # Extern-by-symbol contract (carried VERBATIM)
//!
//! The `extern "C"` block below is carried verbatim from pre-split
//! `d_net.rs:177-273`. These are extern DECLARATIONS, not definitions:
//! every item links BY SYMBOL to a definer in a *different* module --
//! the freeze-zone freeze list (`m_misc.rs`, `g_game.rs`, `d_main.rs`,
//! `m_menu.rs`, `w_checksum.rs`, the doomstat statics) plus the pinned
//! graduates (`M_CheckParm` in `m_argv/lookup.rs`,
//! `W_CheckNumForName` in `w_wad/lookup.rs`) and the intra-wave seam
//! (`D_RegisterLoopCallbacks` / `D_InitNetGame` / `D_StartNetGame` --
//! d_loop's `#[export_name]` pins keep these links alive across its
//! renames; d_loop landed first in this wave for exactly this reason).
//! Never re-point this block to Rust paths (`crate::doom::...`) while
//! the freeze zone exists -- that would silently breach the
//! extern-by-symbol contract and lose the declarer record (the B5
//! lesson: a missed declarer surfaces as a link error only in configs
//! that exercise the symbol).
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! functions only -- plain-English internal names with
//! `#[doc(alias = "OriginalName")]`, upstream-name shims at this root,
//! and `#[export_name]` re-pins exactly where the pre-move surface had
//! `#[no_mangle]` (the two boot entry points; the C-symbol set stays
//! byte-identical). `run_tic` is address-taken only (fn pointer in
//! `DOOM_LOOP_INTERFACE`), never symbol-linked -- alias, no pin.
//! Statics and types keep their names outright: `netcmds` keeps
//! `#[no_mangle]` (wasm export-table member; g_game consumes it by
//! path, `g_game.rs:678`).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `PlayerQuitGame` (d_net.c:45) | `loop_table::handle_player_quit` | glue | alias only; private (not `#[no_mangle]` pre-move); unreachable single-player (fires only when a net peer drops) |
//! | `RunTic` (d_net.c:71) | `loop_table::run_tic` | glue | alias only; address-taken in `DOOM_LOOP_INTERFACE`, never symbol-linked; whole body is orchestration (quit scan -> `netcmds` latch -> `advancedemo` -> `G_Ticker`) |
//! | `LoadGameSettings` (d_net.c:108) | `net_glue::load_game_settings` | glue | alias only; boot-time copy of settings into doomstat globals |
//! | `SaveGameSettings` (d_net.c:139) | `net_glue::save_game_settings` | glue | alias only; boot-time inverse copy; the `lowres_turn = -record && !-longtics` derivation influences demo turn resolution downstream, but the value reaches the sim via g_game's `G_BuildTiccmd`, not here |
//! | `InitConnectData` (d_net.c:159) | `net_glue::init_connect_data` | glue | alias only; boot-time connect-data population (checksum, Freedoom probe, drone flags) |
//! | `DEH_String` (deh_main.h stub) | `net_glue::deh_string` | data | alias only; identity stub, `#[inline(always)]`; private pre-move |
//! | `D_ConnectNetGame` (d_net.c:215) | `net_glue::connect_net_game` | glue | shim + `#[export_name = "D_ConnectNetGame"]` pin (has `#[no_mangle]` pre-move -- wasm export-table member; `d_main.rs:1818` boots through it) |
//! | `D_CheckNetGame` (d_net.c:240) | `net_glue::check_net_game` | glue | shim + `#[export_name = "D_CheckNetGame"]` pin; `d_main.rs:1901` |
//! | (data) `netcmds` (static) | module root (this file) | data | name + `#[no_mangle]` kept verbatim; latched per tic by `run_tic`, consumed by `G_Ticker` (`g_game.rs:1528`) |
//! | (data) `NetConnectDataT` / `NetGameSettingsT` / `LoopInterfaceT` | `protocol_types.rs` | data | names kept; `#[repr(C)]` field order IS the C ABI -- size-pinned by the moved guards |
//! | (data) `NET_MAXPLAYERS` / `SHA1_DIGEST_SIZE`, `EXIT_MSG`, fn-local `EXITMSG`, `DOOM_LOOP_INTERFACE` | `protocol_types.rs` / `net_glue.rs` / `loop_table.rs` | data | names kept (statics ruling); `DOOM_LOOP_INTERFACE` is `pub(super)` so `check_net_game` can hand its address to d_loop |
//! | (data) extern `deathmatch`..`playeringame` statics + helper fn declarations | module root (extern block) | data | carried VERBATIM; definitions are freeze-zone `#[no_mangle]` statics/functions (doomstat.rs, d_main.rs, g_game.rs, r_main.rs, m_misc.rs, w_checksum.rs) and pinned graduates (m_argv, w_wad) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave C2; per-function verdicts in
//! `c2-net-loop-report.md` SS1.4 -- every function is glue or data): this
//! module is BOOT-TIME connect/check glue plus one per-tic orchestrator.
//! The load-bearing parts are ordering and identity, not computation:
//!
//! - The ticcmd that enters the demo stream flows through this module's
//!   seam with d_loop: `run_tic` latches the loop layer's command array
//!   pointer into `netcmds` per tic (d_net.c `netcmds = cmds`), and
//!   `G_Ticker` (`g_game.rs:1528`) copies `netcmds[consoleplayer]`
//!   from it. The latch ORDER inside `run_tic` is the demo-observable
//!   contract: (1) the player-quit scan runs first, (2) `netcmds` is
//!   stored, (3) the `advancedemo` check precedes `G_Ticker` -- the
//!   attract-mode advance lands before the tic, never after.
//! - `save_game_settings`' `lowres_turn` derivation (`-record` &&
//!   !`-longtics`) is boot-time configuration that *influences* demo
//!   turn resolution downstream (via g_game), so the expression may
//!   never drift -- but no per-tic state is computed here.
//! - `DOOM_LOOP_INTERFACE` binds the four per-tic callbacks
//!   (`D_ProcessEvents`, `G_BuildTiccmd`, `run_tic`, `M_Ticker`);
//!   re-binding or reordering them is a behavior change.
//! - `handle_player_quit`'s demo-recording side effect
//!   (`G_CheckDemoStatus`) is multiplayer-only and unreachable in the
//!   single-player build (`playeringame[i] && !ingame[i]` with
//!   `demoplayback == 0`).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_char;
use std::os::raw::c_int;

use crate::doom::d_player::{TiccmdT, MAXPLAYERS};
use crate::types::Boolean;

pub mod loop_table;
pub mod net_glue;
pub mod protocol_types;

/// Pointer to the array of per-player tic commands for the current tic.
///
/// Set by `run_tic` at the start of each tic, pointing into the buffer
/// provided by the loop layer. Downstream code (primarily `g_game.c`) reads
/// `netcmds[consoleplayer]` to obtain the local player's input. Exported as
/// `#[no_mangle]` so that C code in the loop layer can reference it directly.
/// Corresponds to `ticcmd_t *netcmds` in `d_net.c`.
#[no_mangle]
pub static mut netcmds: *mut TiccmdT = std::ptr::null_mut();

// ---------------------------------------------------------------------------
// External C declarations -- carried VERBATIM from pre-split d_net.rs:177-273
// (see the extern-by-symbol contract above).
// ---------------------------------------------------------------------------

extern "C" {
    /// Safe bounded string copy. Copies at most `dst_size - 1` bytes from `src`
    /// to `dst`, always NUL-terminates, and returns non-zero on success.
    /// From `vendor/doomgeneric/m_misc.c`.
    fn M_StringCopy(dst: *mut c_char, src: *const c_char, dst_size: usize) -> Boolean;
    /// Returns the index of a command-line parameter, or 0 if not present.
    /// From `vendor/doomgeneric/m_argv.c`.
    fn M_CheckParm(parm: *const c_char) -> c_int;
    /// Checks whether the current demo recording or playback has finished; stops
    /// it if so. From `vendor/doomgeneric/g_game.c`.
    fn G_CheckDemoStatus() -> c_int;
    /// Advances the game simulation by one tic, processing all player commands.
    /// From `vendor/doomgeneric/g_game.c`.
    fn G_Ticker();
    /// Advances the attract-mode demo loop (title screen, demo playback, etc.).
    /// From `vendor/doomgeneric/d_main.c`.
    fn D_DoAdvanceDemo();
    /// Drains the engine event queue for the current tic, dispatching input
    /// events to the appropriate subsystem. From `vendor/doomgeneric/d_main.c`.
    fn D_ProcessEvents();
    /// Builds the player's tic command for tic `maketic` from the current input
    /// state. From `vendor/doomgeneric/g_game.c`.
    fn G_BuildTiccmd(cmd: *mut TiccmdT, maketic: c_int);
    /// Advances menu animation and input handling by one tic.
    /// From `vendor/doomgeneric/m_menu.c`.
    fn M_Ticker();
    /// Registers the loop interface callback table with the game loop layer so
    /// that `d_loop.c` can invoke per-tic callbacks. From `vendor/doomgeneric/d_loop.c`.
    fn D_RegisterLoopCallbacks(i: *mut LoopInterfaceT);
    /// Initialises the network/demo subsystem and returns non-zero if a network
    /// game is active. From the C side of `vendor/doomgeneric/d_net.c`.
    fn D_InitNetGame(connect_data: *mut NetConnectDataT) -> c_int;
    /// Starts the network game, exchanging settings with the server (or stub).
    /// From the C side of `vendor/doomgeneric/d_net.c`.
    fn D_StartNetGame(settings: *mut NetGameSettingsT, callback: *const ());
    /// Computes a SHA-1 checksum of the WAD directory into `digest` for
    /// net-consistency checks. From `vendor/doomgeneric/w_checksum.c`.
    fn W_Checksum(digest: *mut u8);
    /// Returns the lump number for the given name, or -1 if not found.
    /// From `vendor/doomgeneric/w_wad.c`.
    fn W_CheckNumForName(name: *const c_char) -> c_int;

    /// Deathmatch mode: 0 = cooperative, 1 = deathmatch, 2 = altdeath.
    /// Defined in `doomstat.h`, set during net-game startup.
    static mut deathmatch: c_int;
    /// Starting episode number override (episode-based games only).
    /// Defined in `doomstat.h`.
    static mut startepisode: c_int;
    /// Starting map number override within the episode.
    /// Defined in `doomstat.h`.
    static mut startmap: c_int;
    /// Starting skill level override (0 = baby ... 4 = nightmare).
    /// Defined in `doomstat.h`.
    static mut startskill: c_int;
    /// Save-game slot to load at startup; -1 means no load.
    /// Defined in `doomstat.h`.
    static mut startloadgame: c_int;
    /// Non-zero when the player is using low-resolution (8-bit) turning, for
    /// Vanilla demo compatibility. Defined in `doomstat.h`.
    static mut lowres_turn: c_int;
    /// Non-zero when monsters are disabled (`-nomonsters`).
    /// Defined in `doomstat.h`.
    static mut nomonsters: c_int;
    /// Non-zero when fast monsters are enabled (`-fast`).
    /// Defined in `doomstat.h`.
    static mut fastparm: c_int;
    /// Non-zero when monsters respawn after death (`-respawn`).
    /// Defined in `doomstat.h`.
    static mut respawnparm: c_int;
    /// Net-game time limit in minutes; 0 means no limit.
    /// Defined in `doomstat.h`.
    static mut timelimit: c_int;
    /// Current IWAD game mode (`GameMode_t` integer: shareware, registered,
    /// commercial, retail, or indetermined). Defined in `doomstat.h`.
    static mut gamemode: c_int;
    /// Current IWAD game mission (`GameMission_t` integer: doom, doom2, etc.).
    /// Defined in `doomstat.h`.
    static mut gamemission: c_int;
    /// Executable version being emulated (`GameVersion_t` integer), used
    /// primarily for demo compatibility. Defined in `doomstat.h`.
    static mut gameversion: c_int;
    /// Player view-angle offset in fixed-point angle units; non-zero in
    /// three-screen (spy) mode (`-left`/`-right`). Defined in `doomstat.h`.
    static mut viewangleoffset: c_int;
    /// Non-zero when the game should start automatically without waiting for a
    /// title-screen keypress. Defined in `doomstat.h`.
    static mut autostart: c_int;
    /// Non-zero when running as a network game. Defined in `doomstat.h`.
    static mut netgame: c_int;
    /// Non-zero while a demo is being played back. Defined in `doomstat.h`.
    static mut demoplayback: c_int;
    /// Non-zero while a demo is being recorded. Defined in `doomstat.h`.
    static mut demorecording: c_int;
    /// Per-slot array indicating which player slots are occupied; non-zero
    /// means that player is in-game. Defined in `doomstat.h`.
    static mut playeringame: [c_int; MAXPLAYERS];
}

//* path-stability re-exports: the ABI types keep their module-root paths
//* (`d_loop`'s tic pump imports all three by root path; `sha1/mod.rs`
//* documents against `NetConnectDataT.wad_sha1sum`), and the freeze-zone
//* boot caller keeps the upstream names -- `d_main.rs:422`
//* (`D_ConnectNetGame` / `D_CheckNetGame`, called from `D_DoomMain` at
//* `:1818` / `:1901`). Plain `pub use` of function items; the C symbols
//* are re-pinned at the definitions with `#[export_name = "<C name>"]`,
//* so the wasm/extern symbol name set stays byte-identical to the
//* pre-split module.
pub use protocol_types::{LoopInterfaceT, NetConnectDataT, NetGameSettingsT};

//* upstream-name shims: the two boot entry points (both were
//* `#[no_mangle]` pre-move; wasm export-table members).
pub use net_glue::{
    check_net_game as D_CheckNetGame, connect_net_game as D_ConnectNetGame,
};
