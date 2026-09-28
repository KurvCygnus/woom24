//! The spechit overrun emulation: the stateful half of the catalog's
//! entry-1 / G1 demo-compatibility core (the `-spechit` parse, the
//! census record, and the trample writes into this module's live
//! globals), plus the whole-body regression pins. The pure cores live in
//! `dtmc`; bit-exact with upstream `SpechitOverrun`
//! (`vendor/doomgeneric/p_map.c:1391`).

#![allow(non_upper_case_globals)]

use std::ffi::{c_int, c_uint};

use crate::doom::c_ffi::{line_t, DEFAULT_SPECHIT_MAGIC};
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::m_misc::M_StrToInt;
use crate::doom::p_setup::lines;
use crate::doom::violations::{self, VanillaViolation};

use super::dtmc::{spechit_trample_addr, trample_target, TrampleTarget};
use super::sector::{crushchange, nofit};
use super::state::{numspechit, tmbbox};

/// Emulate the vanilla Doom memory-corruption behaviour when more than
/// `MAXSPECIALCROSS_ORIGINAL` (8) special lines are crossed in a single
/// move.
///
/// In the original `doom2.exe`, `spechit` was a fixed C array on the stack
/// adjacent to `tmbbox`, `crushchange`, and `nofit`. Writing past the end
/// overwrote those variables with computed addresses. This function
/// replicates those overwrites so that demos recorded with vanilla Doom
/// (which relied on the corrupted values) remain sync-compatible.
///
/// The base address defaults to `DEFAULT_SPECHIT_MAGIC` (PrBoom-plus
/// compatible) but can be overridden with the `-spechit <n>` command-line
/// argument.
///
/// ## Technical Details
///
/// The write targets and values are the chocolate/woof doom2.exe model:
/// `addr = spechit_trample_addr(ld.offset_from(lines), baseaddr)` per
/// `numspechit` routed through `trample_target`. The `-spechit` parse and
/// the lazily initialized `baseaddr` stay at this call site (stateful,
/// argv-coupled); every trigger records a
/// `VanillaViolation::SpechitOverrun` census hit BEFORE any arithmetic,
/// so even unmodelable crossing counts leave an audit trail.
///
/// ## On Calling
///
/// Private to this module: called only from `move_::pit_check_line`
/// after `numspechit` has been incremented beyond
/// `MAXSPECIALCROSS_ORIGINAL`. `ld` must be a valid, non-null pointer to
/// a `line_t` that is part of the global `lines` array so that
/// `ld.offset_from(lines)` is well-defined. Single-threaded sim only.
///
/// # Safety
///
/// Writes directly into the global `tmbbox` / `crushchange` / `nofit`
/// statics; see On Calling.
#[doc(alias = "SpechitOverrun")]
pub(super) unsafe fn spechit_overrun(ld: *mut line_t)
{
    violations::record(VanillaViolation::SpechitOverrun);
    static mut baseaddr: c_uint = 0;
    if baseaddr == 0
    {
        let p = M_CheckParmWithArgs(c"-spechit".as_ptr().cast_mut(), 1);
        if p > 0
        {
            M_StrToInt(
                *myargv.add((p + 1) as usize),
                &raw mut baseaddr as *mut c_int,
            );
        }
        else { baseaddr = DEFAULT_SPECHIT_MAGIC; }
    }
    let addr = spechit_trample_addr(ld.offset_from(lines) as usize, baseaddr as c_int);
    match trample_target(numspechit)
    {
        Some(TrampleTarget::Tmbbox(i)) => tmbbox[i] = addr,
        Some(TrampleTarget::Crushchange) => crushchange = addr,
        Some(TrampleTarget::Nofit) => nofit = addr,
        None => eprintln!(
            "SpechitOverrun: Warning: unable to emulate an overrun where numspechit={}",
            numspechit as c_int
        ),
    }
}

//* The doc-bearing test functions below use the module-root shims where a
//* name was renamed (the bounds test drives `PIT_CheckLine` through the
//* shim, proving the shim itself resolves) and the new internal names for
//* in-module items.

