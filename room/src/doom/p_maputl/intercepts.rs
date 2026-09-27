//! The intercept pipeline and its state: the `intercepts` /
//! `intercept_p` / `trace` / `earlyout` statics, the two `PIT_Add*`
//! collection callbacks, the vanilla overrun emulators
//! (`InterceptsMemoryOverrun` / `InterceptsOverrun` -- the catalog
//! entry-2 core), `P_TraverseIntercepts`, and `P_PathTraverse` --
//! bit-exact with the intercept half of
//! `vendor/doomgeneric/p_maputl.c`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{c_int, c_uint, c_void};
use std::ptr;

use crate::doom::c_ffi::{
    divline_t, intercept_t, intercept_t_d, line_t, mobj_t, MAPBLOCKSHIFT, MAPBLOCKSIZE, MAPBTOFRAC,
};
use crate::doom::m_fixed::{fixed_t, FixedDiv, FixedMul, FRACBITS, FRACUNIT};
use crate::doom::violations::{self, VanillaViolation};

use super::blockmap::{P_BlockLinesIterator, P_BlockThingsIterator};
use super::geometry::{
    P_InterceptVector, P_MakeDivline, P_PointOnDivlineSide, P_PointOnLineSide,
};
use super::opening::{lowfloor, openbottom, opentop, openrange};

/// The original Doom intercepts array capacity (128 entries).
///
/// When the number of intercepts exceeds this value the engine emulates the
/// original buffer-overrun behaviour via `InterceptsOverrun`. Matches
/// `MAXINTERCEPTS` in `p_local.h` (Chocolate Doom source).
pub const MAXINTERCEPTS_ORIGINAL: usize = 128;

/// Extended intercepts capacity used by this port.
///
/// 61 extra entries are allocated so that overrun emulation can write into the
/// correct adjacent variables without touching unrelated memory.
pub const MAXINTERCEPTS: usize = MAXINTERCEPTS_ORIGINAL + 61;

/// `P_PathTraverse` flag: collect line intercepts while traversing.
pub const PT_ADDLINES: c_int = 1;

/// `P_PathTraverse` flag: collect thing intercepts while traversing.
pub const PT_ADDTHINGS: c_int = 2;

/// `P_PathTraverse` flag: stop early at the first solid (one-sided) line hit.
pub const PT_EARLYOUT: c_int = 4;

/// Array of intercept records accumulated during a `P_PathTraverse` call.
///
/// Each entry records where the trace ray hit a line or thing along with the
/// parametric fraction `t` along the ray. The array is indexed via
/// `intercept_p`. Entries beyond index `MAXINTERCEPTS_ORIGINAL` trigger
/// `InterceptsOverrun` to emulate vanilla memory layout behaviour.
#[no_mangle]
pub static mut intercepts: [intercept_t; MAXINTERCEPTS] = [intercept_t {
    frac: 0,
    isaline: 0,
    d: intercept_t_d {
        thing: ptr::null_mut(),
    },
}; MAXINTERCEPTS];

/// Write cursor into the `intercepts` array.
///
/// Initialised to `&intercepts[0]` at the start of each `P_PathTraverse`
/// call and advanced by `PIT_AddLineIntercepts` / `PIT_AddThingIntercepts`.
/// `P_TraverseIntercepts` uses `intercept_p - intercepts` to know how many
/// entries were collected.
#[no_mangle]
pub static mut intercept_p: *mut intercept_t = ptr::null_mut();

/// The current path-traverse ray, set by `P_PathTraverse`.
///
/// `PIT_AddLineIntercepts` and `PIT_AddThingIntercepts` read this to
/// determine whether each line/thing crosses the ray.
#[no_mangle]
pub static mut trace: divline_t = divline_t {
    x: 0,
    y: 0,
    dx: 0,
    dy: 0,
};

/// Whether `P_PathTraverse` should exit immediately upon hitting a solid
/// one-sided line. Set from the `PT_EARLYOUT` flag passed to `P_PathTraverse`.
static mut earlyout: c_int = 0;

