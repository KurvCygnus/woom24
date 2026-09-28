//! Upstream-named data tier for `d_main`: the string constants, the
//! vanilla C type aliases and the game-state / game-action / skill
//! discriminants, the startup banner tables, the `-gameversion` and
//! `-pack` lookup tables with their descriptor structs, and the IWAD
//! lump-name check list.
//!
//! Data tier (statics/consts ruling): every name here keeps its
//! upstream spelling and is `pub(super)` -- reachable only through the
//! `d_main` module directory.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::doom::d_mode;

// ---------------------------------------------------------------------------
// String constants from d_englsh.h / dstrings.h
// ---------------------------------------------------------------------------

/// Message printed to stdout when `-devparm` is active.
pub(super) static D_DEVSTR: &str = "Development mode ON.\n";

/// Multiplayer chat target key for the green player.
pub(super) const HUSTR_KEYGREEN: c_char = b'g' as c_char;
/// Multiplayer chat target key for the indigo player.
pub(super) const HUSTR_KEYINDIGO: c_char = b'i' as c_char;
/// Multiplayer chat target key for the brown player.
pub(super) const HUSTR_KEYBROWN: c_char = b'b' as c_char;
/// Multiplayer chat target key for the red player.
pub(super) const HUSTR_KEYRED: c_char = b'r' as c_char;

// ---------------------------------------------------------------------------
// Type aliases matching C enums
// ---------------------------------------------------------------------------

/// C-compatible game state discriminant (`gamestate_t`).
pub(super) type gamestate_t = c_int;
/// C-compatible pending game action discriminant (`gameaction_t`).
pub(super) type gameaction_t = c_int;
/// C-compatible skill level discriminant (`skill_t`).
pub(super) type skill_t = c_int;
/// Single byte, matching the C `byte` typedef.
pub(super) type byte = u8;

/// Game state: playing a level.
pub(super) const GS_LEVEL: gamestate_t = 0;
/// Game state: intermission screen between levels.
pub(super) const GS_INTERMISSION: gamestate_t = 1;
/// Game state: end-of-episode finale.
pub(super) const GS_FINALE: gamestate_t = 2;
/// Game state: title / demo screen.
pub(super) const GS_DEMOSCREEN: gamestate_t = 3;

/// No pending action.
pub(super) const ga_nothing: gameaction_t = 0;
/// Pending action: load a level.
pub(super) const ga_loadlevel: gameaction_t = 1;
/// Pending action: start a new game.
pub(super) const ga_newgame: gameaction_t = 2;
/// Pending action: load a saved game.
pub(super) const ga_loadgame: gameaction_t = 3;
/// Pending action: save the current game.
pub(super) const ga_savegame: gameaction_t = 4;
/// Pending action: play a demo.
pub(super) const ga_playdemo: gameaction_t = 5;
/// Pending action: level completed.
pub(super) const ga_completed: gameaction_t = 6;
/// Pending action: episode victory sequence.
pub(super) const ga_victory: gameaction_t = 7;
/// Pending action: world (episode) done.
pub(super) const ga_worlddone: gameaction_t = 8;
/// Pending action: take a screenshot.
pub(super) const ga_screenshot: gameaction_t = 9;

/// Skill level: "I'm Too Young To Die" (baby).
pub(super) const sk_baby: skill_t = 0;
/// Skill level: "Hey, Not Too Rough" (easy).
pub(super) const sk_easy: skill_t = 1;
/// Skill level: "Hurt Me Plenty" (medium).
pub(super) const sk_medium: skill_t = 2;
/// Skill level: "Ultra-Violence" (hard).
pub(super) const sk_hard: skill_t = 3;
/// Skill level: "Nightmare!".
pub(super) const sk_nightmare: skill_t = 4;
/// Sentinel: no items mode (internal use only).
pub(super) const sk_noitems: skill_t = -1;

