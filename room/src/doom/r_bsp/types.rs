//! The `r_defs.h` mirror vocabulary of the BSP renderer: vertexes, the
//! sector/sidedef/linedef/subsector/seg/node set, `drawseg_t` (its layout
//! is load-bearing for `r_segs`), `ZERO_DRAWSEG`, the compile-time
//! `layout_checks` module, and the dead `visplane_t` mirror. All carried
//! verbatim; nothing here may be "unified" with the `c_ffi` or
//! `p_lights`/`r_things` copies (layout identity is pinned three ways:
//! `layout_checks`, `r_interp`, `c_tests/struct_layouts.rs`).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_short, c_void};
use std::ptr;

use crate::doom::m_fixed::{angle_t, fixed_t};

// ---------------------------------------------------------------------------
// Mirrored C structs (from r_defs.h) — only fields read by r_bsp.c
// ---------------------------------------------------------------------------

/// A map vertex holding a 2-D fixed-point position.
/// Mirrors `vertex_t` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct vertex_t {
    /// X coordinate in fixed-point map units.
    pub x: fixed_t,
    /// Y coordinate in fixed-point map units.
    pub y: fixed_t,
}

/// Opaque forward-declaration of a map object (monster, item, player, etc.).
/// `r_bsp` holds pointers to these but never dereferences them directly.
/// Mirrors `mobj_t` / `mobj_s` from `p_mobj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct mobj_s {
    _opaque: [u8; 0],
}
/// Type alias for [`mobj_s`], matching the C `mobj_t` typedef.
pub type mobj_t = mobj_s;

// thinker_t is 24 bytes in C (prev/next/function pointers).
// r_bsp never dereferences it, but it must have the correct size
// so that sector_t (which contains degenmobj_t, which contains thinker_t)
// matches the C layout of 128 bytes.

/// Thinker linked-list node. Never dereferenced by this module; included
/// only to maintain the correct `sector_t` layout (128 bytes on x86-64).
/// Mirrors `thinker_t` / `thinker_s` from `p_tick.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct thinker_s {
    _prev: *mut c_void,
    _next: *mut c_void,
    _function: *mut c_void,
}
/// Type alias for [`thinker_s`], matching the C `thinker_t` typedef.
pub type thinker_t = thinker_s;

/// A degenerate map object used as a sound origin embedded inside `sector_t`.
/// Contains a [`thinker_t`] prefix followed by map-unit coordinates.
/// Mirrors `degenmobj_t` / `degenmobj_s` from `p_mobj.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct degenmobj_s {
    /// Thinker header (required for correct sector_t layout).
    pub thinker: thinker_t,
    /// X position of the sound origin in map units.
    pub x: fixed_t,
    /// Y position of the sound origin in map units.
    pub y: fixed_t,
    /// Z position of the sound origin in map units.
    pub z: fixed_t,
}

/// A map sector describing the floor/ceiling geometry and lighting of a
/// convex region. Mirrors `sector_t` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t {
    /// Floor height in fixed-point map units.
    pub floorheight: fixed_t,
    /// Ceiling height in fixed-point map units.
    pub ceilingheight: fixed_t,
    /// Flat lump index for the floor texture.
    pub floorpic: c_short,
    /// Flat lump index for the ceiling texture.
    pub ceilingpic: c_short,
    /// Ambient light level (0-255).
    pub lightlevel: c_short,
    /// Special effect number (damage, secret, etc.).
    pub special: c_short,
    /// Sector tag used to link with linedef specials.
    pub tag: c_short,
    /// Sound traversal counter (set during sound propagation).
    pub soundtraversed: c_int,
    /// Last thing to make a sound in this sector.
    pub soundtarget: *mut mobj_t,
    /// Bounding box used for blockmap queries (`BOXLEFT/BOXRIGHT/BOXTOP/BOXBOTTOM`).
    pub blockbox: [c_int; 4],
    /// Degenerate mobj used as the sector's spatial sound origin.
    pub soundorg: degenmobj_s,
    /// Validity counter for single-pass linedef and thing traversal.
    pub validcount: c_int,
    /// Head of the linked list of things (mobjs) in this sector.
    pub thinglist: *mut mobj_t,
    /// Pointer to sector-specific special state (e.g. moving floor/ceiling).
    pub specialdata: *mut c_void,
    /// Number of linedefs bounding this sector.
    pub linecount: c_int,
    /// Pointer to the array of linedef pointers bounding this sector.
    pub lines: *mut *mut line_s,
}

