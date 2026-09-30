//! Vissprite projection: the per-frame pool reset, the pool allocator, the
//! world-sprite projector (which reads the interpolation board's sampled
//! positions), and the BSP per-sector collection pass.

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::{vissprite_t, MINZ};
#[cfg(feature = "rangecheck")]
use crate::i_error;
use crate::doom::info::*;
use crate::doom::m_fixed::{FixedDiv, FixedMul, FRACBITS, FRACUNIT};
use crate::doom::r_bsp::sector_t;
use crate::doom::r_data::{colormaps, spriteoffset, spritetopoffset, spritewidth};
use crate::doom::r_draw::viewwidth;
use crate::doom::r_main::{
    centerxfrac, detailshift, extralight, fixedcolormap, projection, scalelight, validcount,
    viewcos, viewx, viewy, viewz, viewsin, R_PointToAngle,
};
use crate::doom::tables::ANG45;

use super::state::{
    FF_FRAMEMASK, FF_FULLBRIGHT, LIGHTLEVELS, LIGHTSCALESHIFT, LIGHTSEGSHIFT, MAXLIGHTSCALE,
    MAXVISSPRITES, overflowsprite, spritedef_t, spritelights, sprites, vissprite_p, vissprites,
};

#[cfg(feature = "rangecheck")]
use super::state::numsprites;

/// Reset the vissprite pool to empty at the start of each frame.
///
/// Resets `vissprite_p` to the beginning of the `vissprites` array so that
/// the next frame can overwrite all previous entries.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// Must be called once per frame before any call to `R_AddSprites` or
/// `R_ProjectSprite`. No other thread may access `vissprites` concurrently.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_main` imports the upstream name through the root shim.
#[doc(alias = "R_ClearSprites")]
#[export_name = "R_ClearSprites"]
pub unsafe extern "C" fn clear_sprites() { vissprite_p = std::ptr::addr_of_mut!(vissprites[0]); }

/// Allocate the next vissprite slot from the pool and return a pointer to it.
///
/// If the pool is full (`MAXVISSPRITES` entries already allocated this frame)
/// returns a pointer to `overflowsprite` so the caller can write without
/// crashing; the overflow record is silently discarded.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// Must only be called after `R_ClearSprites` has been called this frame.
/// The returned pointer is valid until the next `R_ClearSprites` call.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_NewVisSprite")]
#[export_name = "R_NewVisSprite"]
pub unsafe extern "C" fn new_vis_sprite() -> *mut vissprite_t {
    if vissprite_p == std::ptr::addr_of_mut!(vissprites[0]).add(MAXVISSPRITES) { return &raw mut overflowsprite; }
    vissprite_p = vissprite_p.add(1);
    vissprite_p.sub(1)
}

