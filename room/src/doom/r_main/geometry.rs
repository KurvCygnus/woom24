//! BSP/geometry/trigonometry utilities: point-vs-line side tests, the
//! angle wrappers over the extracted octant classifier, the distance
//! calculator, the no-op ABI init stubs, the wall-scale computation, and
//! the BSP point lookup.

use std::ffi::c_int;

use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::{angle_t, fixed_t, FixedDiv, FixedMul};
use crate::doom::m_fixed::{FRACBITS, FRACUNIT};
use crate::doom::p_setup::{nodes, numnodes, subsectors};
use crate::doom::r_bsp::{node_t, seg_t, subsector_t};
use crate::doom::r_segs::{rw_distance, rw_normalangle};
use crate::doom::tables::{self, ANG90, ANGLETOFINESHIFT};

use super::dtmc::{angle_from_delta, DBITS};
use super::state::{detailshift, projection, viewangle, viewx, viewy};

/// Determine which side of a BSP partition plane a map point lies on.
///
/// Returns `0` for the front (right) side and `1` for the back (left) side.
/// Uses fast sign-bit shortcuts for axis-aligned partitions before falling
/// back to a full cross-product test.
///
/// Equivalent to `R_PointOnSide` in `r_main.c`.
///
/// # Safety
/// `node` must be a valid, non-null pointer to a [`node_t`].
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_bsp`, `point_in_subsector`'s SIM walk, and the wasm export surface
/// reach the upstream name through the root shim / pin.
#[doc(alias = "R_PointOnSide")]
#[export_name = "R_PointOnSide"]
pub unsafe extern "C" fn point_on_side(x: fixed_t, y: fixed_t, node: *const node_t) -> c_int {
    if (*node).dx == 0 {
        if x <= (*node).x {
            return ((*node).dy > 0) as c_int;
        }
        return ((*node).dy < 0) as c_int;
    }
    if (*node).dy == 0 {
        if y <= (*node).y {
            return ((*node).dx < 0) as c_int;
        }
        return ((*node).dx > 0) as c_int;
    }

    let dx = x - (*node).x;
    let dy = y - (*node).y;

    // Try to quickly decide by looking at sign bits.
    if (((*node).dy ^ (*node).dx ^ dx ^ dy) as u32) & 0x80000000 != 0 {
        if (((*node).dy ^ dx) as u32) & 0x80000000 != 0 {
            return 1;
        }
        return 0;
    }

    let left = FixedMul((*node).dy >> FRACBITS, dx);
    let right = FixedMul(dy, (*node).dx >> FRACBITS);

    if right < left {
        0
    } else {
        1
    }
}

/// Determine which side of a seg (map line segment) a point lies on.
///
/// Returns `0` for the front side and `1` for the back side, using the same
/// sign-bit shortcut as [`point_on_side`].
///
/// Equivalent to `R_PointOnSegSide` in `r_main.c`.
///
/// # Safety
/// `line` must be a valid, non-null pointer to a [`seg_t`] whose `v1` and
/// `v2` vertex pointers are also valid.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_things` reaches the upstream name through the root shim.
#[doc(alias = "R_PointOnSegSide")]
#[export_name = "R_PointOnSegSide"]
pub unsafe extern "C" fn point_on_seg_side(x: fixed_t, y: fixed_t, line: *const seg_t) -> c_int {
    let lx = (*(*line).v1).x;
    let ly = (*(*line).v1).y;

    let ldx = (*(*line).v2).x - lx;
    let ldy = (*(*line).v2).y - ly;

    if ldx == 0 {
        if x <= lx {
            return (ldy > 0) as c_int;
        }
        return (ldy < 0) as c_int;
    }
    if ldy == 0 {
        if y <= ly {
            return (ldx < 0) as c_int;
        }
        return (ldx > 0) as c_int;
    }

    let dx = x - lx;
    let dy = y - ly;

    // Try to quickly decide by looking at sign bits.
    if ((ldy ^ ldx ^ dx ^ dy) as u32) & 0x80000000 != 0 {
        if ((ldy ^ dx) as u32) & 0x80000000 != 0 {
            return 1;
        }
        return 0;
    }

    let left = FixedMul(ldy >> FRACBITS, dx);
    let right = FixedMul(dy, ldx >> FRACBITS);

    if right < left {
        0
    } else {
        1
    }
}

