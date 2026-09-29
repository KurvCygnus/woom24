//! The save/load pages: slot rows with header-read descriptions, the
//! save-name editor entry, and the quicksave/quickload flows. The
//! `G_LoadGame`/`G_SaveGame` calls ARE the behavior (simulation
//! transitions triggered from UI; recorded, not extracted -- report
//! §8.4).

use std::ffi::{c_char, c_int, c_void};

use super::consts::{load_end, SAVESTRINGSIZE};
use super::message::start_message;
use super::state::{
    quickSaveSlot, saveCharIndex, saveOldString, saveSlot, saveStringEnter, savegamestrings,
};
use super::text::string_width;
use crate::c_write;
use crate::doom::g_game::{gamestate, netgame, usergame, G_LoadGame, G_SaveGame};
use crate::doom::m_controls::key_menu_confirm;
use crate::doom::m_misc::M_StringCopy;
use crate::doom::p_saveg::P_SaveGameFile;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::v_video::{patch_t, V_DrawPatchDirect};
use crate::doom::w_wad::W_CacheLumpName;

/// Populate `savegamestrings` and `LoadMenu[*].status` by reading the header of each save file.
///
/// Opens each save file by name (via `P_SaveGameFile`), reads the first
/// `SAVESTRINGSIZE` bytes as the description, and marks the slot active.
/// Slots whose file does not exist receive the placeholder text "empty slot"
/// and have their `status` set to 0 (non-selectable in the load menu).
#[doc(alias = "M_ReadSaveStrings")]
pub(super) fn read_save_strings() {
    unsafe {
        for i in 0..load_end {
            let name_ptr = P_SaveGameFile(i as c_int);
            let mut name: [c_char; 256] = [0; 256];
            M_StringCopy(name.as_mut_ptr(), name_ptr, name.len());

            let handle = super::fopen(name.as_ptr() as *const c_char, c"rb".as_ptr());
            if handle.is_null() {
                M_StringCopy(
                    savegamestrings[i].as_mut_ptr(),
                    c"empty slot".as_ptr(),
                    SAVESTRINGSIZE,
                );
                super::tables::LoadMenu[i].status = 0;
                continue;
            }
            super::fread(
                savegamestrings[i].as_mut_ptr() as *mut c_void,
                1,
                SAVESTRINGSIZE,
                handle,
            );
            super::fclose(handle);
            super::tables::LoadMenu[i].status = 1;
        }
    }
}

/// Draw the Load Game menu page: title patch plus one bordered slot row per save slot.
#[doc(alias = "M_DrawLoad")]
pub(super) extern "C" fn draw_load() {
    unsafe {
        V_DrawPatchDirect(
            72,
            28,
            W_CacheLumpName(c"M_LOADG".as_ptr(), 0) as *mut patch_t,
        );

        for i in 0..load_end {
            draw_save_load_border(
                super::tables::LoadDef.x as c_int,
                super::tables::LoadDef.y as c_int + super::consts::LINEHEIGHT * i as c_int,
            );
            super::text::write_text(
                super::tables::LoadDef.x as c_int,
                super::tables::LoadDef.y as c_int + super::consts::LINEHEIGHT * i as c_int,
                savegamestrings[i].as_mut_ptr(),
            );
        }
    }
}

/// Draw the left/center/right border patches around a save-game name text field at `(x, y)`.
#[doc(alias = "M_DrawSaveLoadBorder")]
fn draw_save_load_border(x: c_int, y: c_int) {
    V_DrawPatchDirect(
        x - 8,
        y + 7,
        W_CacheLumpName(c"M_LSLEFT".as_ptr(), 0) as *mut patch_t,
    );

    let mut xi = x;
    for _ in 0..24 {
        V_DrawPatchDirect(
            xi,
            y + 7,
            W_CacheLumpName(c"M_LSCNTR".as_ptr(), 0) as *mut patch_t,
        );
        xi += 8;
    }

    V_DrawPatchDirect(
        xi,
        y + 7,
        W_CacheLumpName(c"M_LSRGHT".as_ptr(), 0) as *mut patch_t,
    );
}

