//! Savegame entry points: `load_game` / `do_load_game` (the
//! `ga_loadgame` side) and `save_game` / `do_save_game` (the
//! `ga_savegame` side, including the temp-file-then-rename finalize
//! sequence).
//!
//! Adjudication (F10 wave C3): `do_load_game` is dtmc whole-body (it
//! re-enters `init_new` -- RNG reset -- and unarchives the entire hashed
//! state); `save_game` / `do_save_game` / `load_game` are glue (a
//! write-only side-channel -- nothing on the demo stream reads a save
//! file; the F9 `save_load_roundtrip` golden pins the file-I/O order,
//! see `shells/web/src/wasm_vfs.rs`). The bodies moved verbatim from
//! pre-split `g_game.rs`; see the module root for the mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::doom::i_system::I_Error;
use crate::doom::m_misc::{M_StringCopy, M_TempFile};
use crate::doom::p_saveg::{
    save_stream, savegame_error, P_ArchivePlayers, P_ArchiveSpecials, P_ArchiveThinkers,
    P_ArchiveWorld, P_ReadSaveGameEOF, P_ReadSaveGameHeader, P_SaveGameFile,
    P_TempSaveGameFile, P_UnArchivePlayers, P_UnArchiveSpecials, P_UnArchiveThinkers,
    P_UnArchiveWorld, P_WriteSaveGameEOF, P_WriteSaveGameHeader,
};
use crate::doom::p_tick::leveltime;
use crate::doom::r_draw::R_FillBackScreen;
use crate::doom::r_main::{setsizeneeded, R_ExecuteSetViewSize};

use super::actions::init_new as G_InitNew;
use super::consts::{ga_loadgame, ga_nothing, SAVEGAMESIZE};
use super::responder::deh_string;
use super::state::{
    consoleplayer, gameaction, gameepisode, gamemap, gameskill, players, savename, sendsave,
    SAVEDESCRIPTION, SAVEGAMESLOT, vanilla_savegame_limit,
};

// ---------------------------------------------------------------------------
// G_LoadGame / G_DoLoadGame
// ---------------------------------------------------------------------------

/// Defer a savegame load: copy `name` into the `savename` buffer and queue
/// `ga_loadgame` for the next `G_Ticker`.
///
/// # Safety
/// `name` must be a valid NUL-terminated C string. The C symbol is pinned
/// (`G_LoadGame`) so the wasm export set stays byte-identical.
#[doc(alias = "G_LoadGame")]
#[export_name = "G_LoadGame"]
pub unsafe extern "C" fn load_game(name: *mut c_char)
{
    M_StringCopy(std::ptr::addr_of_mut!(savename[0]), name, 256);
    gameaction = ga_loadgame;
}

/// Execute the deferred `ga_loadgame` action: open `savename`, validate the
/// header, set up the level via `G_InitNew`, then unarchive players, world
/// geometry, thinkers and specials from `p_saveg`.
///
/// On a missing or corrupt file the function returns silently after the
/// `fopen`; on a bad EOF marker it raises `I_Error("Bad savegame")`.
/// `leveltime` is preserved across the `G_InitNew` call so the unarchived
/// state continues from the saved tic.
///
/// # Safety
/// Mutates a large amount of game state and performs raw file I/O. The C
/// symbol is pinned (`G_DoLoadGame`) so the wasm export set stays
/// byte-identical.
#[doc(alias = "G_DoLoadGame")]
#[export_name = "G_DoLoadGame"]
pub unsafe extern "C" fn do_load_game()
{
    gameaction = ga_nothing;

    save_stream = libc::fopen(
        std::ptr::addr_of!(savename[0]),
        c"rb".as_ptr() as *const libc::c_char,
    );
    if save_stream.is_null()
    {
        return;
    }

    savegame_error = 0;

    if P_ReadSaveGameHeader() == 0
    {
        libc::fclose(save_stream);
        return;
    }

    let savedleveltime = leveltime;

    G_InitNew(gameskill, gameepisode, gamemap);

    leveltime = savedleveltime;

    P_UnArchivePlayers();
    P_UnArchiveWorld();
    P_UnArchiveThinkers();
    P_UnArchiveSpecials();

    if P_ReadSaveGameEOF() == 0
    {
        I_Error(c"Bad savegame".as_ptr());
    }

    libc::fclose(save_stream);

    if setsizeneeded.is_truthy()
    {
        R_ExecuteSetViewSize();
    }

    R_FillBackScreen();
}

