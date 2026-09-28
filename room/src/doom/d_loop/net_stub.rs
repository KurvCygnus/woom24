//! The single-player net stubs and loop plumbing:
//! `handle_disconnected`, `receive_tic`, `start_game_loop`,
//! `start_net_game`, `init_net_game`, `quit_net_game`, and
//! `register_loop_callbacks`.
//!
//! Split out of pre-graduation `d_loop.rs` (F10 wave C2); the bodies
//! moved verbatim (Allman reformat only) -- see the module root for the
//! mapping table and the deterministic ordering constraints. This is the
//! `FEATURE_MULTIPLAYER`-compiled-out half of `d_loop.c`: every net
//! entry point here is a stub or a null-net branch.

use std::os::raw::c_int;

use crate::doom::d_net::{LoopInterfaceT, NetConnectDataT, NetGameSettingsT};
use crate::doom::d_player::TiccmdT;
use crate::i_error;
use crate::types::Boolean;

use super::{
    drone, ticdup, BACKUPTICS, LASTTIME, LOCALPLAYER, LOCAL_PLAYERINGAME, LOOP_INTERFACE,
    NET_MAXPLAYERS, NEW_SYNC, PLAYER_CLASS, RECVTIC, TICDATA, I_AtExit,
};

/// Handles a network disconnection event.
///
/// In drone mode, disconnection is fatal and calls `I_Error`. In normal play
/// the C original prints a diagnostic message; this port omits the `printf`
/// but preserves the drone abort. Corresponds to `D_Disconnected` in
/// `d_loop.c`.
///
/// # Safety
/// Must be called from the single-threaded game loop only. The function reads
/// the mutable global `drone` without synchronisation.
unsafe fn handle_disconnected() { if drone != 0 { i_error!("Disconnected from server in drone mode."); } }

/// Receives a completed set of ticcmds from the network layer for `RECVTIC`.
///
/// If both `ticcmds` and `players_mask` are null, the connection has been
/// lost and `handle_disconnected` is called. Otherwise, the supplied commands
/// are written into `TICDATA`, skipping the local player's slot (to preserve
/// locally-generated input). Increments `RECVTIC` on success.
///
/// Exported for C callers under the pinned upstream symbol; the
/// `FEATURE_MULTIPLAYER` net client calls this function when a server packet
/// is processed.
///
/// # Safety
/// When non-null, `ticcmds` must point to an array of at least
/// `NET_MAXPLAYERS` `TiccmdT` elements, and `players_mask` must point to an
/// array of at least `NET_MAXPLAYERS` `c_int` values.
#[doc(alias = "D_ReceiveTic")]
#[export_name = "D_ReceiveTic"]
pub extern "C" fn receive_tic(ticcmds: *mut TiccmdT, players_mask: *mut c_int)
{
    unsafe
    {
        if ticcmds.is_null() && players_mask.is_null()
        {
            handle_disconnected();
            return;
        }

        for i in 0..NET_MAXPLAYERS
        {
            if drone == 0 && i == LOCALPLAYER as usize { /* This is us.  Don't overwrite it. */ }
            else
            {
                TICDATA[(RECVTIC as usize) % BACKUPTICS].cmds[i] = *ticcmds.add(i);
                TICDATA[(RECVTIC as usize) % BACKUPTICS].ingame[i] = *players_mask.add(i);
            }
        }

        RECVTIC += 1;
    }
}

/// Initialises the loop timer to the current adjusted time.
///
/// Must be called after the screen is set up but before the first call to
/// `try_run_tics`, so that the initial `newtics` calculation in `net_update`
/// is `0` rather than an arbitrary large value. Called from `d_main.c`.
/// Corresponds to `D_StartGameLoop` in `d_loop.c`.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below
/// (`#[no_mangle]` drops with the rename; the symbol name set stays
/// byte-identical).
#[doc(alias = "D_StartGameLoop")]
#[export_name = "D_StartGameLoop"]
pub extern "C" fn start_game_loop() { unsafe { LASTTIME = super::tic_pump::get_adjusted_time() / ticdup; } }

/// Configures game settings for a single-player session and updates globals.
///
/// Sets the console player to `0`, forces `num_players = 1`, disables new
/// sync, sets `extratics = 1`, and sets `ticdup = 1`. Copies `ticdup` and
/// `new_sync` back into the module-level statics used by the loop.
///
/// The `callback` parameter (used in networked play to report lobby readiness)
/// is ignored. Called from `d_net.c`.
///
/// # Safety
/// `settings` must be a valid, non-null pointer to a `NetGameSettingsT`.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below;
/// d_net's extern block (`D_StartNetGame`) links this symbol at every
/// boot. The `_callback: *const ()` parameter type is a carried oddity
/// (upstream passes a fn pointer).
#[doc(alias = "D_StartNetGame")]
#[export_name = "D_StartNetGame"]
pub extern "C" fn start_net_game(settings: *mut NetGameSettingsT, _callback: *const ())
{
    unsafe
    {
        (*settings).consoleplayer = 0;
        (*settings).num_players = 1;
        (*settings).player_classes[0] = PLAYER_CLASS;
        (*settings).new_sync = 0;
        (*settings).extratics = 1;
        (*settings).ticdup = 1;

        // Set the local player and playeringame[] values (chocolate
        // d_loop.c D_StartNetGame parity; both stay constant in this
        // single-player build but keep the upstream data flow).
        LOCALPLAYER = (*settings).consoleplayer;
        for i in 0..NET_MAXPLAYERS { LOCAL_PLAYERINGAME[i] = (i < (*settings).num_players as usize) as c_int; }

        ticdup = (*settings).ticdup;
        NEW_SYNC = (*settings).new_sync;

        // Chocolate rejects non-positive ticdup outright (d_loop.c
        // "D_StartNetGame: invalid ticdup value") instead of letting the
        // tic loop divide by zero later.
        if ticdup < 1
        {
            i_error!(
                "D_StartNetGame: invalid ticdup value {}",
                std::ptr::addr_of!(ticdup).read()
            );
        }
    }
}

