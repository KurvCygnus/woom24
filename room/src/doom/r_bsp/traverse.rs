//! The BSP walk: the per-frame draw-seg rewind, the subsector leaf
//! renderer (visplane registration through the `r_interp` sampled
//! heights, sprite seeding, seg dispatch), and the recursive
//! front-to-back node walk whose `NF_SUBSECTOR` flag arithmetic must stay
//! wrapping-safe.

use std::ffi::c_int;
use std::ptr;

use crate::doom::c_ffi;
use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::fixed_t;
use crate::doom::p_setup::{nodes, segs, subsectors};
use crate::doom::r_interp;
use crate::doom::r_main::{sscount, viewx, viewy, viewz, R_PointOnSide};
use crate::doom::r_plane::{ceilingplane, floorplane, R_FindPlane};
use crate::doom::r_sky::skyflatnum;
use crate::doom::r_things::R_AddSprites;

use super::clipper::{add_line, check_bbox};
use super::state::{drawsegs, ds_p, frontsector, PROBE_FRAME, NF_SUBSECTOR};
use super::types::{node_t, seg_t, subsector_t};

/// Resets the draw-seg write pointer to the beginning of [`drawsegs`],
/// discarding all draw-segs accumulated during the previous frame.
/// Called once per frame by `R_RenderPlayerView` before BSP traversal begins.
///
/// # Safety
/// Must be called from the render thread. Mutates the global [`ds_p`].
#[doc(alias = "R_ClearDrawSegs")]
#[export_name = "R_ClearDrawSegs"]
pub unsafe extern "C" fn clear_draw_segs() {
    ds_p = std::ptr::addr_of_mut!(drawsegs[0]);
}

/// Renders one BSP leaf subsector.
///
/// 1. Increments the subsector counter `sscount`.
/// 2. Sets [`frontsector`] from the subsector's sector pointer.
/// 3. Registers floor and ceiling visplanes with `find_plane` if the
///    viewer can see them (floor below eye, ceiling above eye or sky flat).
/// 4. Adds sprites for all things in the sector via [`R_AddSprites`].
/// 5. Iterates over all segs in the subsector and calls `add_line` for
///    each, which performs frustum clipping and dispatches wall rendering.
///
/// Corresponds to `R_Subsector` in `r_bsp.c`.
///
/// # Safety
/// `num` must be a valid index into the global `subsectors` array (bounds are
/// not checked at runtime to match C behavior). The subsector's `sector`
/// pointer and all `segs` it references must be valid. Globals [`viewz`],
/// `skyflatnum`, [`floorplane`], and [`ceilingplane`] must be accessible.
#[doc(alias = "R_Subsector")]
#[export_name = "R_Subsector"]
pub unsafe extern "C" fn subsector(num: c_int) {
    let num_usize = num as usize;

    sscount += 1;

    let sub = &*(subsectors.add(num_usize) as *mut subsector_t);
    frontsector = sub.sector;
    let mut count = sub.numlines as c_int;
    let mut line = segs.add(sub.firstline as usize) as *mut seg_t;

    // F1 M1: floor/ceiling visplanes register with the interpolation board's
    // sampled heights (live heights when the board is off/uncovered), so
    // flats, wall spans and sprite clipping all agree per frame.
    let interp_floor = r_interp::sector_floor(frontsector as *mut c_ffi::sector_t);
    let interp_ceiling = r_interp::sector_ceiling(frontsector as *mut c_ffi::sector_t);

    if interp_floor < viewz {
        floorplane = R_FindPlane(
            interp_floor,
            (*frontsector).floorpic as c_int,
            (*frontsector).lightlevel as c_int,
        );
    } else {
        floorplane = ptr::null_mut();
    }

    if interp_ceiling > viewz || (*frontsector).ceilingpic as c_int == skyflatnum {
        ceilingplane = R_FindPlane(
            interp_ceiling,
            (*frontsector).ceilingpic as c_int,
            (*frontsector).lightlevel as c_int,
        );
    } else {
        ceilingplane = ptr::null_mut();
    }

    R_AddSprites(frontsector);

    while count > 0 {
        add_line(line);
        line = line.add(1);
        count -= 1;
    }
}

/// Recursively traverses the BSP tree and renders all visible subsectors.
///
/// If `bspnum` has the `NF_SUBSECTOR` flag set it is a leaf: calls
/// [`subsector`] with the subsector index (treating -1 as subsector 0).
///
/// Otherwise loads the [`node_t`] at index `bspnum`, determines which side
/// the viewpoint is on via [`R_PointOnSide`], recurses into the near (front)
/// child first, then checks the far (back) child's bounding box with
/// [`check_bbox`] and recurses into it only if it might be visible.
///
/// This front-to-back ordering ensures that solid walls encountered first
/// (nearer to the player) fill the `solidsegs` occlusion list, pruning
/// distant subtrees early.
///
/// Corresponds to `R_RenderBSPNode` in `r_bsp.c`.
///
/// # Safety
/// `bspnum` must be either a valid node index into the global `nodes` array
/// or a value with the `NF_SUBSECTOR` flag set whose lower bits are a valid
/// subsector index. All node and subsector data must have been loaded by
/// `P_SetupLevel`. Globals [`viewx`], [`viewy`], and the occlusion list must
/// be initialized for the current frame.
#[doc(alias = "R_RenderBSPNode")]
#[export_name = "R_RenderBSPNode"]
pub unsafe extern "C" fn render_bsp_node(bspnum: c_int) {
    // Found a subsector?
    if (bspnum as u32) & NF_SUBSECTOR != 0 {
        let sub_num = if bspnum == -1 {
            0
        } else {
            (bspnum as u32 & !NF_SUBSECTOR) as c_int
        };
        log::trace!(
            "R_RenderBSPNode: frame={} leaf subsector={} (bspnum={:#x})",
            { PROBE_FRAME },
            sub_num,
            bspnum as u32
        );
        subsector(sub_num);
        return;
    }

    let bsp = &*(nodes.add(bspnum as usize) as *const node_t);

    let side = R_PointOnSide(viewx, viewy, bsp);

    log::trace!(
        "R_RenderBSPNode: frame={} node={} side={} front_child={:#x} back_child={:#x}",
        { PROBE_FRAME },
        bspnum,
        side,
        bsp.children[side as usize] as u32,
        bsp.children[(side ^ 1) as usize] as u32,
    );

    render_bsp_node(bsp.children[side as usize] as c_int);

    let back_visible = check_bbox(bsp.bbox[(side ^ 1) as usize].as_ptr() as *mut fixed_t) != 0;
    let back_box = bsp.bbox[(side ^ 1) as usize];
    log::trace!(
        "R_RenderBSPNode: frame={} node={} back_visible={} back_bbox=[L={} R={} T={} B={}]",
        { PROBE_FRAME },
        bspnum,
        back_visible,
        back_box[BBox::LEFT],
        back_box[BBox::RIGHT],
        back_box[BBox::TOP],
        back_box[BBox::BOTTOM],
    );
    if back_visible {
        render_bsp_node(bsp.children[(side ^ 1) as usize] as c_int);
    }
}
