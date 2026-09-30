//! Level-data cache warming: `precache_level`, called by `p_setup` at the
//! end of `P_SetupLevel`. A pure cache-warming pass -- skipped entirely
//! during demo playback, so it has zero frame or state effect on the
//! golden path; the `acp1 == P_MobjThinker` pointer-compare thinker walk
//! moves verbatim.

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::mobj_t;
use crate::doom::g_game::demoplayback;
use crate::doom::p_mobj::P_MobjThinker;
use crate::doom::p_setup::{numsectors, numsides, sectors, sides};
use crate::doom::p_tick::thinkercap;
use crate::doom::r_sky::skytexture;
use crate::doom::r_things::{numsprites, sprites};
use crate::doom::w_wad::{lumpinfo, W_CacheLumpNum};
use crate::doom::z_zone::{PU_CACHE, PU_STATIC, Z_Free, Z_Malloc};

use super::column_cache::textures;
use super::globals::{
    firstflat, firstspritelump, flatmemory, numflats, numtextures, spritememory, texturememory,
};
use super::types::spritedef_t;

/// Warm the WAD lump cache with all graphics used by the current level.
///
/// Skipped entirely during demo playback (`demoplayback != 0`).
///
/// Iterates over sectors, sidedef texture slots, and the thinker list to
/// build presence bitmaps for flats, textures, and sprites, then calls
/// `W_CacheLumpNum(PU_CACHE)` for each lump that is marked present.
/// Accumulates the total lump sizes into `flatmemory`, `texturememory`, and
/// `spritememory` for diagnostic purposes.
///
/// Note: the sky texture is always marked present regardless of which sectors
/// are in the level.
///
/// Called from `p_setup.c`'s `P_SetupLevel` after the level geometry is loaded.
///
/// # Safety
///
/// - All init functions (`R_InitData`, `p_setup` globals) must have run.
/// - Must be called from the game-loop thread; touches global mutable state.
#[doc(alias = "R_PrecacheLevel")]
#[export_name = "R_PrecacheLevel"]
pub unsafe extern "C" fn precache_level() {
    if demoplayback != 0 { return; }

    // Precache flats
    let flatpresent = Z_Malloc(numflats, PU_STATIC, ptr::null_mut()) as *mut u8;
    for i in 0..numflats as usize { *flatpresent.add(i) = 0; }

    for i in 0..numsectors as usize {
        *flatpresent.add((*sectors.add(i)).floorpic as usize) = 1;
        *flatpresent.add((*sectors.add(i)).ceilingpic as usize) = 1;
    }

    flatmemory = 0;
    for i in 0..numflats as usize {
        if *flatpresent.add(i) != 0 {
            let lump = firstflat + i as c_int;
            flatmemory += (*lumpinfo.add(lump as usize)).size;
            W_CacheLumpNum(lump, PU_CACHE);
        }
    }
    Z_Free(flatpresent as *mut c_void);

    // Precache textures
    let texturepresent = Z_Malloc(numtextures, PU_STATIC, ptr::null_mut()) as *mut u8;
    for i in 0..numtextures as usize { *texturepresent.add(i) = 0; }

    for i in 0..numsides as usize {
        *texturepresent.add((*sides.add(i)).toptexture as usize) = 1;
        *texturepresent.add((*sides.add(i)).midtexture as usize) = 1;
        *texturepresent.add((*sides.add(i)).bottomtexture as usize) = 1;
    }
    *texturepresent.add(skytexture as usize) = 1;

    texturememory = 0;
    for i in 0..numtextures as usize {
        if *texturepresent.add(i) == 0 { continue; }
        let texture = *textures.add(i);
        for j in 0..(*texture).patchcount as usize {
            let lump = (*std::ptr::addr_of!((*texture).patches).add(j)).patch;
            texturememory += (*lumpinfo.add(lump as usize)).size;
            W_CacheLumpNum(lump, PU_CACHE);
        }
    }
    Z_Free(texturepresent as *mut c_void);

    // Precache sprites
    let spritepresent = Z_Malloc(numsprites, PU_STATIC, ptr::null_mut()) as *mut u8;
    for i in 0..numsprites as usize { *spritepresent.add(i) = 0; }

    let mut th = thinkercap.next;
    while !std::ptr::eq(th, std::ptr::addr_of!(thinkercap)) {
        if (*th).function.acp1.map(|f| f as usize) == Some(P_MobjThinker as *const () as usize) {
            let mobj = th as *mut mobj_t;
            *spritepresent.add((*mobj).sprite as usize) = 1;
        }
        th = (*th).next;
    }

    spritememory = 0;
    for i in 0..numsprites as usize {
        if *spritepresent.add(i) == 0 { continue; }
        let sprdef = (sprites as *mut spritedef_t).add(i);
        for j in 0..(*sprdef).numframes as usize {
            let sf = (*sprdef).spriteframes.add(j);
            for k in 0..8usize {
                let lump = firstspritelump + (*sf).lump[k] as c_int;
                spritememory += (*lumpinfo.add(lump as usize)).size;
                W_CacheLumpNum(lump, PU_CACHE);
            }
        }
    }
    Z_Free(spritepresent as *mut c_void);
}