/// Initialises networking and registers `quit_net_game` as an at-exit
/// handler.
///
/// In the full Chocolate Doom build, this function connects to a multiplayer
/// server when `-connect` / `-server` / `-autojoin` flags are present. In
/// this single-player port, all multiplayer logic is compiled out; the
/// function only reads the player class from `connect_data` and registers the
/// exit handler. Always returns `0` (false = not connected to a server).
/// Called from `d_net.c`.
///
/// # Safety
/// `connect_data` must be a valid, non-null pointer to a `NetConnectDataT`.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below;
/// d_net's extern block (`D_InitNetGame`) links this symbol at every
/// boot.
#[doc(alias = "D_InitNetGame")]
#[export_name = "D_InitNetGame"]
pub extern "C" fn init_net_game(connect_data: *mut NetConnectDataT) -> c_int
{
    unsafe
    {
        I_AtExit(quit_net_game, Boolean::TRUE);
        PLAYER_CLASS = (*connect_data).player_class;
    }
    0 // false
}

/// Shuts down networking before the process exits.
///
/// In the full build this disconnects the net client and shuts down the
/// server. In this single-player port the function is a no-op. It is
/// registered via `I_AtExit` from [`init_net_game`] and called automatically
/// on both clean exit and error exit.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below
/// (`#[no_mangle]` drops with the rename; the symbol name set stays
/// byte-identical).
#[doc(alias = "D_QuitNetGame")]
#[export_name = "D_QuitNetGame"]
pub extern "C" fn quit_net_game() { /* No-op when FEATURE_MULTIPLAYER is not defined. */ }

/// Registers the loop callback table used by all main-loop operations.
///
/// Must be called before `start_game_loop` or `try_run_tics`. The `i`
/// pointer is stored directly; the caller must ensure the pointed-to
/// `LoopInterfaceT` remains valid for the lifetime of the game loop. Called
/// from `d_net.c`.
///
/// # Safety
/// `i` must be a valid, non-null pointer to a fully-initialised
/// `LoopInterfaceT` with all four function pointers set to non-null values.
///
/// The pre-move wasm/extern symbol is kept with `#[export_name]` below;
/// d_net's extern block (`D_RegisterLoopCallbacks`) links this symbol at
/// every boot.
#[doc(alias = "D_RegisterLoopCallbacks")]
#[export_name = "D_RegisterLoopCallbacks"]
pub extern "C" fn register_loop_callbacks(i: *mut LoopInterfaceT) { unsafe { LOOP_INTERFACE = i; } }

#[cfg(test)]
mod tests
{
    use crate::doom::d_loop::{LOCALPLAYER, LOCAL_PLAYERINGAME, NET_MAXPLAYERS, ticdup};
    use crate::doom::d_net::NetGameSettingsT;

    use super::start_net_game;

    /// Pins the chocolate d_loop.c D_StartNetGame single-player parity: the
    /// stub always reports ticdup=1 (never 0 - the tic loop divides by it),
    /// console player 0, and a filled local_playeringame[] matching
    /// num_players.
    #[test]
    fn d_start_net_game_sets_single_player_parity()
    {
        // Serialise against c_tests/d_loop_c.rs (`ticdup_default_zero`
        // asserts `c_ffi::ticdup == 0` in this same test binary): the
        // parity stub below writes `ticdup`/`LOCALPLAYER`/
        // `LOCAL_PLAYERINGAME`, so both sides' exact-value asserts must
        // hold the shared engine-statics test lock (B1a p_user
        // precedent; parallel unit-test threads can otherwise interleave
        // the write with the sibling's read).
        let _g = crate::doom::violations::ENGINE_STATICS_TEST_LOCK.
            lock().
            unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut settings: NetGameSettingsT = std::mem::zeroed();
            start_net_game(&mut settings, std::ptr::null());

            assert_eq!(settings.ticdup, 1, "stub must force ticdup=1");
            assert_eq!(settings.num_players, 1);
            assert_eq!(settings.consoleplayer, 0);
            assert_eq!(settings.new_sync, 0);
            assert_eq!(settings.extratics, 1);

            assert_eq!(
                std::ptr::addr_of!(ticdup).read(),
                1,
                "ticdup must never be left at 0"
            );
            assert_eq!(LOCALPLAYER, 0);
            assert_eq!(LOCAL_PLAYERINGAME[0], 1, "console player must be in game");
            for i in 1..NET_MAXPLAYERS { assert_eq!(LOCAL_PLAYERINGAME[i], 0, "slot {} must be empty", i); }
        }
    }
}
