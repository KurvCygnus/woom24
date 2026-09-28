//! The tic pump: pacing and input collection (`get_adjusted_time`,
//! `build_new_tic`, `net_update`), availability and skip bookkeeping
//! (`get_low_tic`, `old_net_sync`, `players_in_game`), the per-set
//! squashes (`ticdup_squash`, `single_player_clear`), and the loop
//! driver `try_run_tics`.
//!
//! Split out of pre-graduation `d_loop.rs` (F10 wave C2); the bodies
//! moved verbatim (Allman reformat only) -- see the module root for the
//! mapping table and the deterministic ordering constraints.

use std::os::raw::c_int;

use crate::doom::d_player::TiccmdT;
use crate::doom::i_timer::TICRATE;
use crate::doom::m_fixed::FRACUNIT;
use crate::i_error;

use super::{
    drone, gametic, net_client_connected, offsetms, pump_tic_cap, singletics, ticdup,
    BACKUPTICS, FRAMEON, FRAMESKIP, LASTTIME, LOCALPLAYER, LOCAL_PLAYERINGAME,
    LOOP_INTERFACE, MAKETIC, MAX_NETGAME_STALL_TICS, NET_MAXPLAYERS, NEW_SYNC, OLDNETTICS,
    RECVTIC, SKIPTICS, TICDATA, TiccmdSetT, I_GetTime, I_GetTimeMS, I_Sleep, I_StartTic,
};

/// Returns the current game time in tics, optionally adjusted by [`offsetms`].
///
/// When `NEW_SYNC` is active, the raw millisecond clock is nudged by
/// `offsetms / FRACUNIT` before conversion to tics. Corresponds to
/// `GetAdjustedTime` in `d_loop.c`.
///
/// # Safety
/// Must be called from the single-threaded game loop only. The function reads
/// the mutable globals `NEW_SYNC` and `offsetms` without synchronisation, and
/// calls the C FFI function `I_GetTimeMS`.
pub(super) unsafe fn get_adjusted_time() -> c_int
{
    let mut time_ms = I_GetTimeMS();
    if NEW_SYNC != 0 { time_ms += offsetms / FRACUNIT; }
    (time_ms * TICRATE) / 1000
}

/// Attempts to build one new ticcmd for the local player at `MAKETIC`.
///
/// Polls for OS input, dispatches events, and runs the menu. Skips building a
/// command (returning `false`) in drone mode or when the command buffer is too
/// far ahead of the consumed tic counter. On success, stores the command in
/// `TICDATA` and increments `MAKETIC`. Corresponds to `BuildNewTic` in
/// `d_loop.c`.
///
/// Returns `true` if a new tic was successfully queued, `false` otherwise.
///
/// # Safety
/// `LOOP_INTERFACE` must have been initialised by a prior call to
/// `register_loop_callbacks` and all four function pointers it contains
/// (`ProcessEvents`, `RunMenu`, `BuildTiccmd`, and `RunTic`) must be non-null.
/// Must be called from the single-threaded game loop only, as it reads and
/// writes the mutable globals `gametic`, `ticdup`, `drone`, `NEW_SYNC`,
/// `net_client_connected`, `MAKETIC`, `TICDATA`, and `LOCALPLAYER` without
/// synchronisation.
unsafe fn build_new_tic() -> bool
{
    let gameticdiv = gametic / ticdup;

    I_StartTic();

    let iface = &*LOOP_INTERFACE;
    iface.ProcessEvents.unwrap()();

    // Always run the menu
    iface.RunMenu.unwrap()();

    if drone != 0 { return false; }

    if NEW_SYNC != 0
    {
        if net_client_connected == 0 && MAKETIC - gameticdiv > 2 { return false; }
        if MAKETIC - gameticdiv > 8 { return false; }
    }
    else { if MAKETIC - gameticdiv >= 5 { return false; } }

    let mut cmd = std::mem::zeroed::<TiccmdT>();
    iface.BuildTiccmd.unwrap()(&mut cmd, MAKETIC);

    let slot = (MAKETIC as usize) % BACKUPTICS;
    TICDATA[slot].cmds[LOCALPLAYER as usize] = cmd;
    TICDATA[slot].ingame[LOCALPLAYER as usize] = 1;

    MAKETIC += 1;

    true
}

