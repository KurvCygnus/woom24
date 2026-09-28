//! Game identification and description: `identify_version` (upstream
//! `D_IdentifyVersion`), `init_game_version` (upstream
//! `InitGameVersion`), `set_game_description` (upstream
//! `D_SetGameDescription`), `print_game_version` (upstream
//! `PrintGameVersion`) and their helpers. One-shot boot glue -- the
//! identified `gamemode`/`gamemission`/`gameversion` are upstream
//! *inputs* to the demo surface, not the per-tic surface itself.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use crate::i_error;
use crate::doom::crt::c_snprintf2;
use crate::doom::d_mode;
use crate::doom::doomstat::{gamedescription, gamemission, gamemode, gameversion};
use crate::doom::g_game::G_VanillaVersionCode;
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::m_misc::M_snprintf_clamp;
use crate::doom::w_wad::{lumpinfo, numlumps, W_CheckNumForName};
use crate::doom::z_zone::{Z_Malloc, PU_STATIC};

use self::{get_game_name as GetGameName, set_mission_for_pack_name as SetMissionForPackName};
use super::compat::{deh_string as DEH_String, eq_ci as c_str_eq, ne_ci_n as c_str_ne_n, to_lossy_string as c_str_to_str};
use super::consts::{banners, GAME_VERSIONS, PACKS};

