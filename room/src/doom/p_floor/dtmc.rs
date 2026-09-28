//! Demo-synchronization surface extracted from `p_floor`: the pure
//! plane-movement step decision behind `T_MovePlane` -- the shared
//! overshoot/landing primitive whose exact strict comparisons order
//! every floor, plat, ceiling, and door motion in the demo-pinned
//! simulation.

use std::ffi::c_int;

use crate::doom::m_fixed::fixed_t;

/// One plane-movement step decision of `T_MovePlane`: given the
/// plane's current height, the per-tic speed, the destination, and the
/// movement direction, decide whether the step overshoots `dest`
/// (landing exactly on it this tic) or lands short of it, and return
/// the new height with that verdict. Pure integer form of the four
/// inline overshoot conditions of `vendor/doomgeneric/p_floor.c`
/// (`:42-283` -- no named C function, hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The comparison direction is load-bearing per movement direction:
/// descending steps overshoot when `pos - speed < dest` (strict),
/// ascending when `pos + speed > dest` (strict), so an exact landing
/// (`pos - speed == dest`) is a NORMAL step, not an overshoot -- the
/// plane reaches `dest` only on the next call, exactly as the original
/// conditions order it, and the baseline vectors pin both `==` arms.
/// `P_ChangeSector` is never called here: this extraction is only the
/// arithmetic decision, and every crush-check call/restore pair stays
/// at the `mover::T_MovePlane` call sites in the original order.
///
/// ## On Calling
///
/// Pure integer computation; `direction` is `1` (up) or `-1` (down) --
/// the only values the upstream `switch` arms pass (any other value
/// steps nowhere, mirroring the upstream fall-through). Never fold the
/// `P_ChangeSector` calls in here -- the crush restore order is
/// state-visible. Real map heights and speeds stay far from `i32`
/// range, matching the original body's plain integer arithmetic.
pub fn step_toward(pos: fixed_t, speed: fixed_t, dest: fixed_t, direction: c_int) -> (fixed_t, bool)
{
    match direction
    {
        -1 =>
        {
            if pos - speed < dest { (dest, true) }
            else { (pos - speed, false) }
        }
        1 =>
        {
            if pos + speed > dest { (dest, true) }
            else { (pos + speed, false) }
        }
        _ => (pos, false),
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_floor::dtmc::step_toward;

    /// Baseline contract (F10 wave A2): these vectors were written
    /// and run against the original in-file `T_MovePlane`
    /// overshoot/landing branches BEFORE the decision extracted into
    /// `dtmc::step_toward`, then retargeted -- same vectors, same
    /// results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the step decision -> dtmc -- `T_MovePlane` is the
    /// shared plane primitive of floors, plats, ceilings, and doors,
    /// and the strict comparisons fix when planes land and report
    /// `result_pastdest`; the `P_ChangeSector` call/restore pairs stay
    /// behind at the `mover::T_MovePlane` call sites.
    ///
    /// The `(55, 5, 50, -1)` and `(145, 5, 150, 1)` vectors pin the
    /// `==` cases: an exact landing is a normal step (strict compare),
    /// so the plane only reports destination on the NEXT call.
    #[test]
    fn baseline_step_toward_vectors()
    {
        // DOWN: normal step, overshoot, exact landing.
        assert_eq!(step_toward(100, 5, 50, -1), (95, false));
        assert_eq!(step_toward(52, 5, 50, -1), (50, true));
        assert_eq!(step_toward(55, 5, 50, -1), (50, false));
        // UP: normal step, overshoot, exact landing.
        assert_eq!(step_toward(100, 5, 150, 1), (105, false));
        assert_eq!(step_toward(148, 5, 150, 1), (150, true));
        assert_eq!(step_toward(145, 5, 150, 1), (150, false));
        // Negative heights behave identically (plain integer math).
        assert_eq!(step_toward(-26, 5, -30, -1), (-30, true));
        assert_eq!(step_toward(-10, 5, -30, -1), (-15, false));
        // Zero speed: no movement, never an overshoot.
        assert_eq!(step_toward(100, 0, 100, -1), (100, false));
        assert_eq!(step_toward(100, 0, 100, 1), (100, false));
        // Domain edge: callers only pass -1/1 (the upstream switch
        // arms); any other direction steps nowhere.
        assert_eq!(step_toward(100, 5, 150, 0), (100, false));
    }
}
