//! Local `#[repr(C)]` mirrors for the sight traversal (`divline_t`,
//! `vertex_t`, `subsector_t`, `seg_t`, `node_t`, `sector_t`, `line_t`)
//! plus the re-export of the authoritative `p_telept::mobj_t`, with
//! their size/layout tests -- bit-exact with `p_local.h` / `r_defs.h`.
//!
//! These are NOT the `c_ffi` copies: they are carried verbatim from
//! the pre-graduation flat file, and unification with `c_ffi` is a
//! later, separate decision. `P_CheckSight` computes sector indices
//! via `offset_from` on the LOCAL `sector_t` size, so the layouts here
//! are load-bearing (see the mapping table in [`super`]).

#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

// Reuse the authoritative mobj_t mirror from p_telept.rs to guarantee
// field offsets match the C layout (x=24, subsector=88, height=108).
pub use crate::doom::p_telept::mobj_t;

/// A ray origin and direction in fixed-point map coordinates.
///
/// Used as the primitive for BSP-side tests and intercept computations.
/// The first four fields (`x`, `y`, `dx`, `dy`) share the same layout as the
/// first four fields of `node_t`, allowing C code to cast `node_t*` to
/// `divline_t*` for `P_DivlineSide` calls.
///
/// Corresponds to `divline_t` in `p_local.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct divline_t {
    /// X coordinate of the ray origin (fixed-point map units).
    pub x: c_int,
    /// Y coordinate of the ray origin (fixed-point map units).
    pub y: c_int,
    /// X component of the ray direction (fixed-point).
    pub dx: c_int,
    /// Y component of the ray direction (fixed-point).
    pub dy: c_int,
}

/// A two-dimensional vertex in fixed-point map coordinates.
///
/// Corresponds to `vertex_t` in `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct vertex_t {
    /// X coordinate (fixed-point map units).
    pub x: c_int,
    /// Y coordinate (fixed-point map units).
    pub y: c_int,
}

/// A BSP leaf node grouping a contiguous run of segs.
///
/// Corresponds to `subsector_t` in `r_defs.h`. The 4-byte pad after
/// `firstline` matches the C struct's natural alignment on 64-bit targets.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct subsector_t {
    /// The sector this subsector belongs to.
    pub sector: *mut sector_t,
    /// Number of segs in this subsector.
    pub numlines: i16,
    /// Index of the first seg in the global `segs` array.
    pub firstline: i16,
    _pad: [u8; 4],
}

/// A BSP line segment (half-edge) within a subsector.
///
/// Corresponds to `seg_t` in `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct seg_t {
    /// Start vertex of this segment.
    pub v1: *mut vertex_t,
    /// End vertex of this segment.
    pub v2: *mut vertex_t,
    /// Texture offset along the linedef (fixed-point).
    pub offset: c_int,
    /// Angle of this segment (binary angle measurement).
    pub angle: u32,
    /// Sidedef that provides textures for this seg.
    pub sidedef: *mut c_void,
    /// The linedef this seg was split from.
    pub linedef: *mut line_t,
    /// Sector on the front side of this seg.
    pub frontsector: *mut sector_t,
    /// Sector on the back side of this seg (`NULL` for one-sided lines).
    pub backsector: *mut sector_t,
}

// NOTE: The first 4 fields of node_t have the same layout as divline_t.
// C code casts (divline_t*)node for P_DivlineSide calls. We replicate
// the divline_t header here so we can safely transmute.
/// A BSP internal node dividing the map into front and back half-spaces.
///
/// The first four fields (`x`, `y`, `dx`, `dy`) share the same memory layout
/// as `divline_t` so that the C idiom `P_DivlineSide(x, y, (divline_t*)node)`
/// is reproduced safely via `node_as_divline`.
///
/// Corresponds to `node_t` in `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct node_t {
    /// X coordinate of the partition line origin.
    pub x: c_int,
    /// Y coordinate of the partition line origin.
    pub y: c_int,
    /// X component of the partition line direction.
    pub dx: c_int,
    /// Y component of the partition line direction.
    pub dy: c_int,
    /// Bounding boxes for each child: `bbox[0]` is front, `bbox[1]` is back.
    /// Each box is `[top, bottom, left, right]` in fixed-point map units.
    pub bbox: [[c_int; 4]; 2],
    /// Child indices: front (`children[0]`) and back (`children[1]`).
    /// If bit 15 (`NF_SUBSECTOR`) is set the index refers to a subsector leaf.
    pub children: [u16; 2],
}

/// A map sector defining floor/ceiling geometry and ambient properties.
///
/// Corresponds to `sector_t` in `r_defs.h`. Layout is verified by the
/// `sector_t_layout` unit test.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t {
    /// Floor height (fixed-point map units).
    pub floorheight: c_int,
    /// Ceiling height (fixed-point map units).
    pub ceilingheight: c_int,
    /// Floor flat texture number.
    pub floorpic: i16,
    /// Ceiling flat texture number.
    pub ceilingpic: i16,
    /// Ambient light level (0-255).
    pub lightlevel: i16,
    /// Special behaviour type (e.g. damaging floor, secret).
    pub special: i16,
    /// Sector tag used to link linedefs to sectors.
    pub tag: i16,
    _pad0: [u8; 2],
    /// Sound traversal counter (used by sound propagation).
    pub soundtraversed: c_int,
    /// Last sound-emitting mobj in this sector.
    pub soundtarget: *mut c_void,
    /// Bounding box of the sector in blockmap units.
    pub blockbox: [c_int; 4],
    /// Origin point for sector sounds (opaque 40-byte `mobj_t`-like struct).
    pub soundorg: [u8; 40],
    /// Validity stamp; updated each traversal to avoid duplicate processing.
    pub validcount: c_int,
    /// Head of the linked list of things in this sector.
    pub thinglist: *mut c_void,
    /// In-progress thinker data (used by door/floor/ceiling specials).
    pub specialdata: *mut c_void,
    /// Number of linedefs bounding this sector.
    pub linecount: c_int,
    /// Array of pointers to the linedefs bounding this sector.
    pub lines: *mut *mut c_void,
}

