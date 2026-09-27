//! Boot-time settings exchange: `init_connect_data`,
//! `save_game_settings`, `load_game_settings`, the
//! `connect_net_game` / `check_net_game` boot entry points, the
//! `deh_string` no-op stub, and the `EXIT_MSG` message template.
//!
//! Split out of pre-graduation `d_net.rs` (F10 wave C2); the bodies
//! moved verbatim (Allman reformat only) -- see the module root for the
//! mapping table and the deterministic ordering constraints.

use std::ffi::c_char;
use std::os::raw::c_int;

use crate::doom::d_player::{consoleplayer, MAXPLAYERS};

use super::{
    autostart, deathmatch, fastparm, gamemode, gamemission, gameversion, lowres_turn,
    netgame, nomonsters, playeringame, respawnparm, startepisode, startloadgame, startmap,
    startskill, timelimit, viewangleoffset, D_InitNetGame, D_RegisterLoopCallbacks,
    D_StartNetGame, M_CheckParm, W_CheckNumForName, W_Checksum, NetConnectDataT,
    NetGameSettingsT,
};

/// Apply settings from a `NetGameSettingsT` to the global game variables.
///
/// Called after `D_StartNetGame` returns so that the (possibly server-adjusted)
/// settings are visible to the rest of the engine. Only the fields that have
/// corresponding globals are applied; `ticdup`, `extratics`, `new_sync`, and
/// `random` are intentionally ignored in this single-player build.
///
/// In the original C a `printf` diagnostic is emitted when `lowres_turn` is
/// non-zero; this port omits that print.
///
/// Corresponds to the static `LoadGameSettings(net_gamesettings_t *settings)`
/// in `d_net.c`.
///
/// # Safety
///
/// `settings` must point to a fully initialised `NetGameSettingsT`. All
/// destination globals must be safely mutable at the call site.
unsafe fn load_game_settings(settings: *mut NetGameSettingsT)
{
    deathmatch = (*settings).deathmatch;
    startepisode = (*settings).episode;
    startmap = (*settings).map;
    startskill = (*settings).skill;
    startloadgame = (*settings).loadgame;
    lowres_turn = (*settings).lowres_turn;
    nomonsters = (*settings).nomonsters;
    fastparm = (*settings).fast_monsters;
    respawnparm = (*settings).respawn_monsters;
    timelimit = (*settings).timelimit;
    consoleplayer = (*settings).consoleplayer;

    for i in 0..MAXPLAYERS
    {
        playeringame[i] = if i < (*settings).num_players as usize
        {
            1
        }
        else
        {
            0
        };
    }
}

/// Snapshot the global game variables into a `NetGameSettingsT`.
///
/// Called before `D_StartNetGame` so that the server (or stub) receives the
/// locally-configured session parameters. The `lowres_turn` field is derived
/// from the command-line: it is set when `-record` is active but `-longtics`
/// is not, matching Vanilla demo resolution.
///
/// Corresponds to the static `SaveGameSettings(net_gamesettings_t *settings)`
/// in `d_net.c`.
///
/// # Safety
///
/// `settings` must point to a writable `NetGameSettingsT`. All source
/// globals must be in a valid state.
unsafe fn save_game_settings(settings: *mut NetGameSettingsT)
{
    (*settings).deathmatch = deathmatch;
    (*settings).episode = startepisode;
    (*settings).map = startmap;
    (*settings).skill = startskill;
    (*settings).loadgame = startloadgame;
    (*settings).gameversion = gameversion;
    (*settings).nomonsters = nomonsters;
    (*settings).fast_monsters = fastparm;
    (*settings).respawn_monsters = respawnparm;
    (*settings).timelimit = timelimit;
    (*settings).lowres_turn =
        if M_CheckParm(c"-record".as_ptr()) > 0 && M_CheckParm(c"-longtics".as_ptr()) == 0
        {
            1
        }
        else
        {
            0
        };
}

