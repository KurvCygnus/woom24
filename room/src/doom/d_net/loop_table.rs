//! The per-tic callback side of the net layer: `run_tic` (the loop
//! layer's per-tic entry), `handle_player_quit`, and the
//! `DOOM_LOOP_INTERFACE` callback table.
//!
//! Split out of pre-graduation `d_net.rs` (F10 wave C2); the bodies
//! moved verbatim (Allman reformat only) -- see the module root for the
//! mapping table and the deterministic ordering constraints. `run_tic`'s
//! latch order is the demo-observable contract: quit scan, then the
//! `netcmds` store, then the `advancedemo` check, then `G_Ticker`.

use std::ffi::c_char;
use std::os::raw::c_int;

use crate::doom::d_player::{consoleplayer, players, TiccmdT, MAXPLAYERS};

use super::{
    demorecording, demoplayback, netcmds, playeringame, G_BuildTiccmd, G_CheckDemoStatus,
    G_Ticker, D_DoAdvanceDemo, D_ProcessEvents, M_StringCopy, M_Ticker,
};
use super::net_glue::{deh_string, EXIT_MSG};
use super::protocol_types::LoopInterfaceT;

/// Notify the game that a player has disconnected.
///
/// Formats a "Player N left the game" message into a static 80-byte buffer,
/// marks `playeringame[player_idx]` as false, sets the console player's
/// message pointer to that buffer, and (if demo recording is active) stops the
/// demo via `G_CheckDemoStatus`.
///
/// The message template [`EXIT_MSG`] is DeHackEd-patchable in the original C
/// via `DEH_String`; the Rust `deh_string` stub is a no-op, so the message is
/// always the default string. The player number is encoded by incrementing
/// `exitmsg[7]` (the `'1'` digit in `"Player 1 left the game"`).
///
/// Corresponds to the static `PlayerQuitGame(player_t *player)` in `d_net.c`,
/// where `player_num = player - players` replaces the explicit index argument.
///
/// # Safety
///
/// Caller must ensure `player_idx < MAXPLAYERS` and that the global state
/// (`players`, `playeringame`, `consoleplayer`, `demorecording`) is valid.
unsafe fn handle_player_quit(player_idx: usize)
{
    /// Capacity of [`EXITMSG`], named so `M_StringCopy` does not have to take
    /// a shared reference to the `static mut` to read its length.
    const EXITMSG_LEN: usize = 80;

    /// 80-byte scratch buffer holding the formatted "Player N left the game"
    /// message; its address is passed to the console-player's message pointer.
    static mut EXITMSG: [c_char; EXITMSG_LEN] = [0; EXITMSG_LEN];

    M_StringCopy(
        std::ptr::addr_of_mut!(EXITMSG[0]),
        deh_string(EXIT_MSG.as_ptr() as *const c_char),
        EXITMSG_LEN,
    );

    EXITMSG[7] += player_idx as c_char;

    playeringame[player_idx] = 0;
    (*std::ptr::addr_of_mut!(players[0]).offset(consoleplayer as isize)).message =
        std::ptr::addr_of_mut!(EXITMSG[0]);

    if demorecording != 0
    {
        G_CheckDemoStatus();
    }
}

/// Advance the game by one tic using the provided player commands.
///
/// Called by the loop layer once per tic. Checks whether any player has
/// dropped (present in `playeringame` but absent from `ingame`), notifying via
/// `handle_player_quit` when that happens - but only when not in demo playback,
/// mirroring the original guard. Then stores `cmds` in `netcmds`, optionally
/// advances the demo sequence via `D_DoAdvanceDemo`, and runs one game tic via
/// `G_Ticker`.
///
/// Registered as the `RunTic` slot in `DOOM_LOOP_INTERFACE`. Corresponds to
/// the static `RunTic(ticcmd_t *cmds, boolean *ingame)` in `d_net.c`.
///
/// # Safety
///
/// `cmds` must point to a valid array of at least `MAXPLAYERS` [`TiccmdT`]
/// values. `ingame` must point to a valid array of at least `MAXPLAYERS`
/// `c_int` values. All global game state must be consistently initialised.
#[doc(alias = "RunTic")]
unsafe extern "C" fn run_tic(cmds: *mut TiccmdT, ingame: *mut c_int)
{
    for i in 0..MAXPLAYERS
    {
        if demoplayback == 0 && playeringame[i] != 0 && *ingame.add(i) == 0
        {
            handle_player_quit(i);
        }
    }

    netcmds = cmds;

    extern "C" {
        /// Flag set by the attract-mode sequencer when the demo loop should
        /// advance to the next entry (next demo, intermission screen, etc.).
        /// Defined in `d_main.c`; consumed here to call `D_DoAdvanceDemo`.
        static mut advancedemo: c_int;
    }
    if advancedemo != 0
    {
        D_DoAdvanceDemo();
    }

    G_Ticker();
}

/// The loop callback table registered with `d_loop.c`.
///
/// The static table wires the four per-tic callbacks that the loop layer
/// (`register_loop_callbacks`) needs. It must remain `mut` because the C ABI
/// requires a non-const pointer in `D_RegisterLoopCallbacks`. Corresponds to
/// `doom_loop_interface` in `d_net.c`.
pub(super) static mut DOOM_LOOP_INTERFACE: LoopInterfaceT = LoopInterfaceT {
    ProcessEvents: Some(D_ProcessEvents),
    BuildTiccmd: Some(G_BuildTiccmd),
    RunTic: Some(run_tic),
    RunMenu: Some(M_Ticker),
};