/// Write a single `c_int` value into a specific adjacent memory location to
/// emulate a vanilla Doom buffer overrun past the `intercepts` array.
///
/// Doom's original engine laid out its BSS segment in a fixed order; when
/// `intercepts[]` overflowed, writes would corrupt the adjacent variables in
/// that order. This function reproduces that behaviour for demo compatibility.
/// Each `skip!` / `write_i32!` / `write_i16_arr!` call accounts for one
/// variable in the original layout (offsets noted in the comments).
///
/// Based on PrBoom-plus research by Andrey Budko (entryway).
///
/// # Safety
/// Writes directly through raw pointers to module-level globals. Caller must
/// ensure `location` is within the range covered by the table (0..=~300 bytes).
#[allow(unused_assignments)]
unsafe fn InterceptsMemoryOverrun(location: c_int, value: c_int)
{
    let mut offset: usize = 0;
    let loc = location as usize;

    macro_rules! skip
    {
        ($len:expr) =>
        {
            if offset + $len > loc
            {
                return;
            }
            offset += $len;
        };
    }
    macro_rules! write_i32
    {
        ($len:expr, $ptr:expr) =>
        {
            if offset + $len > loc
            {
                let index = (loc - offset) / 4;
                ($ptr as *mut c_int).add(index).write(value);
                return;
            }
            offset += $len;
        };
    }
    macro_rules! write_i16_arr
    {
        ($len:expr, $ptr:expr) =>
        {
            if offset + $len > loc
            {
                let index = (loc - offset) / 2;
                let p = ($ptr as *mut _ as *mut u8) as *mut i16;
                p.add(index).write((value & 0xffff) as i16);
                p.add(index + 1).write(((value >> 16) & 0xffff) as i16);
                return;
            }
            offset += $len;
        };
    }

    skip!(4); // 0
    skip!(4); // 1 earlyout
    skip!(4); // 2 intercept_p
    write_i32!(4, std::ptr::addr_of_mut!(lowfloor)); // 3 lowfloor
    write_i32!(4, std::ptr::addr_of_mut!(openbottom)); // 4 openbottom
    write_i32!(4, std::ptr::addr_of_mut!(opentop)); // 5 opentop
    write_i32!(4, std::ptr::addr_of_mut!(openrange)); // 6 openrange
    skip!(4); // 7
    skip!(120); // 8 activeplats
    skip!(8); // 9
    write_i32!(4, std::ptr::addr_of_mut!(crate::doom::p_pspr::bulletslope)); // 10 bulletslope
    skip!(4); // 11 swingx
    skip!(4); // 12 swingy
    skip!(4); // 13
    write_i16_arr!(
        40,
        std::ptr::addr_of_mut!(crate::doom::p_setup::playerstarts)
    ); // 14 playerstarts
    skip!(4); // 15 blocklinks
    write_i32!(4, std::ptr::addr_of_mut!(crate::doom::p_setup::bmapwidth)); // 16 bmapwidth
    skip!(4); // 17 blockmap
    write_i32!(4, std::ptr::addr_of_mut!(crate::doom::p_setup::bmaporgx)); // 18 bmaporgx
    write_i32!(4, std::ptr::addr_of_mut!(crate::doom::p_setup::bmaporgy)); // 19 bmaporgy
    skip!(4); // 20 blockmaplump
    write_i32!(4, std::ptr::addr_of_mut!(crate::doom::p_setup::bmapheight)); // 21 bmapheight
}

/// Emulate the vanilla Doom `intercepts[]` buffer overrun for demo fidelity.
///
/// When `num_intercepts` exceeds `MAXINTERCEPTS_ORIGINAL` (128), the original
/// engine would write past the array into adjacent globals. This function
/// calls `InterceptsMemoryOverrun` three times (for `frac`, `isaline`, and
/// `d.thing`) to reproduce those writes.
///
/// # Safety
/// `intercept` must be a valid, non-null pointer to an initialised
/// `intercept_t`.
unsafe fn InterceptsOverrun(num_intercepts: c_int, intercept: *mut intercept_t)
{
    if num_intercepts <= MAXINTERCEPTS_ORIGINAL as c_int
    {
        return;
    }
    violations::record(VanillaViolation::InterceptsOverrun);
    let location = (num_intercepts - MAXINTERCEPTS_ORIGINAL as c_int - 1) * 12;
    InterceptsMemoryOverrun(location, (*intercept).frac);
    InterceptsMemoryOverrun(location + 4, (*intercept).isaline);
    InterceptsMemoryOverrun(location + 8, (*intercept).d.thing as usize as c_int);
}

