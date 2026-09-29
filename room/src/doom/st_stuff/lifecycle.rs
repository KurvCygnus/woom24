//! The status-bar lifecycle: state reset (`init_data`), widget-pointer
//! wiring (`create_widgets`), the level start/stop pair, and the
//! one-time `init`.

use std::ffi::c_int;
use std::ptr;

use super::consts::{
    ST_AMMO0WIDTH, ST_AMMO0X, ST_AMMO0Y, ST_AMMO1X, ST_AMMO1Y, ST_AMMO2X, ST_AMMO2Y, ST_AMMO3X,
    ST_AMMO3Y, ST_AMMOWIDTH, ST_AMMOX, ST_AMMOY, ST_ARMSBGX, ST_ARMSBGY, ST_ARMSX, ST_ARMSXSPACE,
    ST_ARMSY, ST_ARMSYSPACE, ST_ARMORX, ST_ARMORY, ST_FACESX, ST_FACESY, ST_FRAGSWIDTH,
    ST_FRAGSX, ST_FRAGSY, ST_HEALTHX, ST_HEALTHY, ST_KEY0X, ST_KEY0Y, ST_KEY1X, ST_KEY1Y,
    ST_KEY2X, ST_KEY2Y, ST_MAXAMMO0WIDTH, ST_MAXAMMO0X, ST_MAXAMMO0Y, ST_MAXAMMO1X, ST_MAXAMMO1Y,
    ST_MAXAMMO2X, ST_MAXAMMO2Y, ST_MAXAMMO3X, ST_MAXAMMO3Y, ST_WIDTH, ST_HEIGHT,
};
use super::{
    arms, armsbg, faces, keyboxes, keys, lu_palette, oldweaponsowned, plyr, shortnum, st_armson,
    st_chat, st_chatstate, st_clock, st_cursoron, st_firsttime, st_faceindex, st_fragson,
    st_fragscount, st_gamestate, st_notdeathmatch, st_oldchat, st_oldhealth, st_palette,
    st_statusbaron, st_stopped, tallnum, tallpercent, w_ammo, w_arms, w_armsbg, w_armor, w_faces,
    w_frags, w_health, w_keyboxes, w_maxammo, w_ready, st_backing_screen,
};
use crate::doom::d_items::weaponinfo;
use crate::doom::d_player::{NUMAMMO, NUMWEAPONS};
use crate::doom::g_game::{consoleplayer, players};
use crate::doom::i_video::I_SetPalette;
use crate::doom::st_lib::{
    STlib_init, STlib_initBinIcon, STlib_initMultIcon, STlib_initNum, STlib_initPercent,
};
use crate::doom::w_wad::W_CacheLumpNum;
use crate::doom::z_zone::{Z_Malloc, PU_CACHE, PU_STATIC};

/// Reset all internal status-bar state variables to their defaults.
///
/// Sets `st_firsttime`, clears the clock, chat state, cursor, face index, and
/// palette sentinel. Snapshots the current weapon ownership array and resets
/// key-slot values to -1. Calls `STlib_init` to reset the widget library.
///
/// # Safety
///
/// Writes to every internal status-bar state global and to `plyr`, deriving
/// it from `players[consoleplayer]`. Caller must ensure `consoleplayer` is a
/// valid index into `players` (i.e. `D_DoomMain` has set the network/player
/// globals).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `start` reaches the upstream name directly.
#[doc(alias = "ST_initData")]
#[export_name = "ST_initData"]
pub unsafe extern "C" fn init_data() {
    st_firsttime = 1;
    plyr = std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize);
    st_clock = 0;
    st_chatstate = 0; // StartChatState
    st_gamestate = 1; // FirstPersonState
    st_statusbaron = 1;
    st_oldchat = 0;
    st_chat = 0;
    st_cursoron = 0;
    st_faceindex = 0;
    st_palette = -1;
    st_oldhealth = -1;

    for i in 0..NUMWEAPONS {
        oldweaponsowned[i] = (*plyr).weaponowned[i];
    }
    for i in 0..3 {
        keyboxes[i] = -1;
    }

    STlib_init();
}

