//! Shared sector vocabulary of the lighting module: the `repr(C)`
//! partial mirrors of `sector_t` and `line_t` that nine other
//! freeze-zone files read through the module root, with the
//! layout-pinning test module.
//!
//! The mirrors carry only the fields the specials family touches,
//! padded to the exact C layout on the target ABI: the
//! `as *mut c_ffi::sector_t` / `as *mut c_ffi::line_t` casts at the
//! spawn/event call sites rely on that identity, so the layout tests
//! below are load-bearing.

#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

//
// We only need fields up to `specialdata`.  Verified against C layout
// on x86_64 Linux via layout_probe:
//   sector_t size=128, lightlevel@12, special@14, specialdata@104

/// Partial mirror of `sector_t` from `p_local.h`, containing only the fields
/// required by the lighting subsystem.
///
/// The layout is verified by the `sector_t_layout_matches_c` test; padding
/// bytes ensure the Rust struct matches the C ABI on x86_64 Linux.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t
{
    /// Height of the sector floor in fixed-point units.
    pub floorheight: c_int,
    /// Height of the sector ceiling in fixed-point units.
    pub ceilingheight: c_int,
    /// Flat texture index used for the floor.
    pub floorpic: i16,
    /// Flat texture index used for the ceiling.
    pub ceilingpic: i16,
    /// Current ambient light level (0 = dark, 255 = fully bright).
    pub lightlevel: i16,
    /// Sector special type (e.g. damaging floor, secret).
    pub special: i16,
    /// Linedef tag shared with triggers that activate this sector.
    pub tag: i16,
    _pad0: [u8; 2],
    /// Number of sound propagation traversals (BSP sound travel counter).
    pub soundtraversed: c_int,
    /// The map object currently producing sound in this sector, if any.
    pub soundtarget: *mut c_void,
    /// Axis-aligned bounding box of the sector in map coordinates.
    pub blockbox: [c_int; 4],
    /// Degenerate mobj used as the origin point for sector sounds (opaque).
    pub soundorg: [u8; 40], // degenmobj_t (opaque)
    /// Validation counter used during BSP traversal to avoid re-processing.
    pub validcount: c_int,
    /// Linked list of map objects currently standing in this sector.
    pub thinglist: *mut c_void,
    /// Pointer to the active thinker (special effect) operating on this sector.
    pub specialdata: *mut c_void,
    /// Number of linedefs that border this sector.
    pub linecount: c_int,
    /// Pointer to the array of linedefs that border this sector.
    pub lines: *mut *mut line_t,
}

/// Partial mirror of `line_t` from `p_local.h`.
///
/// Only the fields required by the lighting subsystem are included; remaining
/// fields are accessed through the C side.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct line_t
{
    /// First vertex of the linedef (opaque pointer).
    pub v1: *mut c_void,
    /// Second vertex of the linedef (opaque pointer).
    pub v2: *mut c_void,
    /// Horizontal component of the line direction vector (dx = v2.x - v1.x).
    pub dx: c_int,
    /// Vertical component of the line direction vector (dy = v2.y - v1.y).
    pub dy: c_int,
    /// Linedef flags (e.g. two-sided, impassable).
    pub flags: i16,
    /// Linedef special type number (triggers an action when activated).
    pub special: i16,
    /// Tag linking this linedef to sectors with the matching tag.
    pub tag: i16,
    /// Side number indices: `[0]` = front, `[1]` = back (-1 if one-sided).
    pub sidenum: [i16; 2],
    /// Axis-aligned bounding box of the linedef in map coordinates.
    pub bbox: [c_int; 4],
    /// Slope type classification for quick rejection tests.
    pub slopetype: c_int,
    /// Sector on the front side of this linedef.
    pub frontsector: *mut sector_t,
    /// Sector on the back side, or null for one-sided linedefs.
    pub backsector: *mut sector_t,
    /// Validation counter used during traversal to avoid re-processing.
    pub validcount: c_int,
    /// Pointer to an active special effect thinker on this linedef, if any.
    pub specialdata: *mut c_void,
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_lights::{fireflicker_t, glow_t, lightflash_t, sector_t, strobe_t};
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const SECTOR_T_SIZEOF: usize = 128;
    const SECTOR_T_LIGHTLEVEL_OFFSET: usize = 12;
    const SECTOR_T_SPECIAL_OFFSET: usize = 14;
    const SECTOR_T_SPECIALDATA_OFFSET: usize = 104;
    const FIREFLICKER_T_SIZEOF: usize = 48;
    const LIGHTFLASH_T_SIZEOF: usize = 56;
    const STROBE_T_SIZEOF: usize = 56;
    const GLOW_T_SIZEOF: usize = 48;

    /// `sector_t` must keep the exact C layout: the spawn/event call
    /// sites cast it to `c_ffi::sector_t` (for
    /// `P_FindMinSurroundingLight` / `getNextSector`), so the size and
    /// the `lightlevel` / `special` / `specialdata` offsets are
    /// load-bearing across the specials family.
    #[test]
    fn sector_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<sector_t>(),
            SECTOR_T_SIZEOF,
            "sector_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<sector_t>(),
            SECTOR_T_SIZEOF,
        );
        assert_eq!(
            std::mem::offset_of!(sector_t, lightlevel),
            SECTOR_T_LIGHTLEVEL_OFFSET,
        );
        assert_eq!(
            std::mem::offset_of!(sector_t, special),
            SECTOR_T_SPECIAL_OFFSET,
        );
        assert_eq!(
            std::mem::offset_of!(sector_t, specialdata),
            SECTOR_T_SPECIALDATA_OFFSET,
        );
    }

    /// `fireflicker_t` size pin (C: `p_spec.h`): the thinker state is
    /// `Z_Malloc`-allocated by `size_of`, so any drift would corrupt
    /// the zone allocation.
    #[test]
    fn fireflicker_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<fireflicker_t>(), FIREFLICKER_T_SIZEOF,);
    }

    /// `lightflash_t` size pin (C: `p_spec.h`); see
    /// `fireflicker_t_size_matches_c`.
    #[test]
    fn lightflash_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<lightflash_t>(), LIGHTFLASH_T_SIZEOF,);
    }

    /// `strobe_t` size pin (C: `p_spec.h`); see
    /// `fireflicker_t_size_matches_c`.
    #[test]
    fn strobe_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<strobe_t>(), STROBE_T_SIZEOF,);
    }

    /// `glow_t` size pin (C: `p_spec.h`); see
    /// `fireflicker_t_size_matches_c`.
    #[test]
    fn glow_t_size_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<glow_t>(), GLOW_T_SIZEOF,);
    }
}
