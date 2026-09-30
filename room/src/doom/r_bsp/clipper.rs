//! The solid-column clipper: per-frame sentinel reset, the solid/pass
//! wall-segment clipping ladders (including the C `goto crunch`
//! compaction), `add_line`'s backface cull + frustum clip + projection +
//! solid/pass classification, and `check_bbox`'s `CHECKCOORD` corner
//! selection + angle-span clip + coverage test. All frame-golden surface,
//! moved bit-exact; the diagnostic trace probes carry verbatim.

use std::ffi::c_int;

use crate::doom::c_ffi;
use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::fixed_t;
use crate::doom::r_draw::viewwidth;
use crate::doom::r_interp;
use crate::doom::r_main::{clipangle, viewangle, viewangletox, viewx, viewy, R_PointToAngle};
use crate::doom::r_segs::{rw_angle1, R_StoreWallRange};
use crate::doom::tables::{ANG90, ANGLETOFINESHIFT};

use super::state::{
    backsector, curline, frontsector, newend, solidsegs, CHECKCOORD, PROBE_FRAME, seg_index,
};
use super::types::seg_t;

/// Resets the solid-column occlusion list to the two permanent sentinel
/// entries that cover the off-screen left (`-0x7fffffff..-1`) and right
/// (`viewwidth..0x7fffffff`) regions. Also advances the per-frame diagnostic
/// counter used by trace logging.
/// Called once per frame by `R_RenderPlayerView` before BSP traversal begins.
///
/// # Safety
/// Must be called from the render thread. Mutates `solidsegs`, `newend`,
/// and `PROBE_FRAME`. Reads [`viewwidth`] and [`viewangle`].
#[doc(alias = "R_ClearClipSegs")]
#[export_name = "R_ClearClipSegs"]
pub unsafe extern "C" fn clear_clip_segs() {
    solidsegs[0].first = -0x7fffffff;
    solidsegs[0].last = -1;
    solidsegs[1].first = viewwidth;
    solidsegs[1].last = 0x7fffffff;
    newend = std::ptr::addr_of_mut!(solidsegs[0]).add(2);

    // Diagnostic: frame marker. R_ClearClipSegs is called once per frame by
    // R_RenderPlayerView before descending the BSP.
    PROBE_FRAME = PROBE_FRAME.wrapping_add(1);
    log::trace!(
        "=== FRAME {} === viewx={:#x} viewy={:#x} viewangle={:#x} viewwidth={}",
        { PROBE_FRAME },
        { viewx },
        { viewy },
        { viewangle },
        { viewwidth }
    );
}

/// Clips the screen-column range `[first, last]` against the solid occlusion
/// list and renders every visible sub-range by calling [`R_StoreWallRange`].
/// Inserts a new solid entry for any newly covered columns, merging or
/// compacting adjacent/overlapping ranges in `solidsegs`.
///
/// Used for fully opaque walls (single-sided linedefs and closed doors) that
/// completely block everything behind them.
///
/// Corresponds to `R_ClipSolidWallSegment` in `r_bsp.c`.
///
/// # Safety
/// Caller must ensure `solidsegs` and `newend` have been initialized by
/// [`clear_clip_segs`] before this frame. `first` must be <= `last` for a
/// valid seg (a debug log is emitted if violated, but the function still runs
/// to match C behavior). Mutates the global `solidsegs` array and `newend`.
#[doc(alias = "R_ClipSolidWallSegment")]
#[export_name = "R_ClipSolidWallSegment"]
pub unsafe extern "C" fn clip_solid_wall_segment(first: c_int, last: c_int) {
    if first > last { log::debug!("R_ClipSolidWallSegment INVALID: first={} > last={}", first, last); }
    let solidsegs_base = std::ptr::addr_of_mut!(solidsegs[0]);

    let mut start = solidsegs_base;
    while(*start).last < first - 1 { start = start.add(1); }

    if first < (*start).first {
        if last < (*start).first - 1 {
            R_StoreWallRange(first, last);
            let mut next = newend;
            newend = next.add(1);

            while next != start {
                *next = *next.sub(1);
                next = next.sub(1);
            }
            (*next).first = first;
            (*next).last = last;
            return;
        }

        R_StoreWallRange(first, (*start).first - 1);
        (*start).first = first;
    }

    if last <= (*start).last { return; }

    let mut next = start;
    loop {
        let next_plus_1 = next.add(1);
        if last < (*next_plus_1).first - 1 { break; }

        R_StoreWallRange((*next).last + 1, (*next_plus_1).first - 1);
        next = next.add(1);

        if last <= (*next).last {
            (*start).last = (*next).last;
            // "goto crunch"
            if next == start { return; }
            // C: while (next++ != newend) { *++start = *next; }
            loop {
                let old_next = next;
                next = next.add(1);
                if old_next == newend { break; }
                start = start.add(1);
                *start = *next;
            }
            newend = start.add(1);
            return;
        }
    }

    R_StoreWallRange((*next).last + 1, last);
    (*start).last = last;

    // crunch:
    if next == start { return; }

    // C: while (next++ != newend) { *++start = *next; }
    loop {
        let old_next = next;
        next = next.add(1);
        if old_next == newend { break; }
        start = start.add(1);
        *start = *next;
    }
    newend = start.add(1);
}

