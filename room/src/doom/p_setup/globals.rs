//! The global map tables: every `#[no_mangle]` static the loader fills in
//! and the rest of the engine reads. One data home; upstream names and C
//! linkage retained (`p_saveg` links several of them by symbol through its
//! `extern` block, and the web shell takes `addr_of!` on `sectors` /
//! `numsectors` through this module root).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_short, c_void};
use std::ptr;

use crate::doom::c_ffi::{line_t, node_t, sector_t, seg_t, side_t, subsector_t, vertex_t};
use crate::doom::d_player::MAXPLAYERS;

use super::structs::{mapthing_t, MAX_DEATHMATCH_STARTS};

/// Total number of vertexes loaded from the current map's VERTEXES lump.
/// C linkage: referenced by `r_bsp.c` and other renderer modules.
#[no_mangle]
pub static mut numvertexes: c_int = 0;
/// Pointer to the runtime vertex array, allocated from zone memory at
/// `PU_LEVEL`. Each entry holds fixed-point (16.16) x/y coordinates.
/// C linkage: referenced by `p_map.c`, `r_bsp.c`, and many others.
#[no_mangle]
pub static mut vertexes: *mut vertex_t = ptr::null_mut();

/// Total number of segs loaded from the current map's SEGS lump.
/// C linkage: referenced by `r_bsp.c`.
#[no_mangle]
pub static mut numsegs: c_int = 0;
/// Pointer to the runtime seg array, allocated from zone memory at `PU_LEVEL`.
/// C linkage: referenced by `r_bsp.c` and `p_sight.c`.
#[no_mangle]
pub static mut segs: *mut seg_t = ptr::null_mut();

/// Total number of sectors loaded from the current map's SECTORS lump.
/// C linkage: referenced throughout the game and renderer.
#[no_mangle]
pub static mut numsectors: c_int = 0;
/// Pointer to the runtime sector array, allocated from zone memory at
/// `PU_LEVEL`. Sectors own the lighting, floor/ceiling heights, and thing
/// lists used by most game logic.
/// C linkage: referenced throughout the game and renderer.
#[no_mangle]
pub static mut sectors: *mut sector_t = ptr::null_mut();

/// Total number of subsectors loaded from the current map's SSECTORS lump.
/// C linkage: referenced by `r_bsp.c` and `p_sight.c`.
#[no_mangle]
pub static mut numsubsectors: c_int = 0;
/// Pointer to the runtime subsector array, allocated from zone memory at
/// `PU_LEVEL`. Each subsector is a convex polygon leaf of the BSP tree.
/// C linkage: referenced by `r_bsp.c`, `p_map.c`, and `p_sight.c`.
#[no_mangle]
pub static mut subsectors: *mut subsector_t = ptr::null_mut();

/// Total number of BSP nodes loaded from the current map's NODES lump.
/// C linkage: referenced by `r_bsp.c` and `p_sight.c`.
#[no_mangle]
pub static mut numnodes: c_int = 0;
/// Pointer to the runtime BSP node array, allocated from zone memory at
/// `PU_LEVEL`. The root node is `nodes[numnodes - 1]`.
/// C linkage: referenced by `r_bsp.c`, `p_map.c`, and `p_sight.c`.
#[no_mangle]
pub static mut nodes: *mut node_t = ptr::null_mut();

/// Total number of linedefs loaded from the current map's LINEDEFS lump.
/// C linkage: referenced by `p_map.c`, `p_maputl.c`, and others.
#[no_mangle]
pub static mut numlines: c_int = 0;
/// Pointer to the runtime linedef array, allocated from zone memory at
/// `PU_LEVEL`. Each linedef connects two vertexes and references up to two
/// sidedefs.
/// C linkage: referenced throughout the game.
#[no_mangle]
pub static mut lines: *mut line_t = ptr::null_mut();

/// Total number of sidedefs loaded from the current map's SIDEDEFS lump.
/// C linkage: referenced by `r_segs.c` and others.
#[no_mangle]
pub static mut numsides: c_int = 0;
/// Pointer to the runtime sidedef array, allocated from zone memory at
/// `PU_LEVEL`. Sidedefs hold texture indices and offsets; each linedef
/// references one or two.
/// C linkage: referenced by `r_segs.c`, `p_spec.c`, and others.
#[no_mangle]
pub static mut sides: *mut side_t = ptr::null_mut();

