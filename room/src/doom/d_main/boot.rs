//! Boot orchestration: `doom_main` (upstream `D_DoomMain`), the
//! `-playdemo`/`-timedemo` early-return target `doom_loop` (upstream
//! `D_DoomLoop`), and their helpers `add_file` (upstream `D_AddFile`),
//! `print_dehacked_banners` (upstream `PrintDehackedBanners`) and
//! `endoom` (upstream `D_Endoom`).
//!
//! One-shot pre-tic-0 glue: the init ORDER inside `doom_main` is
//! hash-bearing (every F9 harness boots through it) and moved VERBATIM
//! -- never reorder, and never "restore" the upstream never-returns
//! `doom_loop` (this port returns after one boot tic; the host drives
//! frames via the frame entries).

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::{c_write, i_error};
use crate::types::Boolean;
use crate::doom::d_iwad::{D_FindIWAD, D_SaveGameIWADName};
use crate::doom::d_loop::{TryRunTics, D_StartGameLoop};
use crate::doom::d_mode;
use crate::doom::d_net::{D_CheckNetGame, D_ConnectNetGame};
use crate::doom::doomstat::{gamedescription, gamemission, gamemode, modifiedgame};
use crate::doom::g_game::{
    deathmatch, demorecording, forwardmove, gameaction, gamestate, netgame, sidemove, singledemo,
    testcontrols, G_BeginRecording, G_DeferedPlayDemo, G_InitNew, G_LoadGame, G_RecordDemo,
    G_TimeDemo,
};
use crate::doom::hu_stuff::HU_Init;
use crate::doom::i_endoom::I_Endoom;
use crate::doom::i_joystick::I_InitJoystick;
use crate::doom::i_sound::{I_InitMusic, I_InitSound};
use crate::doom::i_system::{I_AtExit, I_PrintBanner, I_PrintDivider, I_PrintStartupBanner};
use crate::doom::i_timer::I_InitTimer;
use crate::doom::i_video::{
    screensaver_mode, I_CheckIsScreensaver, I_DisplayFPSDots, I_EnableLoadingDisk,
    I_GraphicsCheckCommandLine, I_InitGraphics, I_SetGrabMouseCallback, I_SetWindowTitle,
};
use crate::doom::m_argv::{myargc, myargv, M_CheckParm, M_CheckParmWithArgs};
use crate::doom::m_config::{
    M_GetSaveGameDir, M_LoadDefaults, M_SaveDefaults, M_SetConfigDir, M_SetConfigFilenames,
};
use crate::doom::m_menu::M_Init;
use crate::doom::m_misc::M_StringCopy;
use crate::doom::p_saveg::P_SaveGameFile;
use crate::doom::p_setup::P_Init;
use crate::doom::r_main::{R_ExecuteSetViewSize, R_Init};
use crate::doom::s_sound::{musicVolume, sfxVolume, S_Init};
use crate::doom::st_stuff::ST_Init;
use crate::doom::v_video::{V_Init, V_RestoreBuffer};
use crate::doom::w_main::W_ParseCommandLine;
use crate::doom::w_wad::{
    lumpinfo, numlumps, W_AddFile, W_CacheLumpName, W_CheckCorrectIWAD, W_CheckNumForName,
    W_GenerateHashTable,
};
use crate::doom::z_zone::{Z_Init, PU_STATIC};

use self::{
    add_file as D_AddFile, doom_loop as D_DoomLoop, endoom as D_Endoom,
    print_dehacked_banners as PrintDehackedBanners,
};
use super::bind::bind_variables as D_BindVariables;
use super::compat::{
    deh_string as DEH_String, ends_with_ci as c_str_ends_with, to_lossy_string as c_str_to_str,
};
use super::consts::{
    ga_loadgame, ga_playdemo, sk_medium, COPYRIGHT_BANNERS, D_DEVSTR, IWAD_CHECK_NAMES,
};
use super::display::grab_mouse_callback as D_GrabMouseCallback;
use super::entries::tick_entry as doomgeneric_Tick;
use super::identify::{
    identify_version as D_IdentifyVersion, init_game_version as InitGameVersion,
    print_game_version as PrintGameVersion, set_game_description as D_SetGameDescription,
};
use super::sequencing::start_title as D_StartTitle;
use super::state::{
    autostart, bfgedition, devparm, fastparm, iwadfile, main_loop_started, nomonsters, respawnparm,
    savegamedir, show_endoom, startepisode, startloadgame, startmap, startskill, storedemo,
    wipegamestate,
};