/// Populate a `NetConnectDataT` from the current engine state and command line.
///
/// Fills in game mode/mission, drone flags, low-resolution turn mode, the WAD
/// SHA-1 checksum, and the Freedoom detection flag. The `deh_sha1sum` field is
/// left zeroed; in the original C it was filled by `DEH_Checksum`, which is
/// guarded by `#if ORIGCODE` in the vendor source.
///
/// The `-left` flag sets `viewangleoffset` to 90 degrees (ANG90 =
/// `0x40000000`) and marks the client as a drone; `-right` sets it to 270
/// degrees (ANG270 = `0xC0000000`).
///
/// Corresponds to the static `InitConnectData(net_connect_data_t *connect_data)`
/// in `d_net.c`.
///
/// # Safety
///
/// `connect_data` must point to a writable `NetConnectDataT`. Global state
/// (`gamemode`, `gamemission`, `viewangleoffset`) must be valid.
unsafe fn init_connect_data(connect_data: *mut NetConnectDataT)
{
    (*connect_data).max_players = MAXPLAYERS as c_int;
    (*connect_data).drone = 0;

    if M_CheckParm(c"-left".as_ptr()) > 0
    {
        viewangleoffset = 0x40000000u32 as c_int;
        (*connect_data).drone = 1;
    }

    if M_CheckParm(c"-right".as_ptr()) > 0
    {
        viewangleoffset = 0xC0000000u32 as c_int;
        (*connect_data).drone = 1;
    }

    (*connect_data).gamemode = gamemode;
    (*connect_data).gamemission = gamemission;

    (*connect_data).lowres_turn =
        if M_CheckParm(c"-record".as_ptr()) > 0 && M_CheckParm(c"-longtics".as_ptr()) == 0
        {
            1
        }
        else
        {
            0
        };

    W_Checksum((*connect_data).wad_sha1sum.as_mut_ptr());

    let name = b"FREEDOOM\0";
    (*connect_data).is_freedoom = if W_CheckNumForName(name.as_ptr() as *const c_char) >= 0
    {
        1
    }
    else
    {
        0
    };
}

/// Default "Player N left the game" message template.
///
/// The `'1'` at index 7 is incremented in `handle_player_quit` to encode the
/// player number. The string is NUL-terminated and matches the C literal
/// `"Player 1 left the game"` passed to `DEH_String` in `d_net.c`.
pub(super) static EXIT_MSG: [u8; 23] = *b"Player 1 left the game\0";

/// No-op DeHackEd string lookup stub.
///
/// In the original engine `DEH_String` looked up a string in the DeHackEd
/// patch database and returned the replacement if one was loaded. This port
/// does not implement DeHackEd, so the input pointer is returned unchanged.
/// Corresponds to `DEH_String` from `deh_main.h`.
#[doc(alias = "DEH_String")]
#[inline(always)]
pub(super) unsafe fn deh_string(s: *const c_char) -> *const c_char
{
    s
}

/// Initialise the network connection and determine whether a net game is active.
///
/// Builds connect data from the current engine state, passes it to the loop
/// layer's `D_InitNetGame`, and stores the result in `netgame`. If the
/// `-solo-net` command-line parameter is present, `netgame` is forced to true
/// so that the engine behaves as if in a single-player net game (useful for
/// playing back net-game demos).
///
/// Called from `D_DoomMain` during startup. Exported under the pinned
/// upstream symbol (`d_main.c` calls it directly, `d_main.rs:1818`).
/// Corresponds to `D_ConnectNetGame` in `d_net.c`.
///
//* The pre-move wasm/extern symbol is kept with `#[export_name]` below
//* (`#[no_mangle]` drops with the rename; the symbol name set stays
//* byte-identical).
#[doc(alias = "D_ConnectNetGame")]
#[export_name = "D_ConnectNetGame"]
pub extern "C" fn connect_net_game()
{
    unsafe
    {
        let mut connect_data: NetConnectDataT = std::mem::zeroed();
        init_connect_data(&mut connect_data);
        netgame = D_InitNetGame(&mut connect_data);

        if M_CheckParm(c"-solo-net".as_ptr()) > 0
        {
            netgame = 1;
        }
    }
}

/// Finalise net-game setup and synchronise settings with the loop layer.
///
/// If a net game is active, sets `autostart` so the game begins immediately
/// without waiting for a keypress. Registers `DOOM_LOOP_INTERFACE` with the
/// loop layer, then exchanges game settings via `save_game_settings` /
/// `D_StartNetGame` / `load_game_settings` so that both sides agree on map,
/// skill, player count, and so on.
///
/// The diagnostic `DEH_printf` calls present in the C original are omitted in
/// this port.
///
/// Called from `D_DoomMain` after `D_ConnectNetGame`. Exported under the
/// pinned upstream symbol (`d_main.c` calls it directly, `d_main.rs:1901`).
/// Corresponds to `D_CheckNetGame` in `d_net.c`.
///
//* The pre-move wasm/extern symbol is kept with `#[export_name]` below
//* (`#[no_mangle]` drops with the rename; the symbol name set stays
//* byte-identical).
#[doc(alias = "D_CheckNetGame")]
#[export_name = "D_CheckNetGame"]
pub extern "C" fn check_net_game()
{
    unsafe
    {
        if netgame != 0
        {
            autostart = 1;
        }

        D_RegisterLoopCallbacks(&raw mut super::loop_table::DOOM_LOOP_INTERFACE);

        let mut settings: NetGameSettingsT = std::mem::zeroed();
        save_game_settings(&mut settings);
        D_StartNetGame(&mut settings, std::ptr::null());
        load_game_settings(&mut settings);
    }
}
