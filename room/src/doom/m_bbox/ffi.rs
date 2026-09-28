//! The C-linkage surface: the `BBox` slot-index constants and the
//! `#[no_mangle]` `extern "C"` pointer wrappers marshalling to
//! `dtmc`, moved from the pre-graduation `m_bbox.rs` (F10 wave A3).

#![allow(non_snake_case)]

use super::dtmc::{add_to_box, clear_box};
use crate::doom::m_fixed::fixed_t;

/// Indices into a 4-element bounding-box array, matching `m_bbox.h`.
pub struct BBox;

/// Constants exposing the bbox slot indices. Kept as `usize` so they can
/// be used directly to index a `[fixed_t; 4]` without further casts.
impl BBox
{
    /// Top edge slot (largest y). Matches `BOXTOP`.
    #[doc(alias = "BOXTOP")]
    pub const TOP: usize = 0;
    /// Bottom edge slot (smallest y). Matches `BOXBOTTOM`.
    #[doc(alias = "BOXBOTTOM")]
    pub const BOTTOM: usize = 1;
    /// Left edge slot (smallest x). Matches `BOXLEFT`.
    #[doc(alias = "BOXLEFT")]
    pub const LEFT: usize = 2;
    /// Right edge slot (largest x). Matches `BOXRIGHT`.
    #[doc(alias = "BOXRIGHT")]
    pub const RIGHT: usize = 3;
}

/// `void M_ClearBox(fixed_t *box)` — initialises an inverted bbox so
/// subsequent `M_AddToBox` calls shrink it to fit. Pointer wrapper:
/// marshals the raw C pointer to a four-element slice and delegates
/// the computation to [`super::dtmc::clear_box`].
///
/// # Safety
/// `bbox` must point to at least 4 contiguous `fixed_t` values.
#[no_mangle]
pub unsafe extern "C" fn M_ClearBox(bbox: *mut fixed_t) { clear_box(std::slice::from_raw_parts_mut(bbox, 4)); }

/// `void M_AddToBox(fixed_t *box, fixed_t x, fixed_t y)`. Pointer
/// wrapper: marshals the raw C pointer to a four-element slice and
/// delegates the computation to [`super::dtmc::add_to_box`].
///
/// # Safety
/// `bbox` must point to at least 4 contiguous `fixed_t` values.
#[no_mangle]
pub unsafe extern "C" fn M_AddToBox(bbox: *mut fixed_t, x: fixed_t, y: fixed_t) { add_to_box(std::slice::from_raw_parts_mut(bbox, 4), x, y); }

#[cfg(test)]
mod tests
{
    use super::super::dtmc::{add_to_box, clear_box};
    use super::*;

    /// Wrapper round-trip pin (F10 wave A3, A3 report §2.4): the
    /// `#[no_mangle]` pointer wrappers must produce byte-identical
    /// boxes to the pure `dtmc` functions they delegate to.
    #[test]
    fn ffi_wrapper_round_trips_to_dtmc()
    {
        let mut via_ffi: [fixed_t; 4] = [0; 4];
        unsafe
        {
            M_ClearBox(via_ffi.as_mut_ptr());
            M_AddToBox(via_ffi.as_mut_ptr(), -123, 456);
            M_AddToBox(via_ffi.as_mut_ptr(), 789, -1);
        }
        let mut via_dtmc: [fixed_t; 4] = [0; 4];
        clear_box(&mut via_dtmc);
        add_to_box(&mut via_dtmc, -123, 456);
        add_to_box(&mut via_dtmc, 789, -1);
        assert_eq!(via_ffi, via_dtmc);
    }
}