/// Intercept callback: record where the current `trace` ray crosses linedef `ld`.
///
/// Called by `P_BlockLinesIterator` during `P_PathTraverse`. If the ray
/// crosses `ld` (endpoints on opposite sides) and the intersection fraction
/// `t >= 0`, appends an `intercept_t` to `intercepts` and advances
/// `intercept_p`. Uses `P_PointOnDivlineSide` for long traces and
/// `P_PointOnLineSide` for short ones to avoid fixed-point precision issues.
///
/// If `PT_EARLYOUT` is set and the line has no back sector, returns 0 to halt
/// traversal immediately.
///
/// Returns 1 (continue) or 0 (stop).
///
/// Matches `PIT_AddLineIntercepts` in `p_maputl.c`.
///
/// # Safety
/// `ld` must be a valid, non-null pointer to an initialised `line_t`. The
/// global `trace`, `intercepts`, and `intercept_p` must be set up by a
/// preceding `P_PathTraverse` call.
#[no_mangle]
pub extern "C" fn PIT_AddLineIntercepts(ld: *mut line_t) -> c_uint
{
    unsafe
    {
        let ld = &*ld;
        let (s1, s2) = if trace.dx > FRACUNIT * 16
            || trace.dy > FRACUNIT * 16
            || trace.dx < -FRACUNIT * 16
            || trace.dy < -FRACUNIT * 16
        {
            (
                P_PointOnDivlineSide(
                    (*ld.v1).x,
                    (*ld.v1).y,
                    &raw const trace as *const _ as *mut _,
                ),
                P_PointOnDivlineSide(
                    (*ld.v2).x,
                    (*ld.v2).y,
                    &raw const trace as *const _ as *mut _,
                ),
            )
        }
        else
        {
            (
                P_PointOnLineSide(trace.x, trace.y, ld as *const _ as *mut _),
                P_PointOnLineSide(
                    trace.x + trace.dx,
                    trace.y + trace.dy,
                    ld as *const _ as *mut _,
                ),
            )
        };
        if s1 == s2
        {
            return 1;
        }
        let mut dl = divline_t {
            x: 0,
            y: 0,
            dx: 0,
            dy: 0,
        };
        P_MakeDivline(ld as *const _ as *mut _, &mut dl);
        let frac = P_InterceptVector(&raw const trace as *const _ as *mut _, &mut dl);
        if frac < 0
        {
            return 1;
        }
        if earlyout != 0 && frac < FRACUNIT && ld.backsector.is_null()
        {
            return 0;
        }
        (*intercept_p).frac = frac;
        (*intercept_p).isaline = 1;
        (*intercept_p).d.line = ld as *const line_t as *mut line_t;
        InterceptsOverrun(
            intercept_p.offset_from(std::ptr::addr_of_mut!(intercepts[0])) as c_int,
            intercept_p,
        );
        intercept_p = intercept_p.offset(1);
        1
    }
}

