//! The lump loaders: read each BSP/geometry lump of the current map from
//! the WAD into the global map tables. One function per lump, called from
//! `level::setup_level` in a fixed order (the order is the behavior).
//! The missed-backside substitution (`null_sector`) and the thing-filter
//! classification (`dtmc`) are hooked in at their upstream positions.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_short, c_uint, c_ushort, c_void};
use std::ptr;

use crate::doom::c_ffi::{
    line_t, node_t, sector_t, seg_t, side_t, subsector_t, vertex_t, LinedefFlag,
};
use crate::doom::doomstat::gamemode;
use crate::doom::d_mode;
use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::FRACBITS;
use crate::doom::p_mobj::P_SpawnMapThing;
use crate::doom::r_data::{R_FlatNumForName, R_TextureNumForName};
use crate::doom::violations::{self, VanillaViolation};
use crate::doom::w_wad::{W_CacheLumpNum, W_LumpLength, W_ReadLump, W_ReleaseLumpNum};
use crate::doom::z_zone::{PU_LEVEL, PU_STATIC, Z_Malloc};

use super::dtmc::{is_noncommercial_thing, slopetype_of};
use super::globals::{
    bmapheight, bmaporgx, bmaporgy, bmapwidth, blocklinks, blockmap, blockmaplump, lines, nodes,
    numlines, numnodes, numsectors, numsegs, numsides, numsubsectors, numvertexes, sectors, segs,
    sides, subsectors, vertexes,
};
use super::null_sector::sector_at_null_address;
use super::structs::{
    maplinedef_t, mapnode_t, mapsector_t, mapseg_t, mapsidedef_t, mapsubsector_t, mapthing_t,
    mapvertex_t,
};

/// Convert a little-endian `i16` from a WAD file to host byte order.
/// On little-endian hosts this is a no-op cast; on big-endian it swaps.
/// The C original is the `SHORT` macro from `i_swap.h`.
#[doc(alias = "SHORT")]
#[inline]
fn le_i16(x: i16) -> i16 { i16::from_le(x) }