/// Builds new ticcmds for the console player and (in networked play) sends
/// them to peers.
///
/// Computes the number of new tics elapsed since the last call, applies any
/// pending skip, and calls `build_new_tic` for each new tic. In
/// `singletics` mode this function returns immediately (the caller is
/// expected to call `build_new_tic` directly). Corresponds to `NetUpdate` in
/// `d_loop.c`. Called from `d_main.c` and `r_main.c`.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below
/// (`#[no_mangle]` drops with the rename; the symbol name set stays
/// byte-identical).
#[doc(alias = "NetUpdate")]
#[export_name = "NetUpdate"]
pub extern "C" fn net_update()
{
    unsafe
    {
        if singletics != 0 { return; }

        let nowtime = get_adjusted_time() / ticdup;
        let mut newtics = nowtime - LASTTIME;
        LASTTIME = nowtime;

        if SKIPTICS <= newtics
        {
            newtics -= SKIPTICS;
            SKIPTICS = 0;
        }
        else
        {
            SKIPTICS -= newtics;
            newtics = 0;
        }

        for _ in 0..newtics { if !build_new_tic() { break; } }
    }
}

/// Computes the lowest tic that all players have sent input for.
///
/// In single-player mode this is simply `MAKETIC`. In networked play it is
/// `min(MAKETIC, RECVTIC)` to prevent the game from running ahead of
/// unconfirmed network tics. Corresponds to `GetLowTic` in `d_loop.c`.
///
/// # Safety
/// Must be called from the single-threaded game loop only. The function reads
/// the mutable globals `MAKETIC`, `RECVTIC`, `net_client_connected`, and
/// `drone` without synchronisation.
unsafe fn get_low_tic() -> c_int
{
    let mut lowtic = MAKETIC;
    if net_client_connected != 0 && (drone != 0 || RECVTIC < lowtic) { lowtic = RECVTIC; }
    lowtic
}

/// Applies the classic (pre-new-sync) network timing adjustment.
///
/// Increments the frame counter, finds the key player (lowest active index),
/// and if the local player is a follower: nudges `LASTTIME` downward when
/// ahead of received tics, records a frameskip entry, and sets `SKIPTICS = 1`
/// if all four recent frames indicate skipping. Corresponds to `OldNetSync`
/// in `d_loop.c`.
///
/// # Safety
/// Must be called from the single-threaded game loop only. The function reads
/// and writes the mutable globals `FRAMEON`, `LOCAL_PLAYERINGAME`,
/// `LOCALPLAYER`, `MAKETIC`, `RECVTIC`, `LASTTIME`, `FRAMESKIP`,
/// `OLDNETTICS`, and `SKIPTICS` without synchronisation.
unsafe fn old_net_sync()
{
    FRAMEON += 1;

    let mut keyplayer = -1;
    for i in 0..NET_MAXPLAYERS
    {
        if LOCAL_PLAYERINGAME[i] != 0
        {
            keyplayer = i as c_int;
            break;
        }
    }

    if keyplayer < 0 { return; }

    if LOCALPLAYER == keyplayer { /* the key player does not adapt */ }
    else
    {
        if MAKETIC <= RECVTIC { LASTTIME -= 1; }

        FRAMESKIP[(FRAMEON & 3) as usize] = (OLDNETTICS > RECVTIC) as c_int;
        OLDNETTICS = MAKETIC;

        if FRAMESKIP[0] != 0 && FRAMESKIP[1] != 0 && FRAMESKIP[2] != 0 && FRAMESKIP[3] != 0 { SKIPTICS = 1; }
    }
}

/// Returns `true` if there is at least one active player that can drive the
/// game forward.
///
/// In networked mode, checks `LOCAL_PLAYERINGAME`. In single-player mode
/// (drone == 0 and not net-connected) always returns `true`. Corresponds to
/// `PlayersInGame` in `d_loop.c`.
///
/// # Safety
/// Must be called from the single-threaded game loop only. The function reads
/// the mutable globals `net_client_connected`, `drone`, and
/// `LOCAL_PLAYERINGAME` without synchronisation.
unsafe fn players_in_game() -> bool
{
    let mut result = false;
    if net_client_connected != 0 { for i in 0..NET_MAXPLAYERS { result = result || LOCAL_PLAYERINGAME[i] != 0; } }
    if drone == 0 { result = true; }
    result
}

/// Clears one-shot fields from each player's ticcmd in a duplicated tic set.
///
/// When `ticdup > 1`, each group of `ticdup` tics shares the same `TiccmdSetT`
/// slot. Chat characters and `BT_SPECIAL` button events must be cleared after
/// the first tic of the group to avoid repeating them. `BT_SPECIAL` is the
/// high bit (`0x80`) of the `buttons` field. Corresponds to `TicdupSquash` in
/// `d_loop.c`.
///
/// # Safety
/// `set` must be a valid, non-null pointer to a `TiccmdSetT`.
unsafe fn ticdup_squash(set: *mut TiccmdSetT)
{
    const BT_SPECIAL: u8 = 128;
    for i in 0..NET_MAXPLAYERS
    {
        let cmd = &mut (*set).cmds[i];
        cmd.chatchar = 0;
        if cmd.buttons & BT_SPECIAL != 0 { cmd.buttons = 0; }
    }
}

