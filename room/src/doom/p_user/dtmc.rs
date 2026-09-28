//! Demo-synchronization surface extracted from `p_user`: the two pure
//! view-bob computations behind `P_CalcHeight` -- the momentum-derived
//! amplitude clamped to `MAXBOB` and the `leveltime`-phase finetable
//! index -- whose exact integer results are pinned by the baseline
//! vectors below.

use std::ffi::c_int;

use super::state::MAXBOB;
use crate::doom::m_fixed::{fixed_t, FixedMul};
use crate::doom::tables::FINEANGLES;

/// The view-bob amplitude step of `P_CalcHeight`: square the player
/// mobj's X/Y momentum through `FixedMul`, add, halve twice with
/// `>> 2`, and clamp to `MAXBOB` -- the exact value the vanilla
/// `P_CalcHeight` stores into `player->bob` each tic
/// (`vendor/doomgeneric/p_user.c:81-88`; no named C function, hence no
/// `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The order -- `FixedMul(momx, momx) + FixedMul(momy, momy)` THEN
/// `>> 2` THEN the `MAXBOB` clamp -- is load-bearing: the fixed-point
/// squares saturate far sooner than a pre-shifted sum would, and the
/// shift-after-add reproduces vanilla's truncation. `player.bob` feeds
/// the same tic's `viewz` bob offset (a render-interpolation surface,
/// `r_interp`) and the finetable phase lookup, so any change to the
/// shift, the clamp, or the evaluation order moves every moving
/// player's viewz. Plain (non-wrapping) integer operations exactly as
/// the original body.
///
/// ## On Calling
///
/// Pure integer computation over the two momentum components: pass the
/// raw `mo->momx` / `mo->momy` fixed-point values (never pre-shifted
/// copies) and store the result into `player.bob` unmodified. Values
/// with `|mom| > 2^23` overflow the fixed-point square exactly as the
/// original inline body did -- do not add wrapping or saturating
/// "fixes". Single-threaded sim use.
pub fn bob_amplitude(momx: fixed_t, momy: fixed_t) -> fixed_t
{
    let mut bob: fixed_t = FixedMul(momx, momx) + FixedMul(momy, momy);
    bob >>= 2;
    if bob > MAXBOB { bob = MAXBOB; }
    bob
}