/// One side of a two-sided linedef, carrying texture and sector references.
/// Mirrors `side_t` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct side_t {
    /// Horizontal texture offset in fixed-point units.
    pub textureoffset: fixed_t,
    /// Vertical texture offset in fixed-point units.
    pub rowoffset: fixed_t,
    /// Upper (above back sector ceiling) texture number; 0 = none.
    pub toptexture: c_short,
    /// Lower (below back sector floor) texture number; 0 = none.
    pub bottomtexture: c_short,
    /// Middle texture number; 0 = none.
    pub midtexture: c_short,
    /// The sector this sidedef faces.
    pub sector: *mut sector_t,
}

/// Line slope type, used to select the correct axis-aligned bounding-box
/// intersection test for a linedef. Mirrors `slopetype_t` from `r_defs.h`.
#[repr(C)]
pub enum slopetype_t {
    /// Line is perfectly horizontal (dy == 0).
    ST_HORIZONTAL,
    /// Line is perfectly vertical (dx == 0).
    ST_VERTICAL,
    /// Line has positive slope (dy/dx > 0).
    ST_POSITIVE,
    /// Line has negative slope (dy/dx < 0).
    ST_NEGATIVE,
}

/// A map linedef connecting two vertices and potentially separating two
/// sectors. Mirrors `line_t` / `line_s` from `r_defs.h`.
#[repr(C)]
pub struct line_s {
    /// First vertex (start of the line).
    pub v1: *mut vertex_t,
    /// Second vertex (end of the line).
    pub v2: *mut vertex_t,
    /// Horizontal delta `v2.x - v1.x` in fixed-point units.
    pub dx: fixed_t,
    /// Vertical delta `v2.y - v1.y` in fixed-point units.
    pub dy: fixed_t,
    /// Linedef flags (ML_* constants from `doomdef.h`).
    pub flags: c_short,
    /// Special action number (door, platform, exit, etc.).
    pub special: c_short,
    /// Tag linking this linedef to a sector for special activations.
    pub tag: c_short,
    /// Side numbers: `sidenum[0]` = front, `sidenum[1]` = back (-1 = none).
    pub sidenum: [c_short; 2],
    /// Axis-aligned bounding box of the linedef.
    pub bbox: [fixed_t; 4],
    /// Pre-computed slope type for fast bbox intersection.
    pub slopetype: slopetype_t,
    /// Sector on the front (right) side of the linedef.
    pub frontsector: *mut sector_t,
    /// Sector on the back (left) side; null for single-sided lines.
    pub backsector: *mut sector_t,
    /// Validity counter to avoid processing the same linedef twice per query.
    pub validcount: c_int,
    /// Pointer to active special state (e.g. a triggered door thinker).
    pub specialdata: *mut c_void,
}
/// Type alias for [`line_s`], matching the C `line_t` typedef.
pub type line_t = line_s;

/// A BSP leaf convex region made up of one or more segs from the same sector.
/// Mirrors `subsector_t` / `subsector_s` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct subsector_s {
    /// The sector this subsector belongs to.
    pub sector: *mut sector_t,
    /// Number of segs in this subsector.
    pub numlines: c_short,
    /// Index of the first seg in the global `segs` array.
    pub firstline: c_short,
}
/// Type alias for [`subsector_s`], matching the C `subsector_t` typedef.
pub type subsector_t = subsector_s;

