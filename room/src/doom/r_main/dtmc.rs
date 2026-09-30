//! Extracted demo-synchronization surface: the pure octant classifier
//! behind `R_PointToAngle`/`R_PointToAngle2`, its exact-angle shift
//! constant, and the known-vector baseline.
//!
//! The classifier's answers steer SIM turning (`p_enemy` chase/attacks),
//! autoaim (`p_pspr`), sound propagation (`s_sound`), and status-bar
//! tracking (`st_stuff`) -- demo-visible behavior -- so the exact integer
//! sequence is pinned by the baseline vectors below (written against the
//! pre-move inline body) and re-run here unchanged.

use std::ffi::c_uint;

use crate::doom::m_fixed::{angle_t, fixed_t};
use crate::doom::tables::{self, SlopeDiv};
use crate::doom::tables::{ANG180, ANG270, ANG90};

/// `FRACBITS - SLOPEBITS = 16 - 11 = 5`.
///
/// Used to right-shift a fixed-point fraction before indexing `tantoangle[]`
/// in [`crate::doom::r_main::geometry::point_to_dist`]. Matches `DBITS` in
/// `vendor/doomgeneric/tables.h`. Kept beside the extracted core so the
/// exact-angle guarantee stays in one auditable file; the 45-degree-index
/// vector below pins the shift.
// DBITS = FRACBITS - SLOPEBITS = 16 - 11 = 5 (matches vendor/doomgeneric/tables.h).
pub(super) const DBITS: u32 = 5;

