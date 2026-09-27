//! The net protocol ABI mirrors: `NetConnectDataT`
//! (`net_connect_data_t`), `NetGameSettingsT` (`net_gamesettings_t`),
//! and `LoopInterfaceT` (`loop_interface_t`), with the size constants
//! they embed and the compile-time size guards.
//!
//! Split out of pre-graduation `d_net.rs` (F10 wave C2); the types and
//! their docs moved verbatim. Field order IS the C ABI -- the size
//! guards below are the pre-move pins and rerun after the move, same
//! vectors.

#![allow(non_snake_case)]

use std::os::raw::c_int;

use crate::doom::d_player::TiccmdT;

/// Maximum number of players supported by the networking code.
///
/// This is the upper bound used by the net layer, which may be larger than the
/// per-game `MAXPLAYERS` constant (e.g., 4 for Doom). Corresponds to the C
/// macro `NET_MAXPLAYERS` in `net_defs.h`.
const NET_MAXPLAYERS: usize = 8;

/// Byte length of a SHA-1 message digest.
///
/// Used to size the `wad_sha1sum` and `deh_sha1sum` fields in
/// [`NetConnectDataT`]. Corresponds to `SHA1_DIGEST_SIZE` in `sha1.h`.
const SHA1_DIGEST_SIZE: usize = 20;

/// Data sent by a client to the server when establishing a net connection.
///
/// Maps to `net_connect_data_t` in `net_defs.h`. The fields describe the
/// client's game configuration so the server can verify compatibility before
/// starting a game. In this single-player port the struct is populated by
/// `init_connect_data` and passed to the stub `D_InitNetGame`.
///
/// Layout must match the C struct exactly; enforced by the size test in the
/// `tests` module.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetConnectDataT {
    /// Integer encoding of `GameMode_t` (shareware, registered, commercial, …).
    pub gamemode: c_int,
    /// Integer encoding of `GameMission_t` (doom, doom2, heretic, …).
    pub gamemission: c_int,
    /// Non-zero when recording a demo without `-longtics`; limits turn
    /// resolution to 8-bit (Vanilla compatibility).
    pub lowres_turn: c_int,
    /// Non-zero when this client is a drone (left/right screen in 3-screen
    /// setup). Set by `-left` or `-right` command-line parameters.
    pub drone: c_int,
    /// Maximum number of players for this game; normally `MAXPLAYERS`.
    pub max_players: c_int,
    /// Non-zero when the loaded IWAD is Freedoom (detected by the `FREEDOOM`
    /// lump name).
    pub is_freedoom: c_int,
    /// SHA-1 digest of the WAD directory, used for net-consistency checks.
    pub wad_sha1sum: [u8; SHA1_DIGEST_SIZE],
    /// SHA-1 digest of any loaded DeHackEd patches. Always zeroed in this
    /// port; the C original called `DEH_Checksum` (guarded by `#if ORIGCODE`).
    pub deh_sha1sum: [u8; SHA1_DIGEST_SIZE],
    /// Player class (used by Hexen). Always 0 for Doom/Heretic.
    pub player_class: c_int,
}

