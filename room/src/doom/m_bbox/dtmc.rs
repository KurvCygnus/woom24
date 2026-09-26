//! Demo-synchronization surface extracted from `m_bbox`: the pure
//! bounding-box computation whose exact comparisons and sentinels
//! order every line-crossing, thing-hit, and sight extent the boxes
//! feed in the demo-pinned simulation.

use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::fixed_t;

/// Initialise an inverted bounding box so subsequent
/// [`add_to_box`] calls shrink it to fit -- the pure slice form of
/// upstream `M_ClearBox` (`vendor/doomgeneric/m_bbox.c`), whose
/// output feeds the map bounding box, blockmap spans, and sight
/// extents the demo pins.
///
/// ## Technical Details
///
/// The sentinel choice is load-bearing: `TOP`/`RIGHT` are stored as
/// `i32::MIN` and `BOTTOM`/`LEFT` as `i32::MAX`, exactly the C
/// literals, in the C statement order -- the first [`add_to_box`]
/// then collapses the box by its `else if` arms. Any other
/// sentinel changes which points survive the first comparison and
/// shifts every derived collision/sight extent.
///
/// ## On Calling
///
/// `bbox` must have at least the four slots indexed by the `BBox`
/// constants (`TOP`/`BOTTOM`/`LEFT`/`RIGHT` = 0/1/2/3); the C
/// contract is a pointer to four contiguous `fixed_t` values, and
/// the `ffi` wrapper materialises exactly that slice. Pure
/// computation -- no state, no threading assumptions; upstream
/// mutates these boxes only on the single-threaded game tick.
#[doc(alias = "M_ClearBox")]
pub fn clear_box(bbox: &mut [fixed_t])
{
    bbox[BBox::TOP] = i32::MIN;
    bbox[BBox::RIGHT] = i32::MIN;
    bbox[BBox::BOTTOM] = i32::MAX;
    bbox[BBox::LEFT] = i32::MAX;
}

/// Expand a bounding box to contain the point `(x, y)` -- the pure
/// slice form of upstream `M_AddToBox`
/// (`vendor/doomgeneric/m_bbox.c`), whose per-point outcome feeds
/// line-crossing and thing-hit decisions the demo pins.
///
/// ## Technical Details
///
/// The `if` / `else if` asymmetry is vanilla-exact and load-bearing:
/// a point updates LEFT only when it is left of the current LEFT,
/// and is then never compared against RIGHT (the `else if` is
/// skipped) -- so after a [`clear_box`] plus a single point, RIGHT
/// and TOP stay inverted until a later point exceeds them. Removing
/// the `else` would produce a different box for every multi-point
/// sequence and change the outcome of the collision/sight decisions
/// downstream. The comparisons are strict (`<`, `>`), exactly as the
/// C source orders them.
///
/// ## On Calling
///
/// `bbox` must have at least the four slots indexed by the `BBox`
/// constants; `x` and `y` are raw `fixed_t` values, never floats.
/// Do not "repair" the `else if` asymmetry -- it is the pinned
/// vanilla behavior. Pure computation -- no state, no threading
/// assumptions.
#[doc(alias = "M_AddToBox")]
pub fn add_to_box(bbox: &mut [fixed_t], x: fixed_t, y: fixed_t)
{
    if x < bbox[BBox::LEFT]
    {
        bbox[BBox::LEFT] = x;
    }
    else if x > bbox[BBox::RIGHT]
    {
        bbox[BBox::RIGHT] = x;
    }
    if y < bbox[BBox::BOTTOM]
    {
        bbox[BBox::BOTTOM] = y;
    }
    else if y > bbox[BBox::TOP]
    {
        bbox[BBox::TOP] = y;
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::m_bbox::dtmc::{add_to_box, clear_box};
    use crate::doom::m_fixed::fixed_t;

    /// Baseline contract (F10 wave A3): these vectors were run green
    /// against the original `M_ClearBox` / `M_AddToBox` bodies BEFORE
    /// the computation moved here, then re-pointed to the pure slice
    /// functions -- same vectors, same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): both bodies -> dtmc -- their outputs feed
    /// demo-visible collision/sight extents (map bbox `p_setup.rs`,
    /// blockmap span `p_map.rs`, dirtybox `v_video.rs`, collantern
    /// `r_main`/`r_bsp`); the pointer marshalling stays behind at
    /// `ffi`.
    ///
    /// Verifies that after `clear_box` followed by a single
    /// `add_to_box` the box collapses to a point in the LEFT/BOTTOM
    /// slots while the RIGHT/TOP slots stay inverted (an artefact of
    /// the C `if`/`else if`).
    #[test]
    fn clear_then_add_shrinks_to_point()
    {
        let mut bbox: [fixed_t; 4] = [0; 4];
        clear_box(&mut bbox);
        add_to_box(&mut bbox, 10, 20);
        // BBox::TOP=0, BBox::BOTTOM=1, BBox::LEFT=2, BBox::RIGHT=3
        // After clear_box: [INT_MIN, INT_MAX, INT_MAX, INT_MIN]
        // add_to_box(10, 20):
        //   x=10 < BBox::LEFT(INT_MAX) → true → LEFT=10, skips else-if (RIGHT stays INT_MIN)
        //   y=20 < BBox::BOTTOM(INT_MAX) → true → BOTTOM=20, skips else-if (TOP stays INT_MIN)
        assert_eq!(bbox[0], i32::MIN); // top still inverted (else-if branch)
        assert_eq!(bbox[1], 20); // bottom = y
        assert_eq!(bbox[2], 10); // left = x
        assert_eq!(bbox[3], i32::MIN); // right still inverted (else-if branch)
    }

    /// Baseline contract (F10 wave A3): same pre-extraction vectors
    /// as the asymmetric-shrink pin above.
    ///
    /// Verifies that successive `add_to_box` calls correctly expand
    /// the box outward in all four directions.
    #[test]
    fn add_multiple_points_expands_box()
    {
        let mut bbox: [fixed_t; 4] = [0; 4];
        clear_box(&mut bbox);
        add_to_box(&mut bbox, 10, 20);
        add_to_box(&mut bbox, -5, 100);
        add_to_box(&mut bbox, 50, 5);
        assert_eq!(bbox[0], 100); // top
        assert_eq!(bbox[1], 5); // bottom
        assert_eq!(bbox[2], -5); // left
        assert_eq!(bbox[3], 50); // right
    }
}