/// Classify a relative vector `(dx, dy)` into one of eight octants and look
/// up the corresponding BAM angle from the `tantoangle` table.
///
/// This is the pure octant-classification core shared by
/// [`crate::doom::r_main::geometry::point_to_angle`] and
/// [`crate::doom::r_main::geometry::point_to_angle2`]; it never reads or
/// writes any global state. Returns 0 for the zero vector.
///
/// ## Technical Details
///
/// The answers feed SIM-facing callers (AI turning, autoaim, sound angle),
/// so every octant boundary and the `ANG90 - 1` / `ANG180 - 1` /
/// `ANG270 - 1` fenceposts are demo synchronization surface; the vectors in
/// the test module below pin all of them. `SlopeDiv`'s den<512 saturation
/// and the wrapping subtraction in octant 8 are load-bearing.
///
/// ## On Calling
///
/// Raw fixed-point in/out; never debug-assert on overflow (octant 8
/// intentionally wraps). Pure and global-free -- safe to call anywhere,
/// including between a read and a use of the view position.
#[doc(alias = "point_to_angle_from_delta")]
pub fn angle_from_delta(mut dx: fixed_t, mut dy: fixed_t) -> angle_t {
    if dx == 0 && dy == 0 { return 0; }

    if dx >= 0 {
        // dx >= 0
        if dy >= 0 {
            // dy >= 0
            if dx > dy {
                // octant 0
                tables::tantoangle[SlopeDiv(dy as c_uint, dx as c_uint) as usize]
            } else {
                // octant 1
                ANG90 - 1 - tables::tantoangle[SlopeDiv(dx as c_uint, dy as c_uint) as usize]
            }
        } else {
            // dy < 0
            dy = -dy;
            if dx > dy {
                // octant 8
                0u32.wrapping_sub(tables::tantoangle[SlopeDiv(dy as c_uint, dx as c_uint) as usize])
            } else {
                // octant 7
                ANG270 + tables::tantoangle[SlopeDiv(dx as c_uint, dy as c_uint) as usize]
            }
        }
    } else {
        // dx < 0
        dx = -dx;
        if dy >= 0 {
            // dy >= 0
            if dx > dy {
                // octant 3
                ANG180 - 1 - tables::tantoangle[SlopeDiv(dy as c_uint, dx as c_uint) as usize]
            } else {
                // octant 2
                ANG90 + tables::tantoangle[SlopeDiv(dx as c_uint, dy as c_uint) as usize]
            }
        } else {
            // dy < 0
            dy = -dy;
            if dx > dy {
                // octant 4
                ANG180 + tables::tantoangle[SlopeDiv(dy as c_uint, dx as c_uint) as usize]
            } else {
                // octant 5
                ANG270 - 1 - tables::tantoangle[SlopeDiv(dx as c_uint, dy as c_uint) as usize]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_uint;
    use std::sync::Mutex;

    use super::{angle_from_delta, DBITS};
    use crate::doom::m_fixed::{fixed_t, FixedDiv, FRACUNIT};
    use crate::doom::r_main::{viewx, viewy, R_PointToAngle2};
    use crate::doom::tables::{self, SlopeDiv};
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

    /// The DBITS constant was once incorrectly set to 15 instead of
    /// FRACBITS - SLOPEBITS = 16 - 11 = 5.  With the wrong value,
    /// R_PointToDist indexes tantoangle with the wrong shift and
    /// computes completely wrong distances, causing wall-offset /
    /// sprite-clipping glitches.
    #[test]
    fn dbits_is_five() { assert_eq!(DBITS, 5, "DBITS must be FRACBITS - SLOPEBITS = 5"); }

    /// For a 45-degree line (dy == dx), FixedDiv(dy, dx) returns FRACUNIT.
    /// With DBITS = 5 the index into tantoangle is FRACUNIT >> 5 == 2048,
    /// the top of the table.  With DBITS = 15 the index would be 2,
    /// which produces a garbage distance.
    #[test]
    fn r_point_to_dist_45_degree_index() {
        let frac = FixedDiv(FRACUNIT, FRACUNIT); // dy == dx
        let index = (frac as u32 >> DBITS) as usize;
        assert_eq!(
            index, 2048,
            "frac>>DBITS for 45-degree case must index tantoangle[2048]"
        );
    }

    #[test]
    fn r_point_to_angle2_cardinals() {
        unsafe {
            // Due east is exact.
            assert_eq!(R_PointToAngle2(0, 0, FRACUNIT, 0), 0);

            // Due north quantizes to ANG90 - 1 in the fixed-point LUT.
            assert_eq!(R_PointToAngle2(0, 0, 0, FRACUNIT), ANG90 - 1);

            // Due west quantizes to ANG180 - 1.
            assert_eq!(R_PointToAngle2(0, 0, -FRACUNIT, 0), ANG180 - 1);

            // Due south is exact.
            assert_eq!(R_PointToAngle2(0, 0, 0, -FRACUNIT), ANG270);
        }
    }

    /// R_PointToAngle2 must be translation-invariant: the angle from
    /// `(x1, y1)` to `(x2, y2)` equals the angle of the delta vector from
    /// the origin. Confirms the rewrite preserves the original semantics
    /// for nonzero source points.
    #[test]
    fn r_point_to_angle2_translation_invariant() {
        // R_PointToAngle2 itself does not read viewx/viewy, but acquire
        // the lock and restore guard anyway so this test never observes
        // a torn state if a future change introduces a global read.
        let _g = RENDER_GLOBALS_LOCK.lock().unwrap();
        let _vp = ViewPosGuard::new();

        let cases: [(fixed_t, fixed_t); 8] = [
            (FRACUNIT, 0),
            (FRACUNIT, FRACUNIT),
            (0, FRACUNIT),
            (-FRACUNIT, FRACUNIT),
            (-FRACUNIT, 0),
            (-FRACUNIT, -FRACUNIT),
            (0, -FRACUNIT),
            (FRACUNIT, -FRACUNIT),
        ];

        for (dx, dy) in cases.iter().copied() {
            let from_origin = unsafe { R_PointToAngle2(0, 0, dx, dy) };
            let translated = unsafe {
                R_PointToAngle2(
                    100 * FRACUNIT,
                    -50 * FRACUNIT,
                    100 * FRACUNIT + dx,
                    -50 * FRACUNIT + dy,
                )
            };
            assert_eq!(
                from_origin, translated,
                "R_PointToAngle2 must be translation-invariant (dx={}, dy={})",
                dx, dy
            );
        }
    }

    /// Known-vector baseline for the pure octant classifier
    /// [`angle_from_delta`]: the zero vector plus all eight octants plus the
    /// `dx == dy` boundary. The octant selection and the `±1` fenceposts are
    /// the exactness-bearing decisions pinned here; the `tantoangle` lookups
    /// in the expectations are data-tier (precomputed upstream,
    /// `tables.rs`).
    #[test]
    fn angle_from_delta_octant_baseline_vectors() {
        // The zero vector is exact.
        assert_eq!(angle_from_delta(0, 0), 0);

        let idx = |num: c_uint, den: c_uint| tables::tantoangle[SlopeDiv(num, den) as usize];
        let diag = FRACUNIT as c_uint;
        let flat = 2 * FRACUNIT as c_uint;

        // octant 0: dx > dy >= 0.
        assert_eq!(angle_from_delta(2 * FRACUNIT, FRACUNIT), idx(diag, flat));
        // Boundary dx == dy falls into octant 1 (not 0).
        assert_eq!(
            angle_from_delta(FRACUNIT, FRACUNIT),
            ANG90 - 1 - idx(diag, diag)
        );
        // octant 1: dy >= dx > 0.
        assert_eq!(
            angle_from_delta(FRACUNIT, 2 * FRACUNIT),
            ANG90 - 1 - idx(diag, flat)
        );
        // octant 2: dy > |dx|, dx < 0.
        assert_eq!(
            angle_from_delta(-FRACUNIT, 2 * FRACUNIT),
            ANG90 + idx(diag, flat)
        );
        // octant 3: |dx| > dy >= 0, dx < 0.
        assert_eq!(
            angle_from_delta(-2 * FRACUNIT, FRACUNIT),
            ANG180 - 1 - idx(diag, flat)
        );
        // octant 4: |dx| > |dy|, both negative.
        assert_eq!(
            angle_from_delta(-2 * FRACUNIT, -FRACUNIT),
            ANG180 + idx(diag, flat)
        );
        // octant 5: |dy| > |dx|, both negative.
        assert_eq!(
            angle_from_delta(-FRACUNIT, -2 * FRACUNIT),
            ANG270 - 1 - idx(diag, flat)
        );
        // octant 7: |dy| > dx > 0, dy < 0.
        assert_eq!(
            angle_from_delta(FRACUNIT, -2 * FRACUNIT),
            ANG270 + idx(diag, flat)
        );
        // octant 8: dx > |dy| > 0, dy < 0 (wrapping subtraction).
        assert_eq!(
            angle_from_delta(2 * FRACUNIT, -FRACUNIT),
            0u32.wrapping_sub(idx(diag, flat))
        );
    }
}