/// Initialise and bind all status-bar widgets to their screen positions.
///
/// Must be called after `init_data` (which sets `plyr`) and after graphics
/// have been loaded (which populates `tallnum`, `shortnum`, etc.). Creates
/// widgets for: ready ammo, health, armor, arms background, weapons-owned icons,
/// frag counter, face, key slots, and per-ammo-type current/max displays.
///
/// # Safety
///
/// Initialises every `w_*` widget global with raw pointers into `plyr`'s
/// ammo/health/armor/weapon-owned arrays and the cached patch tables.
/// Caller must ensure `init_data` and `load_graphics` ran first so
/// `plyr` is valid and the patches are loaded.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `start` reaches the upstream name directly.
#[doc(alias = "ST_createWidgets")]
#[export_name = "ST_createWidgets"]
pub unsafe extern "C" fn create_widgets() {
    STlib_initNum(
        &raw mut w_ready,
        ST_AMMOX,
        ST_AMMOY,
        std::ptr::addr_of_mut!(tallnum[0]),
        (*plyr)
            .ammo
            .as_mut_ptr()
            .add(weaponinfo[(*plyr).readyweapon as usize].ammo as usize),
        &raw mut st_statusbaron,
        ST_AMMOWIDTH,
    );
    w_ready.data = (*plyr).readyweapon;

    STlib_initPercent(
        &raw mut w_health,
        ST_HEALTHX,
        ST_HEALTHY,
        std::ptr::addr_of_mut!(tallnum[0]),
        &mut (*plyr).health,
        &raw mut st_statusbaron,
        tallpercent,
    );

    STlib_initBinIcon(
        &raw mut w_armsbg,
        ST_ARMSBGX,
        ST_ARMSBGY,
        armsbg,
        &raw mut st_notdeathmatch,
        &raw mut st_statusbaron,
    );

    for i in 0..6 {
        let i_i = i as c_int;
        STlib_initMultIcon(
            std::ptr::addr_of_mut!(w_arms[0]).add(i),
            ST_ARMSX + (i_i % 3) * ST_ARMSXSPACE,
            ST_ARMSY + (i_i / 3) * ST_ARMSYSPACE,
            arms[i].as_mut_ptr(),
            (*plyr).weaponowned.as_mut_ptr().add(i + 1),
            &raw mut st_armson,
        );
    }

    STlib_initNum(
        &raw mut w_frags,
        ST_FRAGSX,
        ST_FRAGSY,
        std::ptr::addr_of_mut!(tallnum[0]),
        &raw mut st_fragscount,
        &raw mut st_fragson,
        ST_FRAGSWIDTH,
    );

    STlib_initMultIcon(
        &raw mut w_faces,
        ST_FACESX,
        ST_FACESY,
        std::ptr::addr_of_mut!(faces[0]),
        &raw mut st_faceindex,
        &raw mut st_statusbaron,
    );

    STlib_initPercent(
        &raw mut w_armor,
        ST_ARMORX,
        ST_ARMORY,
        std::ptr::addr_of_mut!(tallnum[0]),
        &mut (*plyr).armorpoints,
        &raw mut st_statusbaron,
        tallpercent,
    );

    STlib_initMultIcon(
        std::ptr::addr_of_mut!(w_keyboxes[0]).add(0),
        ST_KEY0X,
        ST_KEY0Y,
        std::ptr::addr_of_mut!(keys[0]),
        &mut keyboxes[0],
        &raw mut st_statusbaron,
    );
    STlib_initMultIcon(
        std::ptr::addr_of_mut!(w_keyboxes[0]).add(1),
        ST_KEY1X,
        ST_KEY1Y,
        std::ptr::addr_of_mut!(keys[0]),
        &mut keyboxes[1],
        &raw mut st_statusbaron,
    );
    STlib_initMultIcon(
        std::ptr::addr_of_mut!(w_keyboxes[0]).add(2),
        ST_KEY2X,
        ST_KEY2Y,
        std::ptr::addr_of_mut!(keys[0]),
        &mut keyboxes[2],
        &raw mut st_statusbaron,
    );

    for i in 0..NUMAMMO {
        STlib_initNum(
            std::ptr::addr_of_mut!(w_ammo[0]).add(i),
            match i {
                0 => ST_AMMO0X,
                1 => ST_AMMO1X,
                2 => ST_AMMO2X,
                3 => ST_AMMO3X,
                _ => unreachable!(),
            },
            match i {
                0 => ST_AMMO0Y,
                1 => ST_AMMO1Y,
                2 => ST_AMMO2Y,
                3 => ST_AMMO3Y,
                _ => unreachable!(),
            },
            std::ptr::addr_of_mut!(shortnum[0]),
            (*plyr).ammo.as_mut_ptr().add(i),
            &raw mut st_statusbaron,
            ST_AMMO0WIDTH,
        );
    }

    for i in 0..NUMAMMO {
        STlib_initNum(
            std::ptr::addr_of_mut!(w_maxammo[0]).add(i),
            match i {
                0 => ST_MAXAMMO0X,
                1 => ST_MAXAMMO1X,
                2 => ST_MAXAMMO2X,
                3 => ST_MAXAMMO3X,
                _ => unreachable!(),
            },
            match i {
                0 => ST_MAXAMMO0Y,
                1 => ST_MAXAMMO1Y,
                2 => ST_MAXAMMO2Y,
                3 => ST_MAXAMMO3Y,
                _ => unreachable!(),
            },
            std::ptr::addr_of_mut!(shortnum[0]),
            (*plyr).maxammo.as_mut_ptr().add(i),
            &raw mut st_statusbaron,
            ST_MAXAMMO0WIDTH,
        );
    }
}

