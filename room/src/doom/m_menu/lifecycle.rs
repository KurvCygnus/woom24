//! The menu lifecycle: the per-frame drawer (modal message + page +
//! items + skull), the page switchers, the skull ticker, and the
//! one-time init with its runtime pointer wiring and commercial menu
//! surgery (all verbatim).

use std::ffi::{c_char, c_int};
use std::ptr;

use super::consts::{LINEHEIGHT, SKULLXOFF, quitdoom, readthis};
use super::state::{
    currentMenu, inhelpscreens, itemOn, menuactive, messageLastMenuActive, messageString,
    messageToPrint, screenSize, skullAnimCounter, whichSkull, quickSaveSlot, screenblocks,
};
use super::tables::{
    EpisodeMenu, NewGameMenu, EpiDef, LoadDef, LoadMenu, MainDef, MainMenu, NewDef, OptionsDef,
    OptionsMenu, ReadDef1, ReadDef2, ReadMenu1, ReadMenu2, SaveDef, SaveMenu, SoundDef, SoundMenu,
    skullName,
};
use super::text::{string_height, string_width, write_text};
use super::types::{menu_t, patch_stub};
use crate::doom::d_mode;
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::hu_stuff::hu_font;
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::m_misc::M_StringCopy;
use crate::doom::v_video::{patch_t, V_DrawPatchDirect};
use crate::doom::w_wad::W_CacheLumpName;

/// Draw the menu overlay for the current frame.
///
/// If a modal message (`messageToPrint`) is pending it is drawn centered on
/// screen and the function returns early. Otherwise the current menu page's
/// draw callback is invoked, all item patches are blitted, and the animated
/// skull cursor is drawn at the highlighted item. Called by `D_Display` in
/// `d_main.c` every frame.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` (three call sites incl. the blocking wipe loop)
/// and the oracle reach the upstream name through the root shim.
#[doc(alias = "M_Drawer")]
#[export_name = "M_Drawer"]
pub extern "C" fn drawer() {
    unsafe {
        inhelpscreens = 0;

        if messageToPrint != 0 {
            let mut start: usize = 0;
            super::state::messy = SCREENHEIGHT / 2 - string_height(messageString) / 2;
            while *messageString.add(start) != 0 {
                let mut foundnewline = 0;
                let mut string: [c_char; 80] = [0; 80];
                let remaining = super::strlen(messageString.add(start));
                for i in 0..remaining {
                    if *messageString.add(start + i) == b'\n' as c_char {
                        M_StringCopy(string.as_mut_ptr(), messageString.add(start), string.len());
                        if i < string.len() {
                            string[i] = 0;
                        }
                        foundnewline = 1;
                        start += i + 1;
                        break;
                    }
                }
                if foundnewline == 0 {
                    M_StringCopy(string.as_mut_ptr(), messageString.add(start), string.len());
                    start += super::strlen(string.as_ptr());
                }

                super::state::messx = SCREENWIDTH / 2 - string_width(string.as_mut_ptr()) / 2;
                write_text(super::state::messx, super::state::messy, string.as_mut_ptr());
                let patch = hu_font[0] as *const patch_stub;
                super::state::messy += (*patch).height as c_int;
            }
            return;
        }

        if menuactive == 0 {
            return;
        }

        if let Some(r) = (*currentMenu).routine {
            r();
        }

        let x = (*currentMenu).x as c_int;
        let y = (*currentMenu).y as c_int;
        let max = (*currentMenu).numitems as usize;

        for i in 0..max {
            let name = (*(*currentMenu).menuitems.add(i)).name.as_ptr();
            if *name != 0 {
                V_DrawPatchDirect(
                    x,
                    y + LINEHEIGHT * i as c_int,
                    W_CacheLumpName(name, 0) as *mut patch_t,
                );
            }
        }

        V_DrawPatchDirect(
            x + SKULLXOFF,
            (*currentMenu).y as c_int - 5 + itemOn as c_int * LINEHEIGHT,
            W_CacheLumpName(skullName[whichSkull as usize], 0) as *mut patch_t,
        );
    }
}