/// A wall segment (part of a linedef's front side) used during rendering.
/// Each seg carries pre-computed angle and texture-offset data.
/// Mirrors `seg_t` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct seg_t {
    /// Start vertex of this seg.
    pub v1: *mut vertex_t,
    /// End vertex of this seg.
    pub v2: *mut vertex_t,
    /// Horizontal texture offset along the linedef in fixed-point units.
    pub offset: fixed_t,
    /// Absolute BAM angle of this seg (v1 to v2).
    pub angle: angle_t,
    /// The sidedef that provides texture information for this seg.
    pub sidedef: *mut side_t,
    /// The linedef this seg belongs to.
    pub linedef: *mut line_t,
    /// The sector on the front side of this seg.
    pub frontsector: *mut sector_t,
    /// The sector on the back side of this seg; null for single-sided lines.
    pub backsector: *mut sector_t,
}

/// An interior BSP tree node with a splitting line and two child references.
/// Children are either node indices or, when `NF_SUBSECTOR` is set, subsector
/// leaf indices. Mirrors `node_t` from `r_defs.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct node_t {
    /// X coordinate of the splitting line's origin in fixed-point map units.
    pub x: fixed_t,
    /// Y coordinate of the splitting line's origin in fixed-point map units.
    pub y: fixed_t,
    /// Horizontal delta of the splitting line vector.
    pub dx: fixed_t,
    /// Vertical delta of the splitting line vector.
    pub dy: fixed_t,
    /// Axis-aligned bounding boxes for the two child subtrees:
    /// `bbox[0]` = right child, `bbox[1]` = left child, each encoded as
    /// `[TOP, BOTTOM, LEFT, RIGHT]` in fixed-point map units.
    pub bbox: [[fixed_t; 4]; 2],
    /// Child references: `children[0]` = right, `children[1]` = left.
    /// If `NF_SUBSECTOR` is set in the value, the lower bits are a subsector
    /// index; otherwise the value is a node index.
    pub children: [u16; 2],
}

// ---------------------------------------------------------------------------
// visplane_t — already exported from r_plane, mirror locally for field access
// ---------------------------------------------------------------------------
//
// ! Dead mirror: no r_bsp function ever dereferences a visplane (the
// ! report's zero-use finding). The graduation CARRIES it verbatim --
// ! "graduation moves, does not improve" -- deletion is recorded as
// ! proposed-and-deferred in the mod.rs mapping table; it retires with
// ! the freeze zone.

/// Compile-time array cap for the local `visplane_t` mirror (F1 M2).
/// Must match the value in `r_plane` (boom `MAX_SCREENWIDTH` shape).
const SCREENWIDTH_RP: usize = crate::doom::video_cfg::MAX_SCREENWIDTH as usize;

/// A horizontal floor/ceiling span to be drawn at a fixed height and texture.
/// Mirrored locally from `r_plane` so that `r_plane` pointer fields can
/// be written from this module. The struct must match the C `visplane_t` layout
/// from `r_plane.h` exactly.
#[repr(C)]
#[derive(Clone, Copy)]
struct visplane_t {
    /// Height of this plane in fixed-point map units.
    pub height: fixed_t,
    /// Flat lump index for this plane's texture.
    pub picnum: c_int,
    /// Light level for this plane (0-255).
    pub lightlevel: c_int,
    /// Leftmost screen column covered by this plane.
    pub minx: c_int,
    /// Rightmost screen column covered by this plane.
    pub maxx: c_int,
    /// Padding byte before the `top` array (matches C struct layout).
    pub pad1: u8,
    /// Per-column top clip (y coordinate); `0xff` means unset.
    pub top: [u8; SCREENWIDTH_RP],
    /// Padding byte between `top` and `bottom` arrays.
    pub pad2: u8,
    /// Padding byte before the `bottom` array.
    pub pad3: u8,
    /// Per-column bottom clip (y coordinate); `0xff` means unset.
    pub bottom: [u8; SCREENWIDTH_RP],
    /// Padding byte after the `bottom` array (matches C struct layout).
    pub pad4: u8,
}

// ---------------------------------------------------------------------------
// drawseg_t — MUST match C layout exactly (r_segs.c writes every field)
// ---------------------------------------------------------------------------

