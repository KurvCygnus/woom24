//! Extracted demo-synchronization surface of the level loader: the linedef
//! slope ladder, the reject-padding zone-header formula, the
//! non-commercial thing filter, and the blockmap clamp ladders. Each is
//! the pure computation its caller embeds; the surrounding marshalling,
//! `static mut` access, and loop control (notably the `P_LoadThings`
//! break) stay at the call sites.

use std::ffi::c_int;

use crate::doom::m_fixed::FixedDiv;

use super::structs::{MAPBLOCKSHIFT, ST_HORIZONTAL, ST_NEGATIVE, ST_POSITIVE, ST_VERTICAL};

/// The linedef slope classification of `P_LoadLineDefs`, extracted
/// verbatim from the if-else ladder that set `line_t.slopetype`
/// (`p_setup.c` `P_LoadLineDefs`'s dx/dy sign test). Every linedef in the
/// map carries this value into collision and use-line math for the whole
/// level, so the same `(dx, dy)` must always classify identically.
///
/// ## Technical Details
///
/// The ladder order is load-bearing: `dx == 0` tests FIRST (so the
/// `(0, 0)` degenerate classifies `ST_VERTICAL`), then `dy == 0`, then
/// the sign of `FixedDiv(dy, dx)`. Argument order is `(dx, dy)` but the
/// division is `FixedDiv(dy, dx)` -- swapping either is a silent
/// classification flip. `FixedDiv` overflow saturates in this port (no
/// `I_Error` path), inherited from the m_fixed graduation; the ladder
/// only reads the sign, so saturation preserves classification.
///
/// ## On Calling
///
/// Arguments are raw `c_int` fixed-point deltas as stored in
/// `line_t.dx`/`dy`; never pre-normalize or reorder them. Pure
/// computation -- no state, no threading assumptions.
pub(super) fn slopetype_of(dx: c_int, dy: c_int) -> c_int
{
    if dx == 0 { ST_VERTICAL }
    else if dy == 0 { ST_HORIZONTAL }
    else if FixedDiv(dy, dx) > 0 { ST_POSITIVE }
    else { ST_NEGATIVE }
}

/// The reject-table zone-header initializer of `PadRejectArray`,
/// extracted verbatim from the `rejectpad` array expression (`p_setup.c`
/// `PadRejectArray`). These four words are the modeled `Z_Malloc` block
/// header that vanilla reads when a REJECT lump is undersized -- the
/// workaround documented in `docs/vanilla-workarounds.md` row 4 -- so
/// sight-blocking on affected WADs and demos depends on these exact
/// bytes.
///
/// ## Technical Details
///
/// `[(totallines * 4 + 3) & !3] + 24` is the zone block size field
/// (`totallines` rounded up to a 4-byte boundary plus the 24-byte
/// header), followed by the user word `0`, the `PU_LEVEL` tag `50`, and
/// the zone id `0x1d4a11`. `totallines` must be the value
/// `P_GroupLines` most recently wrote -- the caller reads it through
/// `grouplines`, the single-writer home.
///
/// ## On Calling
///
/// Pure function of `totallines`; returns the whole initializer (the
/// byte-serialization loop and the `-reject_pad_with_ff` tail fill stay
/// at the call site in `reject`). Never recompute the size field inline
/// at a second site -- drift here desyncs glass-hack-adjacent demos.
pub(super) fn reject_pad_words(total_lines: c_int) -> [u32; 4] { [((total_lines * 4 + 3) & !3) as u32 + 24, 0, 50, 0x1d4a11] }

/// The non-commercial monster filter of `P_LoadThings`, extracted
/// verbatim from the ten-type match arm (`p_setup.c` `P_LoadThings`).
/// In shareware/registered modes these Doom II monster types end the
/// thing loop early, which changes how many `P_Random` draws the spawn
/// pass makes -- the break itself stays at the call site in `loaders`
/// (the RNG-sequence semantics live there), only the type set lives
/// here.
///
/// ## Technical Details
///
/// The ten editor numbers (68, 64, 88, 89, 69, 67, 71, 65, 66, 84) are
/// the vanilla set, in the upstream match order. The historical port bug
/// turned the caller's `break` into `continue`; the pure-mirror tests in
/// `loaders` pin the break semantics and this function pins the type
/// set, together covering the regression.
///
/// ## On Calling
///
/// The argument is the byte-swapped `mapthing_t.type` field. Returns
/// whether the caller must stop spawning; the caller decides break vs
/// continue. Pure -- no state, no threading assumptions.
pub(super) fn is_noncommercial_thing(ty: i16) -> bool
{
    // The ten editor numbers in the upstream match-arm order.
    matches!(ty, 68 | 64 | 88 | 89 | 69 | 67 | 71 | 65 | 66 | 84)
}