/// Width of the blockmap grid in 128-unit cells.
/// C linkage: referenced by `p_map.c` and `p_maputl.c` for collision queries.
#[no_mangle]
pub static mut bmapwidth: c_int = 0;
/// Height of the blockmap grid in 128-unit cells.
/// C linkage: referenced by `p_map.c` and `p_maputl.c` for collision queries.
#[no_mangle]
pub static mut bmapheight: c_int = 0;
/// Pointer into `blockmaplump` offset by 4 shorts (past the header).
/// Each entry is an offset from `blockmaplump` to the list of linedefs in that
/// cell, terminated by -1.
/// C linkage: referenced by `p_maputl.c`.
#[no_mangle]
pub static mut blockmap: *mut c_short = ptr::null_mut();
/// Base pointer to the raw blockmap lump data (including the 4-short header).
/// Header layout: `[orgx, orgy, width, height]` in map units (not fixed-point
/// before shifting). Allocated from zone memory at `PU_LEVEL`.
/// C linkage: referenced by `p_maputl.c`.
#[no_mangle]
pub static mut blockmaplump: *mut c_short = ptr::null_mut();
/// X origin of the blockmap in fixed-point (16.16) map coordinates.
/// Subtract from a thing's x to get the blockmap column.
/// C linkage: referenced by `p_maputl.c`.
#[no_mangle]
pub static mut bmaporgx: c_int = 0;
/// Y origin of the blockmap in fixed-point (16.16) map coordinates.
/// Subtract from a thing's y to get the blockmap row.
/// C linkage: referenced by `p_maputl.c`.
#[no_mangle]
pub static mut bmaporgy: c_int = 0;
/// Pointer to the blockmap thing-chain heads, one per cell (`bmapwidth *
/// bmapheight` entries). Each entry is a pointer to the first `mobj_t` in
/// that cell's chain, or null. Zeroed at level load.
/// C linkage: referenced by `p_maputl.c` and `p_map.c`.
#[no_mangle]
pub static mut blocklinks: *mut *mut c_void = ptr::null_mut();

/// Pointer to the precomputed LOS (line-of-sight) reject table.
/// A 2-D bit array indexed by `[sector1 * numsectors + sector2]`; a set bit
/// means the two sectors cannot see each other, so detailed LOS checks are
/// skipped. Allocated from zone memory at `PU_LEVEL`.
/// C linkage: referenced by `p_sight.c`.
#[no_mangle]
pub static mut rejectmatrix: *mut u8 = ptr::null_mut();

/// Array of deathmatch start positions collected from the THINGS lump.
/// Holds up to `MAX_DEATHMATCH_STARTS` entries; the active count is tracked
/// by `deathmatch_p - deathmatchstarts`.
/// C linkage: referenced by `g_game.c`.
#[no_mangle]
pub static mut deathmatchstarts: [mapthing_t; MAX_DEATHMATCH_STARTS] = [mapthing_t {
    x: 0,
    y: 0,
    angle: 0,
    r#type: 0,
    options: 0,
}; MAX_DEATHMATCH_STARTS];

/// Write cursor into `deathmatchstarts`; points to the next free slot.
/// Reset to `&deathmatchstarts[0]` at the start of each `P_SetupLevel`.
/// C linkage: advanced by `P_SpawnMapThing` when spawning deathmatch starts.
#[no_mangle]
pub static mut deathmatch_p: *mut mapthing_t = ptr::null_mut();

/// Per-player single-player start positions, indexed by player number.
/// Holds `MAXPLAYERS` entries; populated by `P_SpawnMapThing` from THINGS
/// with type 1-4 (player starts).
/// C linkage: referenced by `g_game.c` for respawning.
#[no_mangle]
pub static mut playerstarts: [mapthing_t; MAXPLAYERS] = [mapthing_t {
    x: 0,
    y: 0,
    angle: 0,
    r#type: 0,
    options: 0,
}; MAXPLAYERS];

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::p_setup::MAX_DEATHMATCH_STARTS;
    use std::ffi::c_int;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    /// The deathmatch start capacity matches the C `MAX_DEATHMATCH_STARTS`.
    #[test]
    fn max_deathmatch_starts_is_10()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(MAX_DEATHMATCH_STARTS, 10);
    }

    /// Every count/origin static starts zeroed before any level loads.
    #[test]
    fn setup_globals_default_zero()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            assert_eq!(numvertexes, 0);
            assert_eq!(numsegs, 0);
            assert_eq!(numsectors, 0);
            assert_eq!(numsubsectors, 0);
            assert_eq!(numnodes, 0);
            assert_eq!(numlines, 0);
            assert_eq!(numsides, 0);
            assert_eq!(bmapwidth, 0);
            assert_eq!(bmapheight, 0);
            assert_eq!(bmaporgx, 0);
            assert_eq!(bmaporgy, 0);
        }
    }

    /// The count/origin statics keep C `int` width (LP64 c_int guard).
    #[test]
    fn setup_globals_are_c_int_width()
    {
        const _: () = assert!(std::mem::size_of::<c_int>() == 4);
        unsafe
        {
            let _: c_int = numvertexes;
            let _: c_int = numsegs;
            let _: c_int = numsectors;
            let _: c_int = numsubsectors;
            let _: c_int = numnodes;
            let _: c_int = numlines;
            let _: c_int = numsides;
            let _: c_int = bmapwidth;
            let _: c_int = bmapheight;
            let _: c_int = bmaporgx;
            let _: c_int = bmaporgy;
        }
    }
}