extern "C"
{
    // net_dedicated.c / net_query.c — not yet ported:
    /// Runs the dedicated server loop; from `net_dedicated.c` (not yet ported).
    fn NET_DedicatedServer();
    /// Queries the master server for a game list; from `net_query.c` (not yet ported).
    fn NET_MasterQuery();
    /// Queries a specific network address for a game; from `net_query.c` (not yet ported).
    fn NET_QueryAddress(addr: *mut c_char);
    /// Scans the local network (LAN) for games; from `net_query.c` (not yet ported).
    fn NET_LANQuery();

    // C standard library:
    /// Returns the length of the null-terminated C string `s`, excluding the null terminator.
    fn strlen(s: *const c_char) -> usize;
    /// Converts the initial portion of the null-terminated C string `nptr` to `c_int`.
    fn atoi(nptr: *const c_char) -> c_int;
    /// Terminates the process with the given exit `status` code.
    fn exit(status: c_int) -> !;
}

// ---------------------------------------------------------------------------
// D_DoomLoop
// ---------------------------------------------------------------------------

/// Main game loop — never returns.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// `-playdemo`/`-timedemo` early-return arms of [`doom_main`] and the
/// freeze-zone extern declarers link the upstream name (the
/// never-returns phrasing is the upstream doc, kept verbatim -- on this
/// port the function arms the loop latches, runs one boot tic and
/// returns).
#[doc(alias = "D_DoomLoop")]
#[export_name = "D_DoomLoop"]
pub extern "C" fn doom_loop() {
    unsafe {
        if bfgedition != 0 {
            let is_recording = demorecording;
            let is_playdemo = gameaction == ga_playdemo;
            if is_recording != 0 || is_playdemo || netgame != 0 {
                eprintln!(
                    " WARNING: You are playing using one of the Doom Classic\n\
                     IWAD files shipped with the Doom 3: BFG Edition. These are\n\
                     known to be incompatible with the regular IWAD files and\n\
                     may cause demos and network games to get out of sync."
                );
            }
        }

        if demorecording != 0 { G_BeginRecording(); }

        main_loop_started = 1;

        TryRunTics();

        I_SetWindowTitle(gamedescription);
        I_GraphicsCheckCommandLine();

        // Cast the Rust callback to match the C signature
        extern "C" fn grab_cb() -> Boolean {
            D_GrabMouseCallback()
        }
        I_SetGrabMouseCallback(grab_cb);

        I_InitGraphics();
        I_EnableLoadingDisk();

        V_RestoreBuffer();
        R_ExecuteSetViewSize();

        D_StartGameLoop();

        if testcontrols != 0 { wipegamestate = gamestate; }

        doomgeneric_Tick();
    }
}

// ---------------------------------------------------------------------------
// D_AddFile
// ---------------------------------------------------------------------------

/// Add a WAD file to the search path, printing its name to stdout.
///
/// Returns `true` if the file was opened successfully.
///
/// # Safety
///
/// `filename` must be a valid, null-terminated C string pointer that remains live for the
/// duration of the call.
#[doc(alias = "D_AddFile")]
unsafe fn add_file(filename: *mut c_char) -> bool {
    println!(" adding {}", c_str_to_str(filename));
    let handle = W_AddFile(filename);
    !handle.is_null()
}

// ---------------------------------------------------------------------------
// PrintDehackedBanners
// ---------------------------------------------------------------------------

/// Print any copyright banners that have been replaced by DEH patches.
///
/// # Safety
///
/// Reads the `COPYRIGHT_BANNERS` static; must be called after the DEH subsystem is initialized.
#[doc(alias = "PrintDehackedBanners")]
unsafe fn print_dehacked_banners() {
    for i in 0..COPYRIGHT_BANNERS.len() {
        let banner = COPYRIGHT_BANNERS[i].0;
        let deh_s = DEH_String(banner);

        if !std::ptr::eq(deh_s, banner) {
            print!("{}", c_str_to_str(deh_s));

            // Ensure modified banner ends in newline
            let len = strlen(deh_s);
            if len > 0 && *deh_s.add(len.wrapping_sub(1)) != b'\n' as c_char { println!(); }
        }
    }
}