/// Intercept callback: record where the current `trace` ray crosses thing `thing`.
///
/// Called by `P_BlockThingsIterator` during `P_PathTraverse`. Approximates
/// the thing as a diagonal bounding segment (corner-to-corner across its
/// radius), choosing orientation based on `trace` direction sign. If the ray
/// crosses the segment and `t >= 0`, appends an `intercept_t` (with
/// `isaline = 0`) and advances `intercept_p`.
///
/// Returns 1 (continue) always.
///
/// Matches `PIT_AddThingIntercepts` in `p_maputl.c`.
///
/// # Safety
/// `thing` must be a valid, non-null pointer to an initialised `mobj_t`. The
/// global `trace`, `intercepts`, and `intercept_p` must be set up by a
/// preceding `P_PathTraverse` call.
#[no_mangle]
pub extern "C" fn PIT_AddThingIntercepts(thing: *mut mobj_t) -> c_uint
{
    unsafe
    {
        let thing = &*thing;
        // C: tracepositive = (trace.dx ^ trace.dy)>0;
        let tracepositive = (trace.dx ^ trace.dy) > 0;
        let (x1, y1, x2, y2) = if tracepositive
        {
            (
                thing.x - thing.radius,
                thing.y + thing.radius,
                thing.x + thing.radius,
                thing.y - thing.radius,
            )
        }
        else
        {
            (
                thing.x - thing.radius,
                thing.y - thing.radius,
                thing.x + thing.radius,
                thing.y + thing.radius,
            )
        };
        let s1 = P_PointOnDivlineSide(x1, y1, &raw const trace as *const _ as *mut _);
        let s2 = P_PointOnDivlineSide(x2, y2, &raw const trace as *const _ as *mut _);
        if s1 == s2
        {
            return 1;
        }
        let mut dl = divline_t {
            x: x1,
            y: y1,
            dx: x2 - x1,
            dy: y2 - y1,
        };
        let frac = P_InterceptVector(&raw const trace as *const _ as *mut _, &mut dl);
        if frac < 0
        {
            return 1;
        }
        (*intercept_p).frac = frac;
        (*intercept_p).isaline = 0;
        (*intercept_p).d.thing = thing as *const _ as *mut c_void;
        InterceptsOverrun(
            intercept_p.offset_from(std::ptr::addr_of_mut!(intercepts[0])) as c_int,
            intercept_p,
        );
        intercept_p = intercept_p.offset(1);
        1
    }
}

/// Walk the collected intercepts in nearest-first order, calling `func` for each.
///
/// After `P_PathTraverse` has filled `intercepts[0..intercept_p]`, this
/// function iterates them in ascending `frac` order (selection sort: picks the
/// minimum each pass, marks it with `c_int::MAX` after processing). Stops
/// when the nearest remaining intercept has `frac > maxfrac`.
///
/// Returns 1 (true) if `func` accepted every intercept, 0 (false) if `func`
/// returned 0 for any intercept.
///
/// Matches `P_TraverseIntercepts` in `p_maputl.c`.
///
/// # Safety
/// `func` must be a valid function pointer. `intercepts` and `intercept_p`
/// must be in a consistent state as set by `P_PathTraverse` / the `PIT_Add*`
/// callbacks.
#[no_mangle]
pub extern "C" fn P_TraverseIntercepts(
    func: Option<unsafe extern "C" fn(*mut intercept_t) -> c_uint>,
    maxfrac: fixed_t,
) -> c_uint
{
    unsafe
    {
        let count = if intercept_p.is_null()
        {
            0
        }
        else
        {
            intercept_p.offset_from(std::ptr::addr_of_mut!(intercepts[0])) as c_int
        };
        for _ in 0..count
        {
            let mut dist = c_int::MAX;
            let mut in_ptr: *mut intercept_t = ptr::null_mut();
            let mut scan = std::ptr::addr_of_mut!(intercepts[0]);
            while scan < intercept_p
            {
                if (*scan).frac < dist
                {
                    dist = (*scan).frac;
                    in_ptr = scan;
                }
                scan = scan.offset(1);
            }
            if dist > maxfrac
            {
                return 1;
            }
            if func.unwrap()(in_ptr) == 0
            {
                return 0;
            }
            (*in_ptr).frac = c_int::MAX;
        }
        1
    }
}

