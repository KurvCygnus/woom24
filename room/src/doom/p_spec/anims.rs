//! Texture/flat animation state: the `anim_t` runtime record, the
//! compile-time `ANIMDEFS` table, the active-`anims` array with its
//! `lastanim` end pointer, and the map-load resolver `init_pic_anims` --
//! bit-exact with the animation half of `vendor/doomgeneric/p_spec.c`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};
use std::ptr;

use crate::i_error;
use crate::doom::r_data::{R_CheckTextureNumForName, R_FlatNumForName, R_TextureNumForName};
use crate::doom::w_wad::W_CheckNumForName;

use super::consts::MAXANIMS;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Runtime state for a single animated texture or flat sequence.
///
/// Maps to the C `anim_t` typedef in p_spec.c (also used internally in
/// wi_stuff.c with different semantics, but this is the p_spec version).
/// Layout invariant: the struct is `#[repr(C)]` and its size is asserted to
/// be exactly 20 bytes on 64-bit targets (matching the C layout).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct anim_t
{
    /// Non-zero if this animation cycle is for wall textures; zero for flats.
    pub istexture: c_int,
    /// WAD lump number of the last frame in the sequence.
    pub picnum: c_int,
    /// WAD lump number of the first frame in the sequence.
    pub basepic: c_int,
    /// Total number of frames in the cycle (`picnum - basepic + 1`).
    pub numpics: c_int,
    /// Tic-count duration of each frame; the sequence advances every `speed` tics.
    pub speed: c_int,
}

/// Static definition of one animation cycle as loaded from the ANIMDEFS table.
///
/// Maps to the C `animdef_t` typedef in p_spec.c.  The Rust code uses a
/// tuple-slice (`ANIMDEFS`) instead of an array of this struct for animation
/// initialisation; `animdef_t` is retained only for ABI size verification.
/// Layout invariant: 28 bytes on 64-bit (4-byte `istexture`, two 9-byte name
/// arrays padded to alignment, 4-byte `speed`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct animdef_t
{
    /// Non-zero for wall textures, zero for flats; -1 marks the sentinel entry.
    pub istexture: c_int,
    /// Name of the last frame lump (NUL-padded to 9 bytes).
    pub endname: [c_char; 9],
    /// Name of the first frame lump (NUL-padded to 9 bytes).
    pub startname: [c_char; 9],
    /// Frame duration in tics.
    pub speed: c_int,
}

#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(std::mem::size_of::<anim_t>() == 20);
    const _: () = assert!(std::mem::size_of::<animdef_t>() == 28);
}

// ---------------------------------------------------------------------------
// Animation definitions
// ---------------------------------------------------------------------------

/// Compile-time animation definition table, equivalent to C `animdefs[]`.
///
/// Each tuple is `(istexture, endname, startname, speed)`.  A sentinel entry
/// with `istexture == -1` terminates the list.  Pointers are created from
/// C string literals and are valid for the program lifetime. Sourced from
/// p_spec.c `animdefs[]`.
const ANIMDEFS: &[(c_int, *mut c_char, *mut c_char, c_int)] = &[
    (0, c"NUKAGE3".as_ptr().cast_mut(), c"NUKAGE1".as_ptr().cast_mut(), 8),
    (0, c"FWATER4".as_ptr().cast_mut(), c"FWATER1".as_ptr().cast_mut(), 8),
    (0, c"SWATER4".as_ptr().cast_mut(), c"SWATER1".as_ptr().cast_mut(), 8),
    (0, c"LAVA4".as_ptr().cast_mut(), c"LAVA1".as_ptr().cast_mut(), 8),
    (0, c"BLOOD3".as_ptr().cast_mut(), c"BLOOD1".as_ptr().cast_mut(), 8),
    (0, c"RROCK08".as_ptr().cast_mut(), c"RROCK05".as_ptr().cast_mut(), 8),
    (0, c"SLIME04".as_ptr().cast_mut(), c"SLIME01".as_ptr().cast_mut(), 8),
    (0, c"SLIME08".as_ptr().cast_mut(), c"SLIME05".as_ptr().cast_mut(), 8),
    (0, c"SLIME12".as_ptr().cast_mut(), c"SLIME09".as_ptr().cast_mut(), 8),
    (1, c"BLODGR4".as_ptr().cast_mut(), c"BLODGR1".as_ptr().cast_mut(), 8),
    (1, c"SLADRIP3".as_ptr().cast_mut(), c"SLADRIP1".as_ptr().cast_mut(), 8),
    (1, c"BLODRIP4".as_ptr().cast_mut(), c"BLODRIP1".as_ptr().cast_mut(), 8),
    (1, c"FIREWALL".as_ptr().cast_mut(), c"FIREWALA".as_ptr().cast_mut(), 8),
    (1, c"GSTFONT3".as_ptr().cast_mut(), c"GSTFONT1".as_ptr().cast_mut(), 8),
    (1, c"FIRELAVA".as_ptr().cast_mut(), c"FIRELAV3".as_ptr().cast_mut(), 8),
    (1, c"FIREMAG3".as_ptr().cast_mut(), c"FIREMAG1".as_ptr().cast_mut(), 8),
    (1, c"FIREBLU2".as_ptr().cast_mut(), c"FIREBLU1".as_ptr().cast_mut(), 8),
    (1, c"ROCKRED3".as_ptr().cast_mut(), c"ROCKRED1".as_ptr().cast_mut(), 8),
    (1, c"BFALL4".as_ptr().cast_mut(), c"BFALL1".as_ptr().cast_mut(), 8),
    (1, c"SFALL4".as_ptr().cast_mut(), c"SFALL1".as_ptr().cast_mut(), 8),
    (1, c"WFALL4".as_ptr().cast_mut(), c"WFALL1".as_ptr().cast_mut(), 8),
    (1, c"DBRAIN4".as_ptr().cast_mut(), c"DBRAIN1".as_ptr().cast_mut(), 8),
    (-1, c"".as_ptr().cast_mut(), c"".as_ptr().cast_mut(), 0),
];

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