/// The TOP/RIGHT blockmap clamp ladder of `P_GroupLines`' bounding-box
/// pass, extracted verbatim (the `>= max -> max - 1` high clamp). The
/// sector `blockbox` fields bound `P_RadiusAttack`'s block iteration, so
/// a changed clamp changes which cells a blast sweeps.
///
/// ## Technical Details
///
/// `v` is a fixed-point coordinate, `org` the blockmap origin WITH the
/// `+ MAXRADIUS` margin already folded in (the call site passes
/// `bmaporgy - MAXRADIUS` / `bmaporgx - MAXRADIUS`; two's-complement
/// addition makes `(v - org) >> MAPBLOCKSHIFT` bit-identical to the
/// upstream `(v - bmaporgy + MAXRADIUS) >> MAPBLOCKSHIFT`). The high
/// clamp has NO low clamp -- the vanilla ladder is asymmetric and is
/// preserved, never merged into a range clamp.
///
/// ## On Calling
///
/// Call per TOP and RIGHT box edge only; use [`clamp_block_low`]
/// for BOTTOM/LEFT. Pure -- no state, no threading assumptions.
pub(super) fn clamp_block(v: c_int, org: c_int, max: c_int) -> c_int
{
    let block = (v - org) >> MAPBLOCKSHIFT;
    if block >= max { max - 1 }
    else { block }
}