/// Project a map object (thing) into screen space and, if visible, fill a
/// vissprite record for later drawing.
///
/// Transforms the thing's world position relative to the viewpoint, rejects
/// it if behind the view plane or too far off-axis, selects the correct sprite
/// lump based on the player's viewing angle and the thing's rotation set,
/// computes screen-space left/right column bounds, and writes all drawing
/// parameters (scale, light, texture offsets) into a new `vissprite_t`.
/// Exported as `#[no_mangle]` for C callers (called from `R_AddSprites`).
///
/// # Safety
/// - `thing` must be a valid, non-null `mobj_t` pointer.
/// - `sprites`, `numsprites`, `spriteoffset`, `spritewidth`, and
///   `spritetopoffset` must all be initialised by `R_InitSprites` before this
///   function is called.
/// - `viewx`, `viewy`, `viewz`, `viewcos`, `viewsin`, `projection`,
///   `centerxfrac`, `viewwidth`, and `spritelights` must be valid for the
///   current frame.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_ProjectSprite")]
#[export_name = "R_ProjectSprite"]
pub unsafe extern "C" fn project_sprite(thing: *mut c_void) {
    let thing = thing as *mut crate::doom::c_ffi::mobj_t;

    // F1 M1: sprite placement reads the interpolation board's sampled
    // position/angle when uncapped rendering is active; the sample falls back
    // to the live values under the guard set (spawn, missile first pair,
    // paused, teleport snap, board off), keeping the vanilla path unchanged.
    let pos = crate::doom::r_interp::sample_mobj(thing);

    // Transform the origin point.
    let tr_x = pos.x - viewx;
    let tr_y = pos.y - viewy;

    let gxt = FixedMul(tr_x, viewcos);
    let gyt = -FixedMul(tr_y, viewsin);

    let tz = gxt - gyt;

    // Thing is behind view plane?
    if tz < MINZ { return; }

    let xscale = FixedDiv(projection, tz);

    let gxt = -FixedMul(tr_x, viewsin);
    let gyt = FixedMul(tr_y, viewcos);
    let tx = -(gyt + gxt);

    // Too far off the side?
    if tx.abs() > (tz << 2) { return; }

    // Decide which patch to use for sprite relative to player.
    #[cfg(feature = "rangecheck")]
    {
        if(*thing).sprite as u32 >= numsprites as u32 { i_error!("R_ProjectSprite: invalid sprite number {}", (*thing).sprite); }
    }

    let sprdef = &*(sprites as *mut spritedef_t).add((*thing).sprite as usize);
    #[cfg(feature = "rangecheck")]
    {
        if ((*thing).frame & FF_FRAMEMASK) >= sprdef.numframes {
            if sprdef.numframes == 0 {
                return; // sprite lump not present in this WAD — skip silently
            }
            i_error!(
                "R_ProjectSprite [THING]: invalid sprite frame {} : {}",
                (*thing).sprite,
                (*thing).frame
            );
        }
    }
    let sprframe = &*sprdef
        .spriteframes
        .add(((*thing).frame & FF_FRAMEMASK) as usize);

    let (lump, flip): (c_int, c_int);
    if sprframe.rotate == 0 {
        // Use single rotation for all views.
        lump = sprframe.lump[0] as c_int;
        flip = sprframe.flip[0] as c_int;
    } else {
        // Choose a different rotation based on player view.
        let ang = R_PointToAngle(pos.x, pos.y);
        let rot = ((ang
            .wrapping_sub(pos.angle)
            .wrapping_add((ANG45 / 2) * 9))
            >> 29) as usize;
        lump = sprframe.lump[rot] as c_int;
        flip = sprframe.flip[rot] as c_int;
    }

    // Calculate edges of the shape.
    let mut tx = tx - *spriteoffset.add(lump as usize);
    let x1 = (centerxfrac + FixedMul(tx, xscale)) >> FRACBITS;

    // Off the right side?
    if x1 > viewwidth { return; }

    tx += *spritewidth.add(lump as usize);
    let x2 = ((centerxfrac + FixedMul(tx, xscale)) >> FRACBITS) - 1;

    // Off the left side.
    if x2 < 0 { return; }

    // Store information in a vissprite.
    let vis = new_vis_sprite();
    (*vis).mobjflags = (*thing).flags;
    (*vis).scale = xscale << detailshift;
    (*vis).gx = pos.x;
    (*vis).gy = pos.y;
    (*vis).gz = pos.z;
    (*vis).gzt = pos.z + *spritetopoffset.add(lump as usize);
    (*vis).texturemid = (*vis).gzt - viewz;
    (*vis).x1 = if x1 < 0 { 0 } else { x1 };
    (*vis).x2 = if x2 >= viewwidth { viewwidth - 1 } else { x2 };
    let iscale = FixedDiv(FRACUNIT, xscale);

    if flip != 0 {
        (*vis).startfrac = *spritewidth.add(lump as usize) - 1;
        (*vis).xiscale = -iscale;
    } else {
        (*vis).startfrac = 0;
        (*vis).xiscale = iscale;
    }

    if(*vis).x1 > x1 { (*vis).startfrac += (*vis).xiscale * ((*vis).x1 - x1); }
    (*vis).patch = lump;

    // Get light level.
    if (*thing).flags & MF_SHADOW != 0 {
        // Shadow draw.
        (*vis).colormap = ptr::null_mut();
    } else if !fixedcolormap.is_null() {
        // Fixed map.
        (*vis).colormap = fixedcolormap;
    } else if (*thing).frame & FF_FULLBRIGHT != 0 {
        // Full bright.
        (*vis).colormap = colormaps;
    } else {
        // Diminished light.
        let mut index = (xscale >> (LIGHTSCALESHIFT - detailshift as u32)) as usize;
        if index >= MAXLIGHTSCALE { index = MAXLIGHTSCALE - 1; }
        (*vis).colormap = *spritelights.add(index);
    }
}

/// Add all things in a sector to the vissprite list during BSP traversal.
///
/// Called once per unique sector encountered while traversing the BSP tree.
/// Uses `sec.validcount` to skip sectors that were already processed this
/// frame (a sector can appear in multiple subsectors). Sets `spritelights`
/// from the sector's light level, then calls `R_ProjectSprite` for every
/// thing in the sector's thing list.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// - `sec` must be a valid, non-null `sector_t` pointer.
/// - `validcount`, `scalelight`, `extralight` must be initialised for the
///   current frame before this function is called.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_bsp` imports the upstream name through the root shim.
#[doc(alias = "R_AddSprites")]
#[export_name = "R_AddSprites"]
pub unsafe extern "C" fn add_sprites(sec: *mut sector_t) {
    let sec = &*sec;

    // BSP is traversed by subsector.
    // A sector might have been split into several subsectors during BSP building.
    // Thus we check whether it's already added.
    if sec.validcount == validcount { return; }

    // Well, now it will be done.
    (*(sec as *const sector_t as *mut sector_t)).validcount = validcount;

    let lightnum = (sec.lightlevel >> LIGHTSEGSHIFT as i16) as c_int + extralight;

    if lightnum < 0 { spritelights = scalelight[0].as_mut_ptr(); }
    else if lightnum >= LIGHTLEVELS as c_int { spritelights = scalelight[LIGHTLEVELS - 1].as_mut_ptr(); }
    else { spritelights = scalelight[lightnum as usize].as_mut_ptr(); }

    // Handle all things in sector.
    let mut thing = sec.thinglist as *mut crate::doom::c_ffi::mobj_t;
    while !thing.is_null() {
        project_sprite(thing as *mut c_void);
        thing = (*thing).snext as *mut crate::doom::c_ffi::mobj_t;
    }
}