/// A seg prepared for the column renderer, holding scale, silhouette, and
/// sprite-clip arrays. Written by `R_StoreWallRange` and read back during
/// sprite clipping in `R_DrawSprite`. Must match the C `drawseg_t` layout
/// from `r_segs.h` exactly (verified by compile-time assertions in
/// `layout_checks`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct drawseg_t {
    /// The seg that generated this draw-seg.
    pub curline: *mut seg_t,
    /// Leftmost screen column of this draw-seg (inclusive).
    pub x1: c_int,
    /// Rightmost screen column of this draw-seg (inclusive).
    pub x2: c_int,
    /// Projection scale at column `x1`.
    pub scale1: fixed_t,
    /// Projection scale at column `x2`.
    pub scale2: fixed_t,
    /// Per-column scale increment: `(scale2 - scale1) / (x2 - x1)`.
    pub scalestep: fixed_t,
    /// Silhouette flags (`SIL_*`) indicating which edges clip sprites.
    pub silhouette: c_int,
    /// Bottom silhouette height in fixed-point units (for lower-unpegged walls).
    pub bsilheight: fixed_t,
    /// Top silhouette height in fixed-point units (for upper-unpegged walls).
    pub tsilheight: fixed_t,
    /// Pointer into the sprite-top clip column array; null if not needed.
    pub sprtopclip: *mut c_short,
    /// Pointer into the sprite-bottom clip column array; null if not needed.
    pub sprbottomclip: *mut c_short,
    /// Pointer into the masked-texture column array; null for solid walls.
    pub maskedtexturecol: *mut c_short,
}

/// Compile-time zero initializer for [`drawseg_t`], used to fill the static
/// `drawsegs` array before any rendering occurs.
pub(super) const ZERO_DRAWSEG: drawseg_t = drawseg_t {
    curline: ptr::null_mut(),
    x1: 0,
    x2: 0,
    scale1: 0,
    scale2: 0,
    scalestep: 0,
    silhouette: 0,
    bsilheight: 0,
    tsilheight: 0,
    sprtopclip: ptr::null_mut(),
    sprbottomclip: ptr::null_mut(),
    maskedtexturecol: ptr::null_mut(),
};

// Compile-time size and offset checks for structs that must match C layout
// (values verified against vendor/doomgeneric C structs on x86_64 Linux)
/// Compile-time layout assertions ensuring all mirrored C structs have the
/// correct size and field offsets on x86-64 Linux. A compile error here means
/// the Rust struct has diverged from the C layout and ABI compatibility is
/// broken.
#[cfg(target_pointer_width = "64")]
mod layout_checks {
    // This module is intentionally private; it exists only for compile-time assertions.
    use super::*;
    const _: () = assert!(std::mem::size_of::<vertex_t>() == 8);
    const _: () = assert!(std::mem::size_of::<sector_t>() == 128);
    const _: () = assert!(std::mem::offset_of!(sector_t, floorheight) == 0);
    const _: () = assert!(std::mem::offset_of!(sector_t, ceilingheight) == 4);
    const _: () = assert!(std::mem::offset_of!(sector_t, floorpic) == 8);
    const _: () = assert!(std::mem::offset_of!(sector_t, ceilingpic) == 10);
    const _: () = assert!(std::mem::offset_of!(sector_t, lightlevel) == 12);
    const _: () = assert!(std::mem::offset_of!(sector_t, special) == 14);
    const _: () = assert!(std::mem::offset_of!(sector_t, tag) == 16);
    const _: () = assert!(std::mem::offset_of!(sector_t, soundtraversed) == 20);
    const _: () = assert!(std::mem::offset_of!(sector_t, soundtarget) == 24);
    const _: () = assert!(std::mem::offset_of!(sector_t, blockbox) == 32);
    const _: () = assert!(std::mem::offset_of!(sector_t, soundorg) == 48);
    const _: () = assert!(std::mem::offset_of!(sector_t, validcount) == 88);
    const _: () = assert!(std::mem::offset_of!(sector_t, thinglist) == 96);
    const _: () = assert!(std::mem::offset_of!(sector_t, specialdata) == 104);
    const _: () = assert!(std::mem::offset_of!(sector_t, linecount) == 112);
    const _: () = assert!(std::mem::offset_of!(sector_t, lines) == 120);

