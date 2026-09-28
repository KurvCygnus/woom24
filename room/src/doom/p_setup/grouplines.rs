//! The per-sector line tables: `P_GroupLines` resolves subsector sectors,
//! counts and assigns each sector's line list out of one shared flat
//! buffer, and derives the sector bounding boxes with their blockmap
//! clamps (the clamps live in `dtmc`). Also the single-writer home of
//! `totallines`, the count `reject` reads to model the vanilla zone
//! header (workaround row 4).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::{line_t, sector_t};
use crate::doom::m_bbox::{BBox, M_AddToBox, M_ClearBox};
use crate::doom::z_zone::{PU_LEVEL, Z_Malloc};

use super::dtmc::{clamp_block, clamp_block_low};
use super::globals::{
    bmapheight, bmaporgx, bmaporgy, bmapwidth, lines, numlines, numsectors, numsubsectors, sectors,
    segs, subsectors,
};
use super::structs::MAXRADIUS;

/// Running total of front+back linedef-sector references, used to size the
/// per-sector line-pointer buffer in `P_GroupLines`. Module-private; not
/// exported to C. `grouplines` writes it (single writer); `reject` reads it
/// through this path.
pub(super) static mut totallines: c_int = 0;

/// Build per-sector line lists, resolve subsector sectors, and compute sector
/// bounding boxes.
///
/// This function performs three passes over the map geometry after all lumps
/// have been loaded:
///
/// 1. **Subsector sectors**: for each subsector, resolves its sector pointer
///    by following `segs[ss.firstline].sidedef.sector`. This uses the seg's
///    already-resolved sidedef (set during `P_LoadSegs`) rather than the raw
///    `sidenum[0]`, correctly handling back-side segs.
///
/// 2. **Line count and buffer allocation**: counts how many line references
///    each sector accumulates (`totallines` tracks the total for the shared
///    buffer). Two-sided lines contribute to both front and back sectors, but
///    a line shared between the same front and back sector is counted only once
///    per side.
///
/// 3. **Line table assignment**: allocates a single flat buffer of
///    `totallines` line pointers at `PU_LEVEL`, then assigns slices to each
///    sector using a cumulative cursor (not per-sector independent offsets).
///    A second pass over lines fills in the pointers.
///
/// 4. **Bounding boxes and sound origins**: for each sector, computes an
///    axis-aligned bounding box from its lines' vertices, sets the sound
///    origin (`soundorg`) to the bounding-box center, and records the
///    clamped blockmap cell extents in `sector.blockbox`.
///
/// Preconditions: all lump-load functions must have been called first.
/// C callers: `P_SetupLevel` (this file).
#[doc(alias = "P_GroupLines")]
#[export_name = "P_GroupLines"]
pub extern "C" fn group_lines()
{
    unsafe
    {
        // Look up sector number for each subsector.
        let mut ss = subsectors;
        for _ in 0..numsubsectors
        {
            let seg = segs.offset((*ss).firstline as isize);
            // Must use seg->sidedef->sector, not sidenum[0], because the seg
            // may be on side 1 (back side) of the linedef, in which case
            // sidenum[0] points to the wrong side's sector.
            (*ss).sector = (*(*seg).sidedef).sector as *mut c_void;
            ss = ss.add(1);
        }

        // Count number of lines in each sector.
        let mut li = lines;
        totallines = 0;
        for _ in 0..numlines
        {
            totallines += 1;
            let frontsec = (*li).frontsector as *mut sector_t;
            if !frontsec.is_null() { (*frontsec).linecount += 1; }
            let backsec = (*li).backsector as *mut sector_t;
            if !backsec.is_null() && backsec != frontsec
            {
                (*backsec).linecount += 1;
                totallines += 1;
            }
            li = li.add(1);
        }

        // Build line tables for each sector.
        let linebuffer = Z_Malloc(
            totallines * std::mem::size_of::<*mut line_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut *mut line_t;

        let mut current = linebuffer;
        for i in 0..numsectors as usize
        {
            let sec = sectors.add(i);
            (*sec).lines = current as *mut *mut c_void;
            current = current.offset((*sec).linecount as isize);
            (*sec).linecount = 0;
        }

        // Assign lines to sectors.
        for i in 0..numlines as usize
        {
            li = lines.add(i);
            if !(*li).frontsector.is_null()
            {
                let sector = (*li).frontsector as *mut sector_t;
                (*sector)
                    .lines
                    .offset((*sector).linecount as isize)
                    .write(li as *mut c_void);
                (*sector).linecount += 1;
            }
            if !(*li).backsector.is_null() && (*li).frontsector != (*li).backsector
            {
                let sector = (*li).backsector as *mut sector_t;
                (*sector)
                    .lines
                    .offset((*sector).linecount as isize)
                    .write(li as *mut c_void);
                (*sector).linecount += 1;
            }
        }

        // Generate bounding boxes for sectors.
        let mut sector = sectors;
        for _ in 0..numsectors
        {
            let mut bbox: [c_int; 4] = [0; 4];
            M_ClearBox(bbox.as_mut_ptr());

            for j in 0..(*sector).linecount
            {
                let line = *(*sector).lines.offset(j as isize) as *mut line_t;
                M_AddToBox(bbox.as_mut_ptr(), (*(*line).v1).x, (*(*line).v1).y);
                M_AddToBox(bbox.as_mut_ptr(), (*(*line).v2).x, (*(*line).v2).y);
            }

            // Set the degenmobj_t to the middle of the bounding box.
            let soundorg_x = (bbox[BBox::RIGHT] + bbox[BBox::LEFT]) / 2;
            let soundorg_y = (bbox[BBox::TOP] + bbox[BBox::BOTTOM]) / 2;
            // sector->soundorg is a 40-byte degenmobj_t; first two fields are x,y.
            let soundorg_ptr = (*sector).soundorg.as_mut_ptr() as *mut c_int;
            *soundorg_ptr = soundorg_x;
            *soundorg_ptr.add(1) = soundorg_y;

            // Adjust bounding box to map blocks; the clamp ladders are the
            // extracted `dtmc::clamp_block` (TOP/RIGHT) and
            // `dtmc::clamp_block_low` (BOTTOM/LEFT), with the +/- MAXRADIUS
            // margin folded into the origin argument.
            (*sector).blockbox[BBox::TOP] =
                clamp_block(bbox[BBox::TOP], bmaporgy - MAXRADIUS, bmapheight);
            (*sector).blockbox[BBox::BOTTOM] =
                clamp_block_low(bbox[BBox::BOTTOM], bmaporgy + MAXRADIUS);
            (*sector).blockbox[BBox::RIGHT] =
                clamp_block(bbox[BBox::RIGHT], bmaporgx - MAXRADIUS, bmapwidth);
            (*sector).blockbox[BBox::LEFT] =
                clamp_block_low(bbox[BBox::LEFT], bmaporgx + MAXRADIUS);

            sector = sector.add(1);
        }
    }
}

#[cfg(test)]
mod tests
{
    // -----------------------------------------------------------------------
    // Regression: P_GroupLines sector line table cumulative offset
    // -----------------------------------------------------------------------

    /// Simulates the sector line-table pointer assignment from `group_lines`.
    /// Returns the computed start-offset for each sector within the shared
    /// line-buffer.
    ///
    /// The C original advances a single cursor cumulatively:
    ///   sectors[i].lines = linebuffer;
    ///   linebuffer += sectors[i].linecount;
    ///
    /// The buggy Rust port computed each sector's offset independently from
    /// the buffer base (`base + linecount`), causing every sector's line
    /// table to point to the wrong memory.
    fn compute_sector_line_offsets(linecounts: &[usize]) -> Vec<usize>
    {
        let mut offsets = Vec::with_capacity(linecounts.len());
        let mut current: usize = 0;
        for &lc in linecounts
        {
            offsets.push(current);
            current += lc;
        }
        offsets
    }

    /// Cumulative offsets must equal the sum of all preceding linecounts.
    #[test]
    fn p_grouplines_sector_offsets_are_cumulative()
    {
        // Sector linecounts: [10, 5, 3]
        // Expected offsets:  [0, 10, 15]
        let linecounts = [10usize, 5, 3];
        let offsets = compute_sector_line_offsets(&linecounts);
        assert_eq!(offsets, vec![0, 10, 15]);
    }

    /// Single sector → offset 0.
    #[test]
    fn p_grouplines_single_sector_offset_zero()
    {
        let linecounts = [7usize];
        assert_eq!(compute_sector_line_offsets(&linecounts), vec![0]);
    }

    /// Sector with zero lines must still advance correctly for next sector.
    #[test]
    fn p_grouplines_zero_line_sector_preserves_cumulative()
    {
        let linecounts = [5usize, 0, 3];
        // sector 0 → offset 0, sector 1 → offset 5, sector 2 → offset 5
        assert_eq!(compute_sector_line_offsets(&linecounts), vec![0, 5, 5]);
    }

    /// The buggy version (`base + linecount`) would produce wrong results.
    /// This test documents the exact wrong values that the bug produced.
    #[test]
    fn p_grouplines_buggy_offset_produces_wrong_values()
    {
        let linecounts = [10usize, 5, 3];

        // Correct (cumulative):
        let correct = compute_sector_line_offsets(&linecounts);
        assert_eq!(correct, vec![0, 10, 15]);

        // Buggy (independent from base):
        let buggy: Vec<usize> = linecounts.iter().copied().collect();
        assert_eq!(buggy, vec![10, 5, 3]);

        // They must NOT be equal.
        assert_ne!(correct, buggy);
    }

    // -----------------------------------------------------------------------
    // Regression: P_GroupLines subsector sector via seg->sidedef->sector
    // -----------------------------------------------------------------------

    /// Simulates the subsector→sector resolution from `group_lines`.
    ///
    /// The C original uses `seg->sidedef->sector`, which respects the seg's
    /// side (0 = front, 1 = back).  The buggy Rust port hardcoded
    /// `sidenum[0]`, always resolving to the front side's sector regardless
    /// of which side the seg is on.
    ///
    /// Parameters:
    /// - `seg_side`: 0 (front) or 1 (back) – which side of the linedef the seg is on
    /// - `sidenum`: [front_sidenum, back_sidenum]
    /// - `sector_of_sidenum`: maps each sidenum to its sector index
    ///
    /// Returns the resolved sector index.
    fn resolve_subsector_sector_via_sidedef(
        seg_side: usize,
        sidenum: [i16; 2],
        sector_of_sidenum: &[usize],
    ) -> usize
    {
        // Correct: use the seg's already-resolved sidedef, which was set
        // during load_segs to `sides[ldef->sidenum[seg_side]]`.
        // So sector = sector_of_sidenum[sidenum[seg_side]]
        let sidenum_idx = sidenum[seg_side] as usize;
        sector_of_sidenum[sidenum_idx]
    }

    /// The buggy version always used sidenum[0] regardless of seg_side.
    fn resolve_subsector_sector_buggy(
        seg_side: usize,
        sidenum: [i16; 2],
        sector_of_sidenum: &[usize],
    ) -> usize
    {
        let _ = seg_side; // unused – the bug!
        let sidenum_idx = sidenum[0] as usize;
        sector_of_sidenum[sidenum_idx]
    }

    /// Front-side seg (side=0): both methods agree.
    #[test]
    fn p_grouplines_subsector_front_side_agrees()
    {
        // linedef has sidenum[0]=2, sidenum[1]=5
        // sector_of_sidenum[2]=10, sector_of_sidenum[5]=20
        let sidenum = [2i16, 5];
        let sector_map = [0, 0, 10, 0, 0, 20];

        assert_eq!(
            resolve_subsector_sector_via_sidedef(0, sidenum, &sector_map),
            10
        );
        assert_eq!(resolve_subsector_sector_buggy(0, sidenum, &sector_map), 10);
    }

    /// Back-side seg (side=1): correct method uses sidenum[1],
    /// buggy method uses sidenum[0] → wrong sector.
    #[test]
    fn p_grouplines_subsector_back_side_differs()
    {
        // linedef has sidenum[0]=2 (sector 10), sidenum[1]=5 (sector 20)
        let sidenum = [2i16, 5];
        let sector_map = [0, 0, 10, 0, 0, 20];

        // Correct: seg on side 1 → uses sidenum[1]=5 → sector 20
        assert_eq!(
            resolve_subsector_sector_via_sidedef(1, sidenum, &sector_map),
            20
        );

        // Buggy: always uses sidenum[0]=2 → sector 10 (WRONG)
        assert_eq!(resolve_subsector_sector_buggy(1, sidenum, &sector_map), 10);
    }

    /// Ensures the correct and buggy implementations produce different
    /// results for back-side segs, proving the regression test is meaningful.
    #[test]
    fn p_grouplines_subsector_back_side_correct_vs_buggy_differ()
    {
        let sidenum = [3i16, 7];
        let sector_map = [0, 0, 0, 11, 0, 0, 0, 22];

        let correct = resolve_subsector_sector_via_sidedef(1, sidenum, &sector_map);
        let buggy = resolve_subsector_sector_buggy(1, sidenum, &sector_map);

        assert_ne!(
            correct, buggy,
            "back-side seg must resolve to different sectors"
        );
        assert_eq!(correct, 22);
        assert_eq!(buggy, 11);
    }
}
