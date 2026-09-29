//! The startup half of `r_data`: PNAMES/TEXTURE1/TEXTURE2 parsing, the
//! flat and sprite lump ranges, the COLORMAP pointer, and the texture
//! hash table builder. Order inside `init_data` is the behavior (it fixes
//! the "filling-the-box" progress-dot sequence); every WAD parse ladder
//! moves bit-exact.

use std::ffi::{c_char, c_int, c_short, c_uint, c_ushort, c_void, CStr};
use std::ptr;

use crate::i_error;
use crate::doom::crt::c_printf;
use crate::doom::i_system::I_ConsoleStdout;
use crate::doom::m_fixed::FRACBITS;
use crate::doom::m_misc::M_StringCopy;
use crate::doom::w_wad::{
    W_CacheLumpName, W_CacheLumpNum, W_CheckNumForName, W_GetNumForName, W_LumpLength,
    W_LumpNameHash, W_ReleaseLumpName,
};
use crate::doom::z_zone::{PU_CACHE, PU_STATIC, Z_Free, Z_Malloc};

use super::column_cache::{
    generate_lookup, le_i16, le_i32, texturecolumnlump, texturecolumnofs, texturecomposite,
    texturecompositesize, textures, textures_hashtable, texturewidthmask,
};
use super::globals::{
    colormaps, firstflat, firstspritelump, flattranslation, lastflat, lastspritelump, numflats,
    numspritelumps, numtextures, spriteoffset, spritetopoffset, spritewidth, textureheight,
    texturetranslation,
};
use super::types::{mappatch_t, maptexture_t, patch_t, texpatch_t, texture_t};

/// Pass-through for DeHackEd string substitution (not implemented in this port).
///
/// In the upstream Chocolate Doom source `DEH_String` allows patch files to
/// override lump names at runtime.  This port does not support DeHackEd, so
/// the function returns its argument unchanged.
///
/// # Safety
///
/// `s` must be a valid, null-terminated C string for the lifetime of the call.
#[doc(alias = "DEH_String")]
unsafe fn deh_string(s: *const c_char) -> *const c_char {
    s
}

