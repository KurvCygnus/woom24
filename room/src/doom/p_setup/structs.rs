//! Data vocabulary of the level loader: the packed on-disk WAD records,
//! the zone-memory purge tag, the linedef slope classes, and the blockmap
//! arithmetic constants. All names are upstream (data tier); nothing here
//! is logic.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;

use crate::doom::m_fixed::{FRACBITS, FRACUNIT};

/// Zone-memory purge level used when freeing all level data between maps.
/// Matches `PU_PURGELEVEL` in `z_zone.h`; all tags in `[PU_LEVEL,
/// PU_PURGELEVEL)` are freed by `Z_FreeTags` at the start of `P_SetupLevel`.
pub(super) const PU_PURGELEVEL: c_int = 7;

/// Linedef slope type: line is perfectly horizontal (dy == 0).
/// Referenced by `c_ffi::line_t.slopetype`.
pub(super) const ST_HORIZONTAL: c_int = 0;
/// Linedef slope type: line is perfectly vertical (dx == 0).
/// Referenced by `c_ffi::line_t.slopetype`.
pub(super) const ST_VERTICAL: c_int = 1;
/// Linedef slope type: dy/dx > 0 (rises left-to-right).
/// Referenced by `c_ffi::line_t.slopetype`.
pub(super) const ST_POSITIVE: c_int = 2;
/// Linedef slope type: dy/dx < 0 (falls left-to-right).
/// Referenced by `c_ffi::line_t.slopetype`.
pub(super) const ST_NEGATIVE: c_int = 3;

/// Shift to convert a fixed-point map coordinate to a blockmap cell index.
/// Equals `FRACBITS + 7`, i.e. each blockmap cell covers 128 map units.
pub(super) const MAPBLOCKSHIFT: c_int = FRACBITS as c_int + 7;

/// Maximum radius added/subtracted when clamping sector bounding boxes to
/// blockmap cells. 32 map units in fixed-point (32 << FRACBITS).
pub(super) const MAXRADIUS: c_int = 32 * FRACUNIT;

/// Maximum number of deathmatch start positions in a level.
/// Matches `MAX_DEATHMATCH_STARTS` in `p_setup.c`.
pub const MAX_DEATHMATCH_STARTS: usize = 10;

/// On-disk vertex record from the WAD VERTEXES lump.
/// Maps to `mapvertex_t` in `p_local.h`. Coordinates are 16-bit integers
/// in map units; they are sign-extended and shifted left by `FRACBITS` when
/// copied into the runtime `vertex_t`.
#[repr(C, packed)]
pub(super) struct mapvertex_t
{
    pub x: i16,
    pub y: i16,
}

/// On-disk sidedef record from the WAD SIDEDEFS lump.
/// Maps to `mapsidedef_t` in `p_local.h`. Texture offsets are in map units
/// and are shifted left by `FRACBITS` when copied to the runtime `side_t`.
/// Texture name fields are 8-byte null-padded ASCII strings resolved to
/// runtime texture indices by `R_TextureNumForName`.
#[repr(C, packed)]
pub(super) struct mapsidedef_t
{
    pub textureoffset: i16,
    pub rowoffset: i16,
    pub toptexture: [u8; 8],
    pub bottomtexture: [u8; 8],
    pub midtexture: [u8; 8],
    pub sector: i16,
}

/// On-disk linedef record from the WAD LINEDEFS lump.
/// Maps to `maplinedef_t` in `p_local.h`. `sidenum[1]` is -1 for one-sided
/// lines; the runtime `line_t` stores null pointers in that case.
#[repr(C, packed)]
pub(super) struct maplinedef_t
{
    pub v1: i16,
    pub v2: i16,
    pub flags: i16,
    pub special: i16,
    pub tag: i16,
    pub sidenum: [i16; 2],
}

/// On-disk sector record from the WAD SECTORS lump.
/// Maps to `mapsector_t` in `p_local.h`. Floor/ceiling heights are in map
/// units and shifted left by `FRACBITS` in the runtime `sector_t`. Flat name
/// fields are 8-byte null-padded ASCII strings resolved via `R_FlatNumForName`.
#[repr(C, packed)]
pub(super) struct mapsector_t
{
    pub floorheight: i16,
    pub ceilingheight: i16,
    pub floorpic: [u8; 8],
    pub ceilingpic: [u8; 8],
    pub lightlevel: i16,
    pub special: i16,
    pub tag: i16,
}

/// On-disk subsector record from the WAD SSECTORS lump.
/// Maps to `mapsubsector_t` in `p_local.h`. `firstseg` is an index into the
/// segs array; `numsegs` is the count of consecutive segs forming the convex
/// polygon of this subsector.
#[repr(C, packed)]
pub(super) struct mapsubsector_t
{
    pub numsegs: i16,
    pub firstseg: i16,
}

/// On-disk seg record from the WAD SEGS lump.
/// Maps to `mapseg_t` in `p_local.h`. `angle` is a BAM (Binary Angle
/// Measurement) stored in the upper 16 bits of a `u32` after shifting.
/// `offset` is the distance along the linedef to the seg's start vertex,
/// in fixed-point.
#[repr(C, packed)]
pub(super) struct mapseg_t
{
    pub v1: i16,
    pub v2: i16,
    pub angle: i16,
    pub linedef: i16,
    pub side: i16,
    pub offset: i16,
}

/// On-disk BSP node record from the WAD NODES lump.
/// Maps to `mapnode_t` in `p_local.h`. `x`, `y`, `dx`, `dy` define the
/// partition line in map units. `bbox[2][4]` are the bounding boxes for each
/// child subtree. `children[2]` are child indices; the high bit set indicates
/// a subsector leaf.
#[repr(C, packed)]
pub(super) struct mapnode_t
{
    pub x: i16,
    pub y: i16,
    pub dx: i16,
    pub dy: i16,
    pub bbox: [[i16; 4]; 2],
    pub children: [u16; 2],
}

/// On-disk thing record from the WAD THINGS lump, and also the runtime spawn
/// descriptor passed to `P_SpawnMapThing`.
///
/// Maps to `mapthing_t` in `p_local.h`. This struct is `pub` because it is
/// referenced by `deathmatchstarts`, `deathmatch_p`, and `playerstarts`
/// globals that are visible to C. `#[repr(C)]` ensures ABI compatibility;
/// `#[derive(Clone, Copy)]` allows it to be used by value in initialization
/// expressions.
///
/// Fields:
/// - `x`, `y`: map-unit position (not fixed-point)
/// - `angle`: facing direction in degrees (0, 45, 90, ...)
/// - `type`: Doom editor number identifying the thing class
/// - `options`: bit flags (skill levels, deaf, etc.)
///
/// One of three distinct `mapthing_t` types in-tree (this one, `c_ffi`'s,
/// and `p_telept`'s); they are deliberate ABI mirrors -- never unify them.
/// The `P_LoadThings` cast to `p_telept::mapthing_t` is a deliberate bridge.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct mapthing_t
{
    pub x: i16,
    pub y: i16,
    pub angle: i16,
    pub r#type: i16,
    pub options: i16,
}