/// Convert an absolute map coordinate to a view angle (BAM `u32`).
///
/// Subtracts the current [`viewx`]/[`viewy`] to get a relative vector, then
/// classifies the vector into one of eight octants and looks up the angle
/// using the `tantoangle` table. Returns 0 for the view position itself.
///
/// Equivalent to `R_PointToAngle` in `r_main.c`.
///
/// # Safety
/// Reads the global [`viewx`] and [`viewy`]; these must have been
/// initialised by [`crate::doom::r_main::frame::setup_frame`] before this
/// function is called.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the SIM consumers (`p_enemy`, `p_pspr`, `p_mobj`, `p_map`, `p_inter`,
/// `p_user`, `s_sound`, `st_stuff`) and the renderer reach the upstream
/// name through the root shim.
#[doc(alias = "R_PointToAngle")]
#[export_name = "R_PointToAngle"]
pub unsafe extern "C" fn point_to_angle(x: fixed_t, y: fixed_t) -> angle_t {
    angle_from_delta(x - viewx, y - viewy)
}

/// Compute the BAM angle from map point `(x1, y1)` to map point `(x2, y2)`.
///
/// Computes the delta `(x2 - x1, y2 - y1)` and classifies it directly via the
/// shared octant lookup. Unlike the C original (and earlier Rust port), this
/// implementation does not touch the [`viewx`]/[`viewy`] globals, so it is
/// reentrant and safe to call between a read and a use of the view position.
///
/// Equivalent to `R_PointToAngle2` in `r_main.c`, with the global-aliasing
/// trick removed.
///
/// # Safety
/// Pure computation; takes no globals. Marked `unsafe extern "C"` only to
/// keep the C ABI for callers linked against the legacy engine.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the heaviest SIM consumer set (`p_enemy`, `p_pspr`, `p_mobj`, `p_map`,
/// `p_inter`, `p_user`, `s_sound`, `st_stuff`) reaches the upstream name
/// through the root shim.
#[doc(alias = "R_PointToAngle2")]
#[export_name = "R_PointToAngle2"]
pub unsafe extern "C" fn point_to_angle2(
    x1: fixed_t,
    y1: fixed_t,
    x2: fixed_t,
    y2: fixed_t,
) -> angle_t {
    angle_from_delta(x2 - x1, y2 - y1)
}

/// Compute the distance from the current view position to a map point.
///
/// Uses the `tantoangle` and `finesine` tables to compute the Euclidean
/// distance via a cosine projection. Handles `dx == 0` to avoid division by
/// zero (matches the udm1.wad crash fix present in the C source).
///
/// Equivalent to `R_PointToDist` in `r_main.c`.
///
/// # Safety
/// Reads [`viewx`] and [`viewy`], which must have been set by
/// [`crate::doom::r_main::frame::setup_frame`] before this is called in a
/// rendering context.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_segs` reaches the upstream name through the root shim.
#[doc(alias = "R_PointToDist")]
#[export_name = "R_PointToDist"]
pub unsafe extern "C" fn point_to_dist(x: fixed_t, y: fixed_t) -> fixed_t {
    let mut dx = (x - viewx).wrapping_abs();
    let mut dy = (y - viewy).wrapping_abs();

    if dy > dx {
        std::mem::swap(&mut dx, &mut dy);
    }

    let frac = if dx != 0 { FixedDiv(dy, dx) } else { 0 };

    let angle = (tables::tantoangle[(frac as u32 >> DBITS) as usize] + ANG90) >> ANGLETOFINESHIFT;

    FixedDiv(dx, tables::finesine[angle as usize])
}

/// No-op initialiser kept for ABI compatibility.
///
/// In the original Doom source, `R_InitPointToAngle` built the `tantoangle`
/// lookup table at runtime. The table is now precomputed in `tables.c` (and
/// `tables.rs`), so this function has no work to do.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// [`crate::doom::r_main::frame::init`] reaches it through the root shim.
#[doc(alias = "R_InitPointToAngle")]
#[export_name = "R_InitPointToAngle"]
pub extern "C" fn init_point_to_angle() {
    // UNUSED - now getting from tables.c
}

/// No-op initialiser kept for ABI compatibility.
///
/// In the original source, `R_InitTables` computed `finetangent` and
/// `finesine` at runtime. Both tables are now precomputed in `tables.c`
/// (and `tables.rs`).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// [`crate::doom::r_main::frame::init`] reaches it through the root shim.
#[doc(alias = "R_InitTables")]
#[export_name = "R_InitTables"]
pub extern "C" fn init_tables() {
    // UNUSED: now getting from tables.c
}