/// Build the hash table for O(1) texture name lookup.
///
/// Allocates `textures_hashtable` (length `numtextures`), then inserts every
/// texture into the table keyed by `W_LumpNameHash(name) % numtextures`.
/// Collisions are resolved by appending to the end of the bucket's linked
/// list (via `texture_t::next`), which preserves the vanilla Doom behaviour:
/// when two textures share a name, the one with the lower index wins because
/// it is at the head of the chain.
///
/// Called once at the end of `init_textures`.
///
/// # Safety
///
/// - `textures` must be fully populated (`numtextures` valid pointers).
/// - Must not be called more than once per session (would leak the old table).
#[doc(alias = "GenerateTextureHashTable")]
unsafe fn generate_texture_hash_table() {
    textures_hashtable = Z_Malloc(
        ((std::mem::size_of::<*mut texture_t>() * numtextures as usize) as c_int) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut *mut texture_t;

    for i in 0..numtextures as usize {
        *textures_hashtable.add(i) = ptr::null_mut();
    }

    for i in 0..numtextures as usize {
        let tex = *textures.add(i);
        (*tex).index = i as c_int;

        let key = (W_LumpNameHash((*tex).name.as_ptr()) % numtextures as c_uint) as usize;
        let mut rover = textures_hashtable.add(key);

        while !(*rover).is_null() {
            rover = std::ptr::addr_of_mut!((**rover).next);
        }

        (*tex).next = ptr::null_mut();
        *rover = tex;
    }
}

/// Load and initialise all wall textures from the WAD.
///
/// Performs the following steps in order:
/// 1. Reads the PNAMES lump to build a patch-name-to-lump-number table.
/// 2. Reads TEXTURE1 (and TEXTURE2 if present) to obtain texture definitions.
/// 3. Allocates and populates the `textures`, `texturecolumnlump`,
///    `texturecolumnofs`, `texturecomposite`, `texturecompositesize`,
///    `texturewidthmask`, and `textureheight` arrays.
/// 4. Calls `generate_lookup` for each texture to fill the column tables.
/// 5. Allocates `texturetranslation` (identity mapping, overridden later by
///    `p_spec.c` for animated textures).
/// 6. Calls `generate_texture_hash_table` to enable O(1) name lookup.
///
/// Prints progress dots to stdout (if `I_ConsoleStdout` returns non-zero)
/// using the classic Doom "filling-the-box" animation.
///
/// Called by `init_data`, which is called by `r_main.c` (`R_Init`).
///
/// # Safety
///
/// - WAD must be fully loaded (`W_Init` must have been called).
/// - Must be called exactly once per process lifetime.
#[doc(alias = "R_InitTextures")]
#[export_name = "R_InitTextures"]
pub unsafe extern "C" fn init_textures() {
    let mut name: [c_char; 9] = [0; 9];

    let names = W_CacheLumpName(deh_string(c"PNAMES".as_ptr()), PU_STATIC) as *mut c_int;
    let nummappatches = le_i32(*names);
    let name_p = names.add(1) as *mut c_char;

    let patchlookup = Z_Malloc(
        (nummappatches as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;

    for i in 0..nummappatches as usize {
        M_StringCopy(name.as_mut_ptr(), name_p.add(i * 8), name.len());
        *patchlookup.add(i) = W_CheckNumForName(name.as_mut_ptr());
    }
    W_ReleaseLumpName(deh_string(c"PNAMES".as_ptr()));

    let maptex1 = W_CacheLumpName(deh_string(c"TEXTURE1".as_ptr()), PU_STATIC) as *mut c_int;
    let numtextures1 = le_i32(*maptex1);
    let maxoff = W_LumpLength(W_GetNumForName(deh_string(c"TEXTURE1".as_ptr())) as c_uint);
    let directory = maptex1.add(1);

    let mut maptex2: *mut c_int = ptr::null_mut();
    let mut numtextures2: c_int = 0;
    let mut maxoff2: c_int = 0;

    if W_CheckNumForName(deh_string(c"TEXTURE2".as_ptr())) != -1 {
        maptex2 = W_CacheLumpName(deh_string(c"TEXTURE2".as_ptr()), PU_STATIC) as *mut c_int;
        numtextures2 = le_i32(*maptex2);
        maxoff2 = W_LumpLength(W_GetNumForName(deh_string(c"TEXTURE2".as_ptr())) as c_uint);
    }

    numtextures = numtextures1 + numtextures2;

    textures = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<*mut texture_t>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut *mut texture_t;
    texturecolumnlump = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<*mut c_short>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut *mut c_short;
    texturecolumnofs = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<*mut c_ushort>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut *mut c_ushort;
    texturecomposite = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<*mut u8>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut *mut u8;
    texturecompositesize = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;
    texturewidthmask = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;
    textureheight = Z_Malloc(
        (numtextures as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;

    let temp1 = W_GetNumForName(deh_string(c"S_START".as_ptr()));
    let temp2 = W_GetNumForName(deh_string(c"S_END".as_ptr())) - 1;
    let temp3 = ((temp2 - temp1 + 63) / 64) + ((numtextures + 63) / 64);

    if I_ConsoleStdout() != 0 {
        c_printf(c"[".as_ptr());
        for _ in 0..temp3 + 9 {
            c_printf(c" ".as_ptr());
        }
        c_printf(c"]".as_ptr());
        for _ in 0..temp3 + 10 {
            c_printf(c"\x08".as_ptr());
        }
    }

    let mut maptex = maptex1;
    let mut maxoff = maxoff;
    let mut directory = directory;

    for i in 0..numtextures as usize {
        if (i & 63) == 0 {
            c_printf(c".".as_ptr());
        }

        if i == numtextures1 as usize {
            maptex = maptex2;
            maxoff = maxoff2;
            directory = maptex2.add(1);
        }

        let offset = le_i32(*directory) as isize;
        directory = directory.add(1);

        if offset > maxoff as isize {
            i_error!("R_InitTextures: bad texture directory");
        }

        let mtexture = (maptex as *mut u8).offset(offset) as *mut maptexture_t;
        let patchcount = le_i16((*mtexture).patchcount) as c_int;
        let texsize = std::mem::size_of::<texture_t>()
            + std::mem::size_of::<texpatch_t>() * (patchcount as usize - 1);
        let texture = Z_Malloc(texsize as c_int, PU_STATIC, ptr::null_mut()) as *mut texture_t;
        *textures.add(i) = texture;

        (*texture).width = le_i16((*mtexture).width);
        (*texture).height = le_i16((*mtexture).height);
        (*texture).patchcount = patchcount as i16;

        std::ptr::copy_nonoverlapping((*mtexture).name.as_ptr(), (*texture).name.as_mut_ptr(), 8);

        let mpatches_base = std::ptr::addr_of!((*mtexture).patches) as *mut mappatch_t;
        let tpatches_base = std::ptr::addr_of!((*texture).patches) as *mut texpatch_t;

        for j in 0..patchcount as usize {
            let mpatch = mpatches_base.add(j);
            let patch = tpatches_base.add(j);
            (*patch).originx = le_i16((*mpatch).originx);
            (*patch).originy = le_i16((*mpatch).originy);
            (*patch).patch = *patchlookup.add(le_i16((*mpatch).patch) as usize);
            if (*patch).patch == -1 {
                i_error!(
                    "R_InitTextures: Missing patch in texture {}",
                    CStr::from_ptr((*texture).name.as_ptr()).to_string_lossy()
                );
            }
        }

        let twidth = (*texture).width as c_int;
        *texturecolumnlump.add(i) = Z_Malloc(
            (twidth as usize * std::mem::size_of::<c_short>()) as c_int,
            PU_STATIC,
            ptr::null_mut(),
        ) as *mut c_short;
        *texturecolumnofs.add(i) = Z_Malloc(
            (twidth as usize * std::mem::size_of::<c_ushort>()) as c_int,
            PU_STATIC,
            ptr::null_mut(),
        ) as *mut c_ushort;

        let mut j = 1;
        while j * 2 <= twidth {
            j <<= 1;
        }
        *texturewidthmask.add(i) = j - 1;
        *textureheight.add(i) = ((*texture).height as c_int) << FRACBITS;
    }

    Z_Free(patchlookup as *mut c_void);
    W_ReleaseLumpName(deh_string(c"TEXTURE1".as_ptr()));
    if !maptex2.is_null() {
        W_ReleaseLumpName(deh_string(c"TEXTURE2".as_ptr()));
    }

    for i in 0..numtextures as usize {
        generate_lookup(i as c_int);
    }

    texturetranslation = Z_Malloc(
        ((numtextures as usize + 1) * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;
    for i in 0..numtextures as usize {
        *texturetranslation.add(i) = i as c_int;
    }

    generate_texture_hash_table();
}

/// Locate the flat lump range in the WAD and initialise the animation translation table.
///
/// Sets `firstflat`, `lastflat`, and `numflats` from the `F_START` / `F_END`
/// marker lumps.  Allocates `flattranslation` as an identity mapping
/// (`flattranslation[i] = i`) that `p_spec.c` later updates for animated flats.
///
/// Called by `init_data`.
///
/// # Safety
///
/// - WAD must be fully loaded; `F_START` and `F_END` lumps must exist.
/// - Must be called exactly once per process lifetime.
#[doc(alias = "R_InitFlats")]
#[export_name = "R_InitFlats"]
pub unsafe extern "C" fn init_flats() {
    firstflat = W_GetNumForName(deh_string(c"F_START".as_ptr())) + 1;
    lastflat = W_GetNumForName(deh_string(c"F_END".as_ptr())) - 1;
    numflats = lastflat - firstflat + 1;

    flattranslation = Z_Malloc(
        ((numflats as usize + 1) * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;

    for i in 0..numflats as usize {
        *flattranslation.add(i) = i as c_int;
    }
}

/// Load sprite lump header data and populate the sprite metric arrays.
///
/// Locates the sprite lump range (`S_START` / `S_END`), then reads the
/// `patch_t` header of each sprite lump to fill:
/// - `spritewidth[i]`     - patch width in 16.16 fixed-point.
/// - `spriteoffset[i]`    - left-offset in 16.16 fixed-point.
/// - `spritetopoffset[i]` - top-offset in 16.16 fixed-point.
///
/// Only the four-field patch header is read; the actual pixel columns are not
/// loaded at this stage (they are cached on demand during rendering).  Prints
/// a progress dot every 64 lumps.
///
/// Called by `init_data`.
///
/// # Safety
///
/// - WAD must be fully loaded; `S_START` and `S_END` lumps must exist.
/// - Must be called exactly once per process lifetime.
#[doc(alias = "R_InitSpriteLumps")]
#[export_name = "R_InitSpriteLumps"]
pub unsafe extern "C" fn init_sprite_lumps() {
    firstspritelump = W_GetNumForName(deh_string(c"S_START".as_ptr())) + 1;
    lastspritelump = W_GetNumForName(deh_string(c"S_END".as_ptr())) - 1;
    numspritelumps = lastspritelump - firstspritelump + 1;

    spritewidth = Z_Malloc(
        (numspritelumps as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;
    spriteoffset = Z_Malloc(
        (numspritelumps as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;
    spritetopoffset = Z_Malloc(
        (numspritelumps as usize * std::mem::size_of::<c_int>()) as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut c_int;

    for i in 0..numspritelumps as usize {
        if (i & 63) == 0 {
            c_printf(c".".as_ptr());
        }

        let patch = W_CacheLumpNum(firstspritelump + i as c_int, PU_CACHE) as *mut patch_t;
        *spritewidth.add(i) = (le_i16((*patch).width) as c_int) << FRACBITS;
        *spriteoffset.add(i) = (le_i16((*patch).leftoffset) as c_int) << FRACBITS;
        *spritetopoffset.add(i) = (le_i16((*patch).topoffset) as c_int) << FRACBITS;
    }
}

/// Load the COLORMAP lump and set the `colormaps` pointer.
///
/// The COLORMAP lump contains 34 light tables of 256 bytes each, stored as a
/// single contiguous block.  The renderer indexes it as
/// `colormaps + light_level * 256` to look up color remappings for a given
/// lighting level.  The lump is tagged `PU_STATIC` and is never freed.
///
/// Called by `init_data`.
///
/// # Safety
///
/// - WAD must be fully loaded; the `COLORMAP` lump must exist.
/// - Must be called exactly once per process lifetime.
#[doc(alias = "R_InitColormaps")]
#[export_name = "R_InitColormaps"]
pub unsafe extern "C" fn init_colormaps() {
    let lump = W_GetNumForName(deh_string(c"COLORMAP".as_ptr()));
    colormaps = W_CacheLumpNum(lump, PU_STATIC) as *mut u8;
}

/// Locate and initialise all renderer data lumps from the WAD.
///
/// Calls, in order:
/// 1. `init_textures` - wall textures.
/// 2. `init_flats`    - floor/ceiling flats.
/// 3. `init_sprite_lumps` - sprite metrics.
/// 4. `init_colormaps` - lighting tables.
///
/// Prints a progress dot after each phase (in addition to the dots printed
/// by the individual init functions).
///
/// Called by `r_main.c` (`R_Init`) after the WAD has been loaded.
///
/// # Safety
///
/// - All of the above preconditions apply.
/// - Must be called exactly once per process lifetime.
#[doc(alias = "R_InitData")]
#[export_name = "R_InitData"]
pub unsafe extern "C" fn init_data() {
    init_textures();
    c_printf(c".".as_ptr());
    init_flats();
    c_printf(c".".as_ptr());
    init_sprite_lumps();
    c_printf(c".".as_ptr());
    init_colormaps();
}
