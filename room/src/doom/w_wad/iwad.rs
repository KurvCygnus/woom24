//! Launch-time IWAD validation: refuse to start when the supplied
//! IWAD's unique marker lump belongs to a different game mission.

use std::ffi::{c_char, c_int, CStr};

use super::lookup::check_num_for_name;
use crate::doom::d_iwad::D_SuggestGameName;
use crate::doom::d_mode::D_GameMissionString;
use crate::i_error;

/// Refuse to launch when the user supplies an IWAD whose unique
/// marker lump belongs to a different game (e.g. running with
/// `hexen.wad` while the active mission is Doom).
///
/// For each `(mission, lumpname)` pair in `UNIQUE_LUMPS`, if the
/// active `mission` differs but the lump is still present, call
/// `I_Error` with a friendly message suggesting the right binary.
///
/// The C source uses a `PROGRAM_PREFIX` macro for the binary name;
/// this port hardcodes `"doomgeneric"`.
#[doc(alias = "W_CheckCorrectIWAD")]
pub extern "C" fn check_correct_iwad(mission: c_int)
{
    /// One row of the IWAD-mismatch detection table.
    struct UniqueLump
    {
        /// `GameMission_t` value for which this lump is expected.
        mission: c_int,
        /// 8-char lump name (NUL-padded) that uniquely identifies
        /// the mission's IWAD.
        lumpname: &'static [u8],
    }

    /// Table of lumps that uniquely identify each supported IWAD.
    /// Order/contents mirror the `unique_lumps[]` array in `w_wad.c`.
    const UNIQUE_LUMPS: [UniqueLump; 4] = [
        UniqueLump
        {
            mission: 0, // doom
            lumpname: b"POSSA1\0\0",
        },
        UniqueLump
        {
            mission: 6, // heretic
            lumpname: b"IMPXA1\0\0",
        },
        UniqueLump
        {
            mission: 7, // hexen
            lumpname: b"ETTNA1\0\0",
        },
        UniqueLump
        {
            mission: 8, // strife
            lumpname: b"AGRDA1\0\0",
        },
    ];

    unsafe
    {
        for ul in &UNIQUE_LUMPS
        {
            if mission != ul.mission
            {
                let lumpnum = check_num_for_name(ul.lumpname.as_ptr() as *mut c_char);
                if lumpnum >= 0
                {
                    i_error!(
                        "\nYou are trying to use a {} IWAD file with the {}{}binary.\nThis isn't going to work.\nYou probably want to use the {}{}binary.",
                        CStr::from_ptr(D_SuggestGameName(ul.mission, 4)).to_string_lossy(),
                        "doomgeneric",
                        CStr::from_ptr(D_GameMissionString(mission)).to_string_lossy(),
                        "doomgeneric",
                        CStr::from_ptr(D_GameMissionString(ul.mission)).to_string_lossy()
                    );
                }
            }
        }
    }
}