/// A map linedef connecting two vertices and separating two sectors.
///
/// Corresponds to `line_t` in `r_defs.h`. Layout is verified by the
/// `line_t_layout` unit test.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct line_t {
    /// Start vertex.
    pub v1: *mut vertex_t,
    /// End vertex.
    pub v2: *mut vertex_t,
    /// Precomputed `v2.x - v1.x` (fixed-point).
    pub dx: c_int,
    /// Precomputed `v2.y - v1.y` (fixed-point).
    pub dy: c_int,
    /// Linedef flags bitmask (`ML_*` constants from `doomdata.h`).
    pub flags: i16,
    /// Special action type triggered by this line.
    pub special: i16,
    /// Sector tag this line targets.
    pub tag: i16,
    /// Sidedef indices: `sidenum[0]` is front, `sidenum[1]` is back
    /// (`-1` means no back side).
    pub sidenum: [i16; 2],
    /// Axis-aligned bounding box of the line in map units.
    pub bbox: [c_int; 4],
    /// Slope type used for fast side-of-line tests (`ST_HORIZONTAL` etc.).
    pub slopetype: c_int,
    /// Sector on the front side.
    pub frontsector: *mut sector_t,
    /// Sector on the back side (`NULL` for one-sided lines).
    pub backsector: *mut sector_t,
    /// Validity stamp for blockmap/BSP traversal deduplication.
    pub validcount: c_int,
    /// In-progress special data (e.g. active door thinker pointer).
    pub specialdata: *mut c_void,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const DIVLINE_T_SIZEOF: usize = 16;
    const VERTEX_T_SIZEOF: usize = 8;
    const SUBSECTOR_T_SIZEOF: usize = 16;
    const SEG_T_SIZEOF: usize = 56;
    const NODE_T_SIZEOF: usize = 52;
    const SECTOR_T_SIZEOF: usize = 128;
    const LINE_T_SIZEOF: usize = 88;

    #[test]
    fn divline_t_size() {
        assert_eq!(std::mem::size_of::<divline_t>(), DIVLINE_T_SIZEOF);
    }

    #[test]
    fn vertex_t_size() {
        assert_eq!(std::mem::size_of::<vertex_t>(), VERTEX_T_SIZEOF);
    }

    #[test]
    fn subsector_t_size() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<subsector_t>(), SUBSECTOR_T_SIZEOF,);
    }

    #[test]
    fn seg_t_size() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<seg_t>(), SEG_T_SIZEOF,);
    }

    #[test]
    fn node_t_size() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<node_t>(), NODE_T_SIZEOF,);
    }

    #[test]
    fn sector_t_layout() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<sector_t>(), SECTOR_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(sector_t, floorheight), 0);
        assert_eq!(std::mem::offset_of!(sector_t, ceilingheight), 4);
        assert_eq!(std::mem::offset_of!(sector_t, floorpic), 8);
        assert_eq!(std::mem::offset_of!(sector_t, ceilingpic), 10);
        assert_eq!(std::mem::offset_of!(sector_t, lightlevel), 12);
        assert_eq!(std::mem::offset_of!(sector_t, special), 14);
        assert_eq!(std::mem::offset_of!(sector_t, tag), 16);
        assert_eq!(std::mem::offset_of!(sector_t, soundtraversed), 20);
        assert_eq!(std::mem::offset_of!(sector_t, soundtarget), 24);
        assert_eq!(std::mem::offset_of!(sector_t, blockbox), 32);
        assert_eq!(std::mem::offset_of!(sector_t, soundorg), 48);
        assert_eq!(std::mem::offset_of!(sector_t, validcount), 88);
        assert_eq!(std::mem::offset_of!(sector_t, thinglist), 96);
        assert_eq!(std::mem::offset_of!(sector_t, specialdata), 104);
        assert_eq!(std::mem::offset_of!(sector_t, linecount), 112);
        assert_eq!(std::mem::offset_of!(sector_t, lines), 120);
    }

    #[test]
    fn line_t_layout() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<line_t>(), LINE_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(line_t, v1), 0);
        assert_eq!(std::mem::offset_of!(line_t, v2), 8);
        assert_eq!(std::mem::offset_of!(line_t, dx), 16);
        assert_eq!(std::mem::offset_of!(line_t, dy), 20);
        assert_eq!(std::mem::offset_of!(line_t, flags), 24);
        assert_eq!(std::mem::offset_of!(line_t, special), 26);
        assert_eq!(std::mem::offset_of!(line_t, tag), 28);
        assert_eq!(std::mem::offset_of!(line_t, sidenum), 30);
        assert_eq!(std::mem::offset_of!(line_t, bbox), 36);
        assert_eq!(std::mem::offset_of!(line_t, slopetype), 52);
        assert_eq!(std::mem::offset_of!(line_t, frontsector), 56);
        assert_eq!(std::mem::offset_of!(line_t, backsector), 64);
        assert_eq!(std::mem::offset_of!(line_t, validcount), 72);
        assert_eq!(std::mem::offset_of!(line_t, specialdata), 80);
    }
}
