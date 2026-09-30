//! Sprite definition loading: the `sprtemp` frame builder, the WAD lump
//! scan that fills it, and the map-load entry point.

use std::ffi::{c_char, c_int, c_short, CStr};
use std::ptr;

use crate::doom::c_ffi::spriteframe_t;
use crate::doom::crt::strncasecmp;
use crate::doom::doomstat::modifiedgame;
use crate::doom::i_video::SCREENWIDTH;
use crate::i_error;
use crate::doom::r_data::{firstspritelump, lastspritelump};
use crate::doom::w_wad::{lumpinfo, W_GetNumForName};
use crate::doom::z_zone::Z_Malloc;

use super::state::{
    maxframe, negonearray, numsprites, spritedef_t, spritename, sprtemp, sprites,
};

/// Record one WAD lump as a rotation of a sprite frame in `sprtemp`.
///
/// Called exclusively from `R_InitSpriteDefs` for each lump whose name
/// matches the current sprite being processed. If `rotation == 0` the lump
/// is installed for all eight rotations; otherwise it fills exactly one slot.
/// Calls `i_error!` on duplicate or inconsistent assignments.
///
/// # Safety
/// `spritename` must be a valid, null-terminated C string pointer.
/// `lump`, `frame`, and `rotation` must be within the ranges enforced by
/// the guards at the top of the function.
#[doc(alias = "R_InstallSpriteLump")]
unsafe fn install_sprite_lump(lump: c_int, frame: u32, rotation: u32, flipped: c_int) {
    if frame >= 29 || rotation > 8 { i_error!("R_InstallSpriteLump: Bad frame characters in lump {}", lump); }

    if frame as c_int > maxframe { maxframe = frame as c_int; }

    if rotation == 0 {
        // The lump should be used for all rotations.
        if sprtemp[frame as usize].rotate == 0 {
            i_error!(
                "R_InitSprites: Sprite {} frame {} has multip rot=0 lump",
                CStr::from_ptr(spritename).to_string_lossy(),
                char::from(b'A' + frame as u8)
            );
        }
        if sprtemp[frame as usize].rotate == 1 {
            i_error!(
                "R_InitSprites: Sprite {} frame {} has rotations and a rot=0 lump",
                CStr::from_ptr(spritename).to_string_lossy(),
                char::from(b'A' + frame as u8)
            );
        }
        sprtemp[frame as usize].rotate = 0;
        for r in 0..8 {
            sprtemp[frame as usize].lump[r] = (lump - firstspritelump) as c_short;
            sprtemp[frame as usize].flip[r] = flipped as u8;
        }
        return;
    }

    // The lump is only used for one rotation.
    if sprtemp[frame as usize].rotate == 0 {
        i_error!(
            "R_InitSprites: Sprite {} frame {} has rotations and a rot=0 lump",
            CStr::from_ptr(spritename).to_string_lossy(),
            char::from(b'A' + frame as u8)
        );
    }

    sprtemp[frame as usize].rotate = 1;

    // Make 0 based.
    let rot = (rotation - 1) as usize;
    if sprtemp[frame as usize].lump[rot] != -1 {
        i_error!(
            "R_InitSprites: Sprite {} : {} : {} has two lumps mapped to it",
            CStr::from_ptr(spritename).to_string_lossy(),
            char::from(b'A' + frame as u8),
            char::from(b'1' + rot as u8)
        );
    }

    sprtemp[frame as usize].lump[rot] = (lump - firstspritelump) as c_short;
    sprtemp[frame as usize].flip[rot] = flipped as u8;
}