/// Clips the screen-column range `[first, last]` against the solid occlusion
/// list and renders every visible sub-range by calling [`R_StoreWallRange`],
/// but does **not** add any new solid entries to the occlusion list.
///
/// Used for transparent or partial walls (two-sided linedefs with height
/// differences) that let the player see through to the sector behind.
///
/// Corresponds to `R_ClipPassWallSegment` in `r_bsp.c`.
///
/// # Safety
/// Caller must ensure `solidsegs` and `newend` have been initialized by
/// [`clear_clip_segs`] before this frame. `first` must be <= `last` for a
/// valid seg (a debug log is emitted if violated, but the function still runs
/// to match C behavior). Reads the global `solidsegs` array without
/// modifying it.
#[doc(alias = "R_ClipPassWallSegment")]
#[export_name = "R_ClipPassWallSegment"]
pub unsafe extern "C" fn clip_pass_wall_segment(first: c_int, last: c_int) {
    if first > last { log::debug!("R_ClipPassWallSegment INVALID: first={} > last={}", first, last); }
    let solidsegs_base = std::ptr::addr_of_mut!(solidsegs[0]);

    let mut start = solidsegs_base;
    while(*start).last < first - 1 { start = start.add(1); }

    if first < (*start).first {
        if last < (*start).first - 1 {
            R_StoreWallRange(first, last);
            return;
        }

        R_StoreWallRange(first, (*start).first - 1);
    }

    if last <= (*start).last { return; }

    loop {
        let start_plus_1 = start.add(1);
        if last < (*start_plus_1).first - 1 { break; }

        R_StoreWallRange((*start).last + 1, (*start_plus_1).first - 1);
        start = start.add(1);

        if last <= (*start).last { return; }
    }

    R_StoreWallRange((*start).last + 1, last);
}