/// Load the VERTEXES lump and populate the global `vertexes` array.
///
/// Allocates `numvertexes * sizeof(vertex_t)` bytes at `PU_LEVEL`, reads each
/// on-disk `mapvertex_t` (2 × i16 little-endian), and stores the result as
/// fixed-point (16.16) coordinates by shifting left by `FRACBITS`. The raw
/// lump is released after conversion.
///
/// `lump` must be the lump number of the VERTEXES entry for the current map
/// (typically `lumpnum + MapLump::VERTEXES`).
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadVertexes")]
#[export_name = "P_LoadVertexes"]
pub extern "C" fn load_vertexes(lump: c_int)
{
    unsafe
    {
        numvertexes = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapvertex_t>() as c_int;
        vertexes = Z_Malloc(
            numvertexes * std::mem::size_of::<vertex_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut vertex_t;

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ml = data as *mut mapvertex_t;
        let mut li = vertexes;

        for _ in 0..numvertexes
        {
            (*li).x = (le_i16((*ml).x) as c_int) << FRACBITS;
            (*li).y = (le_i16((*ml).y) as c_int) << FRACBITS;
            li = li.add(1);
            ml = ml.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the SEGS lump and populate the global `segs` array.
///
/// Allocates `numsegs * sizeof(seg_t)` bytes at `PU_LEVEL` (zero-initialized
/// via zone memory) and fills each entry from the corresponding on-disk
/// `mapseg_t`. Key conversions:
/// - Vertex indices are resolved to pointers into `vertexes`.
/// - `angle` is shifted left 16 to become a full BAM (Binary Angle
///   Measurement) u32 value.
/// - `offset` is shifted left by `FRACBITS` to become fixed-point.
/// - `sidedef` and `frontsector` are resolved from the linedef's `sidenum`
///   array using the seg's side (0 = front, 1 = back).
/// - For two-sided linedefs, `backsector` is resolved from the opposite
///   sidenum; if that sidenum is out of range, `GetSectorAtNullAddress` is
///   used (glass-hack compatibility).
///
/// `lump` must be the lump number of the SEGS entry for the current map.
/// Precondition: `vertexes`, `lines`, and `sides` must already be loaded.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadSegs")]
#[export_name = "P_LoadSegs"]
pub extern "C" fn load_segs(lump: c_int)
{
    unsafe
    {
        numsegs = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapseg_t>() as c_int;
        segs = Z_Malloc(
            numsegs * std::mem::size_of::<seg_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut seg_t;
        // Zeroed by Z_Malloc internally

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ml = data as *mut mapseg_t;
        let mut li = segs;

        for _ in 0..numsegs
        {
            (*li).v1 = vertexes.offset(le_i16((*ml).v1) as isize);
            (*li).v2 = vertexes.offset(le_i16((*ml).v2) as isize);
            (*li).angle = ((le_i16((*ml).angle) as c_int) << 16) as c_uint;
            (*li).offset = (le_i16((*ml).offset) as c_int) << 16;
            let linedef = le_i16((*ml).linedef) as c_int;
            let ldef = lines.offset(linedef as isize);
            (*li).linedef = ldef;
            let side = le_i16((*ml).side) as c_int;
            (*li).sidedef = sides.offset((*ldef).sidenum[side as usize] as isize);
            (*li).frontsector = (*sides.offset((*ldef).sidenum[side as usize] as isize)).sector;

            if (*ldef).flags & LinedefFlag::TWOSIDED as i16 != 0
            {
                let sidenum = (*ldef).sidenum[side as usize ^ 1];
                if sidenum < 0 || sidenum as c_int >= numsides
                {
                    violations::record(VanillaViolation::MissedBackSideOverrun);
                    (*li).backsector = sector_at_null_address();
                }
                else { (*li).backsector = sides.offset(sidenum as isize).as_ref().unwrap().sector; }
            }
            else { (*li).backsector = ptr::null_mut(); }

            li = li.add(1);
            ml = ml.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the SSECTORS lump and populate the global `subsectors` array.
///
/// Allocates `numsubsectors * sizeof(subsector_t)` bytes at `PU_LEVEL`
/// (zero-initialized) and fills each entry from the corresponding on-disk
/// `mapsubsector_t`. The `numlines` and `firstline` fields (confusingly named
/// in the runtime struct - they actually count/index segs) are byte-swapped
/// from little-endian.
///
/// `lump` must be the lump number of the SSECTORS entry for the current map.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadSubsectors")]
#[export_name = "P_LoadSubsectors"]
pub extern "C" fn load_subsectors(lump: c_int)
{
    unsafe
    {
        numsubsectors =
            W_LumpLength(lump as c_uint) / std::mem::size_of::<mapsubsector_t>() as c_int;
        subsectors = Z_Malloc(
            numsubsectors * std::mem::size_of::<subsector_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut subsector_t;
        // Zeroed by Z_Malloc internally

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ms = data as *mut mapsubsector_t;
        let mut ss = subsectors;

        for _ in 0..numsubsectors
        {
            (*ss).numlines = le_i16((*ms).numsegs);
            (*ss).firstline = le_i16((*ms).firstseg);
            ss = ss.add(1);
            ms = ms.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the SECTORS lump and populate the global `sectors` array.
///
/// Allocates `numsectors * sizeof(sector_t)` bytes at `PU_LEVEL`
/// (zero-initialized) and fills each entry from the corresponding on-disk
/// `mapsector_t`. Floor and ceiling heights are shifted left by `FRACBITS`
/// to become fixed-point. Flat names are resolved to runtime indices via
/// `R_FlatNumForName`. The `thinglist` field is explicitly cleared to null.
///
/// `lump` must be the lump number of the SECTORS entry for the current map.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadSectors")]
#[export_name = "P_LoadSectors"]
pub extern "C" fn load_sectors(lump: c_int)
{
    unsafe
    {
        numsectors = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapsector_t>() as c_int;
        sectors = Z_Malloc(
            numsectors * std::mem::size_of::<sector_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut sector_t;
        // Zeroed by Z_Malloc internally

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ms = data as *mut mapsector_t;
        let mut ss = sectors;

        for _ in 0..numsectors
        {
            (*ss).floorheight = (le_i16((*ms).floorheight) as c_int) << FRACBITS;
            (*ss).ceilingheight = (le_i16((*ms).ceilingheight) as c_int) << FRACBITS;
            (*ss).floorpic = R_FlatNumForName((*ms).floorpic.as_ptr() as *mut c_char) as c_short;
            (*ss).ceilingpic =
                R_FlatNumForName((*ms).ceilingpic.as_ptr() as *mut c_char) as c_short;
            (*ss).lightlevel = le_i16((*ms).lightlevel);
            (*ss).special = le_i16((*ms).special);
            (*ss).tag = le_i16((*ms).tag);
            (*ss).thinglist = ptr::null_mut();
            ss = ss.add(1);
            ms = ms.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the NODES lump and populate the global `nodes` array.
///
/// Allocates `numnodes * sizeof(node_t)` bytes at `PU_LEVEL` and fills each
/// entry from the corresponding on-disk `mapnode_t`. Partition-line origin
/// (`x`, `y`) and direction (`dx`, `dy`) are shifted left by `FRACBITS` to
/// become fixed-point. Bounding-box values are similarly shifted. Child
/// indices are stored as `u16`; the high bit (0x8000) indicates a subsector
/// leaf rather than an internal node.
///
/// `lump` must be the lump number of the NODES entry for the current map.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadNodes")]
#[export_name = "P_LoadNodes"]
pub extern "C" fn load_nodes(lump: c_int)
{
    unsafe
    {
        numnodes = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapnode_t>() as c_int;
        nodes = Z_Malloc(
            numnodes * std::mem::size_of::<node_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut node_t;

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut mn = data as *mut mapnode_t;
        let mut no = nodes;

        for _ in 0..numnodes
        {
            (*no).x = (le_i16((*mn).x) as c_int) << FRACBITS;
            (*no).y = (le_i16((*mn).y) as c_int) << FRACBITS;
            (*no).dx = (le_i16((*mn).dx) as c_int) << FRACBITS;
            (*no).dy = (le_i16((*mn).dy) as c_int) << FRACBITS;
            for j in 0..2usize
            {
                (*no).children[j] = le_i16((*mn).children[j] as i16) as c_ushort;
                for k in 0..4usize { (*no).bbox[j][k] = (le_i16((*mn).bbox[j][k]) as c_int) << FRACBITS; }
            }
            no = no.add(1);
            mn = mn.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the THINGS lump and spawn all map objects for the current level.
///
/// Iterates over each on-disk `mapthing_t` in the lump. In non-commercial
/// (shareware/registered) game modes, monster types that only appear in Doom
/// II are skipped; when the first such type is encountered the loop
/// **breaks** immediately (not `continue`), so no subsequent things are
/// processed either - this matches vanilla Doom's behavior and preserves
/// demo-compatible RNG sequencing.
///
/// Each surviving thing is byte-swapped and passed to `P_SpawnMapThing`.
/// `P_SpawnMapThing` handles player starts, deathmatch starts, and all
/// game objects.
///
/// `lump` must be the lump number of the THINGS entry for the current map.
/// Preconditions: `vertexes`, `lines`, `sides`, `sectors`, and `blockmap`
/// must already be loaded; `deathmatch_p` must be reset to the base of
/// `deathmatchstarts`.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadThings")]
#[export_name = "P_LoadThings"]
pub extern "C" fn load_things(lump: c_int)
{
    unsafe
    {
        let data = W_CacheLumpNum(lump, PU_STATIC);
        let numthings = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapthing_t>() as c_int;
        let mut mt = data as *mut mapthing_t;

        for _ in 0..numthings
        {
            let mut spawn = true;

            if gamemode != d_mode::commercial
            {
                let thing_type = le_i16((*mt).r#type);
                if is_noncommercial_thing(thing_type) { spawn = false; }
            }

            if !spawn { break; }

            let mut spawnthing = mapthing_t {
                x: le_i16((*mt).x),
                y: le_i16((*mt).y),
                angle: le_i16((*mt).angle),
                r#type: le_i16((*mt).r#type),
                options: le_i16((*mt).options),
            };
            P_SpawnMapThing(&raw mut spawnthing as *mut crate::doom::p_telept::mapthing_t);
            mt = mt.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the LINEDEFS lump and populate the global `lines` array.
///
/// Allocates `numlines * sizeof(line_t)` bytes at `PU_LEVEL`
/// (zero-initialized) and fills each entry from the corresponding on-disk
/// `maplinedef_t`. Notable steps per linedef:
/// - Vertex pointers are resolved from indices into `vertexes`.
/// - `dx` and `dy` are computed from the vertex coordinates (fixed-point).
/// - `slopetype` is set by `dtmc::slopetype_of` (`ST_VERTICAL`,
///   `ST_HORIZONTAL`, `ST_POSITIVE`, or `ST_NEGATIVE`).
/// - The linedef's axis-aligned bounding box (`bbox[4]`) is computed from the
///   two vertex coordinates.
/// - Front and back sector pointers are resolved from `sidenum` indices; -1
///   means no side (one-sided line), resulting in a null sector pointer.
///
/// `lump` must be the lump number of the LINEDEFS entry for the current map.
/// Preconditions: `vertexes` and `sides` must already be loaded.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadLineDefs")]
#[export_name = "P_LoadLineDefs"]
pub extern "C" fn load_linedefs(lump: c_int)
{
    unsafe
    {
        numlines = W_LumpLength(lump as c_uint) / std::mem::size_of::<maplinedef_t>() as c_int;
        lines = Z_Malloc(
            numlines * std::mem::size_of::<line_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut line_t;
        // Zeroed by Z_Malloc internally

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut mld = data as *mut maplinedef_t;
        let mut ld = lines;

        for _ in 0..numlines
        {
            (*ld).flags = le_i16((*mld).flags);
            (*ld).special = le_i16((*mld).special);
            (*ld).tag = le_i16((*mld).tag);
            let v1 = vertexes.offset(le_i16((*mld).v1) as isize);
            let v2 = vertexes.offset(le_i16((*mld).v2) as isize);
            (*ld).v1 = v1;
            (*ld).v2 = v2;
            (*ld).dx = (*v2).x - (*v1).x;
            (*ld).dy = (*v2).y - (*v1).y;

            (*ld).slopetype = slopetype_of((*ld).dx, (*ld).dy);

            if (*v1).x < (*v2).x
            {
                (*ld).bbox[BBox::LEFT] = (*v1).x;
                (*ld).bbox[BBox::RIGHT] = (*v2).x;
            }
            else
            {
                (*ld).bbox[BBox::LEFT] = (*v2).x;
                (*ld).bbox[BBox::RIGHT] = (*v1).x;
            }

            if (*v1).y < (*v2).y
            {
                (*ld).bbox[BBox::BOTTOM] = (*v1).y;
                (*ld).bbox[BBox::TOP] = (*v2).y;
            }
            else
            {
                (*ld).bbox[BBox::BOTTOM] = (*v2).y;
                (*ld).bbox[BBox::TOP] = (*v1).y;
            }

            (*ld).sidenum[0] = le_i16((*mld).sidenum[0]);
            (*ld).sidenum[1] = le_i16((*mld).sidenum[1]);

            if (*ld).sidenum[0] != -1
            {
                (*ld).frontsector = sides
                    .offset((*ld).sidenum[0] as isize)
                    .as_ref()
                    .unwrap()
                    .sector as *mut c_void;
            }
            else { (*ld).frontsector = ptr::null_mut(); }

            if (*ld).sidenum[1] != -1
            {
                (*ld).backsector = sides
                    .offset((*ld).sidenum[1] as isize)
                    .as_ref()
                    .unwrap()
                    .sector as *mut c_void;
            }
            else { (*ld).backsector = ptr::null_mut(); }

            ld = ld.add(1);
            mld = mld.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the SIDEDEFS lump and populate the global `sides` array.
///
/// Allocates `numsides * sizeof(side_t)` bytes at `PU_LEVEL`
/// (zero-initialized) and fills each entry from the corresponding on-disk
/// `mapsidedef_t`. Texture offsets are shifted left by `FRACBITS` to become
/// fixed-point. Texture names (8-byte null-padded ASCII) are resolved to
/// runtime indices via `R_TextureNumForName`. The `sector` pointer is
/// resolved from the on-disk sector index into the `sectors` array.
///
/// `lump` must be the lump number of the SIDEDEFS entry for the current map.
/// Precondition: `sectors` must already be loaded.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadSideDefs")]
#[export_name = "P_LoadSideDefs"]
pub extern "C" fn load_sidedefs(lump: c_int)
{
    unsafe
    {
        numsides = W_LumpLength(lump as c_uint) / std::mem::size_of::<mapsidedef_t>() as c_int;
        sides = Z_Malloc(
            numsides * std::mem::size_of::<side_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut side_t;
        // Zeroed by Z_Malloc internally

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut msd = data as *mut mapsidedef_t;
        let mut sd = sides;

        for _ in 0..numsides
        {
            (*sd).textureoffset = (le_i16((*msd).textureoffset) as c_int) << FRACBITS;
            (*sd).rowoffset = (le_i16((*msd).rowoffset) as c_int) << FRACBITS;
            (*sd).toptexture =
                R_TextureNumForName((*msd).toptexture.as_ptr() as *mut c_char) as c_short;
            (*sd).bottomtexture =
                R_TextureNumForName((*msd).bottomtexture.as_ptr() as *mut c_char) as c_short;
            (*sd).midtexture =
                R_TextureNumForName((*msd).midtexture.as_ptr() as *mut c_char) as c_short;
            (*sd).sector = sectors.offset(le_i16((*msd).sector) as isize);
            sd = sd.add(1);
            msd = msd.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

/// Load the BLOCKMAP lump and initialize the blockmap collision grid.
///
/// The blockmap is a WAD-stored acceleration structure dividing the map into
/// 128-unit cells. Each cell contains a null-terminated list of linedefs that
/// pass through it, enabling fast broad-phase collision detection.
///
/// Loading steps:
/// 1. Allocate zone memory for the raw lump and read it with `W_ReadLump`.
/// 2. Set `blockmap = blockmaplump + 4` (skip the 4-short header).
/// 3. Byte-swap all `count = lumplen / 2` shorts from little-endian to native.
/// 4. Extract the header: `bmaporgx`, `bmaporgy` (shifted by `FRACBITS`),
///    `bmapwidth`, `bmapheight`.
/// 5. Allocate and zero-fill the `blocklinks` thing-chain array
///    (`bmapwidth * bmapheight` pointer-sized entries).
///
/// `lump` must be the lump number of the BLOCKMAP entry for the current map.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_LoadBlockMap")]
#[export_name = "P_LoadBlockMap"]
pub extern "C" fn load_blockmap(lump: c_int)
{
    unsafe
    {
        let lumplen = W_LumpLength(lump as c_uint);
        let count = lumplen / 2;

        blockmaplump = Z_Malloc(lumplen, PU_LEVEL, ptr::null_mut()) as *mut c_short;
        W_ReadLump(lump as c_uint, blockmaplump as *mut c_void);
        blockmap = blockmaplump.add(4);

        // Swap all short integers to native byte ordering.
        for i in 0..count { *blockmaplump.add(i as usize) = le_i16(*blockmaplump.add(i as usize)); }

        bmaporgx = (*blockmaplump.add(0) as c_int) << FRACBITS;
        bmaporgy = (*blockmaplump.add(1) as c_int) << FRACBITS;
        bmapwidth = *blockmaplump.add(2) as c_int;
        bmapheight = *blockmaplump.add(3) as c_int;

        let bcount = std::mem::size_of::<*mut c_void>() * bmapwidth as usize * bmapheight as usize;
        blocklinks = Z_Malloc(bcount as c_int, PU_LEVEL, ptr::null_mut()) as *mut *mut c_void;
        libc::memset(blocklinks as *mut c_void, 0, bcount);
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_setup::dtmc::is_noncommercial_thing;

    // -----------------------------------------------------------------------
    // Regression: P_LoadThings break-on-non-commercial-monster
    // -----------------------------------------------------------------------

    /// Simulates the `load_things` thing-filter loop in pure Rust.
    /// Returns the number of things that would actually be spawned.
    ///
    /// In shareware/non-commercial mode the loop must **break** (not continue)
    /// when it hits the first non-commercial monster type.  Using `continue`
    /// instead causes every subsequent thing to also be processed, which
    /// changes the RNG call sequence and breaks demo determinism.
    ///
    /// The type set itself is the extracted `dtmc::is_noncommercial_thing`
    /// (the filter is the real one; the break-at-call-site semantics is what
    /// this mirror pins).
    fn count_spawnable_things(things: &[i16], commercial: bool) -> usize
    {
        let mut count = 0;
        for &thing_type in things
        {
            let mut spawn = true;
            if !commercial { if is_noncommercial_thing(thing_type) { spawn = false; } }
            if !spawn
            {
                break; // MUST break -- was `continue` in the buggy port
            }
            count += 1;
        }
        count
    }

    /// In commercial mode every thing is spawned regardless of type.
    #[test]
    fn p_load_things_commercial_spawns_all()
    {
        // Types: player start, imp, Archvile(64), cacodemon, Revenant(66)
        let things: [i16; 5] = [1, 3001, 64, 3003, 66];
        assert_eq!(count_spawnable_things(&things, true), 5);
    }

    /// In shareware mode the loop stops at the first non-commercial monster.
    /// Things after it must NOT be spawned (regression: was `continue`).
    #[test]
    fn p_load_things_shareware_breaks_at_non_commercial()
    {
        // Types: player start, imp, Archvile(64), cacodemon, Revenant(66)
        // Archvile is non-commercial → loop breaks, only 2 things spawned.
        let things: [i16; 5] = [1, 3001, 64, 3003, 66];
        assert_eq!(count_spawnable_things(&things, false), 2);
    }

    /// Non-commercial monster at the very start → zero things spawned.
    #[test]
    fn p_load_things_shareware_first_thing_non_commercial()
    {
        let things: [i16; 3] = [64, 1, 3001]; // Archvile first
        assert_eq!(count_spawnable_things(&things, false), 0);
    }

    /// No non-commercial monsters → all things spawned in shareware mode.
    #[test]
    fn p_load_things_shareware_no_non_commercial_spawns_all()
    {
        let things: [i16; 4] = [1, 3001, 3003, 3004]; // all shareware types
        assert_eq!(count_spawnable_things(&things, false), 4);
    }

    /// Non-commercial monster at the very end → everything before spawns.
    #[test]
    fn p_load_things_shareware_non_commercial_at_end()
    {
        let things: [i16; 5] = [1, 3001, 3003, 3004, 88]; // Boss Brain last
        assert_eq!(count_spawnable_things(&things, false), 4);
    }
}
