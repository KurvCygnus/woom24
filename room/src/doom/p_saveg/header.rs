//! The save-game header (description string, version string, session
//! scalars, player mask, `leveltime`) and the end-of-file marker byte,
//! as written and read by `g_game` around the archive/unarchive calls.
//! The pure codec pieces of the header live in `dtmc`
//! (`leveltime_pack3`/`unpack3`, `version_bytes`); the byte order and
//! field order here are the on-disk format.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int};

use crate::doom::c_ffi::{SAVEGAME_EOF, VERSIONSIZE};
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::p_tick::leveltime;

use super::dtmc::{leveltime_pack3, leveltime_unpack3, version_bytes};
use super::stream::{read_byte, write_byte};

// `gameskill` / `gameepisode` / `gamemap` / `playeringame` are p_saveg's
// extern-by-symbol declarations of g_game's statics (mod.rs wiring block).
use super::{gameepisode, gamemap, gameskill, playeringame, G_VanillaVersionCode};

/// Maximum length (in bytes, including NUL) of the human-readable save-game
/// description string written at the start of every save file.
/// Corresponds to `SAVESTRINGSIZE` in the C source.
const SAVESTRINGSIZE: usize = 24;

/// Writes the save-game file header to the open `save_stream`.
///
/// The header layout (bytes written in order):
/// 1. `description` string, NUL-padded to `SAVESTRINGSIZE` (24) bytes.
/// 2. Version string `"version N"`, NUL-padded to `VERSIONSIZE` (16) bytes.
/// 3. One byte each: `gameskill`, `gameepisode`, `gamemap`.
/// 4. `MAXPLAYERS` bytes: `playeringame[i]` flags.
/// 5. Three bytes: `leveltime` encoded big-endian as
///    `[bits 23:16, bits 15:8, bits 7:0]` (`dtmc::leveltime_pack3`).
///
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `description` must be a valid, NUL-terminated C string. `save_stream` must
/// be an open, writable `FILE *` with enough capacity to hold the header bytes.
/// The global state `gameskill`, `gameepisode`, `gamemap`, `playeringame`, and
/// `leveltime` must have been set to valid values before this call.
#[doc(alias = "P_WriteSaveGameHeader")]
#[export_name = "P_WriteSaveGameHeader"]
pub extern "C" fn write_save_game_header(description: *const c_char)
{
    unsafe
    {
        let desc = std::ffi::CStr::from_ptr(description);
        let desc_bytes = desc.to_bytes();

        // Write description (padded to SAVESTRINGSIZE)
        for i in 0..SAVESTRINGSIZE
        {
            if i < desc_bytes.len()
            {
                write_byte(desc_bytes[i]);
            }
            else
            {
                write_byte(0);
            }
        }

        // Write version string (VERSIONSIZE bytes): the NUL-padded
        // `"version N\0"` buffer from dtmc::version_bytes.
        for byte in version_bytes(G_VanillaVersionCode())
        {
            write_byte(byte);
        }

        // Write skill, episode, map
        write_byte(gameskill as u8);
        write_byte(gameepisode as u8);
        write_byte(gamemap as u8);

        // Write playeringame
        for i in 0..MAXPLAYERS
        {
            write_byte(playeringame[i] as u8);
        }

        // Write leveltime (3 bytes, big-endian)
        for byte in leveltime_pack3(leveltime as u32)
        {
            write_byte(byte);
        }
    }
}