// ---------------------------------------------------------------------------
// D_Endoom
// ---------------------------------------------------------------------------

/// `I_AtExit` callback: display the ENDOOM lump and exit when the game shuts down.
///
/// Skipped when `show_endoom` is 0, when the main loop never started, or in
/// screensaver / test-controls mode.
///
/// Address-taken only (`I_AtExit` registration in [`doom_main`]),
/// never linker-resolved -- therefore no pin.
#[doc(alias = "D_Endoom")]
extern "C" fn endoom() {
    unsafe {
        // Don't show ENDOOM if disabled, or in screensaver/control test mode.
        // Only show it once the game has actually started.
        if show_endoom == 0
            || main_loop_started == 0
            || M_CheckParm(c"-testcontrols".as_ptr().cast_mut()) > 0
        {
            return;
        }

        if screensaver_mode != 0 { return; }

        let endoom = W_CacheLumpName(DEH_String(c"ENDOOM".as_ptr()), PU_STATIC) as *mut u8;
        I_Endoom(endoom);

        exit(0);
    }
}

// ---------------------------------------------------------------------------
// D_DoomMain
// ---------------------------------------------------------------------------

/// Main entry point — initializes all subsystems.
///
/// The pre-move export symbol is kept with `#[export_name]` below; it
/// is the engine's boot link: `doomgeneric.rs` extern-declares and
/// calls it (the wasm and native shells' `doomgeneric_Create` path),
/// and the vendored C `vendor/doomgeneric/doomgeneric.c:25` references
/// the same symbol. The `myargv` lifetime contract (process-lifetime
/// argv storage; see the three harness docs) is untouched by this move.
#[doc(alias = "D_DoomMain")]
#[export_name = "D_DoomMain"]
pub extern "C" fn doom_main() {
    unsafe {
        let mut file: [c_char; 256] = [0; 256];
        let mut demolumpname: [c_char; 9] = [0; 9];

        I_AtExit(D_Endoom, Boolean::FALSE);

        // Print banner
        I_PrintBanner(c"Room".as_ptr().cast_mut());

        // Init zone memory
        println!("Z_Init: Init zone memory allocation daemon.");
        Z_Init();

        // Check command-line flags
        nomonsters = (M_CheckParm(c"-nomonsters".as_ptr().cast_mut()) != 0) as c_int;
        respawnparm = (M_CheckParm(c"-respawn".as_ptr().cast_mut()) != 0) as c_int;
        fastparm = (M_CheckParm(c"-fast".as_ptr().cast_mut()) != 0) as c_int;
        devparm = (M_CheckParm(c"-devparm".as_ptr().cast_mut()) != 0) as c_int;

        I_DisplayFPSDots(Boolean::from(devparm != 0));

        if M_CheckParm(c"-deathmatch".as_ptr().cast_mut()) != 0 { deathmatch = 1; }

        if M_CheckParm(c"-altdeath".as_ptr().cast_mut()) != 0 { deathmatch = 2; }

        if devparm != 0 { println!("{}", D_DEVSTR); }

        // Config directory
        M_SetConfigDir(ptr::null_mut());

        // Turbo mode
        let p = M_CheckParm(c"-turbo".as_ptr().cast_mut());
        if p > 0 {
            let mut scale: c_int = 200;

            if p < myargc - 1 { scale = atoi(*myargv.add((p + 1) as usize)); }
            scale = scale.clamp(10, 400);

            println!("turbo scale: {}%", scale);

            forwardmove[0] = forwardmove[0] * scale / 100;
            forwardmove[1] = forwardmove[1] * scale / 100;
            sidemove[0] = sidemove[0] * scale / 100;
            sidemove[1] = sidemove[1] * scale / 100;
        }

        // Init subsystems
        println!("V_Init: allocate screens.");
        V_Init();

        // Load configuration
        println!("M_LoadDefaults: Load system defaults.");
        M_SetConfigFilenames(
            c"default.cfg".as_ptr().cast_mut(),
            c"doom.cfg".as_ptr().cast_mut(),
        );
        D_BindVariables();
        M_LoadDefaults();

        I_AtExit(M_SaveDefaults, Boolean::FALSE);

        // Find main IWAD
        iwadfile = D_FindIWAD(1, &raw mut gamemission); // IWAD_MASK_DOOM = 1

        if iwadfile.is_null() { i_error!("Game mode indeterminate.  No IWAD file was found.  Try\nspecifying one with the '-iwad' command line parameter.\n"); }

        modifiedgame = Boolean::FALSE;

        println!("W_Init: Init WADfiles.");
        D_AddFile(iwadfile);

        W_CheckCorrectIWAD(d_mode::doom);

        // Identify version and game version
        D_IdentifyVersion();
        InitGameVersion();

        // BFG Edition check
        if W_CheckNumForName(c"dmenupic".as_ptr()) >= 0 {
            println!("BFG Edition: Using workarounds as needed.");
            bfgedition = 1;

            // BFG changes secret level names
            // (DEH replacements would go here, skipped since no dehacked)
        }

        // Load PWAD files
        modifiedgame = W_ParseCommandLine();

        // Check for -playdemo / -timedemo
        let p = M_CheckParmWithArgs(c"-playdemo".as_ptr().cast_mut(), 1);
        let mut _is_timedemo = false;
        let p = if p == 0 {
            let tp = M_CheckParmWithArgs(c"-timedemo".as_ptr().cast_mut(), 1);
            if tp > 0 { _is_timedemo = true; }
            tp
        } else {
            p
        };

        if p > 0 {
            let arg = *myargv.add((p + 1) as usize);

            // Copy demo name, handle .lmp extension
            if c_str_ends_with(arg, c".lmp".as_ptr()) {
                M_StringCopy(file.as_mut_ptr(), arg, file.len());
            } else {
                let arg_str = std::ffi::CStr::from_ptr(arg).to_string_lossy();
                c_write!(file, "{}.lmp", arg_str);
            }

            if D_AddFile(file.as_mut_ptr()) {
                // Copy lump name from the last loaded lump
                let name = (*lumpinfo.add(numlumps as usize - 1)).name;
                std::ptr::copy_nonoverlapping(name.as_ptr(), demolumpname.as_mut_ptr(), 8);
                demolumpname[8] = 0;
            } else {
                // Still continue like Vanilla Doom
                M_StringCopy(demolumpname.as_mut_ptr(), arg, demolumpname.len());
            }

            println!("Playing demo {}.", c_str_to_str(file.as_ptr()));
        }

        // Note: G_CheckDemoStatus atexit registration is handled via the game flow.

        W_GenerateHashTable();

        // Set game description
        D_SetGameDescription();

        // Savegame directory
        savegamedir = M_GetSaveGameDir(D_SaveGameIWADName(gamemission));

        // Check for -file in shareware
        if modifiedgame.is_truthy() {
            if gamemode == d_mode::shareware { i_error!("\nYou cannot -file with the shareware version. Register!"); }

            // Check for fake IWAD
            if gamemode == d_mode::registered { for i in 0..23 { if W_CheckNumForName(IWAD_CHECK_NAMES[i].0) < 0 { i_error!("\nThis is not the registered version."); } } }
        }

        // Warning about modified sprites
        if W_CheckNumForName(c"SS_START".as_ptr()) >= 0
            || W_CheckNumForName(c"FF_END".as_ptr()) >= 0
        {
            println!(
                " WARNING: The loaded WAD file contains modified sprites or\n\
                 floor textures.  You may want to use the '-merge' command\n\
                 line option instead of '-file'."
            );
        }

        // Print startup banner
        I_PrintStartupBanner(gamedescription);
        PrintDehackedBanners();

        // Freedoom warning
        if W_CheckNumForName(c"FREEDOOM".as_ptr()) >= 0 && W_CheckNumForName(c"FREEDM".as_ptr()) < 0
        {
            println!(
                " WARNING: You are playing using one of the Freedoom IWAD\n\
                 files, which might not work in this port. See this page\n\
                 for more information on how to play using Freedoom:\n\
                 http://www.chocolate-doom.org/wiki/index.php/Freedoom"
            );
            I_PrintDivider();
        }

        // Initialize I subsystems
        println!("I_Init: Setting up machine state.");
        I_CheckIsScreensaver();
        I_InitTimer();
        I_InitJoystick();
        I_InitSound(Boolean::TRUE);
        I_InitMusic();

        // Initial netgame startup
        D_ConnectNetGame();

        // Default skill/episode/map
        startskill = sk_medium;
        startepisode = 1;
        startmap = 1;
        autostart = 0;

        // -skill
        let p = M_CheckParmWithArgs(c"-skill".as_ptr().cast_mut(), 1);
        if p > 0 {
            startskill = *(*myargv.add((p + 1) as usize) as *const u8) as c_int - '1' as i32;
            autostart = 1;
        }

        // -episode
        let p = M_CheckParmWithArgs(c"-episode".as_ptr().cast_mut(), 1);
        if p > 0 {
            startepisode = *(*myargv.add((p + 1) as usize) as *const u8) as c_int - '0' as i32;
            startmap = 1;
            autostart = 1;
        }

        let mut _timelimit: c_int = 0;

        // -timer
        let p = M_CheckParmWithArgs(c"-timer".as_ptr().cast_mut(), 1);
        if p > 0 { _timelimit = atoi(*myargv.add((p + 1) as usize)); }

        // -avg
        if M_CheckParm(c"-avg".as_ptr().cast_mut()) != 0 { _timelimit = 20; }

        // -warp
        let p = M_CheckParmWithArgs(c"-warp".as_ptr().cast_mut(), 1);
        if p > 0 {
            if gamemode == d_mode::commercial { startmap = atoi(*myargv.add((p + 1) as usize)); }
            else {
                startepisode = *(*myargv.add((p + 1) as usize) as *const u8) as c_int - '0' as i32;
                if p + 2 < myargc { startmap = *(*myargv.add((p + 2) as usize) as *const u8) as c_int - '0' as i32; } else { startmap = 1; }
            }
            autostart = 1;
        }

        // -testcontrols
        if M_CheckParm(c"-testcontrols".as_ptr().cast_mut()) > 0 {
            startepisode = 1;
            startmap = 1;
            autostart = 1;
            testcontrols = 1;
        }

        // -loadgame
        let p = M_CheckParmWithArgs(c"-loadgame".as_ptr().cast_mut(), 1);
        if p > 0 { startloadgame = atoi(*myargv.add((p + 1) as usize)); } else { startloadgame = -1; }

        // Init remaining subsystems
        println!("M_Init: Init miscellaneous info.");
        M_Init();

        print!("R_Init: Init DOOM refresh daemon - ");
        R_Init();
        println!();

        println!("P_Init: Init Playloop state.");
        P_Init();

        println!("S_Init: Setting up sound.");
        S_Init(sfxVolume * 8, musicVolume * 8);

        println!("D_CheckNetGame: Checking network game status.");
        D_CheckNetGame();

        PrintGameVersion();

        println!("HU_Init: Setting up heads up display.");
        HU_Init();

        println!("ST_Init: Init status bar.");
        ST_Init();

        // Store demo check
        if gamemode == d_mode::commercial && W_CheckNumForName(c"map01".as_ptr()) < 0 { storedemo = 1; }

        // -statdump
        if M_CheckParmWithArgs(c"-statdump".as_ptr().cast_mut(), 1) > 0 { println!("External statistics registered."); }

        // -record
        let p = M_CheckParmWithArgs(c"-record".as_ptr().cast_mut(), 1);
        if p > 0 {
            G_RecordDemo(*myargv.add((p + 1) as usize));
            autostart = 1;
        }

        // -playdemo
        let p = M_CheckParmWithArgs(c"-playdemo".as_ptr().cast_mut(), 1);
        if p > 0 {
            singledemo = 1;
            G_DeferedPlayDemo(demolumpname.as_ptr());
            D_DoomLoop();
            return;
        }

        // -timedemo
        let p = M_CheckParmWithArgs(c"-timedemo".as_ptr().cast_mut(), 1);
        if p > 0 {
            G_TimeDemo(demolumpname.as_mut_ptr());
            D_DoomLoop();
            return;
        }

        // Load game
        if startloadgame >= 0 {
            M_StringCopy(file.as_mut_ptr(), P_SaveGameFile(startloadgame), file.len());
            G_LoadGame(file.as_mut_ptr());
        }

        // Start new game or title screen
        if gameaction != ga_loadgame {
            if autostart != 0 || netgame != 0 { G_InitNew(startskill, startepisode, startmap); }
            else { D_StartTitle(); }
        }

        D_DoomLoop();
    }
}