/// Compute the texture-mapping scale for a wall column at the given view
/// angle.
///
/// Returns the fixed-point scale factor used to stretch or shrink a wall
/// texture column. Clamps the result to `[256, 64 * FRACUNIT]` to avoid
/// extreme near/far values.
///
/// `rw_distance` (distance from view to the wall normal) and
/// `rw_normalangle` (angle of the wall's outward normal) must be set before
/// calling this function; they are both globals in `r_segs`.
///
/// Equivalent to `R_ScaleFromGlobalAngle` in `r_main.c`.
///
/// # Safety
/// Reads [`viewangle`], [`projection`], [`detailshift`], and the `r_segs`
/// globals [`rw_distance`] and [`rw_normalangle`]. All must be initialised
/// before calling. The wrapping angle arithmetic is load-bearing (the
/// wraparound test below pins it).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_segs` reaches the upstream name through the root shim.
#[doc(alias = "R_ScaleFromGlobalAngle")]
#[export_name = "R_ScaleFromGlobalAngle"]
pub unsafe extern "C" fn scale_from_global_angle(visangle: angle_t) -> fixed_t {
    let anglea = ANG90.wrapping_add(visangle.wrapping_sub(viewangle));
    let angleb = ANG90.wrapping_add(visangle.wrapping_sub(rw_normalangle));

    let sinea = tables::finesine[(anglea >> ANGLETOFINESHIFT) as usize];
    let sineb = tables::finesine[(angleb >> ANGLETOFINESHIFT) as usize];
    let num = FixedMul(projection, sineb) << detailshift;
    let den = FixedMul(rw_distance, sinea);

    if den > num >> 16 {
        let mut scale = FixedDiv(num, den);
        scale = scale.clamp(256, 64 * FRACUNIT);
        scale
    } else {
        64 * FRACUNIT
    }
}

/// Node flag indicating the child index refers to a subsector, not another
/// node. Matches `NF_SUBSECTOR` in `r_local.h`.
pub(super) const NF_SUBSECTOR: u32 = 0x8000;

/// Walk the BSP tree to find the subsector that contains the given map point.
///
/// Starts at the root node (`numnodes - 1`) and descends by calling
/// [`point_on_side`] at each node until reaching a leaf (subsector) indicated
/// by the `NF_SUBSECTOR` flag. Handles the degenerate case of a single
/// subsector (no nodes).
///
/// Equivalent to `R_PointInSubsector` in `r_main.c`.
///
/// # Safety
/// Reads the `nodes` and `subsectors` arrays from `p_setup`; both must be
/// fully populated (i.e. the map must have been loaded) before this function
/// is called.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the SIM consumers (`g_game`, `p_mobj`, `p_map`, `p_maputl`) reach the
/// upstream name through the root shim.
#[doc(alias = "R_PointInSubsector")]
#[export_name = "R_PointInSubsector"]
pub unsafe extern "C" fn point_in_subsector(x: fixed_t, y: fixed_t) -> *mut subsector_t {
    // single subsector is a special case
    if numnodes == 0 {
        return subsectors as *mut subsector_t;
    }

    let mut nodenum = numnodes - 1;

    while (nodenum as u32) & NF_SUBSECTOR == 0 {
        let node = nodes.add(nodenum as usize) as *const node_t;
        let side = point_on_side(x, y, node);
        nodenum = (*node).children[side as usize] as c_int;
    }

    subsectors.add((nodenum as u32 & !NF_SUBSECTOR) as usize) as *mut subsector_t
}