// ---------------------------------------------------------------------------
// G_SaveGame / G_DoSaveGame
// ---------------------------------------------------------------------------

/// Defer a savegame write: latch the slot index and 24-byte description,
/// then set `sendsave` so the next `G_BuildTiccmd` emits a
/// `BT_SPECIAL | BTS_SAVEGAME` ticcmd. `G_Ticker` decodes that and triggers
/// `ga_savegame` -> `G_DoSaveGame`.
///
/// # Safety
/// `description` must be a valid NUL-terminated C string. The C symbol is
/// pinned (`G_SaveGame`) so the wasm export set stays byte-identical.
#[doc(alias = "G_SaveGame")]
#[export_name = "G_SaveGame"]
pub unsafe extern "C" fn save_game(slot: c_int, description: *const c_char)
{
    SAVEGAMESLOT = slot;
    M_StringCopy(std::ptr::addr_of_mut!(SAVEDESCRIPTION[0]), description, 32);
    sendsave = 1;
}

/// Execute the deferred `ga_savegame` action.
///
/// Writes to a temporary file first, then renames it over the real savegame
/// path so a crash mid-save cannot destroy an older save. On
/// `fopen`-failure, a recovery file in the temp directory is opened instead;
/// if both fail, `I_Error` aborts the engine.
///
/// When `vanilla_savegame_limit` is set, exceeding `SAVEGAMESIZE` raises
/// `I_Error("Savegame buffer overrun")` to match the DOS limit; otherwise
/// the save is allowed to grow.
///
/// # Safety
/// Performs raw `libc` file I/O (`fopen`/`ftell`/`fclose`/`remove`/`rename`)
/// and writes through the global `save_stream`. The C symbol is pinned
/// (`G_DoSaveGame`) so the wasm export set stays byte-identical. The libc
/// call sequence (fopen temp -> write -> fclose -> remove/rename) is
/// pinned by the web shell's VFS tests (`shells/web/src/wasm_vfs.rs`) --
/// never reorder the finalize step.
#[doc(alias = "G_DoSaveGame")]
#[export_name = "G_DoSaveGame"]
pub unsafe extern "C" fn do_save_game()
{
    let recovery_savegame_file: *mut c_char;
    let temp_savegame_file = P_TempSaveGameFile();
    let savegame_file = P_SaveGameFile(SAVEGAMESLOT);

    save_stream = libc::fopen(temp_savegame_file, c"wb".as_ptr() as *const libc::c_char);

    if save_stream.is_null()
    {
        let recovery = M_TempFile(c"recovery.dsg".as_ptr().cast_mut());
        recovery_savegame_file = recovery;
        save_stream = libc::fopen(recovery, c"wb".as_ptr() as *const libc::c_char);
        if save_stream.is_null()
        {
            I_Error(
                c"Failed to open either '%s' or '%s' to write savegame.".as_ptr() as *const c_char,
            );
        }
    }
    else
    {
        recovery_savegame_file = ptr::null_mut();
    }

    savegame_error = 0;

    P_WriteSaveGameHeader(std::ptr::addr_of_mut!(SAVEDESCRIPTION[0]));
    P_ArchivePlayers();
    P_ArchiveWorld();
    P_ArchiveThinkers();
    P_ArchiveSpecials();
    P_WriteSaveGameEOF();

    if vanilla_savegame_limit != 0 && libc::ftell(save_stream) > SAVEGAMESIZE
    {
        I_Error(c"Savegame buffer overrun".as_ptr());
    }

    libc::fclose(save_stream);

    if !recovery_savegame_file.is_null()
    {
        I_Error(
            c"Failed to open savegame file '%s' for writing.\nBut your game has been saved to '%s' for recovery.".as_ptr(),
        );
    }

    libc::remove(savegame_file);
    libc::rename(temp_savegame_file, savegame_file);

    gameaction = ga_nothing;
    M_StringCopy(std::ptr::addr_of_mut!(SAVEDESCRIPTION[0]), c"".as_ptr(), 32);

    players[consoleplayer as usize].message = deh_string(c"game saved.".as_ptr()) as *mut c_char;

    R_FillBackScreen();
}
