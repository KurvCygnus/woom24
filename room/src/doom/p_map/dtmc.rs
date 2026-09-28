//! Demo-synchronization surface extracted from `p_map`: the spechit
//! overrun trample model's two pure cores -- the trample address formula
//! and the `.bss`-order case table that decides which engine global each
//! overrun write lands in. Their exact outputs ARE the catalog's entry-1
//! demo-compatibility behavior; the stateful side (the `-spechit` parse,
//! the census record, the writes themselves) stays at the call site in
//! `spechit::spechit_overrun`.

use std::ffi::c_int;

/// Where a spechit overrun write lands in the emulated doom2.exe `.bss`
/// neighbor order (the chocolate/woof mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrampleTarget
{
    /// `tmbbox[index]`, index `0..=3` (overrun sequence slots 9..=12).
    Tmbbox(usize),
    /// `crushchange` (overrun sequence slot 13).
    Crushchange,
    /// `nofit` (overrun sequence slot 14).
    Nofit,
}

/// Compute the emulated overrun address for the crossed linedef at
/// `line_index` (its `offset_from` the global `lines` base) under the
/// spechit base address `baseaddr` -- the pure half of upstream
/// `SpechitOverrun`'s address computation
/// (`vendor/doomgeneric/p_map.c:1391`).
///
/// ## Technical Details
///
/// The formula is `baseaddr + (ld - lines) * 0x3e` with 32-bit wrap,
/// matching the C `int` arithmetic the DOS binary performed. The
/// wrapping ops are load-bearing (the overrun path must never panic,
/// even on the pathological offsets the whole-body tests drive through
/// synthetic pointers) and reproduce the shipping wasm behavior exactly;
/// the pre-extraction body used plain (debug-panicking) arithmetic whose
/// release result is bit-identical. The `0x3e` stride is `size_of
///::<line_t>()` in the DOS build -- it pins the whole trample model.
///
/// ## On Calling
///
/// Arguments are a linedef offset (small, `0..=numlines` for real maps)
/// and a raw `c_int` base (default `DEFAULT_SPECHIT_MAGIC =
/// 0x01C09C98`). Never debug-assert on overflow: the wrap IS the pinned
/// behavior demo playback depends on. Pure computation -- no state, no
/// threading assumptions.
pub fn spechit_trample_addr(line_index: usize, baseaddr: c_int) -> c_int { baseaddr.wrapping_add(line_index.wrapping_mul(0x3e) as c_int) }

/// Decide which engine global a spechit overrun write with the given
/// `numspechit` value lands in -- the pure half of upstream
/// `SpechitOverrun`'s case table
/// (`vendor/doomgeneric/p_map.c:1391`).
///
/// ## Technical Details
///
/// The mapping is the chocolate/woof doom2.exe `.bss` order -- NOT
/// dsda's 13/14 swap and not its dosdoom/tasdoom variant (catalog entry
/// 1's recorded deviation): slots 9..=12 hit `tmbbox[0..=3]`, 13 hits
/// `crushchange`, 14 hits `nofit`. A swap would redirect the emulated
/// corruption and desync every overrun demo; this table is the single
/// source of that decision.
///
/// ## On Calling
///
/// `numspechit` is the raw post-increment crossing counter; `None` for
/// values outside `9..=14` means "vanilla wrote somewhere we do not
/// model" -- the caller emits the upstream warning and writes nothing.
/// Pure computation -- no state, no threading assumptions.
pub fn trample_target(numspechit: c_int) -> Option<TrampleTarget>
{
    match numspechit
    {
        9..=12 => Some(TrampleTarget::Tmbbox((numspechit - 9) as usize)),
        13 => Some(TrampleTarget::Crushchange),
        14 => Some(TrampleTarget::Nofit),
        _ => None,
    }
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_int;

    use crate::doom::c_ffi::DEFAULT_SPECHIT_MAGIC;

    use super::{spechit_trample_addr, trample_target, TrampleTarget};

    /// Baseline contract (F10 wave B2b): the addr vectors were written
    /// and run against the real in-file `SpechitOverrun` body BEFORE the
    /// extraction (see `spechit::tests` for the whole-body drives), then
    /// retargeted here -- same vectors, same results, plus the 32-bit
    /// wrap pins the whole-body drive cannot reach.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): wholly qualifying -- the formula's exact output is
    /// the value vanilla demos observed as trampled `tmbbox` /
    /// `crushchange` / `nofit` state.
    #[test]
    fn baseline_spechit_trample_addr_offsets()
    {
        let magic = DEFAULT_SPECHIT_MAGIC as c_int;
        assert_eq!(spechit_trample_addr(0, magic), magic);
        assert_eq!(spechit_trample_addr(1, magic), magic + 0x3e);
        assert_eq!(spechit_trample_addr(2, magic), magic + 2 * 0x3e);
        assert_eq!(spechit_trample_addr(13, magic), magic + 13 * 0x3e);
        // 32-bit wrap pins: the C int arithmetic wrapped; so must this.
        // c_int::MAX + 0x3e crosses the sign boundary once:
        // 2147483647 + 62 == 2147483709 - 2^32 == -2147483587.
        assert_eq!(
            spechit_trample_addr(1, c_int::MAX),
            c_int::MIN.wrapping_add(0x3d)
        );
        // A zero-offset drive is the identity on the base.
        assert_eq!(spechit_trample_addr(0, c_int::MAX), c_int::MAX);
        assert_eq!(spechit_trample_addr(0, c_int::MIN), c_int::MIN);
        // A zero base stays zero regardless of offset.
        assert_eq!(spechit_trample_addr(3, 0), 3 * 0x3e);
    }

    /// Baseline contract (F10 wave B2b): the case-table vectors were
    /// written and run against the real in-file `SpechitOverrun` body
    /// BEFORE the extraction (whole-body drives in `spechit::tests`),
    /// then retargeted here -- same vectors, same results.
    ///
    /// Adjudication: wholly qualifying -- the target selection IS the
    /// chocolate/woof doom2.exe `.bss` order the catalog documents; dsda's
    /// 13/14 swap is deliberately NOT followed.
    #[test]
    fn baseline_trample_target_chocolate_case_table()
    {
        assert_eq!(trample_target(9), Some(TrampleTarget::Tmbbox(0)));
        assert_eq!(trample_target(10), Some(TrampleTarget::Tmbbox(1)));
        assert_eq!(trample_target(11), Some(TrampleTarget::Tmbbox(2)));
        assert_eq!(trample_target(12), Some(TrampleTarget::Tmbbox(3)));
        assert_eq!(trample_target(13), Some(TrampleTarget::Crushchange));
        assert_eq!(trample_target(14), Some(TrampleTarget::Nofit));
        // Outside the modeled window: nothing is written upstream either.
        assert_eq!(trample_target(8), None);
        assert_eq!(trample_target(15), None);
        assert_eq!(trample_target(0), None);
        assert_eq!(trample_target(-1), None);
    }
}
