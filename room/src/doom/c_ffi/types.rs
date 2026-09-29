//! The C-layout mirror types: the `#[repr(C)]` record mirrors that
//! pointer-casting FFI consumers rely on, plus the two opaque decls for
//! types only ever referenced through pointers.
//!
//! Charter note: these types are NEVER renamed or deduplicated in the
//! graduation sense -- graduated modules hold layout-parallel local
//! mirrors of the same C records (the shadow map is documented in the
//! module root); the copies are structurally identical, so pointer
//! casts between them work today. A dedup into one canonical home is a
//! post-graduation refactor touching ~90 files and does not ride this
//! wave (f1 report §0.4).

#![allow(non_camel_case_types)]

use std::ffi::{c_int, c_short, c_uint, c_ushort, c_void};

// ---------------------------------------------------------------------------
// Opaque types — we only need pointers to these for many FFI signatures.
// ---------------------------------------------------------------------------

/// Opaque animation-state record (`state_t` in `info.h`).
///
/// Only pointers to this type appear in cross-module FFI signatures; the
/// fields are not accessed from Rust here.
pub enum state_t {}
/// Opaque map-object class descriptor (`mobjinfo_t` in `info.h`).
///
/// Only pointers to this type appear in cross-module FFI signatures; the
/// fields are not accessed from Rust here.
pub enum mobjinfo_t {}

// ---------------------------------------------------------------------------
// Minimal repr(C) types needed for p_maputl.c tests.
//
// These definitions match the layouts in p_telept.rs / p_sight.rs so that
// pointer casts between the two are safe.
// ---------------------------------------------------------------------------

/// Runtime vertex record (`vertex_t` in `r_defs.h`).
///
/// Fields `x` and `y` are fixed-point map coordinates.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct vertex_t {
    pub x: c_int,
    pub y: c_int,
}

/// Parametric dividing line (`divline_t` in `p_local.h`).
///
/// Used by `P_PointOnDivlineSide` and the intercept-traversal helpers.
/// `(x, y)` is the start point; `(dx, dy)` is the direction vector in
/// fixed-point map units.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct divline_t {
    pub x: c_int,
    pub y: c_int,
    pub dx: c_int,
    pub dy: c_int,
}

/// Runtime BSP subsector (`subsector_t` in `r_defs.h`).
///
/// `sector` points to the owning `sector_t`; `firstline` indexes into the
/// segs array and `numlines` is the count of segs forming the convex
/// polygon for this subsector. Trailing padding aligns to 8 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct subsector_t {
    pub sector: *mut c_void,
    pub numlines: i16,
    pub firstline: i16,
    _pad: [u8; 4],
}

/// On-disk thing record (`mapthing_t` in `doomdata.h`).
///
/// 10-byte packed record from the WAD THINGS lump describing a spawn point.
/// `x` and `y` are in map units (not fixed-point), `angle` is degrees,
/// `type` is the Doom editor number, and `options` is a bit mask of skill
/// and multiplayer flags.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct mapthing_t {
    pub x: i16,
    pub y: i16,
    pub angle: i16,
    pub r#type: i16,
    pub options: i16,
}

/// Runtime sector record (`sector_t` in `r_defs.h`).
///
/// Layout mirrors the C definition exactly so pointer casts between the
/// Rust and C halves of the engine remain sound. `soundorg` is opaque
/// `degenmobj_t` storage (40 bytes); explicit `_pad0` reflects the C
/// compiler's natural padding after the four packed `c_short` fields.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t {
    pub floorheight: c_int,
    pub ceilingheight: c_int,
    pub floorpic: c_short,
    pub ceilingpic: c_short,
    pub lightlevel: c_short,
    pub special: c_short,
    pub tag: c_short,
    _pad0: [u8; 2],
    pub soundtraversed: c_int,
    pub soundtarget: *mut c_void,
    pub blockbox: [c_int; 4],
    pub soundorg: [u8; 40],
    pub validcount: c_int,
    pub thinglist: *mut c_void,
    pub specialdata: *mut c_void,
    pub linecount: c_int,
    pub lines: *mut *mut c_void,
}