/// Reads and validates the save-game header from `save_stream`.
///
/// Returns `1` (true) on success, `0` (false) if the version string does not
/// match `G_VanillaVersionCode()`. On success, `gameskill`, `gameepisode`,
/// `gamemap`, `playeringame`, and `leveltime` are updated from the stream.
///
/// Called by `g_game.c` (`G_DoLoadGame`).
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned at the start of
/// a save file written by `P_WriteSaveGameHeader`. On success, `gameskill`,
/// `gameepisode`, `gamemap`, `playeringame`, and `leveltime` are overwritten
/// with values from the stream.
#[doc(alias = "P_ReadSaveGameHeader")]
#[export_name = "P_ReadSaveGameHeader"]
pub extern "C" fn read_save_game_header() -> c_int
{
    unsafe
    {
        // Skip description (SAVESTRINGSIZE bytes)
        for _ in 0..SAVESTRINGSIZE
        {
            read_byte();
        }

        // Read version string
        let mut read_vcheck = [0u8; VERSIONSIZE];
        for i in 0..VERSIONSIZE
        {
            read_vcheck[i] = read_byte();
        }

        // Compare version against the NUL-padded expected buffer
        // (dtmc::version_bytes); the compare itself stays at the call site.
        if read_vcheck != version_bytes(G_VanillaVersionCode())
        {
            return 0; // bad version
        }

        // Read skill, episode, map
        gameskill = read_byte() as c_int;
        gameepisode = read_byte() as c_int;
        gamemap = read_byte() as c_int;

        // Read playeringame
        for i in 0..MAXPLAYERS
        {
            playeringame[i] = read_byte() as c_int;
        }

        // Read leveltime (3 bytes, big-endian)
        leveltime = leveltime_unpack3([read_byte(), read_byte(), read_byte()]) as c_int;

        1 // success
    }
}

/// Reads the end-of-file marker byte from `save_stream`.
///
/// Returns `1` if the byte equals `SAVEGAME_EOF` (`0x1d`), otherwise `0`.
/// A mismatch indicates a truncated or corrupt save file.
/// Called by `g_game.c` after all game state has been unarchived.
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned immediately
/// after the last unarchived byte, i.e. where `P_WriteSaveGameEOF` wrote its
/// marker. Reading from an invalid or exhausted stream is undefined behaviour.
#[doc(alias = "P_ReadSaveGameEOF")]
#[export_name = "P_ReadSaveGameEOF"]
pub extern "C" fn read_save_game_eof() -> c_int
{
    unsafe
    {
        let value = read_byte();
        if value == SAVEGAME_EOF
        {
            1
        }
        else
        {
            0
        }
    }
}

/// Writes the end-of-file marker byte (`SAVEGAME_EOF` = `0x1d`) to
/// `save_stream`.
///
/// Written after all game state has been archived; checked by
/// `P_ReadSaveGameEOF` on load to detect truncation.
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `save_stream` must be an open, writable `FILE *`. This must be called after
/// all archive functions have finished writing so that the marker byte appears
/// at the correct position for `P_ReadSaveGameEOF` to validate.
#[doc(alias = "P_WriteSaveGameEOF")]
#[export_name = "P_WriteSaveGameEOF"]
pub extern "C" fn write_save_game_eof()
{
    unsafe
    {
        write_byte(SAVEGAME_EOF);
    }
}

#[cfg(test)]
mod tests
{
    // Tests moved with their subjects (F10 wave B4b); they are pure
    // constant checks and need no shared-state lock.

    use super::SAVESTRINGSIZE;
    use crate::doom::c_ffi::{SAVEGAME_EOF, VERSIONSIZE};

    const SAVEGAME_EOF_VAL: u8 = 0x1d;
    const VERSIONSIZE_VAL: usize = 16;
    const SAVESTRINGSIZE_VAL: usize = 24;

    #[test]
    fn constants_match_c()
    {
        assert_eq!(SAVEGAME_EOF, SAVEGAME_EOF_VAL);
        assert_eq!(VERSIONSIZE, VERSIONSIZE_VAL);
        assert_eq!(SAVESTRINGSIZE, SAVESTRINGSIZE_VAL);
    }

    #[test]
    fn eof_marker_value()
    {
        assert_eq!(SAVEGAME_EOF, 0x1d);
    }

    #[test]
    fn version_string_length()
    {
        assert_eq!(VERSIONSIZE, 16);
        let test_str = "version 110"; // typical version string
        assert!(test_str.len() < VERSIONSIZE);
    }

    #[test]
    fn save_string_size()
    {
        assert_eq!(SAVESTRINGSIZE, 24);
    }
}