#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, c_void};
    use std::sync::Mutex;

    use crate::doom::c_ffi::{line_t, mobj_t, sector_t, vertex_t, DEFAULT_SPECHIT_MAGIC};
    use crate::doom::m_argv::{myargc, myargv};
    use crate::doom::m_bbox::BBox;
    use crate::doom::m_fixed::{fixed_t, FRACUNIT};
    use crate::doom::p_map::PIT_CheckLine;
    use crate::doom::p_map::consts::{MAXSPECIALCROSS, MF_MISSILE};
    use crate::doom::p_map::sector::{crushchange, nofit};
    use crate::doom::p_map::state::{
        ceilingline, numspechit, spechit, tmbbox, tmceilingz, tmdropoffz, tmfloorz, tmthing,
    };
    use crate::doom::p_maputl::{lowfloor, openbottom, openrange, opentop};
    use crate::doom::p_setup::lines;
    use crate::doom::violations::{self, VanillaViolation};

    use super::spechit_overrun;

    static LOCK: Mutex<()> = Mutex::new(());

    /// Drives `PIT_CheckLine` with a synthetic two-sided special line while
    /// `numspechit` is already at/after the array bound, and asserts the push
    /// stays inside `spechit` (the words directly after the array act as the
    /// sentinel). This pins the woof/dsda-style bound on the spechit store:
    /// the pre-fix port wrote `spechit[numspechit]` unguarded, trampling
    /// whatever .bss symbol the linker had placed after the array (observed
    /// in the browser as the ticdup=0 freeze). Shared fixture: both spechit
    /// tests below drive exactly this construction; do not duplicate it.
    fn drive_pit_check_line_past_spechit_bound()
    {
        unsafe
        {
            // Snapshot every global PIT_CheckLine touches.
            let spechit_before = std::ptr::addr_of!(spechit).read();
            let numspechit_before = numspechit;
            let tmbbox_before = tmbbox;
            let tmthing_before = tmthing;
            let tmceilingz_before = tmceilingz;
            let tmfloorz_before = tmfloorz;
            let tmdropoffz_before = tmdropoffz;
            let ceilingline_before = ceilingline;
            let (opentop_b, openbottom_b, openrange_b, lowfloor_b) =
                (opentop, openbottom, openrange, lowfloor);
            let (myargc_b, myargv_b) = (myargc, myargv);

            // Synthetic map: a vertical two-sided special line at x=0.
            let mut line: Box<line_t> = Box::new(std::mem::zeroed());
            let mut v1: Box<vertex_t> = Box::new(std::mem::zeroed());
            let mut v2: Box<vertex_t> = Box::new(std::mem::zeroed());
            let mut front: Box<sector_t> = Box::new(std::mem::zeroed());
            let mut back: Box<sector_t> = Box::new(std::mem::zeroed());
            let mut mo: Box<mobj_t> = Box::new(std::mem::zeroed());

            v1.x = 0;
            v1.y = -16 * FRACUNIT;
            v2.x = 0;
            v2.y = 16 * FRACUNIT;
            line.v1 = &mut *v1;
            line.v2 = &mut *v2;
            line.dx = 0;
            line.dy = 32 * FRACUNIT;
            line.slopetype = 1; // ST_VERTICAL
            line.sidenum = [0, 1];
            line.bbox = [
                16 * FRACUNIT,  // BBox::TOP
                -16 * FRACUNIT, // BBox::BOTTOM
                0,              // BBox::LEFT
                0,              // BBox::RIGHT
            ];
            line.special = 1;
            front.ceilingheight = 128 * FRACUNIT;
            front.floorheight = 0;
            back.ceilingheight = 128 * FRACUNIT;
            back.floorheight = 0;
            line.frontsector = &mut *front as *mut sector_t as *mut c_void;
            line.backsector = &mut *back as *mut sector_t as *mut c_void;

            mo.flags = MF_MISSILE; // skips the blocking-flag checks
            tmthing = &mut *mo;

            // Viewing box straddles the line (LEFT < 0 < RIGHT) so
            // P_BoxOnLineSide returns -1 and the special is recorded.
            tmbbox[BBox::TOP] = 8 * FRACUNIT;
            tmbbox[BBox::BOTTOM] = -8 * FRACUNIT;
            tmbbox[BBox::RIGHT] = 8 * FRACUNIT;
            tmbbox[BBox::LEFT] = -8 * FRACUNIT;

            tmceilingz = 256 * FRACUNIT;
            tmfloorz = -64 * FRACUNIT;
            tmdropoffz = -64 * FRACUNIT;
            myargc = 0;
            myargv = std::ptr::null_mut();

            let after_end = std::ptr::addr_of!(spechit) as *const c_int;
            let sentinel_before: [c_int; 8] = core::array::from_fn(|i| after_end.add(20 + i).read());

            // Push while the counter is already at and past the bound: the
            // pre-fix build wrote spechit[20] and spechit[25] here.
            for expected in [20, 25]
            {
                numspechit = expected;
                let rc = PIT_CheckLine(&mut *line);
                assert_eq!(rc, 1, "line must not block");
                assert_eq!(numspechit, expected + 1, "counter must still advance");
            }

            let sentinel_after: [c_int; 8] = core::array::from_fn(|i| after_end.add(20 + i).read());
            assert_eq!(
                sentinel_after, sentinel_before,
                "spechit overflow wrote past the array (tmbbox/static neighbors clobbered)"
            );
            for i in 0..MAXSPECIALCROSS
            {
                assert!(
                    spechit_before[i].is_null() || spechit_before[i] == spechit[i],
                    "in-bounds spechit slot {} unexpectedly rewritten",
                    i
                );
            }

            // Restore.
            std::ptr::addr_of_mut!(spechit).write(spechit_before);
            numspechit = numspechit_before;
            tmbbox = tmbbox_before;
            tmthing = tmthing_before;
            tmceilingz = tmceilingz_before;
            tmfloorz = tmfloorz_before;
            tmdropoffz = tmdropoffz_before;
            ceilingline = ceilingline_before;
            opentop = opentop_b;
            openbottom = openbottom_b;
            openrange = openrange_b;
            lowfloor = lowfloor_b;
            myargc = myargc_b;
            myargv = myargv_b;
        }
    }

    #[test]
    fn pit_check_line_push_stays_in_bounds_beyond_the_array()
    {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drive_pit_check_line_past_spechit_bound();
    }

    #[test]
    fn spechit_emulation_records_census_hit()
    {
        // Census window: besides the module lock below, the crate-wide
        // census lock must be held so no sibling census test's reset_all()
        // zeroes the counter mid-window (see violations.rs).
        let _census = violations::CENSUS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        violations::reset_all();
        // Drive PIT_CheckLine across the emulation threshold exactly as the
        // existing bounds test does (shared fixture, no duplicated
        // construction); the emulation trigger inside spechit_overrun must
        // leave a census hit behind.
        drive_pit_check_line_past_spechit_bound();
        assert!(
            violations::hits(VanillaViolation::SpechitOverrun) > 0,
            "crossing the spechit emulation threshold must record a census hit"
        );
    }

    /// Baseline fixture for the addr/case-table dtmc extraction (F10 §2.3,
    /// wave B2b): the vectors below were written and run against the
    /// monolithic-file `SpechitOverrun` body BEFORE that body moved to
    /// `p_map/spechit.rs` + `p_map/dtmc.rs`, then retargeted after the
    /// move -- same vectors, same results. The pure-core unit vectors
    /// live in `dtmc::tests`; these drives cover the stateful call site.
    ///
    /// Drives the real `spechit_overrun` with the global `lines` base
    /// pointed at a synthetic 4-line array (so `ld.offset_from(lines)` is
    /// exactly `line_index`) and `numspechit` set to `count`, with
    /// sentinel values pre-set in every trample-target slot; returns the
    /// full target-slot state so the caller can assert which slot took
    /// the write. Snapshot/restores every global it touches. The caller
    /// must hold the module `LOCK`.
    unsafe fn drive_spechit_overrun(count: c_int, line_index: usize) -> ([fixed_t; 4], c_int, c_int)
    {
        // Snapshot everything the emulation (or this drive) touches.
        let tmbbox_before = tmbbox;
        let crushchange_before = crushchange;
        let nofit_before = nofit;
        let numspechit_before = numspechit;
        let lines_before = lines;
        let (myargc_b, myargv_b) = (myargc, myargv);

        // `baseaddr` (a function-local static inside spechit_overrun) must
        // hold the DEFAULT_SPECHIT_MAGIC default: nothing in the suite
        // passes `-spechit`, so keep argv empty and let the default arm
        // run. First-call initialization is idempotent for this suite.
        myargc = 0;
        myargv = std::ptr::null_mut();

        let mut fake_lines: [line_t; 4] = std::array::from_fn(|_| unsafe { std::mem::zeroed() });

        // Sentinels in every trample-target slot: "which slot took the
        // write" is exactly what the case-table baseline pins.
        tmbbox = [0x1111_0000, 0x2222_0000, 0x3333_0000, 0x4444_0000];
        crushchange = 0x5555_0000;
        nofit = 0x6666_0000;

        lines = fake_lines.as_mut_ptr();
        numspechit = count;
        spechit_overrun(&mut fake_lines[line_index]);
        let result = (tmbbox, crushchange, nofit);

        // Restore.
        tmbbox = tmbbox_before;
        crushchange = crushchange_before;
        nofit = nofit_before;
        numspechit = numspechit_before;
        lines = lines_before;
        myargc = myargc_b;
        myargv = myargv_b;
        result
    }

    /// Baseline (F10 §2.3): the overrun address formula is
    /// `baseaddr + (ld - lines) * 0x3e` with `baseaddr` defaulting to
    /// `DEFAULT_SPECHIT_MAGIC` (`0x01C09C98`). Verified here against the
    /// real body with synthetic linedef offsets 0, 1, and 2 before the
    /// extraction into `spechit_trample_addr`.
    #[test]
    fn baseline_spechit_addr_formula_uses_linedef_offset()
    {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let magic = DEFAULT_SPECHIT_MAGIC as c_int;
            // numspechit 9 lands the write in tmbbox[0]; the address is
            // baseaddr + (ld - lines) * 0x3e for the synthetic offset.
            let (tmb, cc, nf) = drive_spechit_overrun(9, 1);
            assert_eq!(tmb[0], magic + 0x3e, "offset 1 -> MAGIC + 1*0x3e");
            assert_eq!(tmb[1], 0x2222_0000, "only tmbbox[0] is the count-9 target");
            assert_eq!(cc, 0x5555_0000, "crushchange untouched at count 9");
            assert_eq!(nf, 0x6666_0000, "nofit untouched at count 9");
            let (tmb, _, _) = drive_spechit_overrun(9, 2);
            assert_eq!(tmb[0], magic + 2 * 0x3e, "offset 2 -> MAGIC + 2*0x3e");
            let (tmb, _, _) = drive_spechit_overrun(9, 0);
            assert_eq!(tmb[0], magic, "offset 0 -> MAGIC exactly");
        }
    }

    /// Baseline (F10 §2.3): the case table follows the chocolate/woof
    /// doom2.exe `.bss` order -- NOT dsda's 13/14 swap: 9..=12 ->
    /// `tmbbox[0..=3]`, 13 -> `crushchange`, 14 -> `nofit`; counts
    /// outside 9..=14 write nothing (the warning arm). Each written
    /// address is `MAGIC + (ld - lines) * 0x3e` for the synthetic offset.
    #[test]
    fn baseline_spechit_case_table_maps_chocolate_targets()
    {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let magic = DEFAULT_SPECHIT_MAGIC as c_int;
            let sentinels = [0x1111_0000, 0x2222_0000, 0x3333_0000, 0x4444_0000];
            for slot in 0..4usize
            {
                let count = 9 + slot as c_int;
                let (tmb, cc, nf) = drive_spechit_overrun(count, slot);
                for i in 0..4usize
                {
                    let expected = if i == slot
                    {
                        magic + (slot as c_int) * 0x3e
                    }
                    else { sentinels[i] };
                    assert_eq!(tmb[i], expected, "count {count}: tmbbox[{i}]");
                }
                assert_eq!(cc, 0x5555_0000, "crushchange untouched at count {count}");
                assert_eq!(nf, 0x6666_0000, "nofit untouched at count {count}");
            }
            let (tmb, cc, nf) = drive_spechit_overrun(13, 0);
            assert_eq!(cc, magic, "count 13 -> crushchange (chocolate, not dsda's nofit)");
            assert_eq!(tmb, sentinels, "tmbbox untouched at count 13");
            assert_eq!(nf, 0x6666_0000, "nofit untouched at count 13");
            let (tmb, cc, nf) = drive_spechit_overrun(14, 0);
            assert_eq!(nf, magic, "count 14 -> nofit (chocolate, not dsda's crushchange)");
            assert_eq!(tmb, sentinels, "tmbbox untouched at count 14");
            assert_eq!(cc, 0x5555_0000, "crushchange untouched at count 14");
            for outside in [8, 15]
            {
                let (tmb, cc, nf) = drive_spechit_overrun(outside, 0);
                assert_eq!(tmb, sentinels, "count {outside} writes no tmbbox slot");
                assert_eq!(cc, 0x5555_0000, "count {outside} leaves crushchange");
                assert_eq!(nf, 0x6666_0000, "count {outside} leaves nofit");
            }
        }
    }
}