/// Runtime line definition (`line_t` in `r_defs.h`).
///
/// A line is bounded by two vertices, has a direction vector `(dx, dy)`,
/// a `flags` bit mask of [`LinedefFlag`](crate::doom::c_ffi::LinedefFlag)
/// entries, an optional `special`
/// trigger, sector tags, and `sidenum[2]` indices (-1 for one-sided lines).
/// `frontsector` and `backsector` are opaque `sector_t*` pointers; cast
/// them to [`sector_t`] when access is required.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct line_t {
    pub v1: *mut vertex_t,
    pub v2: *mut vertex_t,
    pub dx: c_int,
    pub dy: c_int,
    pub flags: c_short,
    pub special: c_short,
    pub tag: c_short,
    pub sidenum: [c_short; 2],
    pub bbox: [c_int; 4],
    pub slopetype: c_int,
    pub frontsector: *mut c_void,
    pub backsector: *mut c_void,
    pub validcount: c_int,
    pub specialdata: *mut c_void,
}

/// Intercept record produced by `P_PathTraverse` (`intercept_t` in
/// `p_local.h`).
///
/// `frac` is the fractional position along the trace (fixed-point in
/// `[0, FRACUNIT]`). `isaline` selects the active arm of [`intercept_t_d`]:
/// non-zero means `d.line`, zero means `d.thing`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct intercept_t {
    pub frac: c_int,
    pub isaline: c_int, // boolean
    pub d: intercept_t_d,
}

/// Untagged union holding the intercepted entity (`intercept_t.d` in
/// `p_local.h`).
///
/// Active arm is selected by [`intercept_t::isaline`]. Reading the inactive
/// arm is undefined behaviour; callers must consult `isaline` first.
#[repr(C)]
#[derive(Clone, Copy)]
pub union intercept_t_d {
    pub thing: *mut c_void,
    pub line: *mut line_t,
}

/// Runtime map-object record (`mobj_t` in `p_mobj.h`).
///
/// Layout matches the C definition exactly, including the 24-byte
/// `thinker_t` prefix (`thinker_prev`, `thinker_next`, `thinker_fn`) and
/// the explicit `_padN` fillers required by the C compiler's alignment
/// rules on 64-bit platforms. See the project memory note on `MobjStub` for
/// the consequences of getting this layout wrong.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct mobj_t {
    pub thinker_prev: *mut c_void,
    pub thinker_next: *mut c_void,
    pub thinker_fn: *mut c_void,
    pub x: c_int,
    pub y: c_int,
    pub z: c_int,
    _pad0: [u8; 4],
    pub snext: *mut c_void,
    pub sprev: *mut c_void,
    pub angle: c_uint,
    pub sprite: c_int,
    pub frame: c_int,
    _pad1: [u8; 4],
    pub bnext: *mut c_void,
    pub bprev: *mut c_void,
    pub subsector: *mut c_void,
    pub floorz: c_int,
    pub ceilingz: c_int,
    pub radius: c_int,
    pub height: c_int,
    pub momx: c_int,
    pub momy: c_int,
    pub momz: c_int,
    pub validcount: c_int,
    pub type_: c_int,
    _pad2: [u8; 4],
    pub info: *mut mobjinfo_t,
    pub tics: c_int,
    _pad3: [u8; 4],
    pub state: *mut state_t,
    pub flags: c_int,
    pub health: c_int,
    pub movedir: c_int,
    pub movecount: c_int,
    pub target: *mut c_void,
    pub reactiontime: c_int,
    pub threshold: c_int,
    pub player: *mut c_void,
    pub lastlook: c_int,
    pub spawnpoint: mapthing_t,
    _pad4: [u8; 2],
    pub tracer: *mut c_void,
}

// ---------------------------------------------------------------------------
// Runtime renderer types (r_defs.h) — used by unported r_segs.c / r_things.c
// ---------------------------------------------------------------------------

/// SideDef: visual appearance of a wall segment (`side_t` in `r_defs.h`).
///
/// Two-byte `_pad` matches the C compiler's natural alignment after the
/// three packed `c_short` texture indices and before the pointer field.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct side_t {
    pub textureoffset: c_int,
    pub rowoffset: c_int,
    pub toptexture: c_short,
    pub bottomtexture: c_short,
    pub midtexture: c_short,
    _pad: [u8; 2],
    pub sector: *mut sector_t,
}

/// LineSeg: a BSP-split segment of a line definition (`seg_t` in
/// `r_defs.h`).
///
/// `offset` is the distance along the parent linedef to `v1` in
/// fixed-point. `angle` is a BAM (Binary Angle Measurement) facing
/// direction. `frontsector` is always non-null; `backsector` is null
/// for one-sided segments (per the C comment in `r_defs.h`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct seg_t {
    pub v1: *mut vertex_t,
    pub v2: *mut vertex_t,
    pub offset: c_int,
    pub angle: c_uint,
    pub sidedef: *mut side_t,
    pub linedef: *mut line_t,
    pub frontsector: *mut sector_t,
    pub backsector: *mut sector_t,
}