/// Expand a bounding box so that it encloses the given map-coordinate point.
///
/// Equivalent to `R_AddPointToBox` in `r_main.c`. Modifies the four
/// fixed-point values at `box_` in the [`BBox`] layout (`LEFT`, `RIGHT`,
/// `BOTTOM`, `TOP`).
///
/// # Safety
/// `box_` must point to a valid, writable array of at least four `fixed_t`
/// values laid out in [`BBox`] index order.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the automap/`maputil` consumers reach the upstream name through the
/// root shim.
#[doc(alias = "R_AddPointToBox")]
#[export_name = "R_AddPointToBox"]
pub unsafe extern "C" fn add_point_to_box(x: c_int, y: c_int, box_: *mut fixed_t) {
    if x < *box_.add(BBox::LEFT) {
        *box_.add(BBox::LEFT) = x;
    }
    if x > *box_.add(BBox::RIGHT) {
        *box_.add(BBox::RIGHT) = x;
    }
    if y < *box_.add(BBox::BOTTOM) {
        *box_.add(BBox::BOTTOM) = y;
    }
    if y > *box_.add(BBox::TOP) {
        *box_.add(BBox::TOP) = y;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::doom::m_fixed::{fixed_t, FRACUNIT};
    use crate::doom::r_bsp::node_t;
    use crate::doom::r_main::{
        detailshift, projection, viewangle, viewx, viewy, R_PointOnSide, R_PointToAngle2,
        R_PointToDist, R_ScaleFromGlobalAngle,
    };
    use crate::doom::r_segs::{rw_distance, rw_normalangle};
    use crate::doom::tables::{ANG180, ANG270, ANG90};

    /// Process-wide guard serialising tests that read or write the renderer
    /// globals (`viewx`, `viewy`, `viewangle`, ...). `cargo test` runs tests
    /// in parallel by default; without this lock, two tests touching these
    /// statics could race and tear each other's state.
    static RENDER_GLOBALS_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that snapshots `viewx`/`viewy` on construction and
    /// restores them on drop, so an assertion panic cannot leave the
    /// globals in a perturbed state for any subsequent test that
    /// happens to acquire the lock.
    struct ViewPosGuard {
        saved_x: fixed_t,
        saved_y: fixed_t,
    }

    impl ViewPosGuard {
        fn new() -> Self {
            unsafe {
                Self {
                    saved_x: viewx,
                    saved_y: viewy,
                }
            }
        }
    }

    impl Drop for ViewPosGuard {
        fn drop(&mut self) {
            unsafe {
                viewx = self.saved_x;
                viewy = self.saved_y;
            }
        }
    }

    #[test]
    fn r_point_to_dist_reasonable_values() {
        let _g = RENDER_GLOBALS_LOCK.lock().unwrap();
        let _vp = ViewPosGuard::new();
        unsafe {
            viewx = 0;
            viewy = 0;

            // Straight ahead: distance should be approximately FRACUNIT.
            let dist_ahead = R_PointToDist(FRACUNIT, 0);
            assert!(dist_ahead > 0, "distance straight ahead must be positive");
            assert!(
                (dist_ahead - FRACUNIT).abs() < FRACUNIT / 4,
                "distance straight ahead should be near FRACUNIT, got {}",
                dist_ahead
            );

            // 45-degree diagonal: distance should be larger than straight ahead.
            let dist_diag = R_PointToDist(FRACUNIT, FRACUNIT);
            assert!(
                dist_diag > dist_ahead,
                "diagonal distance ({}) must exceed straight-ahead distance ({})",
                dist_diag,
                dist_ahead
            );

            // Same point: distance must be exactly zero.
            let dist_zero = R_PointToDist(0, 0);
            assert_eq!(dist_zero, 0, "distance to view position must be zero");
        }
    }

    /// R_PointToAngle2 used to set `viewx = x1; viewy = y1` and delegate to
    /// R_PointToAngle, clobbering the view-position globals. The refactored
    /// implementation must compute the angle purely from the delta and leave
    /// `viewx` / `viewy` untouched.
    #[test]
    fn r_point_to_angle2_preserves_view_globals() {
        let _g = RENDER_GLOBALS_LOCK.lock().unwrap();
        let _vp = ViewPosGuard::new();
        unsafe {
            viewx = 12345;
            viewy = -6789;

            let _ = R_PointToAngle2(1000, 2000, 3000, 4000);

            assert_eq!(viewx, 12345, "R_PointToAngle2 must not modify viewx");
            assert_eq!(viewy, -6789, "R_PointToAngle2 must not modify viewy");
        }
    }

    /// R_ScaleFromGlobalAngle previously used plain `+` and `-` on
    /// angle_t values, which panics in debug builds on wraparound.
    /// It must use wrapping_add / wrapping_sub.
    #[test]
    fn r_scale_from_global_angle_wraparound_no_panic() {
        unsafe {
            viewangle = 0;
            rw_normalangle = 0;
            projection = FRACUNIT;
            rw_distance = FRACUNIT;
            detailshift = 0;

            // Angles near the u32 boundary must not panic.
            let _ = R_ScaleFromGlobalAngle(u32::MAX);
            let _ = R_ScaleFromGlobalAngle(0);
            let _ = R_ScaleFromGlobalAngle(ANG90);
            let _ = R_ScaleFromGlobalAngle(ANG180);
            let _ = R_ScaleFromGlobalAngle(ANG270);
            let _ = R_ScaleFromGlobalAngle(viewangle.wrapping_sub(1));
            let _ = R_ScaleFromGlobalAngle(viewangle.wrapping_add(1));
        }
    }

    #[test]
    fn r_point_on_side_vertical_line() {
        unsafe {
            // Node with dx == 0, dy > 0 (vertical line, northward).
            let node = node_t {
                x: 0,
                y: 0,
                dx: 0,
                dy: FRACUNIT,
                bbox: [[0; 4]; 2],
                children: [0; 2],
            };
            // Point to the left of the line -> side 1 (from C logic).
            assert_eq!(R_PointOnSide(-FRACUNIT, 0, &node), 1);
            // Point to the right of the line -> side 0.
            assert_eq!(R_PointOnSide(FRACUNIT, 0, &node), 0);
            // Point exactly on the line -> x <= node.x, side depends on dy > 0 -> 1.
            assert_eq!(R_PointOnSide(0, 0, &node), 1);
        }
    }

}