/// Trace a ray from `(x1, y1)` to `(x2, y2)` through the blockmap.
///
/// Steps through each blockmap cell the ray crosses (up to 64 cells), calling
/// `P_BlockLinesIterator` and/or `P_BlockThingsIterator` (depending on
/// `flags`) to collect intercepts via `PIT_AddLineIntercepts` /
/// `PIT_AddThingIntercepts`. After traversal, calls `P_TraverseIntercepts`
/// with `trav` to process all hits in nearest-first order.
///
/// The origin is nudged by `FRACUNIT` if it falls exactly on a blockmap grid
/// line to avoid ambiguous cell assignments.
///
/// Returns 1 if the traverser accepted all intercepts, 0 if it rejected one
/// (signalling a hit / early stop).
///
/// Matches `P_PathTraverse` in `p_maputl.c`.
///
/// # Safety
/// The global map data from `p_setup` must be loaded. `trav` must be a valid
/// function pointer or `None`. All coordinates are in fixed-point map units.
#[no_mangle]
pub extern "C" fn P_PathTraverse(
    x1: fixed_t,
    y1: fixed_t,
    x2: fixed_t,
    y2: fixed_t,
    flags: c_int,
    trav: Option<unsafe extern "C" fn(*mut intercept_t) -> c_uint>,
) -> c_uint
{
    unsafe
    {
        earlyout = flags & PT_EARLYOUT;
        crate::doom::r_main::validcount = crate::doom::r_main::validcount.wrapping_add(1);
        intercept_p = std::ptr::addr_of_mut!(intercepts[0]);
        let mut x1 = x1;
        let mut y1 = y1;
        if ((x1 - crate::doom::p_setup::bmaporgx) & (MAPBLOCKSIZE - 1)) == 0
        {
            x1 += FRACUNIT;
        }
        if ((y1 - crate::doom::p_setup::bmaporgy) & (MAPBLOCKSIZE - 1)) == 0
        {
            y1 += FRACUNIT;
        }
        trace.x = x1;
        trace.y = y1;
        trace.dx = x2 - x1;
        trace.dy = y2 - y1;
        let x1 = x1 - crate::doom::p_setup::bmaporgx;
        let y1 = y1 - crate::doom::p_setup::bmaporgy;
        let xt1 = x1 >> MAPBLOCKSHIFT;
        let yt1 = y1 >> MAPBLOCKSHIFT;
        let x2 = x2 - crate::doom::p_setup::bmaporgx;
        let y2 = y2 - crate::doom::p_setup::bmaporgy;
        let xt2 = x2 >> MAPBLOCKSHIFT;
        let yt2 = y2 >> MAPBLOCKSHIFT;
        let (mapxstep, partial, ystep) = if xt2 > xt1
        {
            (
                1,
                FRACUNIT - ((x1 >> MAPBTOFRAC) & (FRACUNIT - 1)),
                FixedDiv(y2 - y1, (x2 - x1).wrapping_abs()),
            )
        }
        else if xt2 < xt1
        {
            (
                -1,
                (x1 >> MAPBTOFRAC) & (FRACUNIT - 1),
                FixedDiv(y2 - y1, (x2 - x1).wrapping_abs()),
            )
        }
        else
        {
            (0, FRACUNIT, 256 * FRACUNIT)
        };
        let mut yintercept = (y1 >> MAPBTOFRAC) + FixedMul(partial, ystep);
        let (mapystep, partial, xstep) = if yt2 > yt1
        {
            (
                1,
                FRACUNIT - ((y1 >> MAPBTOFRAC) & (FRACUNIT - 1)),
                FixedDiv(x2 - x1, (y2 - y1).wrapping_abs()),
            )
        }
        else if yt2 < yt1
        {
            (
                -1,
                (y1 >> MAPBTOFRAC) & (FRACUNIT - 1),
                FixedDiv(x2 - x1, (y2 - y1).wrapping_abs()),
            )
        }
        else
        {
            (0, FRACUNIT, 256 * FRACUNIT)
        };
        let mut xintercept = (x1 >> MAPBTOFRAC) + FixedMul(partial, xstep);
        let mut mapx = xt1;
        let mut mapy = yt1;
        for _ in 0..64
        {
            if flags & PT_ADDLINES != 0
                && P_BlockLinesIterator(
                    mapx,
                    mapy,
                    Some(PIT_AddLineIntercepts as unsafe extern "C" fn(*mut line_t) -> c_uint),
                ) == 0
            {
                return 0;
            }
            if flags & PT_ADDTHINGS != 0
                && P_BlockThingsIterator(
                    mapx,
                    mapy,
                    Some(PIT_AddThingIntercepts as unsafe extern "C" fn(*mut mobj_t) -> c_uint),
                ) == 0
            {
                return 0;
            }
            if mapx == xt2 && mapy == yt2
            {
                break;
            }
            if (yintercept >> FRACBITS) == mapy
            {
                yintercept += ystep;
                mapx += mapxstep;
            }
            else if (xintercept >> FRACBITS) == mapx
            {
                xintercept += xstep;
                mapy += mapystep;
            }
        }
        P_TraverseIntercepts(trav, FRACUNIT)
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::ffi::c_int;
    use std::sync::Mutex;

    use crate::doom::violations::{self, VanillaViolation};

    static LOCK: Mutex<()> = Mutex::new(());

    /// Baseline contract (F10 wave B2a): this layout pin was written and
    /// run GREEN against the pre-move `InterceptsMemoryOverrun` body
    /// BEFORE the graduation split, then moved here -- same vectors,
    /// same results (F10 §2.3).
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the layout table IS the surface -- the byte-faithful
    /// vanilla `.bss` order; a reorder desyncs overrun demos.
    ///
    /// Driving `InterceptsMemoryOverrun` with the byte offset of each
    /// writable slot must write exactly the mapped live global, and
    /// driving it with the offset of a `skip!` slot must write nothing.
    /// Offsets are the cumulative step lengths of the walk (4 bytes per
    /// `c_int` neighbor, 120 for activeplats, 40 for playerstarts).
    /// Statics are read into locals before asserting: `assert_eq!` on
    /// a `static mut` directly would create a shared reference.
    #[test]
    fn intercepts_memory_overrun_layout_walks_vanilla_bss_order()
    {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            // Snapshot every global the emulation can trample.
            let (lf0, ob0, ot0, or0) = (lowfloor, openbottom, opentop, openrange);
            let bs0 = crate::doom::p_pspr::bulletslope;
            let ps_starts = std::ptr::addr_of!(crate::doom::p_setup::playerstarts).read();
            let (bw0, bx0, by0, bh0) = (
                crate::doom::p_setup::bmapwidth,
                crate::doom::p_setup::bmaporgx,
                crate::doom::p_setup::bmaporgy,
                crate::doom::p_setup::bmapheight,
            );
            let pp = std::ptr::addr_of!(crate::doom::p_setup::playerstarts) as *const i16;
            let (ps0, ps1) = (*pp.add(0), *pp.add(1));

            // Slot 3 -> lowfloor (slots 0-2 skipped: 12 bytes).
            InterceptsMemoryOverrun(12, 3);
            let (lf, ob) = (lowfloor, openbottom);
            assert_eq!(lf, 3, "slot 3 must write lowfloor");
            assert_eq!(ob, ob0, "slot 3 must not write openbottom");
            lowfloor = lf0;

            // Slot 4 -> openbottom.
            InterceptsMemoryOverrun(16, 4);
            let (ob, lf) = (openbottom, lowfloor);
            assert_eq!(ob, 4, "slot 4 must write openbottom");
            assert_eq!(lf, lf0, "slot 4 must not write lowfloor");
            openbottom = ob0;

            // Slot 5 -> opentop.
            InterceptsMemoryOverrun(20, 5);
            let ot = opentop;
            assert_eq!(ot, 5, "slot 5 must write opentop");
            opentop = ot0;

            // Slot 6 -> openrange.
            InterceptsMemoryOverrun(24, 6);
            let orr = openrange;
            assert_eq!(orr, 6, "slot 6 must write openrange");
            openrange = or0;

            // Slot 10 -> bulletslope (slots 7-9 skipped: 132 bytes).
            InterceptsMemoryOverrun(160, 10);
            let bs = crate::doom::p_pspr::bulletslope;
            assert_eq!(bs, 10, "slot 10 must write bulletslope");
            crate::doom::p_pspr::bulletslope = bs0;

            // Slot 14 -> playerstarts as 16-bit pairs (40-byte slot).
            InterceptsMemoryOverrun(176, 0x1357_9BDF);
            let (p0, p1) = (*pp.add(0), *pp.add(1));
            assert_eq!(p0, (0x1357_9BDF & 0xffff) as i16, "slot 14 low half");
            assert_eq!(p1, ((0x1357_9BDF >> 16) & 0xffff) as i16, "slot 14 high half");
            crate::doom::p_setup::playerstarts = ps_starts;

            // Slot 16 -> bmapwidth (slot 15 skipped).
            InterceptsMemoryOverrun(220, 16);
            let bw = crate::doom::p_setup::bmapwidth;
            assert_eq!(bw, 16, "slot 16 must write bmapwidth");
            crate::doom::p_setup::bmapwidth = bw0;

            // Slot 18 -> bmaporgx (slot 17 skipped).
            InterceptsMemoryOverrun(228, 18);
            let bx = crate::doom::p_setup::bmaporgx;
            assert_eq!(bx, 18, "slot 18 must write bmaporgx");
            crate::doom::p_setup::bmaporgx = bx0;

            // Slot 19 -> bmaporgy.
            InterceptsMemoryOverrun(232, 19);
            let by = crate::doom::p_setup::bmaporgy;
            assert_eq!(by, 19, "slot 19 must write bmaporgy");
            crate::doom::p_setup::bmaporgy = by0;

            // Slot 21 -> bmapheight (slot 20 skipped).
            InterceptsMemoryOverrun(240, 21);
            let bh = crate::doom::p_setup::bmapheight;
            assert_eq!(bh, 21, "slot 21 must write bmapheight");
            crate::doom::p_setup::bmapheight = bh0;

            // Skip slots write nothing: 4 (earlyout), 8 (intercept_p),
            // 32 (unknown), 100 (activeplats), 156 (unknown), 164
            // (swingx), 168 (swingy), 172 (unknown), 216 (blocklinks),
            // 224 (blockmap), 236 (blockmaplump).
            for loc in [4, 8, 32, 100, 156, 164, 168, 172, 216, 224, 236]
            {
                InterceptsMemoryOverrun(loc, 0x51AB_C0DE);
                let (lf, ob, ot, orr) = (lowfloor, openbottom, opentop, openrange);
                let bs = crate::doom::p_pspr::bulletslope;
                let (bw, bx, by, bh) = (
                    crate::doom::p_setup::bmapwidth,
                    crate::doom::p_setup::bmaporgx,
                    crate::doom::p_setup::bmaporgy,
                    crate::doom::p_setup::bmapheight,
                );
                let (p0, p1) = (*pp.add(0), *pp.add(1));
                assert_eq!(lf, lf0, "skip slot {loc} wrote lowfloor");
                assert_eq!(ob, ob0, "skip slot {loc} wrote openbottom");
                assert_eq!(ot, ot0, "skip slot {loc} wrote opentop");
                assert_eq!(orr, or0, "skip slot {loc} wrote openrange");
                assert_eq!(bs, bs0, "skip slot {loc} wrote bulletslope");
                assert_eq!(bw, bw0, "skip slot {loc} wrote bmapwidth");
                assert_eq!(bx, bx0, "skip slot {loc} wrote bmaporgx");
                assert_eq!(by, by0, "skip slot {loc} wrote bmaporgy");
                assert_eq!(bh, bh0, "skip slot {loc} wrote bmapheight");
                assert_eq!(p0, ps0, "skip slot {loc} wrote playerstarts[0].x");
                assert_eq!(p1, ps1, "skip slot {loc} wrote playerstarts[0].y");
            }
        }
    }

    /// Baseline contract (F10 wave B2a): written and run GREEN against
    /// the pre-move `InterceptsOverrun` body BEFORE the split, then
    /// moved here -- same vectors, same results (F10 §2.3).
    ///
    /// Trigger pin: within the 128-entry bound the emulator is a
    /// strict no-op (no census, no writes); the first entry past it
    /// records an `InterceptsOverrun` census hit (location
    /// `(129 - 128 - 1) * 12 == 0` lands inside skipped slot 0, so the
    /// census hit is the observable). Census window mirrors the p_map
    /// spechit census test: `CENSUS_TEST_LOCK` plus `reset_all`.
    #[test]
    fn intercepts_overrun_trigger_records_census_hit()
    {
        let _census = violations::CENSUS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        violations::reset_all();
        unsafe
        {
            let mut ic: intercept_t = std::mem::zeroed();
            let ic_ptr = std::ptr::addr_of_mut!(ic);
            InterceptsOverrun(MAXINTERCEPTS_ORIGINAL as c_int, ic_ptr);
            InterceptsOverrun(MAXINTERCEPTS_ORIGINAL as c_int - 5, ic_ptr);
            assert_eq!(
                violations::hits(VanillaViolation::InterceptsOverrun),
                0,
                "within the vanilla bound the overrun emulator must be a no-op"
            );
            InterceptsOverrun(MAXINTERCEPTS_ORIGINAL as c_int + 1, ic_ptr);
            assert!(
                violations::hits(VanillaViolation::InterceptsOverrun) > 0,
                "past the vanilla bound the overrun emulator must record a census hit"
            );
        }
    }
}