/// Start the status bar subsystem for a new game or level.
///
/// Calls `stop` if already running, then re-initialises state, creates
/// widgets, and clears `st_stopped`. Called from `G_DoLoadLevel` in `g_game.c`.
///
/// # Safety
///
/// Reads/mutates `st_stopped` and runs `init_data`/`create_widgets`,
/// which mutate every status-bar global and require the WAD/player subsystems
/// to be initialised (see those functions' own safety contracts). Caller must
/// ensure `init` has run once at startup.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_mobj/mapthings.rs:129` imports the upstream name through the
/// root shim.
#[doc(alias = "ST_Start")]
#[export_name = "ST_Start"]
pub unsafe extern "C" fn start() {
    if st_stopped == 0 {
        stop();
    }
    init_data();
    create_widgets();
    st_stopped = 0;
}

/// Stop the status bar subsystem and restore the normal palette.
///
/// Resets the display palette to `PLAYPAL` index 0 and sets `st_stopped`.
/// Safe to call when already stopped (guard at the top). Called from `G_WorldDone`
/// and `start` in `g_game.c`.
///
/// # Safety
///
/// Reads/mutates `st_stopped` and calls `I_SetPalette` with the cached
/// `lu_palette` lump. Caller must ensure `load_data` has cached the
/// palette lump and the video backend is ready.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `start` reaches the upstream name directly.
#[doc(alias = "ST_Stop")]
#[export_name = "ST_Stop"]
pub unsafe extern "C" fn stop() {
    if st_stopped != 0 {
        return;
    }
    I_SetPalette(W_CacheLumpNum(lu_palette, PU_CACHE) as *mut u8);
    st_stopped = 1;
}

/// One-time initialisation of the status bar subsystem.
///
/// Loads all WAD graphics into `PU_STATIC` memory and allocates the backing
/// screen buffer. Must be called exactly once during startup, before `start`.
/// Called from `G_InitNew` in `g_game.c`.
///
/// # Safety
///
/// Allocates `st_backing_screen` via `Z_Malloc` and runs `load_data`,
/// which mutates the cached patch globals. Caller must ensure the WAD and
/// zone-memory subsystems are initialised, and that this function is called
/// exactly once.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs:507` imports the upstream name through the root
/// shim and the differential oracle declares the symbol directly.
#[doc(alias = "ST_Init")]
#[export_name = "ST_Init"]
pub unsafe extern "C" fn init() {
    super::assets::load_data();
    st_backing_screen = Z_Malloc(ST_WIDTH * ST_HEIGHT, PU_STATIC, ptr::null_mut()) as *mut u8;
}