/// Session-wide game settings exchanged between the server and all clients.
///
/// Maps to `net_gamesettings_t` in `net_defs.h`. `save_game_settings` copies
/// global state into this struct before calling `D_StartNetGame`; afterwards
/// `load_game_settings` applies the (possibly server-modified) values back to
/// the globals.
///
/// Layout must match the C struct exactly; enforced by the size test in the
/// `tests` module.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetGameSettingsT {
    /// Number of game tics to duplicate per network packet (for lag
    /// compensation). Always 1 in single-player.
    pub ticdup: c_int,
    /// Number of extra tics to send ahead of time for smoother play.
    pub extratics: c_int,
    /// Deathmatch mode: 0 = cooperative, 1 = deathmatch, 2 = altdeath.
    pub deathmatch: c_int,
    /// Starting episode number (Doom/Heretic episode-based games).
    pub episode: c_int,
    /// Non-zero when monsters are disabled (`-nomonsters`).
    pub nomonsters: c_int,
    /// Non-zero when fast monsters are enabled (`-fast`).
    pub fast_monsters: c_int,
    /// Non-zero when monsters respawn (`-respawn`).
    pub respawn_monsters: c_int,
    /// Starting map number within the episode.
    pub map: c_int,
    /// Starting skill level (0 = baby … 4 = nightmare).
    pub skill: c_int,
    /// `GameVersion_t` integer identifying which executable to emulate.
    pub gameversion: c_int,
    /// Non-zero when turn resolution is limited to 8-bit (Vanilla demo compat).
    pub lowres_turn: c_int,
    /// Non-zero when the new network sync protocol is in use.
    pub new_sync: c_int,
    /// Per-level time limit in minutes for deathmatch (0 = no limit).
    pub timelimit: c_int,
    /// Save-game slot to load at startup (-1 = no load).
    pub loadgame: c_int,
    /// Random seed (Strife only).
    pub random: c_int,
    /// Number of human players in this session.
    pub num_players: c_int,
    /// Index of the local player (0-based) within the player array.
    pub consoleplayer: c_int,
    /// Per-player class array, indexed by player number (Hexen only).
    pub player_classes: [c_int; NET_MAXPLAYERS],
}

/// Callback table registered with the game loop layer.
///
/// Maps to `loop_interface_t` in `d_loop.h`. The loop layer (`d_loop.c`) calls
/// these function pointers at the appropriate points during each tic. All
/// fields are `Option<unsafe extern "C" fn(…)>` because the C struct uses raw
/// function pointers that may theoretically be null, though in practice all
/// four are always set before use.
///
/// Layout must match the C struct exactly; enforced by the size test in the
/// `tests` module.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoopInterfaceT {
    /// Called once per tic to drain the event queue. Bound to `D_ProcessEvents`.
    pub ProcessEvents: Option<unsafe extern "C" fn()>,
    /// Called to build a new [`TiccmdT`] from current input state. Bound to
    /// `G_BuildTiccmd`. The second argument is `maketic`, the tic number being
    /// constructed.
    pub BuildTiccmd: Option<unsafe extern "C" fn(*mut TiccmdT, c_int)>,
    /// Called to advance the game by one tic with the given player commands.
    /// The `ingame` array has one boolean per player indicating whether that
    /// player is still connected. Bound to the Rust `run_tic` function.
    pub RunTic: Option<unsafe extern "C" fn(*mut TiccmdT, *mut c_int)>,
    /// Called once per tic to run the menu system. Bound to `M_Ticker`.
    pub RunMenu: Option<unsafe extern "C" fn()>,
}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const NET_CONNECT_DATA_T_SIZEOF: usize = 68;
    const NET_GAMESETTINGS_T_SIZEOF: usize = 100;
    const LOOP_INTERFACE_T_SIZEOF: usize = 32;

    #[test]
    fn net_connect_data_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<NetConnectDataT>(),
            NET_CONNECT_DATA_T_SIZEOF,
            "NetConnectDataT size mismatch: Rust={}, expected={}",
            std::mem::size_of::<NetConnectDataT>(),
            NET_CONNECT_DATA_T_SIZEOF,
        );
    }

    #[test]
    fn net_gamesettings_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<NetGameSettingsT>(),
            NET_GAMESETTINGS_T_SIZEOF,
            "NetGameSettingsT size mismatch: Rust={}, expected={}",
            std::mem::size_of::<NetGameSettingsT>(),
            NET_GAMESETTINGS_T_SIZEOF,
        );
    }

    #[test]
    fn loop_interface_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<LoopInterfaceT>(),
            LOOP_INTERFACE_T_SIZEOF,
            "LoopInterfaceT size mismatch: Rust={}, expected={}",
            std::mem::size_of::<LoopInterfaceT>(),
            LOOP_INTERFACE_T_SIZEOF,
        );
    }
}