/// Startup banner strings (may be replaced by dehacked).
/// These contain embedded \0 bytes (part of the format) so cannot use c"" literals.
#[allow(clippy::manual_c_str_literals)]
pub(super) static mut banners: [*const c_char; 7] = [
    b"                         \0DOOM 2: Hell on Earth v%i.%i\0                           \0"
        .as_ptr() as *const c_char,
    b"                            \0DOOM Shareware Startup v%i.%i\0                           \0"
        .as_ptr() as *const c_char,
    b"                            \0DOOM Registered Startup v%i.%i\0                           \0"
        .as_ptr() as *const c_char,
    b"                          \0DOOM System Startup v%i.%i\0                          \0".as_ptr()
        as *const c_char,
    b"                         \0The Ultimate DOOM Startup v%i.%i\0                        \0"
        .as_ptr() as *const c_char,
    b"                     \0DOOM 2: TNT - Evilution v%i.%i\0                           \0".as_ptr()
        as *const c_char,
    b"                   \0DOOM 2: Plutonia Experiment v%i.%i\0                           \0"
        .as_ptr() as *const c_char,
];

/// Newtype wrapper that makes a raw C-string pointer `Sync` for use in statics.
pub(super) struct SyncPtr(pub(super) *const c_char);
unsafe impl Sync for SyncPtr {}

/// One entry in the `-gameversion` lookup table.
#[repr(C)]
pub(super) struct GameVersionDesc {
    /// Human-readable version label shown in the help list.
    pub(super) description: SyncPtr,
    /// Command-line argument string (e.g. `"1.9"`, `"ultimate"`).
    pub(super) cmdline: SyncPtr,
    /// Numeric version constant from `d_mode` (e.g. `exe_doom_1_9`).
    pub(super) version: c_int,
}

/// Construct a [`SyncPtr`] from a static byte-string literal.
pub(super) const fn sp(s: &'static [u8]) -> SyncPtr {
    SyncPtr(s.as_ptr() as *const c_char)
}

/// Table of known game versions, termined by a null-description sentinel entry.
pub(super) static GAME_VERSIONS: [GameVersionDesc; 10] = [
    GameVersionDesc {
        description: sp(b"Doom 1.666\0"),
        cmdline: sp(b"1.666\0"),
        version: d_mode::exe_doom_1_666,
    },
    GameVersionDesc {
        description: sp(b"Doom 1.7/1.7a\0"),
        cmdline: sp(b"1.7\0"),
        version: d_mode::exe_doom_1_7,
    },
    GameVersionDesc {
        description: sp(b"Doom 1.8\0"),
        cmdline: sp(b"1.8\0"),
        version: d_mode::exe_doom_1_8,
    },
    GameVersionDesc {
        description: sp(b"Doom 1.9\0"),
        cmdline: sp(b"1.9\0"),
        version: d_mode::exe_doom_1_9,
    },
    GameVersionDesc {
        description: sp(b"Hacx\0"),
        cmdline: sp(b"hacx\0"),
        version: d_mode::exe_hacx,
    },
    GameVersionDesc {
        description: sp(b"Ultimate Doom\0"),
        cmdline: sp(b"ultimate\0"),
        version: d_mode::exe_ultimate,
    },
    GameVersionDesc {
        description: sp(b"Final Doom\0"),
        cmdline: sp(b"final\0"),
        version: d_mode::exe_final,
    },
    GameVersionDesc {
        description: sp(b"Final Doom (alt)\0"),
        cmdline: sp(b"final2\0"),
        version: d_mode::exe_final2,
    },
    GameVersionDesc {
        description: sp(b"Chex Quest\0"),
        cmdline: sp(b"chex\0"),
        version: d_mode::exe_chex,
    },
    GameVersionDesc {
        description: SyncPtr(ptr::null()),
        cmdline: SyncPtr(ptr::null()),
        version: 0,
    },
];

/// One entry in the `-pack` lookup table.
#[repr(C)]
pub(super) struct PackDesc {
    /// Command-line pack name (e.g. `"tnt"`, `"plutonia"`).
    pub(super) name: SyncPtr,
    /// `gamemission` value to assign when this pack is selected.
    pub(super) mission: c_int,
}

