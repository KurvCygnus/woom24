//! The mission-naming helper for WAD-load diagnostics: map a
//! `GameMission_t` value to its canonical short name, verbatim port of
//! `D_GameMissionString` in `vendor/doomgeneric/d_mode.c`.
//! 
//! The C enum vocabulary is lowercase (`doom`, `pack_tnt`, ...); names
//! are verbatim upstream data and the match arms below reference them
//! as-is, so the two name lints are silenced file-wide.
#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};

use super::consts::{doom, doom2, heretic, hexen, pack_chex, pack_hacx, pack_plut, pack_tnt, strife};

/// Return a NUL-terminated C string naming the given mission.
///
/// Returns one of `"doom"`, `"doom2"`, `"tnt"`, `"plutonia"`, `"hacx"`,
/// `"chex"`, `"heretic"`, `"hexen"`, `"strife"`, or `"none"` for unrecognised
/// values. The returned pointer refers to a static string literal embedded in
/// the binary and must not be freed or written through.
///
/// The return type is `*mut c_char` to match the C signature, but the memory
/// is read-only; writing to it is undefined behaviour, as it would be in the C
/// original.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// sole live consumer is the graduated `w_wad/iwad.rs`, served by the
/// upstream-name shim at the module root.
///
/// Corresponds to `D_GameMissionString` in `d_mode.c`.
#[doc(alias = "D_GameMissionString")]
#[export_name = "D_GameMissionString"]
pub extern "C" fn game_mission_string(mission: c_int) -> *mut c_char
{
    match mission
    {
        doom => c"doom".as_ptr().cast_mut(),
        doom2 => c"doom2".as_ptr().cast_mut(),
        pack_tnt => c"tnt".as_ptr().cast_mut(),
        pack_plut => c"plutonia".as_ptr().cast_mut(),
        pack_hacx => c"hacx".as_ptr().cast_mut(),
        pack_chex => c"chex".as_ptr().cast_mut(),
        heretic => c"heretic".as_ptr().cast_mut(),
        hexen => c"hexen".as_ptr().cast_mut(),
        strife => c"strife".as_ptr().cast_mut(),
        _ => c"none".as_ptr().cast_mut(),
    }
}

/// The three mission-naming vectors from the pre-move test module,
/// moved unchanged and re-pointed to the graduated name with cross-root
/// consts imported by module-root path (F10 §1 convention (b)).
#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, CStr};

    use super::*;
    use crate::doom::d_mode::{doom, doom2, heretic, hexen, none, pack_chex, pack_hacx, pack_plut, pack_tnt, strife};

    /// Verifies that `D_GameMissionString` returns `"heretic"` for the Heretic mission.
    #[test]
    fn game_mission_string_heretic()
    {
        let ptr = game_mission_string(heretic);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "heretic");
    }

    /// Verifies that an unrecognised mission value returns `"none"`.
    #[test]
    fn game_mission_string_unknown()
    {
        let ptr = game_mission_string(42);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "none");
    }

    /// D_GameMissionString returns the right string for every known mission.
    #[test]
    fn game_mission_string_all_missions()
    {
        use std::ffi::CStr;
        let cases: &[(c_int, &str)] = &[
            (doom, "doom"),
            (doom2, "doom2"),
            (pack_tnt, "tnt"),
            (pack_plut, "plutonia"),
            (pack_hacx, "hacx"),
            (pack_chex, "chex"),
            (heretic, "heretic"),
            (hexen, "hexen"),
            (strife, "strife"),
            (none, "none"),
        ];
        for (mission, expected) in cases
        {
            let ptr = game_mission_string(*mission);
            let s = unsafe { CStr::from_ptr(ptr) };
            assert_eq!(s.to_str().unwrap(), *expected, "mission={mission}");
        }
    }
}
