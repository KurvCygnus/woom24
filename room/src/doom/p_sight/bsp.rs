//! BSP and subsector traversal for line-of-sight: the per-segment
//! portal walk (`P_CrossSubsector`), the recursive BSP walk
//! (`P_CrossBSPNode`), and the `node_as_divline` cast spelling --
//! bit-exact with the traversal half of `vendor/doomgeneric/p_sight.c`.
//! The catalog entry-5 consumption sites live here (see
//! `docs/vanilla-workarounds.md`).

#![allow(non_snake_case)]

use std::os::raw::c_int;

use crate::doom::c_ffi::LinedefFlag;
use crate::doom::m_fixed::FixedDiv;
use crate::doom::p_setup::{
    nodes as p_setup_nodes, numsubsectors, segs as p_setup_segs, subsectors as p_setup_subsectors,
};
use crate::doom::r_main::validcount;
use crate::i_error;

use super::dtmc::{divline_side, intercept_vector2};
use super::state::{bottomslope, sightzstart, strace, t2x, t2y, topslope};
use super::types::{divline_t, node_t, seg_t, subsector_t};

/// BSP node flag: child index has this bit set when the child is a subsector
/// (leaf) rather than an internal node. Matches `NF_SUBSECTOR` in `p_local.h`.
const NF_SUBSECTOR: u32 = 0x8000;

/// Test whether the global sight ray (`strace`) passes through subsector `num`
/// without being blocked.
///
/// Iterates every seg in the subsector. For each linedef crossed by the sight
/// ray, checks whether the portal is tall enough to let the ray through given
/// the current `topslope` / `bottomslope` bounds. Narrows those bounds when a
/// two-sided line is partially occluding.
///
/// Returns `true` if the ray is unobstructed through this subsector.
///
/// Matches `P_CrossSubsector` in `p_sight.c`.
pub(super) fn P_CrossSubsector(num: c_int) -> bool {
    unsafe {
        let strace_copy = strace;
        let numsubsectors_val = *std::ptr::addr_of!(numsubsectors);
        if num >= numsubsectors_val {
            i_error!(
                "P_CrossSubsector: ss {} with numss = {}",
                num,
                numsubsectors_val
            );
        }

        let sub = &*(p_setup_subsectors as *mut subsector_t).add(num as usize);

        let mut count = sub.numlines as c_int;
        let mut seg_ptr = (p_setup_segs as *mut seg_t).add(sub.firstline as usize);

        while count > 0 {
            let seg = &*seg_ptr;
            seg_ptr = seg_ptr.add(1);
            count -= 1;

            let line = &mut *seg.linedef;

            // Already checked other side?
            if line.validcount == validcount {
                continue;
            }

            line.validcount = validcount;

            let v1 = &*line.v1;
            let v2 = &*line.v2;

            let s1 = divline_side(v1.x, v1.y, &strace_copy);
            let s2 = divline_side(v2.x, v2.y, &strace_copy);

            // Line isn't crossed?
            if s1 == s2 {
                continue;
            }

            let divl = divline_t {
                x: v1.x,
                y: v1.y,
                dx: v2.x - v1.x,
                dy: v2.y - v1.y,
            };

            let s1 = divline_side(strace_copy.x, strace_copy.y, &divl);
            let s2 = divline_side(t2x, t2y, &divl);

            // Line isn't crossed?
            if s1 == s2 {
                continue;
            }

            // Backsector may be NULL if this is an "impassible glass" hack line.
            // IMPORTANT: C uses line->backsector (the original linedef side), NOT
            // seg->backsector (which is the BSP-split sub-side and may differ).
            if line.backsector.is_null() {
                return false;
            }

            // Stop because it is not two sided anyway.
            // Also must use line->flags, not a re-read through seg->linedef.
            if line.flags & LinedefFlag::TWOSIDED as i16 == 0 {
                return false;
            }

            let front = seg.frontsector;
            let back = seg.backsector;

            let front_floor = (*front).floorheight;
            let front_ceil = (*front).ceilingheight;
            let back_floor = (*back).floorheight;
            let back_ceil = (*back).ceilingheight;

            // No wall to block sight with?
            if front_floor == back_floor && front_ceil == back_ceil {
                continue;
            }

            // Possible occluder.
            let opentop = if front_ceil < back_ceil {
                front_ceil
            } else {
                back_ceil
            };

            let openbottom = if front_floor > back_floor {
                front_floor
            } else {
                back_floor
            };

            // Quick test for totally closed doors.
            if openbottom >= opentop {
                return false;
            }

            let frac = intercept_vector2(&strace_copy, &divl);

            if front_floor != back_floor {
                let slope = FixedDiv(openbottom - sightzstart, frac);
                if slope > bottomslope {
                    bottomslope = slope;
                }
            }

            if front_ceil != back_ceil {
                let slope = FixedDiv(opentop - sightzstart, frac);
                if slope < topslope {
                    topslope = slope;
                }
            }

            if topslope <= bottomslope {
                return false;
            }
        }

        true
    }
}

/// Extract the first 4 fields of a node_t as a divline_t.
/// The C code casts `(divline_t*)node` for P_DivlineSide calls;
/// since both structs share the same header layout (x, y, dx, dy),
/// this produces identical results.
#[inline]
pub(super) fn node_as_divline(node: &node_t) -> divline_t {
    divline_t {
        x: node.x,
        y: node.y,
        dx: node.dx,
        dy: node.dy,
    }
}

/// Recursively cross a BSP node (or leaf subsector) to test sight.
///
/// Determines which side of the partition the trace origin lies on, crosses
/// that subtree first, then crosses the other side only if the trace endpoint
/// is on a different side of the partition.
///
/// Returns `true` if the sight ray is unobstructed through `bspnum`.
///
/// Matches `P_CrossBSPNode` in `p_sight.c`.
pub(super) fn P_CrossBSPNode(bspnum: c_int) -> bool {
    unsafe {
        let strace_copy = strace;
        if bspnum & NF_SUBSECTOR as c_int != 0 {
            if bspnum == -1 {
                return P_CrossSubsector(0);
            } else {
                return P_CrossSubsector(bspnum & !(NF_SUBSECTOR as c_int));
            }
        }

        let bsp = &*(p_setup_nodes as *mut node_t).add(bspnum as usize);
        let bsp_div = node_as_divline(bsp);

        let side = divline_side(strace_copy.x, strace_copy.y, &bsp_div);
        let side = if side == 2 { 0 } else { side };

        // Cross the starting side.
        if !P_CrossBSPNode(bsp.children[side as usize] as c_int) {
            return false;
        }

        // The partition plane is crossed here.
        let bsp_div2 = node_as_divline(bsp);
        let t2_side = divline_side(t2x, t2y, &bsp_div2);
        if side == t2_side {
            return true;
        }

        // Cross the ending side.
        P_CrossBSPNode(bsp.children[(side ^ 1) as usize] as c_int)
    }
}