/// Clips and conditionally renders one seg from the current subsector.
///
/// 1. Computes view-relative angles for both seg endpoints.
/// 2. Back-face culls if the angular span is >= 180 degrees (ANG180).
/// 3. Clips the angular range to the view frustum (`±clipangle`).
/// 4. Projects the clipped angles to screen columns via `viewangletox`.
/// 5. Rejects degenerate single-column segs (`x1 == x2`).
/// 6. Classifies the seg as solid (single-sided line or closed door) or
///    pass-through (window, height delta, or mid-texture), and dispatches
///    to [`clip_solid_wall_segment`] or [`clip_pass_wall_segment`]
///    accordingly.
///
/// Sets the global [`curline`] to `line` and [`backsector`] to the back
/// sector pointer before calling the clip functions.
///
/// Corresponds to `R_AddLine` in `r_bsp.c`.
///
/// # Safety
/// `line` must point to a valid, initialized [`seg_t`] whose `v1`, `v2`,
/// `sidedef`, `linedef`, `frontsector`, and (if non-null) `backsector`
/// pointers are all valid. Globals [`viewangle`], [`clipangle`],
/// [`viewangletox`], [`frontsector`], and `rw_angle1` must have been
/// initialized before the current frame. Must be called only during BSP
/// traversal (i.e. within `subsector`).
#[doc(alias = "R_AddLine")]
pub(super) unsafe fn add_line(line: *mut seg_t) {
    curline = line;

    let orig_angle1 = R_PointToAngle((*(*line).v1).x, (*(*line).v1).y);
    let orig_angle2 = R_PointToAngle((*(*line).v2).x, (*(*line).v2).y);

    let span = orig_angle1.wrapping_sub(orig_angle2);

    // Back side?
    if span >= 0x8000_0000 {
        log::trace!(
            "R_AddLine SKIP backface: orig_a1={:#x} orig_a2={:#x} va={:#x}",
            orig_angle1,
            orig_angle2,
            { viewangle }
        );
        return;
    }

    rw_angle1 = orig_angle1;
    let mut angle1 = orig_angle1.wrapping_sub(viewangle);
    let mut angle2 = orig_angle2.wrapping_sub(viewangle);

    let clipangle_d2 = clipangle.wrapping_mul(2);

    let mut tspan = angle1.wrapping_add(clipangle);
    if tspan > clipangle_d2 {
        tspan = tspan.wrapping_sub(clipangle_d2);
        if tspan >= span {
            log::trace!("R_AddLine SKIP off-left: orig_a1={:#x} orig_a2={:#x} va={:#x} a1={:#x} a2={:#x} span={:#x} tspan={:#x}", orig_angle1, orig_angle2, { viewangle }, angle1, angle2, span, tspan);
            return;
        }
        angle1 = clipangle;
    }

    tspan = clipangle.wrapping_sub(angle2);
    if tspan > clipangle_d2 {
        tspan = tspan.wrapping_sub(clipangle_d2);
        if tspan >= span {
            log::trace!("R_AddLine SKIP off-right: orig_a1={:#x} orig_a2={:#x} va={:#x} a1={:#x} a2={:#x} span={:#x} tspan={:#x}", orig_angle1, orig_angle2, { viewangle }, angle1, angle2, span, tspan);
            return;
        }
        angle2 = 0u32.wrapping_sub(clipangle);
    }

    let idx1 = ((angle1.wrapping_add(ANG90)) >> ANGLETOFINESHIFT) as usize;
    let idx2 = ((angle2.wrapping_add(ANG90)) >> ANGLETOFINESHIFT) as usize;
    let x1 = viewangletox[idx1];
    let x2 = viewangletox[idx2];

    // Log walls that land in the right portion of the screen
    if x2 >= 200 || x1 >= 200 {
        log::trace!(
            "R_AddLine wall: orig_a1={:#x} orig_a2={:#x} va={:#x} clip={:#x} a1={:#x} a2={:#x} idx1={} idx2={} x1={} x2={}",
            orig_angle1, orig_angle2, { viewangle }, { clipangle }, angle1, angle2, idx1, idx2, x1, x2
        );
    }
    // Diagnostic: log every surviving wall (full screen) at TRACE.
    log::trace!(
        "R_AddLine projected: frame={} seg={} x1={} x2={} a1={:#x} a2={:#x}",
        { PROBE_FRAME },
        seg_index(line),
        x1,
        x2,
        angle1,
        angle2
    );

    if x1 == x2 {
        log::trace!(
            "R_AddLine SKIP x1==x2: orig_a1={:#x} orig_a2={:#x} x1={} x2={}",
            orig_angle1,
            orig_angle2,
            x1,
            x2
        );
        return;
    }

    backsector = (*line).backsector;

    // Single sided line?
    if backsector.is_null() {
        log::trace!(
            "R_AddLine classify: frame={} seg={} x1={} x2={} decision=SOLID(1-sided) back=null front={:p}",
            { PROBE_FRAME }, seg_index(line), x1, x2, frontsector as *const _
        );
        clip_solid_wall_segment(x1, x2 - 1);
        return;
    }

    // Closed door
    // F1 M1: sector heights read through the interpolation board (sample
    // falls back to the live heights when the board is off/uncovered).
    let ffh = r_interp::sector_floor(frontsector as *mut c_ffi::sector_t);
    let fch = r_interp::sector_ceiling(frontsector as *mut c_ffi::sector_t);
    let bfh = r_interp::sector_floor(backsector as *mut c_ffi::sector_t);
    let bch = r_interp::sector_ceiling(backsector as *mut c_ffi::sector_t);
    if bch <= ffh || bfh >= fch
    {
        log::trace!(
            "R_AddLine classify: frame={} seg={} x1={} x2={} decision=SOLID(closed-door) back={:p} f.ch={} f.fh={} b.ch={} b.fh={}",
            { PROBE_FRAME }, seg_index(line), x1, x2, backsector as *const _,
            fch, ffh, bch, bfh
        );
        clip_solid_wall_segment(x1, x2 - 1);
        return;
    }

    // Window
    if bch != fch || bfh != ffh
    {
        log::trace!(
            "R_AddLine classify: frame={} seg={} x1={} x2={} decision=PASS(window) back={:p} f.ch={} f.fh={} b.ch={} b.fh={}",
            { PROBE_FRAME }, seg_index(line), x1, x2, backsector as *const _,
            fch, ffh, bch, bfh
        );
        clip_pass_wall_segment(x1, x2 - 1);
        return;
    }

    // Reject empty lines
    if (*backsector).ceilingpic == (*frontsector).ceilingpic
        && (*backsector).floorpic == (*frontsector).floorpic
        && (*backsector).lightlevel == (*frontsector).lightlevel
        && (*(*curline).sidedef).midtexture == 0
    {
        log::trace!(
            "R_AddLine classify: frame={} seg={} x1={} x2={} decision=REJECT(empty)",
            { PROBE_FRAME },
            seg_index(line),
            x1,
            x2
        );
        return;
    }

    log::trace!(
        "R_AddLine classify: frame={} seg={} x1={} x2={} decision=PASS(midtex/lighting) midtex={}",
        { PROBE_FRAME },
        seg_index(line),
        x1,
        x2,
        (*(*curline).sidedef).midtexture
    );
    clip_pass_wall_segment(x1, x2 - 1);
}

