//! WAD asset load/unload: the shared lump-name walker and its load and
//! unload callbacks, bracketed by the `ST_load*`/`ST_unload*` entry
//! points.

use std::ffi::c_char;
use std::ffi::c_int;
use std::ptr;

use super::consts::{ST_NUMPAINFACES, ST_NUMSTRAIGHTFACES};
use super::{
    arms, armsbg, faces, faceback, keys, lu_palette, shortnum, sbar, tallnum, tallpercent,
};
use crate::c_write;
use crate::doom::d_player::NUMCARDS;
use crate::doom::g_game::consoleplayer;
use crate::doom::v_video::patch_t;
use crate::doom::w_wad::{W_CacheLumpName, W_GetNumForName, W_ReleaseLumpName};
use crate::doom::z_zone::PU_STATIC;

/// Function pointer type for the load/unload callback used by `load_unload_graphics`.
///
/// The callback receives a WAD lump name and a pointer to the patch pointer that
/// should be updated: either cached (load path) or released and nulled (unload path).
type LoadCallback = unsafe extern "C" fn(*mut c_char, *mut *mut patch_t);

/// Walk every status-bar lump name and invoke `callback` for each.
///
/// Shared by `load_graphics` (which passes `load_callback`) and
/// `unload_graphics` (which passes `unload_callback`). The face lump names
/// are generated dynamically; all other names are string literals.
///
/// # Safety
///
/// `callback` is invoked as an `unsafe extern "C"` function and is given raw
/// pointers into the static `tallnum`/`shortnum`/`faces`/etc. arrays. Caller
/// must pass a callback that respects those pointers' validity and only
/// reads/writes the single patch slot supplied. The WAD subsystem must be
/// initialised before the load variant is invoked.
unsafe fn load_unload_graphics(callback: LoadCallback) {
    let mut namebuf = [0i8; 9];

    for i in 0..10i32 {
        c_write!(namebuf, "STTNUM{}", i);
        callback(namebuf.as_mut_ptr(), &mut tallnum[i as usize]);
        c_write!(namebuf, "STYSNUM{}", i);
        callback(namebuf.as_mut_ptr(), &mut shortnum[i as usize]);
    }

    callback(c"STTPRCNT".as_ptr().cast_mut(), &raw mut tallpercent);

    for i in 0..NUMCARDS as c_int {
        c_write!(namebuf, "STKEYS{}", i);
        callback(namebuf.as_mut_ptr(), &mut keys[i as usize]);
    }

    callback(c"STARMS".as_ptr().cast_mut(), &raw mut armsbg);

    for i in 0..6i32 {
        c_write!(namebuf, "STGNUM{}", i + 2);
        callback(namebuf.as_mut_ptr(), &mut arms[i as usize][0]);
        arms[i as usize][1] = shortnum[(i + 2) as usize];
    }

    c_write!(namebuf, "STFB{}", consoleplayer as c_int);
    callback(namebuf.as_mut_ptr(), &raw mut faceback);

    callback(c"STBAR".as_ptr().cast_mut(), &raw mut sbar);

    let mut facenum: c_int = 0;
    for i in 0..ST_NUMPAINFACES {
        for j in 0..ST_NUMSTRAIGHTFACES {
            c_write!(namebuf, "STFST{}{}", i, j);
            callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
            facenum += 1;
        }
        c_write!(namebuf, "STFTR{}0", i);
        callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
        facenum += 1;
        c_write!(namebuf, "STFTL{}0", i);
        callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
        facenum += 1;
        c_write!(namebuf, "STFOUCH{}", i);
        callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
        facenum += 1;
        c_write!(namebuf, "STFEVL{}", i);
        callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
        facenum += 1;
        c_write!(namebuf, "STFKILL{}", i);
        callback(namebuf.as_mut_ptr(), &mut faces[facenum as usize]);
        facenum += 1;
    }

    callback(c"STFGOD0".as_ptr().cast_mut(), &mut faces[facenum as usize]);
    facenum += 1;
    callback(
        c"STFDEAD0".as_ptr().cast_mut(),
        &mut faces[facenum as usize],
    );
}

/// Load callback: cache the named WAD lump and store the pointer in `*variable`.
///
/// # Safety
///
/// Caller must ensure `lumpname` is a valid NUL-terminated C-string pointer
/// recognised by `W_CacheLumpName`, and `variable` is a valid, non-null,
/// properly aligned pointer to a `*mut patch_t` slot the function may overwrite.
#[doc(alias = "ST_loadCallback")]
unsafe extern "C" fn load_callback(lumpname: *mut c_char, variable: *mut *mut patch_t) {
    *variable = W_CacheLumpName(lumpname, PU_STATIC) as *mut patch_t;
}

/// Cache all status-bar graphics from the WAD into `PU_STATIC` memory.
///
/// # Safety
///
/// Mutates every cached patch pointer in the status-bar globals
/// (`tallnum`, `shortnum`, `tallpercent`, `keys`, `armsbg`, `arms`,
/// `faceback`, `sbar`, `faces`). Caller must ensure the WAD subsystem is
/// initialised and that no other code is reading these pointers concurrently.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `load_data` reaches the upstream name directly.
#[doc(alias = "ST_loadGraphics")]
#[export_name = "ST_loadGraphics"]
pub unsafe extern "C" fn load_graphics() {
    load_unload_graphics(load_callback);
}

/// Cache the palette lump number and load all status-bar graphics.
///
/// # Safety
///
/// Writes the global `lu_palette` and delegates to `load_graphics`, which
/// mutates the cached patch pointers. Caller must ensure the WAD subsystem
/// is initialised.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle::init` reaches the upstream name directly.
#[doc(alias = "ST_loadData")]
#[export_name = "ST_loadData"]
pub unsafe extern "C" fn load_data() {
    lu_palette = W_GetNumForName(c"PLAYPAL".as_ptr());
    load_graphics();
}

/// Unload callback: release the named WAD lump and null the pointer in `*variable`.
///
/// # Safety
///
/// Caller must ensure `lumpname` is a valid NUL-terminated C-string pointer
/// previously cached via `W_CacheLumpName`, and `variable` is a valid,
/// non-null, properly aligned pointer to a `*mut patch_t` slot that this
/// function may overwrite with null.
#[doc(alias = "ST_unloadCallback")]
unsafe extern "C" fn unload_callback(lumpname: *mut c_char, variable: *mut *mut patch_t) {
    W_ReleaseLumpName(lumpname);
    *variable = ptr::null_mut();
}

/// Release all status-bar graphics and null their pointers.
///
/// # Safety
///
/// Nulls every cached patch pointer in the status-bar globals and releases
/// their WAD lumps. Caller must ensure no other code is currently using
/// those patches (e.g. drawing must be stopped via `lifecycle::stop`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `unload_data` reaches the upstream name directly.
#[doc(alias = "ST_unloadGraphics")]
#[export_name = "ST_unloadGraphics"]
pub unsafe extern "C" fn unload_graphics() {
    load_unload_graphics(unload_callback);
}

/// Release all status-bar data (currently delegates to `unload_graphics`).
///
/// # Safety
///
/// Delegates to `unload_graphics`; same invariants apply (status bar must
/// not currently be drawing).
///
/// The pre-move export symbol is kept with `#[export_name]` below.
#[doc(alias = "ST_unloadData")]
#[export_name = "ST_unloadData"]
pub unsafe extern "C" fn unload_data() {
    unload_graphics();
}