/// Marks all non-local player slots as inactive in a tic set.
///
/// Used in single-player mode to ensure that only the local player's slot
/// drives the game tick. Corresponds to `SinglePlayerClear` in `d_loop.c`.
///
/// # Safety
/// `set` must be a valid, non-null pointer to a `TiccmdSetT`.
unsafe fn single_player_clear(set: *mut TiccmdSetT) { for i in 0..NET_MAXPLAYERS { if i != LOCALPLAYER as usize { (*set).ingame[i] = 0; } } }

/// Attempts to advance the game by as many tics as time permits.
///
/// This is the heart of the game loop. Each frame the renderer calls this
/// function, which:
/// 1. Computes the elapsed real tics since the last call.
/// 2. Builds new ticcmds (or calls `build_new_tic` once in singletics mode).
/// 3. Determines how many tics to run (`counts`), capped so the game does not
///    outrun available input or real time.
/// 4. Waits (with 1 ms sleeps) until enough input is available, returning
///    early if real time has advanced.
/// 5. For each count: copies `ingame` flags, calls `RunTic` via the loop
///    interface, increments `gametic`, and squashes one-shot fields for the
///    next `ticdup` sub-tic.
///
/// Called from `d_main.c`. Corresponds to `TryRunTics` in `d_loop.c`.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below
/// (`#[no_mangle]` drops with the rename; the symbol name set stays
/// byte-identical).
#[doc(alias = "TryRunTics")]
#[export_name = "TryRunTics"]
pub extern "C" fn try_run_tics()
{
    unsafe
    {
        let entertic = I_GetTime() / ticdup;
        static mut OLDENTERTICS: c_int = 0;
        let realtics = entertic - OLDENTERTICS;
        OLDENTERTICS = entertic;

        if singletics != 0 { build_new_tic(); }
        else { net_update(); }

        let mut lowtic = get_low_tic();
        let availabletics = lowtic - gametic / ticdup;

        let counts: c_int = if NEW_SYNC != 0 { availabletics }
        else
        {
            let mut c: c_int;
            if realtics < availabletics - 1 { c = realtics + 1; }
            else if realtics < availabletics { c = realtics; }
            else { c = availabletics; }

            if c < 1 { c = 1; }

            if net_client_connected != 0 { old_net_sync(); }

            c
        };

        let mut counts = if counts < 1 { 1 } else { counts };

        // Frame/pump cap (woom24 F1 M1 policy — see `pump_tic_cap`). The
        // vanilla path leaves the cap at 0 and is untouched.
        if pump_tic_cap > 0 && counts > pump_tic_cap { counts = pump_tic_cap; }

        while !players_in_game() || lowtic < gametic / ticdup + counts
        {
            net_update();
            lowtic = get_low_tic();

            if lowtic < gametic / ticdup { i_error!("TryRunTics: lowtic < gametic"); }

            // Still no tics to run? Sleep until some are available. The give
            // up is gated on still being short AND on MAX_NETGAME_STALL_TICS
            // stall tics having passed, exactly as in chocolate d_loop.c; the
            // previous port build returned after a single tic boundary even
            // when NetUpdate had just delivered the missing tic, which let
            // TryRunTics return having run ZERO tics and made the renderer
            // draw unsimulated state (//! the browser title-screen freeze
            // window - see task-8-report.md).
            if lowtic < gametic / ticdup + counts
            {
                if I_GetTime() / ticdup - entertic >= MAX_NETGAME_STALL_TICS { return; }

                I_Sleep(1);
            }
        }

        while counts > 0
        {
            counts -= 1;

            if !players_in_game() { return; }

            let set = &mut TICDATA[((gametic / ticdup) as usize) % BACKUPTICS] as *mut TiccmdSetT;

            if net_client_connected == 0 { single_player_clear(set); }

            for _ in 0..ticdup
            {
                if gametic / ticdup > lowtic { i_error!("gametic>lowtic"); }

                for i in 0..NET_MAXPLAYERS { LOCAL_PLAYERINGAME[i] = (*set).ingame[i]; }

                // Render-side interpolation latch (F1 M1): the oldleveltime
                // mirror + board aging happen BEFORE the tic's movement, the
                // capture walker right AFTER it (dsda-style centralized
                // once-per-tic capture, `r_fps.c:331-340`). Single call site;
                // the board is a render-side copy and never writes sim state.
                crate::doom::r_interp::begin_tic();

                let iface = &*LOOP_INTERFACE;
                iface.RunTic.unwrap()((*set).cmds.as_mut_ptr(), (*set).ingame.as_mut_ptr());
                gametic += 1;

                crate::doom::r_interp::end_tic_and_capture();

                ticdup_squash(set);
            }

            net_update();
        }
    }
}
