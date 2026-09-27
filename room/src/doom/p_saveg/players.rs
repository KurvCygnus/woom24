//! The player-record archive/unarchive pair: all active players, 4-byte
//! aligned, in slot order. `unarchive_players` nulls `mo`/`message`/
//! `attacker` for the load path to rebuild (the thinkers pass re-links
//! `mo`).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use crate::doom::d_player::{players, MAXPLAYERS};

use super::records::{read_player_record, write_player_record};
use super::stream::{read_padding, write_padding};

// `playeringame` is p_saveg's extern-by-symbol declaration of g_game's
// static (mod.rs wiring block).
use super::playeringame;

/// Serializes all active players to `save_stream`.
///
/// Iterates over `players[0..MAXPLAYERS]`; skips slots where
/// `playeringame[i] == 0`. Each active player record is 4-byte aligned
/// (`saveg_write_pad`) then written by `saveg_write_player_t`.
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `save_stream` must be an open, writable `FILE *` with sufficient capacity.
/// The `players` array and `playeringame` flags must be fully initialized for
/// all `MAXPLAYERS` slots. Must be called after `P_WriteSaveGameHeader` and
/// before `P_WriteSaveGameEOF`.
#[doc(alias = "P_ArchivePlayers")]
#[export_name = "P_ArchivePlayers"]
pub unsafe extern "C" fn archive_players()
{
    for i in 0..MAXPLAYERS
    {
        if playeringame[i] == 0
        {
            continue;
        }
        write_padding();
        write_player_record(&players[i]);
    }
}

/// Deserializes all active players from `save_stream`.
///
/// For each active player slot, aligns the stream (`saveg_read_pad`) then
/// reads the player record. After reading, `mo`, `message`, and `attacker` are
/// reset to null pointers; they will be restored when thinkers are unarchived
/// by `P_UnArchiveThinkers`.
/// Called by `g_game.c` (`G_DoLoadGame`).
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned at the byte
/// sequence written by `P_ArchivePlayers`. `playeringame` must already have
/// been populated by `P_ReadSaveGameHeader` so the active-slot bitmask is
/// correct. After this call, `players[i].mo`, `.message`, and `.attacker` are
/// null and must not be dereferenced until thinkers are unarchived.
#[doc(alias = "P_UnArchivePlayers")]
#[export_name = "P_UnArchivePlayers"]
pub unsafe extern "C" fn unarchive_players()
{
    for i in 0..MAXPLAYERS
    {
        if playeringame[i] == 0
        {
            continue;
        }
        read_padding();
        read_player_record(&mut players[i]);

        // will be set when unarc thinker
        players[i].mo = std::ptr::null_mut();
        players[i].message = std::ptr::null_mut();
        players[i].attacker = std::ptr::null_mut();
    }
}