/// Load the game from save slot `choice` and close all menus.
#[doc(alias = "M_LoadSelect")]
pub(super) extern "C" fn load_select(choice: c_int) {
    unsafe {
        let mut name: [c_char; 256] = [0; 256];
        M_StringCopy(name.as_mut_ptr(), P_SaveGameFile(choice), name.len());
        G_LoadGame(name.as_mut_ptr());
        super::lifecycle::clear_menus();
    }
}

/// Navigate to the Load Game menu page (suppressed during network games).
#[doc(alias = "M_LoadGame")]
pub(super) extern "C" fn load_game(_choice: c_int) {
    unsafe {
        if netgame != 0 {
            start_message(
                c"you can't do load while in a net game!\n\npress a key."
                    .as_ptr()
                    .cast_mut(),
                None,
                0,
            );
            return;
        }
        super::lifecycle::setup_next_menu(&raw mut super::tables::LoadDef);
    }
    read_save_strings();
}

/// Draw the Save Game menu page: title, bordered slot rows, and a blinking cursor when editing.
#[doc(alias = "M_DrawSave")]
pub(super) extern "C" fn draw_save() {
    unsafe {
        V_DrawPatchDirect(
            72,
            28,
            W_CacheLumpName(c"M_SAVEG".as_ptr(), 0) as *mut patch_t,
        );
        for i in 0..load_end {
            draw_save_load_border(
                super::tables::LoadDef.x as c_int,
                super::tables::LoadDef.y as c_int + super::consts::LINEHEIGHT * i as c_int,
            );
            super::text::write_text(
                super::tables::LoadDef.x as c_int,
                super::tables::LoadDef.y as c_int + super::consts::LINEHEIGHT * i as c_int,
                savegamestrings[i].as_mut_ptr(),
            );
        }

        if saveStringEnter != 0 {
            let w = string_width(savegamestrings[saveSlot as usize].as_mut_ptr());
            super::text::write_text(
                super::tables::LoadDef.x as c_int + w,
                super::tables::LoadDef.y as c_int + super::consts::LINEHEIGHT * saveSlot as c_int,
                c"_".as_ptr().cast_mut(),
            );
        }
    }
}

/// Commit the save to `slot`, clear the menus, and record the slot as the quicksave target.
#[doc(alias = "M_DoSave")]
pub(super) fn do_save(slot: c_int) {
    unsafe {
        G_SaveGame(slot, savegamestrings[slot as usize].as_ptr());
        super::lifecycle::clear_menus();
        if quickSaveSlot == -2 {
            quickSaveSlot = slot;
        }
    }
}

/// Enter string-editing mode for save slot `choice`, preserving the old description.
#[doc(alias = "M_SaveSelect")]
pub(super) extern "C" fn save_select(choice: c_int) {
    unsafe {
        saveStringEnter = 1;
        saveSlot = choice;
        M_StringCopy(
            std::ptr::addr_of_mut!(saveOldString[0]),
            savegamestrings[choice as usize].as_ptr(),
            SAVESTRINGSIZE,
        );
        if super::strcmp(
            savegamestrings[choice as usize].as_ptr(),
            c"empty slot".as_ptr(),
        ) == 0
        {
            savegamestrings[choice as usize][0] = 0;
        }
        saveCharIndex = super::strlen(savegamestrings[choice as usize].as_ptr()) as c_int;
    }
}

