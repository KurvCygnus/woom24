//! The menu tables: the nine item arrays and nine page descriptors
//! (whose `menuitems`/`prevMenu` pointer cross-links are wired at
//! runtime by `lifecycle::init`), the `gammamsg` status strings
//! (workaround entry 9's model -- path AND bytes pinned by
//! `g_game/dtmc.rs`), the skull cursor lumps, and the quit-sound
//! tables. Data tier -- upstream names and shapes.

use std::ffi::c_int;
use std::ptr;

use std::ffi::c_char;

use super::consts::{ep_end, load_end, main_end, newg_end, opt_end, read1_end, read2_end, sound_end};
use super::pages::{
    change_detail, change_messages, choose_skill, draw_episode, draw_main_menu,
    draw_new_game, draw_options, draw_read_this1, draw_read_this2, draw_sound, end_game, episode,
    finish_read_this, music_vol, new_game, options_item, quit_doom, read_this, read_this2,
    size_display, sfx_vol, sound_item,
};
use super::saveload::{draw_load, draw_save, load_game, load_select, save_game, save_select};
use super::types::{make_gamma_msg, menu_t, mi};
use crate::doom::sounds::Sfx;

/// Item array for the root main menu (New Game, Options, Load, Save, Read This, Quit).
pub(super) static mut MainMenu: [super::types::menuitem_t; 6] = [
    mi(1, b"M_NGAME\0\0\0", Some(new_game), b'n'),
    mi(1, b"M_OPTION\0\0", Some(options_item), b'o'),
    mi(1, b"M_LOADG\0\0\0", Some(load_game), b'l'),
    mi(1, b"M_SAVEG\0\0\0", Some(save_game), b's'),
    mi(1, b"M_RDTHIS\0\0", Some(read_this), b'r'),
    mi(1, b"M_QUITG\0\0\0", Some(quit_doom), b'q'),
];

/// Item array for the episode selection menu (E1-E4).
pub(super) static mut EpisodeMenu: [super::types::menuitem_t; 4] = [
    mi(1, b"M_EPI1\0\0\0\0", Some(episode), b'k'),
    mi(1, b"M_EPI2\0\0\0\0", Some(episode), b't'),
    mi(1, b"M_EPI3\0\0\0\0", Some(episode), b'i'),
    mi(1, b"M_EPI4\0\0\0\0", Some(episode), b't'),
];

/// Item array for the skill-level selection menu.
pub(super) static mut NewGameMenu: [super::types::menuitem_t; 5] = [
    mi(1, b"M_JKILL\0\0\0", Some(choose_skill), b'i'),
    mi(1, b"M_ROUGH\0\0\0", Some(choose_skill), b'h'),
    mi(1, b"M_HURT\0\0\0\0", Some(choose_skill), b'h'),
    mi(1, b"M_ULTRA\0\0\0", Some(choose_skill), b'u'),
    mi(1, b"M_NMARE\0\0\0", Some(choose_skill), b'n'),
];

/// Item array for the Options menu (end game, messages, detail, screen size, mouse sensitivity, sound).
pub(super) static mut OptionsMenu: [super::types::menuitem_t; 8] = [
    mi(1, b"M_ENDGAM\0\0", Some(end_game), b'e'),
    mi(1, b"M_MESSG\0\0\0", Some(change_messages), b'm'),
    mi(1, b"M_DETAIL\0\0", Some(change_detail), b'g'),
    mi(2, b"M_SCRNSZ\0\0", Some(size_display), b's'),
    mi(-1, b"", None, 0),
    mi(2, b"M_MSENS\0\0\0", Some(super::pages::change_sensitivity), b'm'),
    mi(-1, b"", None, 0),
    mi(1, b"M_SVOL\0\0\0\0", Some(sound_item), b's'),
];

/// Single-item array for the first "Read This" help page (advances to page 2).
pub(super) static mut ReadMenu1: [super::types::menuitem_t; 1] =
    [mi(1, b"", Some(read_this2), 0)];

/// Single-item array for the second "Read This" help page (returns to main menu).
pub(super) static mut ReadMenu2: [super::types::menuitem_t; 1] =
    [mi(1, b"", Some(finish_read_this), 0)];

/// Item array for the Sound Volume menu (SFX slider, music slider).
pub(super) static mut SoundMenu: [super::types::menuitem_t; 4] = [
    mi(2, b"M_SFXVOL\0\0", Some(sfx_vol), b's'),
    mi(-1, b"", None, 0),
    mi(2, b"M_MUSVOL\0\0", Some(music_vol), b'm'),
    mi(-1, b"", None, 0),
];

/// Item array for the Load Game menu (6 save slots).
pub(super) static mut LoadMenu: [super::types::menuitem_t; 6] = [
    mi(1, b"", Some(load_select), b'1'),
    mi(1, b"", Some(load_select), b'2'),
    mi(1, b"", Some(load_select), b'3'),
    mi(1, b"", Some(load_select), b'4'),
    mi(1, b"", Some(load_select), b'5'),
    mi(1, b"", Some(load_select), b'6'),
];

