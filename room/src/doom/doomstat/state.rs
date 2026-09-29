//! The five `doomstat.c` statics. Names and `#[no_mangle]` exports are the
//! upstream symbol set and never change.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::doom::d_mode;
use crate::types::Boolean;

/// Identifies which IWAD variant is loaded (shareware, registered, retail, etc.).
///
/// Corresponds to `GameMode_t gamemode` in `doomstat.c`.  The value is one of
/// the `d_mode` integer constants (e.g., `d_mode::indetermined`,
/// `d_mode::shareware`, `d_mode::registered`, `d_mode::retail`).
/// Initialised to `indetermined`; set by `D_IdentifyVersion` during startup.
/// Referenced by virtually every module that needs to branch on game edition.
#[no_mangle]
pub static mut gamemode: c_int = d_mode::indetermined;

/// Identifies the game mission/product line (Doom, Doom II, TNT, Plutonia, etc.).
///
/// Corresponds to `GameMission_t gamemission` in `doomstat.c`.  The value is
/// one of the `d_mode` integer constants (e.g., `d_mode::doom`,
/// `d_mode::doom2`).  Initialised to `doom`; updated alongside `gamemode`
/// during IWAD detection.
#[no_mangle]
pub static mut gamemission: c_int = d_mode::doom;

/// Identifies the target engine version for compatibility purposes.
///
/// Corresponds to `GameVersion_t gameversion` in `doomstat.c`.  Controls
/// which demo format is used, whether certain Vanilla quirks are active, etc.
/// Initialised to `exe_final2` and may be overridden by `-gameversion` on the
/// command line.
#[no_mangle]
pub static mut gameversion: c_int = d_mode::exe_final2;

/// Human-readable description of the loaded IWAD (e.g. `"DOOM Registered"`).
///
/// Corresponds to `char *gamedescription` in `doomstat.c`.  Set by
/// `D_SetGameDescription` during startup and used by the window title and
/// help text.  Null until initialised.
#[no_mangle]
pub static mut gamedescription: *mut c_char = ptr::null_mut();

/// `true` if any PWAD files have been loaded on top of the base IWAD.
///
/// Corresponds to `boolean modifiedgame` in `doomstat.c`.  Set in
/// `W_CheckNumLumps` whenever a non-IWAD WAD is added.  When true, savegame
/// compatibility warnings may be shown and certain strict-mode features are
/// disabled.
#[no_mangle]
pub static mut modifiedgame: Boolean = Boolean::FALSE;

#[cfg(test)]
mod tests {
    use crate::doom::doomstat::{gamedescription, gamemission, gamemode, gameversion, modifiedgame};
    use crate::doom::d_mode;
    use crate::types::Boolean;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn defaults_match_c() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(gamemode, d_mode::indetermined);
            assert_eq!(gamemission, d_mode::doom);
            assert_eq!(gameversion, d_mode::exe_final2);
            assert!(gamedescription.is_null());
            assert_eq!(modifiedgame, Boolean::FALSE);
        }
    }
}
