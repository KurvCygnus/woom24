//! The pure map-query geometry family: sidedef/sector resolution and the
//! surrounding-height/light/tag sweeps every mover thinker consumes when it
//! picks its travel targets -- bit-exact with the utility half of
//! `vendor/doomgeneric/p_spec.c` (`getSide` .. `P_FindMinSurroundingLight`).

#![allow(non_snake_case)]

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::{line_t, sector_t, side_t, LinedefFlag};
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_setup::{numsectors, sectors, sides};
use crate::i_error;

use super::consts::MAX_ADJOINING_SECTORS;

/// Returns a pointer to the `side_t` on a given side of a line bounding a sector.
///
/// Mirrors `getSide` in `p_spec.c`.
///
/// ## Technical Details
///
/// Pure map query: `currentSector` indexes the global `sectors` array, `line`
/// indexes the sector's own line list (not the global `lines` array), and the
/// line's `sidenum[side]` selects the entry in the global `sides` array. The
/// returned pointer feeds the texture/offset reads of the floor/ceiling
/// movers, so the sidenum indexing order is demo-visible through the targets
/// those movers pick.
///
/// ## On Calling
///
/// `side`: 0 for front, 1 for back. The result is valid as long as the map
/// data is loaded. Out-of-range indices are not modeled and never occur in
/// loaded-map data (verbatim upstream arithmetic).
///
/// # Safety
///
/// `currentSector` must index a loaded sector, `line` an existing entry of
/// its line list, and `side` the value 0 or 1.
#[doc(alias = "getSide")]
#[export_name = "getSide"]
pub unsafe extern "C" fn get_side(currentSector: c_int, line: c_int, side: c_int) -> *mut side_t
{
    let line_ptr = *(*sectors.offset(currentSector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    let side_idx = (*line_ptr).sidenum[side as usize];
    sides.offset(side_idx as isize)
}

/// Returns a pointer to the `sector_t` on a given side of a line bounding a sector.
///
/// Mirrors `getSector` in `p_spec.c`.
///
/// ## Technical Details
///
/// Pure map query through the same line/sidenum walk as [`get_side`], ending
/// at the side's `sector` pointer. The floor/ceiling/plat action loops use
/// the back-side result as the sector whose heights a mover targets, so a
/// wrong side resolution desyncs any demo that crosses the line.
///
/// ## On Calling
///
/// `side`: 0 for front sector, 1 for back sector. Callers must guard against
/// the back sector being null (one-sided line) before dereferencing.
///
/// # Safety
///
/// Same bounds contract as [`get_side`].
#[doc(alias = "getSector")]
#[export_name = "getSector"]
pub unsafe extern "C" fn get_sector(
    currentSector: c_int,
    line: c_int,
    side: c_int,
) -> *mut sector_t
{
    let line_ptr = *(*sectors.offset(currentSector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    let side_idx = (*line_ptr).sidenum[side as usize];
    (*sides.offset(side_idx as isize)).sector
}

/// Returns non-zero if the given line on a sector's boundary is two-sided.
///
/// Mirrors `twoSided` in `p_spec.c`.
///
/// ## Technical Details
///
/// Returns the `ML_TWOSIDED` flag value (4) masked out of the line's flags,
/// or zero for one-sided lines. The raw flag value (not a normalized
/// boolean) is the contract -- callers compare against zero only.
///
/// ## On Calling
///
/// `sector` indexes the global `sectors` array, `line` the sector's own line
/// list. Pure query over loaded map data.
///
/// # Safety
///
/// `sector` must index a loaded sector and `line` an existing entry of its
/// line list.
#[doc(alias = "twoSided")]
#[export_name = "twoSided"]
pub unsafe extern "C" fn two_sided(sector: c_int, line: c_int) -> c_int
{
    let line_ptr = *(*sectors.offset(sector as isize))
        .lines
        .offset(line as isize) as *mut line_t;
    ((*line_ptr).flags as c_int) & (LinedefFlag::TWOSIDED as c_int)
}

/// Returns the sector on the opposite side of `line` from `sec`, or null.
///
/// Mirrors `getNextSector` in `p_spec.c`.
///
/// ## Technical Details
///
/// The neighbor oracle every surrounding sweep below is built on: one-sided
/// lines (no `ML_TWOSIDED` flag) return the null arm; otherwise the front or
/// back sector that is not `sec` is the neighbor. The null arm is what keeps
/// the `P_Find*` loops honest on map borders, and the donut effect relies on
/// it returning the exact opposite sector.
///
/// ## On Calling
///
/// `line` must be a loaded linedef, `sec` one of its two sectors. Pure query.
///
/// # Safety
///
/// `line` must be a valid, non-null linedef pointer; `sec` must be a valid
/// sector pointer that is one of `line`'s two sectors.
#[doc(alias = "getNextSector")]
#[export_name = "getNextSector"]
pub unsafe extern "C" fn get_next_sector(
    line: *mut line_t,
    sec: *mut sector_t,
) -> *mut sector_t
{
    if ((*line).flags as c_int) & (LinedefFlag::TWOSIDED as c_int) == 0
    {
        return ptr::null_mut();
    }
    if (*line).frontsector == sec as *mut c_void
    {
        return (*line).backsector as *mut sector_t;
    }
    (*line).frontsector as *mut sector_t
}

/// Returns the lowest floor height among all sectors neighbouring `sec`.
///
/// Mirrors `P_FindLowestFloorSurrounding` in `p_spec.c`.
///
/// ## Technical Details
///
/// Walks every line bounding `sec`, finds the sector on the other side via
/// [`get_next_sector`], and returns the minimum floor height seen. The
/// initial value is the sector's own floor height (not a sentinel), so a
/// sector with no two-sided lines returns its own floor -- the value a
/// `lowerFloor`-family mover targets, hence demo-observable.
///
/// ## On Calling
///
/// `sec` must be a loaded sector. Heights are fixed-point 16.16 (`c_int`).
///
/// # Safety
///
/// `sec` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindLowestFloorSurrounding")]
#[export_name = "P_FindLowestFloorSurrounding"]
pub unsafe extern "C" fn lowest_floor_surrounding(sec: *mut sector_t) -> c_int
{
    let mut floor = (*sec).floorheight;
    for i in 0..(*sec).linecount
    {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = get_next_sector(check, sec);
        if other.is_null()
        {
            continue;
        }
        if (*other).floorheight < floor
        {
            floor = (*other).floorheight;
        }
    }
    floor
}

/// Returns the highest floor height among all sectors neighbouring `sec`.
///
/// Mirrors `P_FindHighestFloorSurrounding` in `p_spec.c`.
///
/// ## Technical Details
///
/// Same sweep as [`lowest_floor_surrounding`] with the `-500 * FRACUNIT`
/// C sentinel initial value (effectively negative infinity for practical
/// map heights): a sector with no two-sided neighbors returns the sentinel
/// itself, and movers then raise to that impossible height only in
/// deliberately malformed maps. Baseline vectors pin the sentinel.
///
/// ## On Calling
///
/// `sec` must be a loaded sector. Heights are fixed-point 16.16.
///
/// # Safety
///
/// `sec` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindHighestFloorSurrounding")]
#[export_name = "P_FindHighestFloorSurrounding"]
pub unsafe extern "C" fn highest_floor_surrounding(sec: *mut sector_t) -> c_int
{
    let mut floor = -500 * FRACUNIT;
    for i in 0..(*sec).linecount
    {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = get_next_sector(check, sec);
        if other.is_null()
        {
            continue;
        }
        if (*other).floorheight > floor
        {
            floor = (*other).floorheight;
        }
    }
    floor
}

/// Returns the next floor height above `currentheight` among neighbouring sectors.
///
/// Mirrors `P_FindNextHighestFloor` in `p_spec.c`, including the vanilla
/// adjoining-sector overrun emulation (see
/// `docs/vanilla-workarounds.md` entry 13).
///
/// ## Technical Details
///
/// Collects every neighbouring floor above `currentheight` into a fixed
/// `MAX_ADJOINING_SECTORS + 2` array and returns the minimum, i.e. the
/// lowest floor still above the current one -- the target of
/// `raiseFloor`-family movers and stair builders. The overrun arms are the
/// demo surface on dense maps: at 21 stored neighbors
/// (`h == MAX_ADJOINING_SECTORS + 1`) the vanilla write lands on the stack
/// slot of `height` itself, raising the filter threshold for later
/// neighbors (emulated verbatim); at `h == MAX_ADJOINING_SECTORS + 2`
/// vanilla would crash, here `i_error!` fires with chocolate's message.
///
/// ## On Calling
///
/// `sec` must be a loaded sector, `currentheight` a fixed-point height.
/// The 23-qualifying-neighbor drive in `tests` pins that the shadow write
/// (not an array-bounds "fix") keeps later neighbors out of the crash arm.
///
/// # Safety
///
/// `sec` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindNextHighestFloor")]
#[export_name = "P_FindNextHighestFloor"]
pub unsafe extern "C" fn next_highest_floor(sec: *mut sector_t, currentheight: c_int) -> c_int
{
    let mut height = currentheight;
    let mut heightlist: [c_int; MAX_ADJOINING_SECTORS + 2] = [0; MAX_ADJOINING_SECTORS + 2];
    let mut h = 0;

    for i in 0..(*sec).linecount
    {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = get_next_sector(check, sec);
        if other.is_null()
        {
            continue;
        }
        if (*other).floorheight > height
        {
            if h == MAX_ADJOINING_SECTORS + 1
            {
                height = (*other).floorheight;
            }
            else if h == MAX_ADJOINING_SECTORS + 2
            {
                i_error!("Sector with more than 22 adjoining sectors. Vanilla will crash here");
            }
            heightlist[h] = (*other).floorheight;
            h += 1;
        }
    }

    if h == 0
    {
        return currentheight;
    }

    let mut min = heightlist[0];
    for i in 1..h
    {
        if heightlist[i] < min
        {
            min = heightlist[i];
        }
    }
    min
}

/// Returns the lowest ceiling height among all sectors neighbouring `sec`.
///
/// Mirrors `P_FindLowestCeilingSurrounding` in `p_spec.c`.
///
/// ## Technical Details
///
/// Same neighbor sweep with the `INT_MAX` sentinel initial value: a sector
/// with no two-sided neighbors returns `INT_MAX`, which ceiling movers
/// interpret as "no obstruction". Heights are fixed-point 16.16.
///
/// ## On Calling
///
/// `sec` must be a loaded sector.
///
/// # Safety
///
/// `sec` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindLowestCeilingSurrounding")]
#[export_name = "P_FindLowestCeilingSurrounding"]
pub unsafe extern "C" fn lowest_ceiling_surrounding(sec: *mut sector_t) -> c_int
{
    let mut height = c_int::MAX;
    for i in 0..(*sec).linecount
    {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = get_next_sector(check, sec);
        if other.is_null()
        {
            continue;
        }
        if (*other).ceilingheight < height
        {
            height = (*other).ceilingheight;
        }
    }
    height
}

/// Returns the highest ceiling height among all sectors neighbouring `sec`.
///
/// Mirrors `P_FindHighestCeilingSurrounding` in `p_spec.c`.
///
/// ## Technical Details
///
/// Same neighbor sweep with a 0 sentinel: a sector with no two-sided
/// neighbors returns 0. Heights are fixed-point 16.16.
///
/// ## On Calling
///
/// `sec` must be a loaded sector.
///
/// # Safety
///
/// `sec` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindHighestCeilingSurrounding")]
#[export_name = "P_FindHighestCeilingSurrounding"]
pub unsafe extern "C" fn highest_ceiling_surrounding(sec: *mut sector_t) -> c_int
{
    let mut height = 0;
    for i in 0..(*sec).linecount
    {
        let check = *(*sec).lines.offset(i as isize) as *mut line_t;
        let other = get_next_sector(check, sec);
        if other.is_null()
        {
            continue;
        }
        if (*other).ceilingheight > height
        {
            height = (*other).ceilingheight;
        }
    }
    height
}

/// Finds the next sector whose tag matches `line->tag`, searching from `start + 1`.
///
/// Mirrors `P_FindSectorFromLineTag` in `p_spec.c`.
///
/// ## Technical Details
///
/// The iterator protocol of every tag-based action loop (donut, door,
/// floor, ceiling, plats): each round passes the previous hit as `start`,
/// the scan resumes at `start + 1`, and -1 terminates the caller's
/// `while`-loop. The `start + 1` off-by-one shape is the contract -- a
/// scan from `start` itself would loop forever on the first hit.
///
/// ## On Calling
///
/// Start with `start = -1` to see sector 0. Pure query over the global
/// `sectors` array and `numsectors` count.
///
/// # Safety
///
/// `line` must be a valid, non-null linedef pointer.
#[doc(alias = "P_FindSectorFromLineTag")]
#[export_name = "P_FindSectorFromLineTag"]
pub unsafe extern "C" fn sector_from_line_tag(line: *mut line_t, start: c_int) -> c_int
{
    for i in (start + 1)..numsectors
    {
        if (*sectors.offset(i as isize)).tag == (*line).tag
        {
            return i;
        }
    }
    -1
}

/// Returns the minimum light level among all sectors neighbouring `sector`,
/// clamped to `max` from above.
///
/// Mirrors `P_FindMinSurroundingLight` in `p_spec.c`.
///
/// ## Technical Details
///
/// Same neighbor sweep over raw `i16` light levels (valid Doom range
/// 0-255, compared as `c_int`): the smallest neighbor light wins, and if
/// every neighbor is at or above `max`, `max` is returned. The strobe and
/// flash spawners in p_lights target this value, so it is demo-visible.
///
/// ## On Calling
///
/// `sector` must be a loaded sector, `max` the caller's current light
/// bound (p_lights passes the sector's own light level).
///
/// # Safety
///
/// `sector` must be a valid, non-null pointer to a loaded sector.
#[doc(alias = "P_FindMinSurroundingLight")]
#[export_name = "P_FindMinSurroundingLight"]
pub unsafe extern "C" fn min_surrounding_light(sector: *mut sector_t, max: c_int) -> c_int
{
    let mut min = max;
    for i in 0..(*sector).linecount
    {
        let line = *(*sector).lines.offset(i as isize) as *mut line_t;
        let check = get_next_sector(line, sector);
        if check.is_null()
        {
            continue;
        }
        if ((*check).lightlevel as c_int) < min
        {
            min = (*check).lightlevel as c_int;
        }
    }
    min
}

#[cfg(test)]
mod tests
{
    use std::ffi::{c_short, c_void};

    use crate::doom::c_ffi::{line_t, sector_t, side_t, LinedefFlag};
    use crate::doom::m_fixed::FRACUNIT;
    use crate::doom::p_setup::{numsectors, sectors, sides};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    use super::*;

    // --- F10 wave B3b dtmc baseline vectors (retargeted post-move; F10 spec §2.3) ---

    //* The vectors below were written and run GREEN against the pre-move
    //* in-file bodies in commit `3841018` (they called `getSide`,
    //* `getSector`, `twoSided`, `getNextSector`, and the `P_Find*`
    //* originals inside `p_spec.rs`), then retargeted onto these renamed
    //* functions -- same vectors, same results. The synthetic-sector
    //* fixture is unchanged from the baseline commit.

    /// Synthetic sector graph for the geometry drives: a center sector
    /// (index 0) ringed by one line per neighbor, plus full
    /// snapshot/restore of the `p_setup` map globals the family reads
    /// (`sectors`, `sides`, `numsectors`). Line `i` of the center is
    /// two-sided, `front` = center, `back` = neighbor `i`, with
    /// `sidenum = [2i, 2i + 1]`. `one_sided` adds that many flag-less
    /// lines after the ring lines (their `get_next_sector` arm is the
    /// null return the report names). Caller sets heights/light/tags on
    /// the returned vectors before installing. (Field names avoid the
    /// module statics this test module imports.)
    struct GeometryFixture
    {
        sec_storage: Vec<sector_t>,
        side_storage: Vec<side_t>,
        line_storage: Vec<line_t>,
        center_line_ptrs: Vec<*mut c_void>,
    }

    impl GeometryFixture
    {
        fn build(ring_lines: usize, one_sided: usize) -> Self
        {
            let total_lines = ring_lines + one_sided;
            let mut sec_list = vec![unsafe { std::mem::zeroed::<sector_t>() }; ring_lines + 1];
            let mut side_list = vec![unsafe { std::mem::zeroed::<side_t>() }; total_lines * 2];
            let mut line_list = vec![unsafe { std::mem::zeroed::<line_t>() }; total_lines];
            let mut center_line_ptrs: Vec<*mut c_void> = Vec::with_capacity(total_lines);

            let center = &mut sec_list[0] as *mut sector_t;
            for i in 0..total_lines
            {
                let line = &mut line_list[i] as *mut line_t;
                if i < ring_lines
                {
                    let back = &mut sec_list[i + 1] as *mut sector_t;
                    unsafe
                    {
                        (*line).flags = LinedefFlag::TWOSIDED as c_short;
                        (*line).sidenum = [(i * 2) as c_short, (i * 2 + 1) as c_short];
                        (*line).frontsector = center as *mut c_void;
                        (*line).backsector = back as *mut c_void;
                        side_list[i * 2].sector = center;
                        side_list[i * 2 + 1].sector = back;
                    }
                }
                else
                {
                    // One-sided: no TWOSIDED flag, no back sector, the
                    // front sidedef still resolves through `sides`.
                    unsafe
                    {
                        (*line).sidenum = [(i * 2) as c_short, -1];
                        (*line).frontsector = center as *mut c_void;
                        side_list[i * 2].sector = center;
                    }
                }
                center_line_ptrs.push(line as *mut c_void);
            }

            unsafe
            {
                (*center).linecount = total_lines as c_int;
                (*center).lines = center_line_ptrs.as_mut_ptr();
            }

            GeometryFixture
            {
                sec_storage: sec_list,
                side_storage: side_list,
                line_storage: line_list,
                center_line_ptrs,
            }
        }

        /// Center sector pointer (index 0).
        fn center(&mut self) -> *mut sector_t
        {
            &mut self.sec_storage[0] as *mut sector_t
        }

        /// Neighbor sector pointer (ring index `i`, sector `i + 1`).
        fn neighbor(&mut self, i: usize) -> *mut sector_t
        {
            &mut self.sec_storage[i + 1] as *mut sector_t
        }

        /// Line pointer (center line `i`).
        fn line(&mut self, i: usize) -> *mut line_t
        {
            &mut self.line_storage[i] as *mut line_t
        }

        //* Installs the fixture into the p_setup globals the geometry
        //* family reads and returns the prior values for
        //* [`GeometryFixture::restore`]. The writes are plain
        //* static-mut assignments (no references); the reads use
        //* addr_of! so no shared-reference lint fires.
        unsafe fn install(&mut self) -> (*mut sector_t, *mut side_t, c_int)
        {
            let sectors_before = std::ptr::addr_of!(sectors).read();
            let sides_before = std::ptr::addr_of!(sides).read();
            let numsectors_before = std::ptr::addr_of!(numsectors).read();
            sectors = self.sec_storage.as_mut_ptr();
            sides = self.side_storage.as_mut_ptr();
            numsectors = self.sec_storage.len() as c_int;
            (sectors_before, sides_before, numsectors_before)
        }

        unsafe fn restore(before: (*mut sector_t, *mut side_t, c_int))
        {
            sectors = before.0;
            sides = before.1;
            numsectors = before.2;
        }
    }

    /// Baseline vectors (F10 §2.3, wave B3b) for the pure map
    /// queries: `two_sided` (upstream `twoSided`), `get_side`,
    /// `get_sector`, `get_next_sector` (including the null arm for
    /// one-sided lines), and the sentinel-initialised surrounding
    /// sweeps `lowest_floor_surrounding`,
    /// `highest_floor_surrounding` (the `-500 * FRACUNIT`
    /// no-neighbor sentinel), `lowest_ceiling_surrounding` (the
    /// `INT_MAX` sentinel), `highest_ceiling_surrounding` (the 0
    /// sentinel), and `min_surrounding_light` (the `max` clamp).
    /// Written against the pre-move bodies (commit `3841018`),
    /// retargeted -- same vectors, same results.
    #[test]
    fn baseline_geometry_query_vectors()
    {
        let _engine = ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        unsafe
        {
            let mut fx = GeometryFixture::build(1, 1);

            // Neighbor + center geometry.
            fx.sec_storage[0].floorheight = 16 * FRACUNIT;
            fx.sec_storage[0].ceilingheight = 160 * FRACUNIT;
            fx.sec_storage[1].floorheight = -8 * FRACUNIT;
            fx.sec_storage[1].ceilingheight = 128 * FRACUNIT;
            fx.sec_storage[1].lightlevel = 96;

            let before = fx.install();

            // two_sided: the two-sided ring line carries the flag bit,
            // the one-sided line (index 1) carries none.
            assert_eq!(two_sided(0, 0), LinedefFlag::TWOSIDED as c_int);
            assert_eq!(two_sided(0, 1), 0);

            // get_side resolves the line's sidenum through `sides`:
            // line 0 has sidenum [0, 1], so side 0/1 land on the first
            // two side_t entries.
            assert_eq!(get_side(0, 0, 0), fx.side_storage.as_mut_ptr());
            assert_eq!(get_side(0, 0, 1), fx.side_storage.as_mut_ptr().offset(1));

            // get_sector resolves the side's sector: front = center,
            // back = the ring neighbor.
            assert_eq!(get_sector(0, 0, 0), fx.center());
            assert_eq!(get_sector(0, 0, 1), fx.neighbor(0));

            // get_next_sector: opposite side of the line from `sec`;
            // one-sided (flag-less) lines return the null arm.
            assert_eq!(get_next_sector(fx.line(0), fx.center()), fx.neighbor(0));
            assert_eq!(get_next_sector(fx.line(0), fx.neighbor(0)), fx.center());
            assert!(get_next_sector(fx.line(1), fx.center()).is_null());

            // Surrounding sweeps: the two-sided neighbor participates,
            // the one-sided line is skipped, and a center with only the
            // one-sided line sees its sentinel initial value.
            assert_eq!(lowest_floor_surrounding(fx.center()), -8 * FRACUNIT);
            assert_eq!(highest_floor_surrounding(fx.center()), -8 * FRACUNIT);
            assert_eq!(lowest_ceiling_surrounding(fx.center()), 128 * FRACUNIT);
            assert_eq!(highest_ceiling_surrounding(fx.center()), 128 * FRACUNIT);
            assert_eq!(min_surrounding_light(fx.center(), 160), 96);
            // The clamp: no neighbor below `max` returns `max` itself.
            assert_eq!(min_surrounding_light(fx.center(), 96), 96);

            GeometryFixture::restore(before);

            // Sentinel drives: an isolated center whose only line is
            // one-sided never sees a neighbor, so each sweep returns
            // its initial value (own floor, -500 * FRACUNIT, INT_MAX,
            // 0).
            let mut iso = GeometryFixture::build(0, 1);
            iso.sec_storage[0].floorheight = 42 * FRACUNIT;
            let iso_before = iso.install();
            assert_eq!(lowest_floor_surrounding(iso.center()), 42 * FRACUNIT);
            assert_eq!(highest_floor_surrounding(iso.center()), -500 * FRACUNIT);
            assert_eq!(lowest_ceiling_surrounding(iso.center()), c_int::MAX);
            assert_eq!(highest_ceiling_surrounding(iso.center()), 0);
            GeometryFixture::restore(iso_before);
        }
    }

    /// Baseline vectors for `next_highest_floor` (upstream
    /// `P_FindNextHighestFloor`): the vanilla adjoining-sector overrun
    /// boundary. Drives with 20, 21 and 22 qualifying neighbors all
    /// return the true minimum. The 23-line drive is the one that makes
    /// the `h == MAX_ADJOINING_SECTORS + 1` stack-shadow arm
    /// OBSERVABLE (the vanilla write that lands on `height` instead of
    /// the array): the 22nd qualifying neighbor raises the filter
    /// threshold, so the 23rd (between `currentheight` and the 22nd's
    /// height) never qualifies -- with the arm, the sweep returns the
    /// min of 22; without it (an "array-bounds fix") the 23rd would
    /// qualify, reach `h == MAX_ADJOINING_SECTORS + 2` and hit the
    /// `i_error!` (the vanilla crash; never driven here, it aborts).
    /// Also pins the no-higher-neighbor return of `currentheight`.
    /// Written against the pre-move body (commit `3841018`,
    /// mutation-checked there: disabling the shadow arm aborts this
    /// drive through the `i_error!` path), retargeted -- same vectors,
    /// same results.
    #[test]
    fn baseline_next_highest_floor_20_21_22_overrun_boundary()
    {
        let _engine = ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        unsafe
        {
            let current = 50 * FRACUNIT;

            // 20/21/22 qualifying neighbors: all stored (the 22nd
            // enters the shadow arm, harmless as the last sweep
            // entry), minimum lands at a different index per drive so
            // the min-selection sweep is load-bearing every time.
            for (count, expect_min_at) in [(20usize, 7usize), (21, 0), (22, 21)]
            {
                let mut fx = GeometryFixture::build(count, 0);
                for i in 0..count
                {
                    fx.sec_storage[i + 1].floorheight =
                        current + ((i as c_int * 7 + 13) % (count as c_int * 7) + 6) * FRACUNIT;
                }
                fx.sec_storage[expect_min_at + 1].floorheight = current + FRACUNIT;

                let before = fx.install();
                assert_eq!(
                    next_highest_floor(fx.center(), current),
                    current + FRACUNIT,
                    "drive with {count} qualifying neighbors"
                );
                GeometryFixture::restore(before);
            }

            // 23 lines, 23rd shadow-guarded: neighbors 0..=20 sit at
            // +10..=30, the 22nd at +100 (enters the shadow arm and
            // raises the filter threshold), the 23rd at +50 (above
            // `currentheight`, below the raised threshold -> never
            // qualifies). Result: min of the 22-entry list. A port
            // that "fixed" the overrun into a plain bounds-checked
            // store would qualify the 23rd, reach h == 22 and abort.
            let mut fx = GeometryFixture::build(23, 0);
            for i in 0..21
            {
                fx.sec_storage[i + 1].floorheight = current + (10 + i as c_int) * FRACUNIT;
            }
            fx.sec_storage[22].floorheight = current + 100 * FRACUNIT;
            fx.sec_storage[23].floorheight = current + 50 * FRACUNIT;

            let before = fx.install();
            assert_eq!(next_highest_floor(fx.center(), current), current + 10 * FRACUNIT);
            GeometryFixture::restore(before);

            // No neighbor above `currentheight`: the qualifying set is
            // empty and the function returns `currentheight` itself.
            let mut fx = GeometryFixture::build(2, 0);
            fx.sec_storage[1].floorheight = current;
            fx.sec_storage[2].floorheight = current - FRACUNIT;
            let before = fx.install();
            assert_eq!(next_highest_floor(fx.center(), current), current);
            GeometryFixture::restore(before);
        }
    }

    /// Baseline vectors for `sector_from_line_tag` (upstream
    /// `P_FindSectorFromLineTag`): the tag scan starts at `start + 1`
    /// (so `start = -1` sees sector 0), resumes after the previous hit
    /// (the donut/door action loops' iterator protocol), and returns
    /// -1 once the scan runs off the end. Written against the pre-move
    /// body (commit `3841018`), retargeted -- same vectors, same
    /// results.
    #[test]
    fn baseline_sector_from_line_tag_scan_vectors()
    {
        let _engine = ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        unsafe
        {
            let mut fx = GeometryFixture::build(2, 0);
            fx.sec_storage[0].tag = 7;
            fx.sec_storage[1].tag = 9;
            fx.sec_storage[2].tag = 7;
            let mut tag_line: Box<line_t> = Box::new(std::mem::zeroed());
            tag_line.tag = 7;
            let tag_ptr = &mut *tag_line as *mut line_t;

            let before = fx.install();
            assert_eq!(sector_from_line_tag(tag_ptr, -1), 0);
            assert_eq!(sector_from_line_tag(tag_ptr, 0), 2);
            assert_eq!(sector_from_line_tag(tag_ptr, 2), -1);
            (*tag_ptr).tag = 42;
            assert_eq!(sector_from_line_tag(tag_ptr, -1), -1);
            GeometryFixture::restore(before);
        }
    }
}