/// Array of active animation state records, one per registered animation cycle.
///
/// Populated by `P_InitPicAnims`; only entries in `anims[0..lastanim)` are
/// valid.  Exported with C linkage (`#[no_mangle]`) for access from p_spec.c
/// and the renderer (r_data.c reads `texturetranslation`/`flattranslation`
/// which are updated in `P_UpdateSpecials` based on this array).
#[no_mangle]
pub static mut anims: [anim_t; MAXANIMS] = [anim_t {
    istexture: 0,
    picnum: 0,
    basepic: 0,
    numpics: 0,
    speed: 0,
}; MAXANIMS];

/// Pointer one-past the last valid entry in `anims[]`.
///
/// Acts as an end-iterator: the range `anims..lastanim` contains every active
/// animation cycle.  Null on startup, set by `P_InitPicAnims`.  Exported with
/// C linkage for symmetry with the C declaration `extern anim_t* lastanim`.
#[no_mangle]
pub static mut lastanim: *mut anim_t = ptr::null_mut();

// ---------------------------------------------------------------------------
// DEH_String shim — identity when dehacked is disabled.
// ---------------------------------------------------------------------------

/// Returns `s` unchanged; a no-op shim for the Dehacked string-replacement
/// hook that exists in the full Chocolate Doom build.
///
/// In the C source, `DEH_String` may redirect a hard-coded string to a
/// Dehacked patch string.  Because this port does not support Dehacked, the
/// shim simply returns its argument so that all call sites compile without
/// conditional compilation guards.
///
/// # Safety
///
/// `s` must be a valid, non-null pointer to a NUL-terminated C string for the
/// duration of the call (the pointer is returned unchanged and must remain valid
/// for however long the caller uses it).
#[doc(alias = "DEH_String")]
#[inline(always)]
unsafe fn deh_string(s: *mut c_char) -> *mut c_char
{
    s
}

// ---------------------------------------------------------------------------
// init_pic_anims
// ---------------------------------------------------------------------------

/// Initialises the animated texture and flat cycle table from `ANIMDEFS`.
///
/// Iterates `ANIMDEFS` until the sentinel entry (`istexture == -1`).  For
/// each entry it resolves the start and end lump numbers via
/// `R_TextureNumForName` / `R_FlatNumForName` (skipping entries whose start
/// lump does not exist in the WAD).  The resolved `anim_t` record is written
/// into the `anims` array and `lastanim` is advanced.
///
/// Calls `I_Error` (via `i_error!`) if an animation cycle contains fewer than
/// two frames.
///
/// Called once at map load time from C (p_spec.c `P_SpawnSpecials` via the
/// game initialisation path).
///
/// # Safety
///
/// Writes the module-global `anims` / `lastanim` statics; must run before
/// `P_UpdateSpecials` first reads them (the map-load order guarantees this).
#[doc(alias = "P_InitPicAnims")]
#[export_name = "P_InitPicAnims"]
pub unsafe extern "C" fn init_pic_anims()
{
    lastanim = std::ptr::addr_of_mut!(anims[0]);
    for &(istexture, endname, startname, speed) in ANIMDEFS
    {
        if istexture == -1
        {
            break;
        }
        let startname = deh_string(startname);
        let endname = deh_string(endname);

        if istexture != 0
        {
            if R_CheckTextureNumForName(startname) == -1
            {
                continue;
            }
            (*lastanim).picnum = R_TextureNumForName(endname);
            (*lastanim).basepic = R_TextureNumForName(startname);
        }
        else
        {
            if W_CheckNumForName(startname) == -1
            {
                continue;
            }
            (*lastanim).picnum = R_FlatNumForName(endname);
            (*lastanim).basepic = R_FlatNumForName(startname);
        }

        (*lastanim).istexture = istexture;
        (*lastanim).numpics = (*lastanim).picnum - (*lastanim).basepic + 1;

        if (*lastanim).numpics < 2
        {
            i_error!(
                "P_InitPicAnims: bad cycle from {} to {}",
                std::ffi::CStr::from_ptr(startname).to_string_lossy(),
                std::ffi::CStr::from_ptr(endname).to_string_lossy()
            );
        }

        (*lastanim).speed = speed;
        lastanim = lastanim.offset(1);
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn anim_t_layout_matches_c()
    {
        assert_eq!(std::mem::size_of::<anim_t>(), 20);
    }

    #[test]
    fn animdef_t_layout_matches_c()
    {
        assert_eq!(std::mem::size_of::<animdef_t>(), 28);
    }

    #[test]
    fn animation_defs_terminated()
    {
        let last = ANIMDEFS[ANIMDEFS.len() - 1];
        assert_eq!(last.0, -1);
    }
}