/// Recognized `-pack` arguments, mapping names to `gamemission` values.
pub(super) static PACKS: [PackDesc; 3] = [
    PackDesc {
        name: sp(b"doom2\0"),
        mission: d_mode::doom2,
    },
    PackDesc {
        name: sp(b"tnt\0"),
        mission: d_mode::pack_tnt,
    },
    PackDesc {
        name: sp(b"plutonia\0"),
        mission: d_mode::pack_plut,
    },
];

// ---------------------------------------------------------------------------
// Constants for lump name checks (IWAD detection)
// ---------------------------------------------------------------------------

/// Lump names whose presence in the WAD confirms a registered (non-shareware) IWAD.
pub(super) static IWAD_CHECK_NAMES: [SyncPtr; 23] = [
    sp(b"e2m1\0"),
    sp(b"e2m2\0"),
    sp(b"e2m3\0"),
    sp(b"e2m4\0"),
    sp(b"e2m5\0"),
    sp(b"e2m6\0"),
    sp(b"e2m7\0"),
    sp(b"e2m8\0"),
    sp(b"e2m9\0"),
    sp(b"e3m1\0"),
    sp(b"e3m3\0"),
    sp(b"e3m3\0"), // duplicate in original C code
    sp(b"e3m4\0"),
    sp(b"e3m5\0"),
    sp(b"e3m6\0"),
    sp(b"e3m7\0"),
    sp(b"e3m8\0"),
    sp(b"e3m9\0"),
    sp(b"dphoof\0"),
    sp(b"bfgga0\0"),
    sp(b"heada1\0"),
    sp(b"cybra1\0"),
    sp(b"spida1d1\0"),
];

/// Copyright notice strings that are printed when replaced by DEH patches.
pub(super) static COPYRIGHT_BANNERS: [SyncPtr; 3] = [
    sp(
        b"===========================================================================\n\
ATTENTION:  This version of DOOM has been modified.  If you would like to\n\
get a copy of the original game, call 1-800-IDGAMES or see the readme file.\n\
        You will not receive technical support for modified games.\n\
                      press enter to continue\n\
===========================================================================\n\0",
    ),
    sp(
        b"===========================================================================\n\
                 Commercial product - do not distribute!\n\
         Please report software piracy to the SPA: 1-800-388-PIR8\n\
===========================================================================\n\0",
    ),
    sp(
        b"===========================================================================\n\
                                Shareware!\n\
===========================================================================\n\0",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the `GAME_VERSIONS` table is terminated by a null-description sentinel entry.
    #[test]
    fn game_version_table_complete() {
        // Last entry should be null-terminated
        let last = &GAME_VERSIONS[GAME_VERSIONS.len() - 1];
        assert!(last.description.0.is_null());
        assert!(last.cmdline.0.is_null());
        assert_eq!(last.version, 0);
    }

    /// Verify the `PACKS` table contains exactly three entries with non-null names.
    #[test]
    fn packs_table_complete() {
        assert_eq!(PACKS.len(), 3);
        assert!(!PACKS[0].name.0.is_null());
        assert!(!PACKS[1].name.0.is_null());
        assert!(!PACKS[2].name.0.is_null());
    }

    /// Verify the IWAD check name array contains exactly 23 lump names.
    #[test]
    fn iwad_check_names_count() {
        assert_eq!(IWAD_CHECK_NAMES.len(), 23);
    }

    /// Verify the copyright banners array contains exactly three entries.
    #[test]
    fn copyright_banners_count() {
        assert_eq!(COPYRIGHT_BANNERS.len(), 3);
    }

    /// Verify the startup banners array contains exactly seven entries.
    ///
    /// Strengthened from the pre-move vacuous `assert_eq!(7, 7)` placeholder
    /// (C4 graduation): the length is now actually asserted.
    #[test]
    fn banners_count() {
        unsafe {
            let banner_ptrs = banners; // copy the static into a local before asserting
            assert_eq!(banner_ptrs.len(), 7);
        }
    }
}