// NOTE: These indices MUST match the C enum in vendor/doomgeneric/m_bbox.h
// (and the matching Rust constants in m_bbox) which stores bbox fields as
// [TOP, BOTTOM, LEFT, RIGHT]. An earlier version had LEFT/RIGHT/TOP/BOTTOM
// ordering here which made R_CheckBBox compare viewx against y-coordinates
// (and viewy against x-coordinates), producing axis-scrambled visibility
// tests — far BSP subtrees were not pruned and rendering descended into the
// wrong parts of the map, causing walls to "disappear" and a different room
// to show through (classic Doom HOM variant).

/// Tests whether a BSP node bounding box might contain any visible geometry
/// from the player's current viewpoint.
///
/// Returns 1 if any part of the box could be visible, 0 if it is entirely
/// hidden behind already-drawn solid walls.
///
/// The test works in three stages:
/// 1. Classify the viewer's position relative to the box (left/inside/right
///    on each axis) and look up the two "most extreme" diagonal corners in
///    `CHECKCOORD`.
/// 2. Compute view-relative BAM angles to those corners and clip against
///    the horizontal frustum (`±clipangle`).
/// 3. Project to screen columns and check whether the column range is fully
///    covered by a single entry in `solidsegs`.
///
/// Corresponds to `R_CheckBBox` in `r_bsp.c`.
///
/// # Safety
/// `bspcoord` must point to a valid 4-element `fixed_t` array laid out as
/// `[TOP, BOTTOM, LEFT, RIGHT]` (matching `BBox::TOP` etc. from `m_bbox`).
/// Globals [`viewx`], [`viewy`], [`viewangle`], [`clipangle`],
/// [`viewangletox`], and `solidsegs` must have been initialized for the
/// current frame.
#[doc(alias = "R_CheckBBox")]
#[export_name = "R_CheckBBox"]
pub unsafe extern "C" fn check_bbox(bspcoord: *mut fixed_t) -> c_int {
    let boxx = if viewx <= *bspcoord.add(BBox::LEFT) { 0 }
    else if viewx < *bspcoord.add(BBox::RIGHT) { 1 }
    else { 2 };

    let boxy = if viewy >= *bspcoord.add(BBox::TOP) { 0 }
    else if viewy > *bspcoord.add(BBox::BOTTOM) { 1 }
    else { 2 };

    let boxpos = (boxy << 2) + boxx;
    if boxpos == 5 { return 1; }

    let cc = CHECKCOORD[boxpos as usize];
    let x1 = *bspcoord.add(cc[0] as usize);
    let y1 = *bspcoord.add(cc[1] as usize);
    let x2 = *bspcoord.add(cc[2] as usize);
    let y2 = *bspcoord.add(cc[3] as usize);

    let mut angle1 = R_PointToAngle(x1, y1).wrapping_sub(viewangle);
    let mut angle2 = R_PointToAngle(x2, y2).wrapping_sub(viewangle);

    let span = angle1.wrapping_sub(angle2);

    if span >= 0x8000_0000 { return 1; }

    let clipangle_d2 = clipangle.wrapping_mul(2);

    let mut tspan = angle1.wrapping_add(clipangle);
    if tspan > clipangle_d2 {
        tspan = tspan.wrapping_sub(clipangle_d2);
        if tspan >= span { return 0; }
        angle1 = clipangle;
    }

    tspan = clipangle.wrapping_sub(angle2);
    if tspan > clipangle_d2 {
        tspan = tspan.wrapping_sub(clipangle_d2);
        if tspan >= span { return 0; }
        angle2 = 0u32.wrapping_sub(clipangle);
    }

    let sx1 = viewangletox[((angle1.wrapping_add(ANG90)) >> ANGLETOFINESHIFT) as usize];
    let sx2 = viewangletox[((angle2.wrapping_add(ANG90)) >> ANGLETOFINESHIFT) as usize];

    if sx1 == sx2 { return 0; }
    let sx2 = sx2 - 1;

    let solidsegs_base = std::ptr::addr_of!(solidsegs[0]);
    let mut start = solidsegs_base;
    while(*start).last < sx2 { start = start.add(1); }

    if sx1 >= (*start).first && sx2 <= (*start).last { return 0; }

    1
}
