//! Demo-synchronization surface extracted from `p_tick`: the thinker
//! removal sentinel pair whose exact all-ones bit pattern gates when
//! thinkers stop acting in `P_RunThinkers`' lazy-removal pass.

#![allow(non_camel_case_types)]

use super::actionf_t;

/// Produce the thinker removal sentinel `(actionf_v)(-1)`: the exact
/// all-ones function-pointer bit pattern that marks a thinker for lazy
/// removal and gates when it stops acting. The upstream counterpart is
/// the inline `(actionf_v)(-1)` literal of `p_tick.c` (`:76` where
/// `P_RemoveThinker` stores it, `:101` where `P_RunThinkers` compares
/// it) -- no named C function, hence no `#[doc(alias)]`.
///
/// ## Technical Details
///
/// The pattern is load-bearing: `usize::MAX` transmuted into a function
/// pointer is a value no real code address can take, so a single `==`
/// compare reliably separates removal markers from live callbacks. The
/// `transmute::<usize, _>` construction IS the contract -- the compiler
/// would reject the same bit pattern written as a constant function
/// pointer -- and it must keep agreeing bit-for-bit with the freeze-zone
/// duplicate in `p_mobj.rs` (which reproduces the identical transmute);
/// a different discriminant (e.g. an `Option::None`-based redesign)
/// breaks mid-tic removal detection.
///
/// ## On Calling
///
/// Pure computation over no state. Compare the result only through
/// `==` on the union's `acv` variant (`is_sentinel`); never dereference
/// or call the produced pointer -- it is a bit pattern, not a callable
/// address. Upstream stores and checks the sentinel only on the
/// single-threaded game tick; the callers here (`P_RemoveThinker`,
/// `P_RunThinkers`) preserve exactly that.
pub fn sentinel_ac() -> Option<unsafe extern "C" fn()>
{
    // -1 as pointer-sized integer, reinterpreted as a function pointer.
    Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn()>(usize::MAX) })
}

/// Return `true` when `f` holds the removal sentinel
/// (`(actionf_v)(-1)`) -- i.e. the thinker has been marked for lazy
/// removal and must be unlinked and freed instead of acting. The
/// compare half of the inline upstream check at `p_tick.c:101`.
///
/// ## Technical Details
///
/// The comparison reads the union through its `acv` variant, so the
/// all-ones pattern is compared as a no-argument function pointer
/// regardless of which variant the thinker last stored -- the exact C
/// semantics of `function.acv == (actionf_v)(-1)`. The compare is
/// total: any real code address differs from `usize::MAX`, so a live
/// thinker can never test as removed.
///
/// ## On Calling
///
/// Pass the thinker's `function` field by value; the union read is the
/// `unsafe` inside, performed on the `acv` variant only. Same
/// single-threaded tick assumption as upstream -- the sentinel is
/// stored and checked within one `P_RunThinkers` pass.
pub fn is_sentinel(f: actionf_t) -> bool
{
    unsafe { f.acv == sentinel_ac() }
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_void;

    use crate::doom::p_tick::dtmc::{is_sentinel, sentinel_ac};
    use crate::doom::p_tick::actionf_t;

    /// Baseline contract (F10 graduate #4): seed vector, retained from
    /// the pre-graduation suite (`sentinel_is_all_ones`) -- the
    /// sentinel round-trips to the pointer-sized all-ones pattern.
    #[test]
    fn sentinel_is_all_ones()
    {
        let s = sentinel_ac();
        assert_eq!(usize::MAX, s.map(|f| f as usize).unwrap_or(0));
    }

    /// Baseline contract (F10 graduate #4): these vectors were written
    /// and run against the original file-local `sentinel_ac` /
    /// `is_sentinel` bodies BEFORE the pair extracted into `dtmc`,
    /// then retargeted to the extracted functions -- same vectors,
    /// same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the sentinel pair -> dtmc -- the exact all-ones
    /// bit pattern gates when thinkers stop acting (`P_RunThinkers`'
    /// lazy-removal compare) and must keep agreeing bit-for-bit with
    /// the freeze-zone duplicate in `p_mobj.rs`; the sentinel store,
    /// the list walk, and every `static mut` access stay behind at
    /// the call sites.
    #[test]
    fn baseline_sentinel_discrimination()
    {
        // The produced sentinel is recognized: store it, compare with
        // `==` on `acv` -- the whole `P_RemoveThinker` / `P_RunThinkers`
        // lazy-removal contract.
        assert!(is_sentinel(actionf_t { acv: sentinel_ac() }));
        // A null `acv` is a live (no-op) function slot, not the
        // sentinel: freshly initialized thinkers hold exactly this.
        assert!(!is_sentinel(actionf_t { acv: None }));
        // A genuine function pointer never collides with the all-ones
        // pattern: live thinkers must never test as removed.
        assert!(!is_sentinel(actionf_t { acp1: Some(probe) }));
    }

    /// Single-argument probe used as a genuine function pointer in the
    /// baseline vectors; its address must differ from the sentinel.
    unsafe extern "C" fn probe(_p: *mut c_void) {}
}