/// The view-bob phase step of `P_CalcHeight`: scale `leveltime` by
/// `FINEANGLES / 20` (one full sine period every 20 tics) and mask to
/// the finetable range -- the exact index the vanilla `P_CalcHeight`
/// uses for its `finesine` bob lookup (`p_user.c:101`; no named C
/// function, hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// C masks with `& FINEMASK`; the Rust port masks with
/// `& (FINEANGLES - 1)` -- the same value (`FINEMASK == FINEANGLES -
/// 1`), a pre-existing shape difference carried verbatim and pinned by
/// `baseline_bob_helper_known_vectors` below. The mask is what keeps
/// the index inside `finesine`'s 8192-entry sine family: unmasked,
/// `409 * leveltime` passes the table's 10240-entry end at
/// `leveltime = 26` and the lookup panics instead of wrapping (the
/// baseline body-level vector discriminates exactly this). Plain
/// arithmetic as in the original body: `leveltime` never goes negative
/// in a live level.
///
/// ## On Calling
///
/// Pure integer computation: pass the `leveltime` global as read at
/// the call site and index `finesine` with the returned value
/// directly. Do not widen the arithmetic -- the C original computed
/// the product in `int` and so does this helper.
pub fn bob_phase(leveltime: c_int) -> usize { ((FINEANGLES as c_int / 20 * leveltime) & (FINEANGLES as c_int - 1)) as usize }

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::d_player::{PlayerT, CF_NOMOMENTUM};
    use crate::doom::m_fixed::FRACUNIT;
    use crate::doom::p_telept::mobj_t;
    use crate::doom::p_tick::leveltime;
    use crate::doom::p_user::{onground, P_CalcHeight};
    use crate::doom::tables::{finesine, FINEMASK};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Vanilla constants pinned by value here so the vectors stay
    /// byte-stable across the graduation move: `MAXBOB` (16 pixels in
    /// fixed-point) and `VIEWHEIGHT` (41 map units).
    const MAXBOB: fixed_t = 0x100000;
    const VIEWHEIGHT: fixed_t = 41 * FRACUNIT;

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file bob math of `P_CalcHeight`
    /// BEFORE the amplitude computation extracted into
    /// `dtmc::bob_amplitude`, then retargeted -- same vectors, same
    /// results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the amplitude -> dtmc -- `player.bob` feeds the same
    /// tic's `viewz` bob offset; the exact FixedMul-square, `>> 2`, and
    /// `MAXBOB` clamp are what every moving player's viewz depends on.
    /// The vector values pass through the whole `P_CalcHeight` body:
    /// the FRACUNIT/2 and MAXBOB-clamp and zero results below were
    /// captured green against the pre-extraction body.
    #[test]
    fn baseline_bob_amplitude_vectors()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            onground = 1;
            leveltime = 0;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // `P_CalcHeight` touches only the fields assigned below,
            // the `onground` / `leveltime` statics, and the player's
            // `bob` / `viewz` / `viewheight` family.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let mp: *mut mobj_t = &mut mo;
            p.mo = mp as *mut _;

            // (FRACUNIT, FRACUNIT): FixedMul(F, F) = F, sum 2F, >> 2
            // gives FRACUNIT/2.
            (*mp).momx = FRACUNIT;
            (*mp).momy = FRACUNIT;
            P_CalcHeight(&mut p);
            assert_eq!(p.bob, FRACUNIT / 2);

            // Clamp arm: momx = 32*FRACUNIT -> FixedMul = 1 << 26, so
            // bob = 1 << 24 before the clamp -- over MAXBOB.
            (*mp).momx = 32 * FRACUNIT;
            (*mp).momy = 0;
            P_CalcHeight(&mut p);
            assert_eq!(p.bob, MAXBOB);

            // Zero momentum: zero bob.
            (*mp).momx = 0;
            (*mp).momy = 0;
            P_CalcHeight(&mut p);
            assert_eq!(p.bob, 0);

            // Leave no trace: onground's process default is what
            // `onground_defaults_to_zero` pins.
            onground = 0;
            leveltime = 0;
        }
    }

    /// Baseline contract (F10 wave B1): the vanilla double write of
    /// `viewz` in `P_CalcHeight`'s `CF_NOMOMENTUM`/airborne branch
    /// (clamp `z + VIEWHEIGHT`, then overwrite `z + viewheight` --
    /// `p_user.c:92-97`) carried VERBATIM, never fixed. Captured green
    /// against the pre-move body; re-run green after.
    #[test]
    fn baseline_viewz_double_write_quirk_vectors()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            onground = 1;
            leveltime = 0;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // the early-return branch touches only the fields assigned
            // below plus `onground` / `leveltime`.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let mp: *mut mobj_t = &mut mo;
            p.mo = mp as *mut _;
            p.cheats = CF_NOMOMENTUM;
            p.viewheight = 6 * FRACUNIT;
            (*mp).z = 10 * FRACUNIT;
            (*mp).ceilingz = 100 * FRACUNIT;

            // First write (z + VIEWHEIGHT, ceiling-clamped) must be
            // overwritten by the second (z + viewheight): the final
            // viewz uses viewheight, NOT VIEWHEIGHT.
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, 10 * FRACUNIT + 6 * FRACUNIT);
            assert_ne!(p.viewz, 10 * FRACUNIT + VIEWHEIGHT);

            // Airborne arm (onground = 0, no cheat): same branch, same
            // double write.
            p.cheats = 0;
            onground = 0;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, 10 * FRACUNIT + 6 * FRACUNIT);

            // Ground branch (cheats = 0, onground = 1): SINGLE write plus
            // a clamp that SURVIVES -- the double write is branch-A only.
            // With viewheight pushed up inside P_CalcHeight, the clamped
            // viewz lands exactly on ceilingz - 4F here; a mutant that
            // dropped the clamp (or applied the branch-A overwrite) would
            // fail this assert.
            (*mp).ceilingz = 30 * FRACUNIT;
            onground = 1;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, 30 * FRACUNIT - 4 * FRACUNIT);

            // Branch A under a low ceiling (CF_NOMOMENTUM forces the
            // double-write path): even when the FIRST write's clamp would
            // bite (clamp value 26F differs from the final write 16F),
            // the final `z + viewheight` write still wins. A mutant that
            // keeps the clamp (drops the overwrite) fails this assert.
            p.cheats = CF_NOMOMENTUM;
            p.viewheight = 6 * FRACUNIT;
            p.deltaviewheight = 0;
            onground = 1;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, 10 * FRACUNIT + 6 * FRACUNIT);
            assert_ne!(p.viewz, 30 * FRACUNIT - 4 * FRACUNIT);

            // Leave no trace (see baseline_bob_amplitude_vectors).
            onground = 0;
            leveltime = 0;
        }
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file phase math of `P_CalcHeight`
    /// BEFORE the finetable index extracted into `dtmc::bob_phase`,
    /// then retargeted -- same vectors, same results.
    ///
    /// Adjudication: the phase -> dtmc -- the `FINEANGLES / 20 *
    /// leveltime` product masked to the finetable range selects the
    /// `finesine` entry the bob offset reads. The `leveltime = 21`
    /// vector crosses the `FINEANGLES - 1` boundary and the
    /// `leveltime = 26` vector pins the mask's existence: unmasked,
    /// `409 * 26 = 10634` would index past the 10240-entry table.
    #[test]
    fn baseline_bob_phase_vectors()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            onground = 1;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // the bob path touches only the fields assigned below, the
            // `onground` / `leveltime` statics, and the player's
            // `bob` / `viewz` fields. `playerstate = 1` (PST_DEAD)
            // skips the viewheight step machine so `viewz` is exactly
            // `z + viewheight + bob`.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let mp: *mut mobj_t = &mut mo;
            p.mo = mp as *mut _;
            p.playerstate = 1;
            p.viewheight = VIEWHEIGHT;
            (*mp).z = 0;
            (*mp).ceilingz = 1000 * FRACUNIT;
            // bob = FixedMul(FRACUNIT, FRACUNIT) >> 2 = FRACUNIT/4, so
            // the offset is FixedMul(bob / 2, finesine[phase]) with
            // bob / 2 = 8192.
            (*mp).momx = FRACUNIT;
            (*mp).momy = 0;

            // leveltime 0: phase 0. This table's finesine[0] = 25 (not
            // 0), so the offset is FixedMul(8192, 25) = 3 -- pinned as
            // an expression AND as the literal sum.
            leveltime = 0;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, VIEWHEIGHT + FixedMul(8192, finesine[0]));
            assert_eq!(p.viewz, VIEWHEIGHT + 3);

            // leveltime 20: 409 * 20 = 8180, still inside the table
            // without the mask (no wrap yet).
            leveltime = 20;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, VIEWHEIGHT + FixedMul(8192, finesine[8180]));

            // leveltime 21: 409 * 21 = 8589 wraps under the
            // FINEANGLES - 1 mask to 8589 & 8191 = 397 (modulo, NOT
            // minus 8191). finesine[8589] == finesine[397] through the
            // table's 2048-entry overlap, so this vector alone does not
            // discriminate the mask -- the leveltime-26 vector below
            // does.
            leveltime = 21;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, VIEWHEIGHT + FixedMul(8192, finesine[397]));
            assert_eq!(finesine[8589], finesine[397]);

            // leveltime 26: 409 * 26 = 10634 is past the table's end
            // (10240 entries) -- an UNMASKED index would panic here.
            // The mask folds it to 10634 & 8191 = 2442.
            leveltime = 26;
            P_CalcHeight(&mut p);
            assert_eq!(p.viewz, VIEWHEIGHT + FixedMul(8192, finesine[2442]));
            assert_eq!(finesine[2442], 62558);

            // Leave no trace (see baseline_bob_amplitude_vectors).
            onground = 0;
            leveltime = 0;
        }
    }

    /// Baseline contract (F10 wave B1): the extracted helpers
    /// reproduce the exact values the body-level tests above captured
    /// against the pre-extraction `P_CalcHeight` body -- same vectors,
    /// same results. The `bob_phase` assertions additionally pin the
    /// `& (FINEANGLES - 1)` == `& FINEMASK` equivalence explicitly (C
    /// writes `&FINEMASK`).
    #[test]
    fn baseline_bob_helper_known_vectors()
    {
        // Amplitude: square-sum >> 2, MAXBOB clamp, zero.
        assert_eq!(bob_amplitude(FRACUNIT, FRACUNIT), FRACUNIT / 2);
        assert_eq!(bob_amplitude(-FRACUNIT, -FRACUNIT), FRACUNIT / 2);
        assert_eq!(bob_amplitude(32 * FRACUNIT, 0), MAXBOB);
        assert_eq!(bob_amplitude(32 * FRACUNIT, 32 * FRACUNIT), MAXBOB);
        assert_eq!(bob_amplitude(0, 0), 0);

        // Phase: FINEANGLES / 20 * leveltime, masked to the finetable
        // range. 409 * 21 wraps to 397 (modulo 8192); 409 * 26 would
        // overrun the table unmasked.
        assert_eq!(bob_phase(0), 0);
        assert_eq!(bob_phase(5), 2045);
        assert_eq!(bob_phase(20), 8180);
        assert_eq!(bob_phase(21), 397);
        assert_eq!(bob_phase(26), 2442);

        // C writes `&FINEMASK`; the helper masks with
        // `& (FINEANGLES - 1)` -- the same value.
        assert_eq!(FINEMASK, FINEANGLES as c_int - 1);
        assert_eq!(bob_phase(21), (409 * 21) & FINEMASK as usize);
    }
}