    const _: () = assert!(std::mem::size_of::<side_t>() == 24);
    const _: () = assert!(std::mem::offset_of!(side_t, textureoffset) == 0);
    const _: () = assert!(std::mem::offset_of!(side_t, rowoffset) == 4);
    const _: () = assert!(std::mem::offset_of!(side_t, toptexture) == 8);
    const _: () = assert!(std::mem::offset_of!(side_t, bottomtexture) == 10);
    const _: () = assert!(std::mem::offset_of!(side_t, midtexture) == 12);
    const _: () = assert!(std::mem::offset_of!(side_t, sector) == 16);

    const _: () = assert!(std::mem::size_of::<slopetype_t>() == 4);

    const _: () = assert!(std::mem::size_of::<line_t>() == 88);
    const _: () = assert!(std::mem::offset_of!(line_t, v1) == 0);
    const _: () = assert!(std::mem::offset_of!(line_t, v2) == 8);
    const _: () = assert!(std::mem::offset_of!(line_t, dx) == 16);
    const _: () = assert!(std::mem::offset_of!(line_t, dy) == 20);
    const _: () = assert!(std::mem::offset_of!(line_t, flags) == 24);
    const _: () = assert!(std::mem::offset_of!(line_t, special) == 26);
    const _: () = assert!(std::mem::offset_of!(line_t, tag) == 28);
    const _: () = assert!(std::mem::offset_of!(line_t, sidenum) == 30);
    const _: () = assert!(std::mem::offset_of!(line_t, bbox) == 36);
    const _: () = assert!(std::mem::offset_of!(line_t, slopetype) == 52);
    const _: () = assert!(std::mem::offset_of!(line_t, frontsector) == 56);
    const _: () = assert!(std::mem::offset_of!(line_t, backsector) == 64);
    const _: () = assert!(std::mem::offset_of!(line_t, validcount) == 72);
    const _: () = assert!(std::mem::offset_of!(line_t, specialdata) == 80);

    const _: () = assert!(std::mem::size_of::<subsector_t>() == 16);
    const _: () = assert!(std::mem::offset_of!(subsector_t, sector) == 0);
    const _: () = assert!(std::mem::offset_of!(subsector_t, numlines) == 8);
    const _: () = assert!(std::mem::offset_of!(subsector_t, firstline) == 10);

    const _: () = assert!(std::mem::size_of::<seg_t>() == 56);
    const _: () = assert!(std::mem::offset_of!(seg_t, v1) == 0);
    const _: () = assert!(std::mem::offset_of!(seg_t, v2) == 8);
    const _: () = assert!(std::mem::offset_of!(seg_t, offset) == 16);
    const _: () = assert!(std::mem::offset_of!(seg_t, angle) == 20);
    const _: () = assert!(std::mem::offset_of!(seg_t, sidedef) == 24);
    const _: () = assert!(std::mem::offset_of!(seg_t, linedef) == 32);
    const _: () = assert!(std::mem::offset_of!(seg_t, frontsector) == 40);
    const _: () = assert!(std::mem::offset_of!(seg_t, backsector) == 48);

    const _: () = assert!(std::mem::size_of::<node_t>() == 52);
    const _: () = assert!(std::mem::offset_of!(node_t, x) == 0);
    const _: () = assert!(std::mem::offset_of!(node_t, y) == 4);
    const _: () = assert!(std::mem::offset_of!(node_t, dx) == 8);
    const _: () = assert!(std::mem::offset_of!(node_t, dy) == 12);
    const _: () = assert!(std::mem::offset_of!(node_t, bbox) == 16);
    const _: () = assert!(std::mem::offset_of!(node_t, children) == 48);

    const _: () = assert!(std::mem::size_of::<drawseg_t>() == 64);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, curline) == 0);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, x1) == 8);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, x2) == 12);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, scale1) == 16);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, scale2) == 20);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, scalestep) == 24);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, silhouette) == 28);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, bsilheight) == 32);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, tsilheight) == 36);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, sprtopclip) == 40);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, sprbottomclip) == 48);
    const _: () = assert!(std::mem::offset_of!(drawseg_t, maskedtexturecol) == 56);

    const _: () = assert!(std::mem::size_of::<thinker_t>() == 24);
    const _: () = assert!(std::mem::size_of::<degenmobj_s>() == 40);
}