/// Close all menus by setting `menuactive` to 0.
#[doc(alias = "M_ClearMenus")]
pub(super) fn clear_menus() {
    unsafe {
        menuactive = 0;
    }
}

/// Switch to `menudef` as the active menu page and restore its last-highlighted item.
#[doc(alias = "M_SetupNextMenu")]
pub(super) fn setup_next_menu(menudef: *mut menu_t) {
    unsafe {
        currentMenu = menudef;
        itemOn = (*currentMenu).lastOn;
    }
}

/// Advance the skull cursor animation by one game tic.
///
/// Decrements `skullAnimCounter` and toggles `whichSkull` every 8 tics.
/// Called by `G_Ticker` in `g_game.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs:331` and `d_net/mod.rs:174` extern-declare the symbol
/// and `d_net/loop_table.rs:113` binds its ADDRESS into the loop table
/// (`RunMenu: Some(M_Ticker)`) -- the pin is mandatory.
#[doc(alias = "M_Ticker")]
#[export_name = "M_Ticker"]
pub extern "C" fn ticker() {
    unsafe {
        skullAnimCounter -= 1;
        if skullAnimCounter <= 0 {
            whichSkull ^= 1;
            skullAnimCounter = 8;
        }
    }
}

/// One-time initialisation of the menu subsystem.
///
/// Resets all state variables, wires `menuitems` pointers and `prevMenu`
/// cross-links that cannot be set at static-initialisation time (Rust forbids
/// raw-pointer cross-references between statics), trims the episode menu to
/// three items for pre-Ultimate builds, and removes "Read This" from the main
/// menu in commercial mode. Called once from `D_DoomMain` in `d_main.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` and the oracle reach the upstream name through the
/// root shim.
#[doc(alias = "M_Init")]
#[export_name = "M_Init"]
pub extern "C" fn init() {
    unsafe {
        currentMenu = &raw mut MainDef;
        menuactive = 0;
        itemOn = (*currentMenu).lastOn;
        whichSkull = 0;
        skullAnimCounter = 10;
        screenSize = screenblocks - 3;
        messageToPrint = 0;
        messageString = ptr::null_mut();
        messageLastMenuActive = menuactive;
        quickSaveSlot = -1;

        // Set menuitems pointers (couldn't be done at static init time)
        MainDef.menuitems = std::ptr::addr_of_mut!(MainMenu[0]);
        EpiDef.menuitems = std::ptr::addr_of_mut!(EpisodeMenu[0]);
        NewDef.menuitems = std::ptr::addr_of_mut!(NewGameMenu[0]);
        OptionsDef.menuitems = std::ptr::addr_of_mut!(OptionsMenu[0]);
        ReadDef1.menuitems = std::ptr::addr_of_mut!(ReadMenu1[0]);
        ReadDef2.menuitems = std::ptr::addr_of_mut!(ReadMenu2[0]);
        SoundDef.menuitems = std::ptr::addr_of_mut!(SoundMenu[0]);
        LoadDef.menuitems = std::ptr::addr_of_mut!(LoadMenu[0]);
        SaveDef.menuitems = std::ptr::addr_of_mut!(SaveMenu[0]);

        // Set up prevMenu cross-links
        EpiDef.prevMenu = &raw mut MainDef;
        NewDef.prevMenu = &raw mut EpiDef;
        OptionsDef.prevMenu = &raw mut MainDef;
        ReadDef1.prevMenu = &raw mut MainDef;
        ReadDef2.prevMenu = &raw mut ReadDef1;
        SoundDef.prevMenu = &raw mut OptionsDef;
        LoadDef.prevMenu = &raw mut MainDef;
        SaveDef.prevMenu = &raw mut MainDef;

        match gamemode {
            d_mode::commercial => {
                ptr::write(&mut MainMenu[readthis], ptr::read(&MainMenu[quitdoom]));
                MainDef.numitems -= 1;
                MainDef.y += 8;
                NewDef.prevMenu = &raw mut MainDef;
            }
            d_mode::shareware | d_mode::registered | d_mode::retail => {}
            _ => {}
        }

        if gameversion < d_mode::exe_ultimate {
            EpiDef.numitems -= 1;
        }
    }
}