/// BSP node: partitions space into two sub-trees (`node_t` in `r_defs.h`).
///
/// `(x, y)` is a point on the partition line; `(dx, dy)` is the direction
/// vector. `bbox[side]` is the bounding box of each child subtree;
/// `children[side]` is a node or subsector index, with the high bit set
/// to indicate a subsector leaf.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct node_t {
    pub x: c_int,
    pub y: c_int,
    pub dx: c_int,
    pub dy: c_int,
    pub bbox: [[c_int; 4]; 2],
    pub children: [c_ushort; 2],
}

/// Draw segment: one visible wall segment produced by the BSP traversal
/// (`drawseg_t` in `r_defs.h`).
///
/// Built by `R_StoreWallRange`; consumed by sprite clipping and the wall
/// rasteriser. `silhouette` is a bit mask indicating whether the segment
/// occludes from below (`SIL_BOTTOM`), above (`SIL_TOP`), or both.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct drawseg_t {
    pub curline: *mut seg_t,
    pub x1: c_int,
    pub x2: c_int,
    pub scale1: c_int,
    pub scale2: c_int,
    pub scalestep: c_int,
    pub silhouette: c_int,
    pub bsilheight: c_int,
    pub tsilheight: c_int,
    pub sprtopclip: *mut c_short,
    pub sprbottomclip: *mut c_short,
    pub maskedtexturecol: *mut c_short,
}

/// Visible sprite: a thing that is (partly) visible in the current frame
/// (`vissprite_t` in `r_defs.h`).
///
/// Built by `R_ProjectSprite`; sorted by `scale` (depth) before
/// rasterisation. `colormap` selects light level / palette translation
/// per-column.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct vissprite_t {
    pub prev: *mut vissprite_t,
    pub next: *mut vissprite_t,
    pub x1: c_int,
    pub x2: c_int,
    pub gx: c_int,
    pub gy: c_int,
    pub gz: c_int,
    pub gzt: c_int,
    pub startfrac: c_int,
    pub scale: c_int,
    pub xiscale: c_int,
    pub texturemid: c_int,
    pub patch: c_int,
    _pad: [u8; 4],
    pub colormap: *mut u8,
    pub mobjflags: c_int,
    _pad2: [u8; 4],
}

/// One animation-frame of a sprite, matching r_defs.h `spriteframe_t`.
///
/// Layout (28 bytes):
///   +0  rotate (boolean/int – 4 bytes)
///   +4  lump[8] (short[8] – 16 bytes)
///   +20 flip[8] (byte[8] – 8 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct spriteframe_t {
    /// 0 = use frame 0 for all rotations; 1 = use rotation-specific lumps.
    pub rotate: c_int,
    /// WAD lump index (relative to firstspritelump) for each of 8 rotations.
    pub lump: [c_short; 8],
    /// 1 = horizontally flip the lump for this rotation; 0 = no flip.
    pub flip: [u8; 8],
}

// ---------------------------------------------------------------------------
// i_scale.c
// Screen-scaling mode descriptors.
// ---------------------------------------------------------------------------

/// A screen scaling mode descriptor, matching `screen_mode_t` in `i_video.h`.
///
/// Layout on 64-bit (32 bytes):
///   +0  width          (int, 4 bytes)
///   +4  height         (int, 4 bytes)
///   +8  InitMode       (fn ptr, 8 bytes)
///   +16 DrawScreen     (fn ptr, 8 bytes)
///   +24 poor_quality   (int/boolean, 4 bytes)
///   +28 [4 bytes tail-padding to align struct to 8]
#[repr(C)]
pub struct screen_mode_t {
    /// Output buffer width in pixels.
    pub width: c_int,
    /// Output buffer height in pixels.
    pub height: c_int,
    /// Optional initialiser called once with the game palette.
    pub init_mode: Option<unsafe extern "C" fn(*mut u8)>,
    /// Draw function: copies src buffer → dest buffer for the given rectangle.
    pub draw_screen: Option<unsafe extern "C" fn(c_int, c_int, c_int, c_int) -> c_int>,
    /// True when this mode uses blended interpolation (lower visual quality).
    pub poor_quality: c_int,
}