/// Build the `sprites` lookup table from a null-terminated list of 4-character
/// sprite names.
///
/// Scans every sprite-range WAD lump, decodes frame letter and rotation digit
/// from the lump name, and calls `R_InstallSpriteLump` to populate `sprtemp`.
/// After processing all lumps for a sprite, validates that every referenced
/// frame has a complete rotation set, then allocates a permanent `spriteframe_t`
/// array and copies `sprtemp` into it.
///
/// In a modified game (`modifiedgame` true) lump numbers are resolved through
/// `W_GetNumForName` to respect WAD replacement ordering.
///
/// # Safety
/// `namelist` must be a null-terminated array of valid C string pointers.
/// `firstspritelump` and `lastspritelump` from `r_data` must already be
/// initialised before this function is called.
#[doc(alias = "R_InitSpriteDefs")]
unsafe fn init_sprite_defs(namelist: *mut *mut c_char) {
    let mut check = namelist;
    while !(*check).is_null() { check = check.add(1); }

    numsprites = check.offset_from(namelist) as c_int;

    if numsprites == 0 { return; }

    sprites = Z_Malloc(
        (numsprites as usize * std::mem::size_of::<spritedef_t>()) as c_int,
        1, // PU_STATIC
        ptr::null_mut(),
    );

    let start = firstspritelump - 1;
    let end = lastspritelump + 1;

    for i in 0..numsprites {
        spritename = *namelist.add(i as usize);
        ptr::write_bytes(std::ptr::addr_of_mut!(sprtemp[0]), 0xFF, 29);

        maxframe = -1;

        // Scan the lumps, filling in the frames for whatever is found.
        for l in (start + 1)..end {
            let li = &*lumpinfo.add(l as usize);
            if strncasecmp(li.name.as_ptr(), spritename, 4) == 0 {
                let frame = (li.name[4] as u8 - b'A') as u32;
                let rotation = (li.name[5] as u8 - b'0') as u32;

                let patched = if modifiedgame.is_truthy() {
                    // Need a null-terminated copy for W_GetNumForName
                    let mut name_buf: [c_char; 9] = [0; 9];
                    ptr::copy_nonoverlapping(li.name.as_ptr(), name_buf.as_mut_ptr(), 8);
                    W_GetNumForName(name_buf.as_mut_ptr())
                } else {
                    l
                };

                install_sprite_lump(patched, frame, rotation, 0);

                if li.name[6] != 0 {
                    let frame = (li.name[6] as u8 - b'A') as u32;
                    let rotation = (li.name[7] as u8 - b'0') as u32;
                    install_sprite_lump(l, frame, rotation, 1);
                }
            }
        }

        // Check the frames that were found for completeness.
        if maxframe == -1 {
            let spr = &mut *(sprites as *mut spritedef_t).add(i as usize);
            spr.numframes = 0;
            continue;
        }

        maxframe += 1;

        for frame in 0..maxframe {
            match sprtemp[frame as usize].rotate {
                -1 => {
                    // No rotations were found for that frame at all.
                    i_error!(
                        "R_InitSprites: No patches found for {} frame {}",
                        CStr::from_ptr(spritename).to_string_lossy(),
                        char::from(b'A' + frame as u8)
                    );
                }
                0 => {
                    // Only the first rotation is needed.
                }
                1 => {
                    // Must have all 8 frames.
                    for rotation in 0..8 {
                        if sprtemp[frame as usize].lump[rotation] == -1 {
                            i_error!(
                                "R_InitSprites: Sprite {} frame {} is missing rotations",
                                CStr::from_ptr(spritename).to_string_lossy(),
                                char::from(b'A' + frame as u8)
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        // Allocate space for the frames present and copy sprtemp to it.
        let spr = &mut *(sprites as *mut spritedef_t).add(i as usize);
        spr.numframes = maxframe;
        spr.spriteframes = Z_Malloc(
            (maxframe as usize * std::mem::size_of::<spriteframe_t>()) as c_int,
            1, // PU_STATIC
            ptr::null_mut(),
        ) as *mut spriteframe_t;
        ptr::copy_nonoverlapping(
            std::ptr::addr_of!(sprtemp[0]),
            spr.spriteframes,
            maxframe as usize,
        );
    }
}

/// Initialise the sprite system at program start.
///
/// Fills `negonearray` with `-1` (used as the ceiling clip sentinel for psprites),
/// then delegates to `R_InitSpriteDefs` to build the sprite frame lookup table.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// `namelist` must be a null-terminated array of valid C string pointers, each
/// pointing to a 4-character sprite name. Must be called after WAD loading
/// (`W_InitMultipleFiles`) has set up `firstspritelump` and `lastspritelump`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_setup/level.rs` imports the upstream name through the root shim.
#[doc(alias = "R_InitSprites")]
#[export_name = "R_InitSprites"]
pub unsafe extern "C" fn init_sprites(namelist: *mut *mut c_char) {
    for i in 0..SCREENWIDTH { negonearray[i as usize] = -1; }
    init_sprite_defs(namelist);
}
