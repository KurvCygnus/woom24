//! The screen-palette effects: damage red shifts, bonus gold shifts,
//! the radiation-suit palette, and the Chex Quest gore-free
//! substitution.

use std::ffi::c_int;

use super::consts::{
    NUMBONUSPALS, NUMREDPALS, RADIATIONPAL, STARTBONUSPALS, STARTREDPALS,
};
use super::{lu_palette, plyr, st_palette};
use crate::doom::d_mode;
use crate::doom::doomstat::gameversion;
use crate::doom::i_video::I_SetPalette;
use crate::doom::w_wad::W_CacheLumpNum;
use crate::doom::z_zone::PU_CACHE;

/// Apply the appropriate screen palette based on the player's current status.
///
/// Priority order (highest first):
/// 1. Damage / berserk: red palette shift scaled by damage count.
/// 2. Bonus pickup: gold/yellow palette shift scaled by bonus count.
/// 3. Radiation suit: fixed `RADIATIONPAL` index.
/// 4. Normal: palette 0.
///
/// In Chex Quest the red-damage palettes are replaced with `RADIATIONPAL` to
/// avoid gore. Only calls `I_SetPalette` when the palette index changes.
///
/// # Safety
///
/// Dereferences the global `plyr` pointer and mutates `st_palette`. Calls
/// `W_CacheLumpNum`/`I_SetPalette` so the WAD subsystem and video backend
/// must be initialised, and `lu_palette` must hold a valid lump number set
/// up by `assets::load_data`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `drawer::drawer` reaches the upstream name through the root shim.
#[doc(alias = "ST_doPaletteStuff")]
#[export_name = "ST_doPaletteStuff"]
pub unsafe extern "C" fn apply_palette() {
    let mut palette: c_int;
    let mut cnt = (*plyr).damagecount;

    if (*plyr).powers[1] != 0 {
        // pw_strength
        let bzc = 12 - ((*plyr).powers[1] >> 6);
        if bzc > cnt {
            cnt = bzc;
        }
    }

    if cnt != 0 {
        palette = (cnt + 7) >> 3;
        if palette >= NUMREDPALS {
            palette = NUMREDPALS - 1;
        }
        palette += STARTREDPALS;
    } else if (*plyr).bonuscount != 0 {
        palette = ((*plyr).bonuscount + 7) >> 3;
        if palette >= NUMBONUSPALS {
            palette = NUMBONUSPALS - 1;
        }
        palette += STARTBONUSPALS;
    } else if (*plyr).powers[3] > 4 * 32 || (*plyr).powers[3] & 8 != 0 {
        // pw_ironfeet
        palette = RADIATIONPAL;
    } else {
        palette = 0;
    }

    if gameversion == d_mode::exe_chex
        && (STARTREDPALS..STARTREDPALS + NUMREDPALS).contains(&palette)
    {
        palette = RADIATIONPAL;
    }

    if palette != st_palette {
        st_palette = palette;
        let pal = (W_CacheLumpNum(lu_palette, PU_CACHE) as *mut u8).add((palette * 768) as usize);
        I_SetPalette(pal);
    }
}