/// The BOTTOM/LEFT blockmap clamp ladder of `P_GroupLines`' bounding-box
/// pass, extracted verbatim (the `< 0 -> 0` low clamp, unbounded high).
/// Counterpart to [`super::clamp_block`]; see there for why the two
/// ladders stay separate.
///
/// ## Technical Details
///
/// `org` is the blockmap origin WITH the `- MAXRADIUS` margin folded in
/// (the call site passes `bmaporgy + MAXRADIUS` / `bmaporgx +
/// MAXRADIUS`). The low clamp has NO high clamp -- vanilla shape.
///
/// ## On Calling
///
/// Call per BOTTOM and LEFT box edge only. Pure -- no state, no
/// threading assumptions.
pub(super) fn clamp_block_low(v: c_int, org: c_int) -> c_int
{
    let block = (v - org) >> MAPBLOCKSHIFT;
    if block < 0 { 0 }
    else { block }
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_setup::structs::{MAXRADIUS, MAPBLOCKSHIFT, ST_HORIZONTAL, ST_NEGATIVE,
                                       ST_POSITIVE, ST_VERTICAL};

    // --- F10 wave B4a baseline vectors, retargeted post-move (F10 §2.3) ---

    /// Baseline vectors: both axis rails, both same-sign and mixed-sign
    /// diagonals, and the `(0, 0)` degenerate that pins the dx-first
    /// ladder order. This port's `FixedDiv` saturates on overflow instead
    /// of aborting (m_fixed graduation note), so no input is forbidden
    /// here beyond a zero `dx` -- which the ladder never reaches
    /// `FixedDiv` with. Retargeted onto `dtmc::slopetype_of` (pre-move
    /// commit `ede5d6d` ran the same vectors against the in-file
    /// transcription of `P_LoadLineDefs`'s ladder).
    #[test]
    fn baseline_slopetype_ladder()
    {
        assert_eq!(super::slopetype_of(0, 7), ST_VERTICAL);
        assert_eq!(super::slopetype_of(0, -7), ST_VERTICAL);
        assert_eq!(super::slopetype_of(0, 0), ST_VERTICAL);
        assert_eq!(super::slopetype_of(7, 0), ST_HORIZONTAL);
        assert_eq!(super::slopetype_of(-7, 0), ST_HORIZONTAL);
        assert_eq!(super::slopetype_of(7, 7), ST_POSITIVE);
        assert_eq!(super::slopetype_of(-7, -7), ST_POSITIVE);
        assert_eq!(super::slopetype_of(7, -7), ST_NEGATIVE);
        assert_eq!(super::slopetype_of(-7, 7), ST_NEGATIVE);
    }

    /// Baseline vectors: zero lines (no rounding), one line (the `& !3`
    /// rounding edge: `7 & !3 == 4`), the report's seven-line vector
    /// (`31 & !3 == 28`), and an exact multiple (`32`, no rounding).
    /// Retargeted onto `dtmc::reject_pad_words` (pre-move commit
    /// `ede5d6d` ran the same vectors against the in-file transcription
    /// of `PadRejectArray`'s initializer).
    #[test]
    fn baseline_reject_pad_words()
    {
        assert_eq!(super::reject_pad_words(0), [24, 0, 50, 0x1d4a11]);
        assert_eq!(super::reject_pad_words(1), [28, 0, 50, 0x1d4a11]);
        assert_eq!(super::reject_pad_words(7), [52, 0, 50, 0x1d4a11]);
        assert_eq!(super::reject_pad_words(8), [56, 0, 50, 0x1d4a11]);
    }

    /// Baseline vectors over both ladder shapes: clamp-high, clamp-low,
    /// in-range, a one-cell-up origin, and a sub-cell input where the
    /// `+/- MAXRADIUS` term visibly bumps the quotient (`MAXRADIUS` is a
    /// quarter cell, so a value in the top quarter cell rounds up through
    /// the high ladder only). The final pair feeds the SAME `v` through
    /// both ladders and pins the one-cell divergence the `+/- MAXRADIUS`
    /// asymmetry produces. Retargeted onto `dtmc::clamp_block` /
    /// `dtmc::clamp_block_low` with the margin folded into the origin
    /// argument (pre-move commit `ede5d6d` ran the same vectors against
    /// the in-file transcriptions of the `P_GroupLines` ladders, which
    /// carry the margin inline; `(v - (org -/+ MAXRADIUS)) >> MAPBLOCKSHIFT`
    /// is two's-complement-identical to the upstream expression).
    #[test]
    fn baseline_blockbox_clamp_ladders()
    {
        let high = |v: i32, org: i32, max: i32| super::clamp_block(v, org - MAXRADIUS, max);
        let low = |v: i32, org: i32| super::clamp_block_low(v, org + MAXRADIUS);

        // clamp-high: raw 3 >= max 3 -> 2.
        assert_eq!(high(4 << MAPBLOCKSHIFT, 1 << MAPBLOCKSHIFT, 3), 2);
        // in-range, nonzero origin: raw 2 stays 2.
        assert_eq!(high(3 << MAPBLOCKSHIFT, 1 << MAPBLOCKSHIFT, 16), 2);
        // + MAXRADIUS bumps the quotient: 3 << 21 is exactly three
        // quarters of a cell, so the high ladder rounds up to 1.
        assert_eq!(high(3 << 21, 0, 16), 1);
        // clamp-low: raw -1 -> 0.
        assert_eq!(low(1 << MAPBLOCKSHIFT, 1 << MAPBLOCKSHIFT), 0);
        // in-range low side: raw 2 stays 2.
        assert_eq!(low(4 << MAPBLOCKSHIFT, 1 << MAPBLOCKSHIFT), 2);
        // - MAXRADIUS keeps the quotient down: the same `3 << 21` input
        // lands at 0 through the low ladder (no bump, no clamp).
        assert_eq!(low(3 << 21, 0), 0);
        // Same `v` through both ladders diverges by one full cell.
        assert_eq!(high(1 << MAPBLOCKSHIFT << 1, 0, 16), 2);
        assert_eq!(low(1 << MAPBLOCKSHIFT << 1, 0), 1);
    }

    /// The margin folding must be arithmetic, not cosmetic:
    /// `MAXRADIUS >> MAPBLOCKSHIFT == 0`, so a vector whose low bits
    /// straddle the cell boundary is what actually observes the folded
    /// margin. Guards against a future constant change silently voiding
    /// the vectors above.
    #[test]
    fn margin_is_subcell()
    {
        assert_eq!(MAXRADIUS >> MAPBLOCKSHIFT, 0);
        assert_eq!(MAXRADIUS, 32 << 16);
    }
}