/// Navigate to the Save Game menu page (suppressed when not in-game or not at `GS_LEVEL`).
#[doc(alias = "M_SaveGame")]
pub(super) extern "C" fn save_game(_choice: c_int) {
    const GS_LEVEL: c_int = 0;
    unsafe {
        if usergame == 0 {
            start_message(
                c"you can't save if you aren't playing!\n\npress a key."
                    .as_ptr()
                    .cast_mut(),
                None,
                0,
            );
            return;
        }
        if gamestate != GS_LEVEL {
            return;
        }
        super::lifecycle::setup_next_menu(&raw mut super::tables::SaveDef);
    }
    read_save_strings();
}

/// Perform a quicksave to the previously chosen slot, or open the save menu if none was chosen.
///
/// Plays `Sfx::Oof` and returns immediately if the game is not in progress.
/// If `quickSaveSlot` is -1, opens the save menu and sets it to -2 to indicate
/// "waiting for slot selection". Otherwise shows a y/n confirmation prompt.
#[doc(alias = "M_QuickSave")]
pub(super) fn quick_save() {
    const GS_LEVEL: c_int = 0;
    unsafe {
        if usergame == 0 {
            S_StartSound(std::ptr::null_mut(), Sfx::Oof as c_int);
            return;
        }
        if gamestate != GS_LEVEL {
            return;
        }
        if quickSaveSlot < 0 {
            super::responder::start_control_panel();
            read_save_strings();
            super::lifecycle::setup_next_menu(&raw mut super::tables::SaveDef);
            quickSaveSlot = -2;
            return;
        }
        static mut QUICK_SAVE_MSG: [c_char; 80] = [0; 80];
        let slot_str = std::ffi::CStr::from_ptr(savegamestrings[quickSaveSlot as usize].as_ptr())
            .to_string_lossy();
        c_write!(
            QUICK_SAVE_MSG,
            "quicksave over your game named\n\n'{}'?\n\npress y or n.",
            slot_str
        );
        start_message(
            std::ptr::addr_of_mut!(QUICK_SAVE_MSG[0]),
            Some(quick_save_response),
            1,
        );
    }
}

/// Confirmation callback for the quicksave y/n prompt; saves if `key` is the confirm key.
#[doc(alias = "M_QuickSaveResponse")]
pub(super) extern "C" fn quick_save_response(key: c_int) {
    unsafe {
        if key == key_menu_confirm {
            do_save(quickSaveSlot);
            S_StartSound(std::ptr::null_mut(), Sfx::Swtchx as c_int);
        }
    }
}

/// Perform a quickload from the previously chosen slot, or show an error if none was chosen.
///
/// Suppressed during network games. Shows a y/n confirmation before loading.
#[doc(alias = "M_QuickLoad")]
pub(super) fn quick_load() {
    unsafe {
        if netgame != 0 {
            start_message(
                c"you can't quickload during a netgame!\n\npress a key."
                    .as_ptr()
                    .cast_mut(),
                None,
                0,
            );
            return;
        }
        if quickSaveSlot < 0 {
            start_message(
                c"you haven't picked a quicksave slot yet!\n\npress a key.".as_ptr() as *mut c_char,
                None,
                0,
            );
            return;
        }
        static mut QUICK_LOAD_MSG: [c_char; 80] = [0; 80];
        let slot_str = std::ffi::CStr::from_ptr(savegamestrings[quickSaveSlot as usize].as_ptr())
            .to_string_lossy();
        c_write!(
            QUICK_LOAD_MSG,
            "do you want to quickload the game named\n\n'{}'?\n\npress y or n.",
            slot_str
        );
        start_message(
            std::ptr::addr_of_mut!(QUICK_LOAD_MSG[0]),
            Some(quick_load_response),
            1,
        );
    }
}

/// Confirmation callback for the quickload y/n prompt; loads if `key` is the confirm key.
#[doc(alias = "M_QuickLoadResponse")]
pub(super) extern "C" fn quick_load_response(key: c_int) {
    unsafe {
        if key == key_menu_confirm {
            load_select(quickSaveSlot);
            S_StartSound(std::ptr::null_mut(), Sfx::Swtchx as c_int);
        }
    }
}