extern "C" {
    /// Returns the length of the null-terminated C string `s`, excluding the null terminator.
    fn strlen(s: *const c_char) -> usize;
    /// Compares two null-terminated C strings lexicographically; returns 0 if equal.
    fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    /// Returns non-zero if `c` is a whitespace character per the current locale.
    fn isspace(c: c_int) -> c_int;
    /// Copies `n` bytes from `src` to `dest`, handling overlapping regions correctly.
    fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

/// Find out what version of Doom is playing.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `boot::doom_main` calls the upstream name through the in-module
/// alias.
#[doc(alias = "D_IdentifyVersion")]
#[export_name = "D_IdentifyVersion"]
pub extern "C" fn identify_version() {
    unsafe {
        if gamemission == d_mode::none {
            for i in 0..numlumps {
                let name_ptr = (*lumpinfo.add(i as usize)).name.as_ptr();

                if !c_str_ne_n(name_ptr, c"MAP01".as_ptr(), 8) {
                    gamemission = d_mode::doom2;
                    break;
                }
                else if !c_str_ne_n(name_ptr, c"E1M1".as_ptr(), 8) {
                    gamemission = d_mode::doom;
                    break;
                }
            }

            if gamemission == d_mode::none { i_error!("Unknown or invalid IWAD file."); }
        }

        // Make sure gamemode is set up correctly
        let logical_mission = logical_gamemission();
        if logical_mission == d_mode::doom {
            // Doom 1. But which version?
            if W_CheckNumForName(c"E4M1".as_ptr()) > 0 { gamemode = d_mode::retail; }
            else if W_CheckNumForName(c"E3M1".as_ptr()) > 0 { gamemode = d_mode::registered; }
            else { gamemode = d_mode::shareware; }
        }
        else {
            // Doom 2 of some kind.
            gamemode = d_mode::commercial;

            // Manually override gamemission with -pack
            let p = M_CheckParmWithArgs(c"-pack".as_ptr().cast_mut(), 1);
            if p > 0 { SetMissionForPackName(*myargv.add((p + 1) as usize)); }
        }
    }
}

/// Return the logical game mission, collapsing pack-specific variants to their base mission.
///
/// Mirrors the `logical_gamemission` macro from `doomstat.h`: `pack_chex` maps to `doom` and
/// `pack_hacx` maps to `doom2`; all other values are returned unchanged.
///
/// # Safety
///
/// Reads the `gamemission` global; must be called in a context where the global is initialized.
///
/// Known duplicate of `g_game`'s mirror of the same macro -- kept both;
/// deduping across modules is post-freeze-zone work.
unsafe fn logical_gamemission() -> c_int {
    if gamemission == d_mode::pack_chex { d_mode::doom }
    else if gamemission == d_mode::pack_hacx { d_mode::doom2 }
    else { gamemission }
}

/// Set the gamedescription string.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `boot::doom_main` calls the upstream name through the in-module
/// alias.
#[doc(alias = "D_SetGameDescription")]
#[export_name = "D_SetGameDescription"]
pub extern "C" fn set_game_description() {
    unsafe {
        let is_freedoom = W_CheckNumForName(c"FREEDOOM".as_ptr()) >= 0;
        let is_freedm = W_CheckNumForName(c"FREEDM".as_ptr()) >= 0;

        gamedescription = c"Unknown".as_ptr().cast_mut();

        let logical_mission = logical_gamemission();
        if logical_mission == d_mode::doom {
            // Doom 1. But which version?
            if is_freedoom { gamedescription = GetGameName(c"Freedoom: Phase 1".as_ptr().cast_mut()); }
            else if gamemode == d_mode::retail { gamedescription = GetGameName(c"The Ultimate DOOM".as_ptr().cast_mut()); }
            else if gamemode == d_mode::registered { gamedescription = GetGameName(c"DOOM Registered".as_ptr().cast_mut()); }
            else if gamemode == d_mode::shareware { gamedescription = GetGameName(c"DOOM Shareware".as_ptr().cast_mut()); }
        }
        else {
            // Doom 2 of some kind.
            if is_freedoom {
                if is_freedm { gamedescription = GetGameName(c"FreeDM".as_ptr().cast_mut()); }
                else { gamedescription = GetGameName(c"Freedoom: Phase 2".as_ptr().cast_mut()); }
            }
            else if logical_mission == d_mode::doom2 { gamedescription = GetGameName(c"DOOM 2: Hell on Earth".as_ptr().cast_mut()); }
            else if logical_mission == d_mode::pack_plut { gamedescription = GetGameName(c"DOOM 2: Plutonia Experiment".as_ptr().cast_mut()); }
            else if logical_mission == d_mode::pack_tnt { gamedescription = GetGameName(c"DOOM 2: TNT - Evilution".as_ptr().cast_mut()); }
        }
    }
}

/// Return the game name string.
///
/// If any startup banner has been replaced by a DEH patch, the patched string is formatted and
/// returned (allocated from the zone heap). Otherwise `gamename` is returned unchanged.
///
/// # Safety
///
/// `gamename` must be a valid, null-terminated C string pointer that remains live for the
/// duration of the call. Each element of the `banners` static array must be a valid,
/// null-terminated C string (guaranteed for the compile-time string literals stored there).
#[doc(alias = "GetGameName")]
unsafe fn get_game_name(gamename: *mut c_char) -> *mut c_char {
    for i in 0..7 {
        let banner = banners[i];
        let deh_sub = DEH_String(banner);

        // Has been replaced?
        if !std::ptr::eq(deh_sub, banner) {
            let gamename_size = strlen(deh_sub) + 10;
            let expanded =
                Z_Malloc(gamename_size as c_int, PU_STATIC, ptr::null_mut()) as *mut c_char;

            let version = G_VanillaVersionCode();
            // SAFETY: `deh_sub` is a valid, null-terminated C string allocated by Z_Malloc via
            // DEH_String and live for the duration of this call. The format string always contains
            // exactly two `%i` specifiers (matching the banner patterns), and `version/100` /
            // `version%100` are both `c_int` values that satisfy them. `expanded` has room for
            // `gamename_size` bytes. Dynamic format from DEH — cannot use c_write! (not a literal).
            let result = c_snprintf2(
                expanded,
                gamename_size,
                deh_sub,
                version / 100,
                version % 100,
            );
            M_snprintf_clamp(expanded, gamename_size, result);

            // Trim leading spaces
            let mut start = 0;
            while *expanded.add(start) != 0 && isspace(*expanded.add(start) as c_int) != 0 { start += 1; }
            if start > 0 {
                let remaining = strlen(expanded).wrapping_sub(start) + 1;
                memmove(
                    expanded as *mut c_void,
                    expanded.add(start) as *const c_void,
                    remaining,
                );
            }

            // Trim trailing spaces
            let mut len = strlen(expanded);
            while len > 0 && isspace(*expanded.add(len.wrapping_sub(1)) as c_int) != 0 {
                *expanded.add(len.wrapping_sub(1)) = 0;
                len -= 1;
            }

            return expanded;
        }
    }

    gamename
}

/// Set `gamemission` from the `-pack` argument string, or abort with an error if unrecognized.
///
/// # Safety
///
/// `pack_name` must be a valid, null-terminated C string pointer that remains live for the
/// duration of the call.
#[doc(alias = "SetMissionForPackName")]
unsafe fn set_mission_for_pack_name(pack_name: *mut c_char) {
    for pack in &PACKS { if c_str_eq(pack_name, pack.name.0) { gamemission = pack.mission; return; } }

    println!("Valid mission packs are:");
    for pack in &PACKS {
        if pack.name.0.is_null() { break; }
        println!("\t{}", c_str_to_str(pack.name.0));
    }

    i_error!("Unknown mission pack name");
}

/// Set `gameversion` from the `-gameversion` argument, or auto-detect it from `gamemode` and
/// `gamemission`. Also adjusts `gamemode` / `gamemission` for version compatibility.
///
/// # Safety
///
/// Reads and writes multiple game-state globals; must be called after `D_IdentifyVersion`.
#[doc(alias = "InitGameVersion")]
pub(super) unsafe fn init_game_version() {
    let p = M_CheckParmWithArgs(c"-gameversion".as_ptr().cast_mut(), 1);

    if p > 0 {
        let arg = *myargv.add((p + 1) as usize);

        let mut found = false;
        for gv in &GAME_VERSIONS {
            if gv.description.0.is_null() { break; }
            if strcmp(arg, gv.cmdline.0) == 0 {
                gameversion = gv.version;
                found = true;
                break;
            }
        }

        if !found {
            println!("Supported game versions:");
            for gv in &GAME_VERSIONS {
                if gv.description.0.is_null() { break; }
                println!(
                    "\t{} ({})",
                    c_str_to_str(gv.cmdline.0),
                    c_str_to_str(gv.description.0)
                );
            }

            i_error!("Unknown game version");
        }
    }
    else {
        // Determine automatically
        if gamemission == d_mode::pack_chex { gameversion = d_mode::exe_chex; }
        else if gamemission == d_mode::pack_hacx { gameversion = d_mode::exe_hacx; }
        else if gamemode == d_mode::shareware || gamemode == d_mode::registered { gameversion = d_mode::exe_doom_1_9; }
        else if gamemode == d_mode::retail { gameversion = d_mode::exe_ultimate; }
        else if gamemode == d_mode::commercial {
            if gamemission == d_mode::doom2 { gameversion = d_mode::exe_doom_1_9; }
            else { gameversion = d_mode::exe_final; } // Final Doom: tnt or plutonia; defaults to the first Final Doom executable
        }
    }

    // Original exe does not support retail - 4th episode not supported
    if gameversion < d_mode::exe_ultimate && gamemode == d_mode::retail { gamemode = d_mode::registered; }

    // EXEs prior to Final Doom do not support Final Doom
    if gameversion < d_mode::exe_final
        && gamemode == d_mode::commercial
        && (gamemission == d_mode::pack_tnt || gamemission == d_mode::pack_plut)
    {
        gamemission = d_mode::doom2;
    }
}

/// Print a startup message identifying which original executable version is being emulated.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `boot::doom_main` calls the upstream name through the in-module
/// alias.
#[doc(alias = "PrintGameVersion")]
#[export_name = "PrintGameVersion"]
pub extern "C" fn print_game_version() {
    unsafe {
        for gv in &GAME_VERSIONS {
            if gv.description.0.is_null() { break; }
            if gv.version == gameversion {
                println!(
                    "Emulating the behavior of the '{}' executable.",
                    c_str_to_str(gv.description.0)
                );
                break;
            }
        }
    }
}