/// Item array for the Save Game menu (6 save slots).
pub(super) static mut SaveMenu: [super::types::menuitem_t; 6] = [
    mi(1, b"", Some(save_select), b'1'),
    mi(1, b"", Some(save_select), b'2'),
    mi(1, b"", Some(save_select), b'3'),
    mi(1, b"", Some(save_select), b'4'),
    mi(1, b"", Some(save_select), b'5'),
    mi(1, b"", Some(save_select), b'6'),
];

/// Page descriptor for the root main menu.
pub(super) static mut MainDef: menu_t = menu_t {
    numitems: main_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_main_menu),
    x: 97,
    y: 64,
    lastOn: 0,
};

/// Page descriptor for the episode selection menu.
pub(super) static mut EpiDef: menu_t = menu_t {
    numitems: ep_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_episode),
    x: 48,
    y: 63,
    lastOn: 0,
};

/// Page descriptor for the skill-level selection menu; defaults to "Hurt Me Plenty" (index 2).
pub(super) static mut NewDef: menu_t = menu_t {
    numitems: newg_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_new_game),
    x: 48,
    y: 63,
    lastOn: 2,
};

/// Page descriptor for the Options menu.
pub(super) static mut OptionsDef: menu_t = menu_t {
    numitems: opt_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_options),
    x: 60,
    y: 37,
    lastOn: 0,
};

/// Page descriptor for the first "Read This" help screen.
/// The skull position (x, y) may be adjusted at runtime by `draw_read_this1`
/// depending on the game version.
pub(super) static mut ReadDef1: menu_t = menu_t {
    numitems: read1_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_read_this1),
    x: 280,
    y: 185,
    lastOn: 0,
};

/// Page descriptor for the second "Read This" help screen (Doom 1.x only, non-commercial).
pub(super) static mut ReadDef2: menu_t = menu_t {
    numitems: read2_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_read_this2),
    x: 330,
    y: 175,
    lastOn: 0,
};

/// Page descriptor for the Sound Volume menu.
pub(super) static mut SoundDef: menu_t = menu_t {
    numitems: sound_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_sound),
    x: 80,
    y: 64,
    lastOn: 0,
};

/// Page descriptor for the Load Game menu.
pub(super) static mut LoadDef: menu_t = menu_t {
    numitems: load_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_load),
    x: 80,
    y: 54,
    lastOn: 0,
};

/// Page descriptor for the Save Game menu (reuses `load_end` count and `LoadDef` geometry).
pub(super) static mut SaveDef: menu_t = menu_t {
    numitems: load_end as i16,
    prevMenu: ptr::null_mut(),
    menuitems: ptr::null_mut(),
    routine: Some(draw_save),
    x: 80,
    y: 54,
    lastOn: 0,
};

/// Gamma-correction status messages displayed when the player cycles the gamma level.
///
/// Index matches the `usegamma` value (0-4). Exported for C callers in `m_config.c`.
/// `gammamsg[0]`'s bytes are workaround entry 9's GAMMALVL0 model
/// (`docs/vanilla-workarounds.md`, commercial map33 par time) -- the
/// live reader is `g_game/dtmc.rs` (`gammalvl0_prefix_i32`) with a test
/// guard, so name, path, AND bytes are pinned; never reword.
#[no_mangle]
pub static mut gammamsg: [[c_char; 26]; 5] = [
    make_gamma_msg("Gamma correction OFF"),
    make_gamma_msg("Gamma correction level 1"),
    make_gamma_msg("Gamma correction level 2"),
    make_gamma_msg("Gamma correction level 3"),
    make_gamma_msg("Gamma correction level 4"),
];

/// WAD lump names for the two skull cursor animation frames.
pub(super) static mut skullName: [*const c_char; 2] =
    [c"M_SKULL1".as_ptr(), c"M_SKULL2".as_ptr()];

/// Sound effects played on quit confirmation for Doom episode games (cycled by `gametic`).
pub(super) static mut quitsounds: [c_int; 8] = [
    Sfx::Pldeth as c_int,
    Sfx::Dmpain as c_int,
    Sfx::Popain as c_int,
    Sfx::Slop as c_int,
    Sfx::Telept as c_int,
    Sfx::Posit1 as c_int,
    Sfx::Posit3 as c_int,
    Sfx::Sgtatk as c_int,
];

/// Sound effects played on quit confirmation for Doom II (commercial) builds (cycled by `gametic`).
pub(super) static mut quitsounds2: [c_int; 8] = [
    Sfx::Vilact as c_int,
    Sfx::Getpow as c_int,
    Sfx::Boscub as c_int,
    Sfx::Slop as c_int,
    Sfx::Skeswg as c_int,
    Sfx::Kntdth as c_int,
    Sfx::Bspact as c_int,
    Sfx::Sgtatk as c_int,
];
