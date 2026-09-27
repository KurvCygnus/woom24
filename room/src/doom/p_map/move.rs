//! The movement/collision core: the teleport stomp, the blockmap
//! position check with its linedef/thing callbacks, `try_move` with the
//! bounded spechit drain, and the post-sector-move height clip --
//! bit-exact with the movement half of `vendor/doomgeneric/p_map.c`.
//! The guarded spechit store and the emulation trigger (catalog entry 1
//! / G1) live in `pit_check_line` here.

use std::ffi::{c_int, c_uint};
use std::ptr;

use crate::doom::c_ffi::{
    line_t, mobj_t, sector_t, subsector_t, LinedefFlag, MAPBLOCKSHIFT,
};
use crate::doom::g_game::gamemap;
use crate::doom::info::MobjInfo;
use crate::doom::m_bbox::BBox;
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::{P_DamageMobj, P_TouchSpecialThing};
use crate::doom::p_maputl::{
    lowfloor, openbottom, opentop, P_BlockLinesIterator, P_BlockThingsIterator, P_BoxOnLineSide,
    P_LineOpening, P_PointOnLineSide, P_SetThingPosition, P_UnsetThingPosition,
};
use crate::doom::p_mobj::P_SetMobjState;
use crate::doom::p_setup::{bmaporgx, bmaporgy, lines};
use crate::doom::p_spec::P_CrossSpecialLine;
use crate::doom::r_main::{validcount, R_PointInSubsector};

use super::consts::{
    DEH_DEFAULT_SPECIES_INFIGHTING, MAXRADIUS, MAXSPECIALCROSS, MAXSPECIALCROSS_ORIGINAL,
    MF_DROPOFF, MF_FLOAT, MF_MISSILE, MF_NOCLIP, MF_PICKUP, MF_SHOOTABLE, MF_SKULLFLY,
    MF_SOLID, MF_SPECIAL, MF_TELEPORT, MT_BRUISER, MT_KNIGHT, MT_PLAYER,
};
use super::spechit::spechit_overrun;
use super::state::{
    ceilingline, floatok, numspechit, spechit, tmbbox, tmceilingz, tmdropoffz, tmflags, tmfloorz,
    tmthing, tmx, tmy, TeleptMobj,
};

/// Blockmap iterator callback used by `teleport_move` to stomp any thing
/// occupying the teleport destination.
///
/// Returns 1 (true) to continue iteration in most cases; returns 0 (false)
/// only when a non-boss monster would need to stomp something it is not
/// allowed to stomp, aborting the teleport.
///
/// ## Technical Details
///
/// The 10000-damage stomp decision is demo-visible: the
/// `gamemap != 30` gate (boss-brain maps only) and the player-null check
/// decide which things get stomped, and the damage lands in the demo
/// state hash via health and thing removal.
///
/// ## On Calling
///
/// C ABI callback passed to `P_BlockThingsIterator` by `teleport_move`
/// (retained via the `PIT_StompThing` export pin); never call it
/// directly. `thing` must be a valid, non-null pointer to an initialised
/// `mobj_t`; `tmthing` / `tmx` / `tmy` must have been set by the caller
/// before invocation.
///
/// # Safety
///
/// Raw `mobj_t` dereferences throughout; see On Calling.
#[doc(alias = "PIT_StompThing")]
#[export_name = "PIT_StompThing"]
pub unsafe extern "C" fn pit_stomp_thing(thing: *mut mobj_t) -> c_uint
{
    let thing = &*thing;
    if thing.flags & MF_SHOOTABLE == 0
    {
        return 1;
    }
    let blockdist = thing.radius + (*tmthing).radius;
    if (thing.x - tmx).wrapping_abs() >= blockdist || (thing.y - tmy).wrapping_abs() >= blockdist
    {
        return 1;
    }
    if std::ptr::eq(thing, tmthing)
    {
        return 1;
    }
    if (*tmthing).player.is_null() && gamemap != 30
    {
        return 0;
    }
    P_DamageMobj(
        thing as *const _ as *mut TeleptMobj,
        tmthing as *mut TeleptMobj,
        tmthing as *mut TeleptMobj,
        10000,
    );
    1
}

/// Teleport a thing to `(x, y)`, stomping any occupant at the destination.
///
/// Unlike `try_move`, this function does not check line-of-sight,
/// step height, or dropoff constraints — it is intended for teleporter
/// effects and spawn placement. Returns 1 on success, 0 if a non-boss
/// monster cannot stomp the occupant.
///
/// ## Technical Details
///
/// The re-link order (unset position, assign floorz/ceilingz/x/y, set
/// position) and the `numspechit = 0` reset are statement-order
/// load-bearing: the reset interacts with the spechit crossed-line
/// protocol, and the sector/blockmap relink order is what a demo's
/// subsequent collision queries observe.
///
/// ## On Calling
///
/// `thing` must be a valid, non-null pointer to a live `mobj_t` that is
/// already linked into the map (sector list and blockmap); blockmap and
/// sector structures must be fully initialised. Raw fixed-point
/// coordinates in; single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` and map-structure dereferences throughout; see On Calling.
#[doc(alias = "P_TeleportMove")]
#[export_name = "P_TeleportMove"]
pub unsafe extern "C" fn teleport_move(thing: *mut mobj_t, x: fixed_t, y: fixed_t) -> c_uint
{
    tmthing = thing;
    tmflags = (*thing).flags;
    tmx = x;
    tmy = y;

    tmbbox[BBox::TOP] = y + (*tmthing).radius;
    tmbbox[BBox::BOTTOM] = y - (*tmthing).radius;
    tmbbox[BBox::RIGHT] = x + (*tmthing).radius;
    tmbbox[BBox::LEFT] = x - (*tmthing).radius;

    let newsubsec = R_PointInSubsector(x, y) as *mut subsector_t;
    ceilingline = ptr::null_mut();
    let sec = (*newsubsec).sector as *mut sector_t;
    tmfloorz = (*sec).floorheight;
    tmdropoffz = tmfloorz;
    tmceilingz = (*sec).ceilingheight;

    validcount = validcount.wrapping_add(1);
    numspechit = 0;

    let xl = (tmbbox[BBox::LEFT] - bmaporgx - MAXRADIUS) >> MAPBLOCKSHIFT;
    let xh = (tmbbox[BBox::RIGHT] - bmaporgx + MAXRADIUS) >> MAPBLOCKSHIFT;
    let yl = (tmbbox[BBox::BOTTOM] - bmaporgy - MAXRADIUS) >> MAPBLOCKSHIFT;
    let yh = (tmbbox[BBox::TOP] - bmaporgy + MAXRADIUS) >> MAPBLOCKSHIFT;

    for bx in xl..=xh
    {
        for by in yl..=yh
        {
            if P_BlockThingsIterator(bx, by, Some(pit_stomp_thing)) == 0
            {
                return 0;
            }
        }
    }

    P_UnsetThingPosition(thing);
    (*thing).floorz = tmfloorz;
    (*thing).ceilingz = tmceilingz;
    (*thing).x = x;
    (*thing).y = y;
    P_SetThingPosition(thing);

    1
}

/// Blockmap linedef iterator callback for `check_position`.
///
/// Tests whether linedef `ld` blocks the current move. If the line is
/// two-sided and passable, `tmfloorz`, `tmceilingz`, and
/// `tmdropoffz` are updated to reflect the opening. Special linedefs
/// are recorded in `spechit` for later processing by `try_move`.
///
/// Returns 1 to continue iteration, 0 to abort (line blocks the move).
///
/// ## Technical Details
///
/// This is the catalog entry-1 / G1 site: the spechit push is
/// BOUNDS-GUARDED (mirrors woof/dsda-prboom2, where the spechit store is
/// always in-bounds and only the counting continues past the array) while
/// `numspechit` itself advances unbounded, and counts past
/// `MAXSPECIALCROSS_ORIGINAL` (8) trigger `spechit_overrun`'s emulated
/// writes. Vanilla doom2.exe trampled its own DOS `.bss` layout here;
/// that layout does not exist in this port, so an unguarded write would
/// clobber arbitrary wasm `.bss` neighbors instead (observed as the
/// browser ticdup=0 freeze). Demo compatibility for overrun demos is
/// preserved by the emulated writes. The bbox reject test and
/// `P_BoxOnLineSide` run BEFORE the push, so a trample can never affect
/// its own call -- only later lines in the same walk.
///
/// ## On Calling
///
/// C ABI callback passed to `P_BlockLinesIterator` by `check_position`
/// (retained via the `PIT_CheckLine` export pin); never call it
/// directly. `ld` must be a valid, non-null pointer to an initialised
/// `line_t`; `tmthing`, `tmbbox`, `tmfloorz`, `tmceilingz`, and
/// `tmdropoffz` must have been initialised by `check_position`.
///
/// # Safety
///
/// Raw `line_t` / `mobj_t` and opening-global accesses throughout; see
/// On Calling.
#[doc(alias = "PIT_CheckLine")]
#[export_name = "PIT_CheckLine"]
pub unsafe extern "C" fn pit_check_line(ld: *mut line_t) -> c_uint
{
    let ld = &*ld;
    if tmbbox[BBox::RIGHT] <= ld.bbox[BBox::LEFT]
        || tmbbox[BBox::LEFT] >= ld.bbox[BBox::RIGHT]
        || tmbbox[BBox::TOP] <= ld.bbox[BBox::BOTTOM]
        || tmbbox[BBox::BOTTOM] >= ld.bbox[BBox::TOP]
    {
        return 1;
    }
    if P_BoxOnLineSide(std::ptr::addr_of_mut!(tmbbox[0]), ld as *const _ as *mut _) != -1
    {
        return 1;
    }
    if ld.backsector.is_null()
    {
        return 0;
    }
    if (*tmthing).flags & MF_MISSILE == 0
    {
        if (ld.flags as c_int) & (LinedefFlag::BLOCKING as c_int) != 0
        {
            return 0;
        }
        if (*tmthing).player.is_null()
            && (ld.flags as c_int) & (LinedefFlag::BLOCKMONSTERS as c_int) != 0
        {
            return 0;
        }
    }
    P_LineOpening(ld as *const _ as *mut _);
    if opentop < tmceilingz
    {
        tmceilingz = opentop;
        ceilingline = ld as *const _ as *mut _;
    }
    if openbottom > tmfloorz
    {
        tmfloorz = openbottom;
    }
    if lowfloor < tmdropoffz
    {
        tmdropoffz = lowfloor;
    }
    if ld.special != 0
    {
        //? Bounds-guarded push (mirrors woof/dsda-prboom2, where the spechit
        //? store is always in-bounds and only the counting continues past the
        //? array). Vanilla doom2.exe trampled its own DOS .bss layout here;
        //? that layout does not exist in this port, so an unguarded write
        //? would clobber arbitrary wasm .bss neighbors instead (observed as
        //? the browser ticdup=0 freeze). Demo compatibility for overrun
        //? demos is preserved by SpechitOverrun's emulated writes below.
        if numspechit >= 0 && (numspechit as usize) < MAXSPECIALCROSS
        {
            spechit[numspechit as usize] = ld as *const _ as *mut _;
        }
        numspechit += 1;
        if numspechit > MAXSPECIALCROSS_ORIGINAL
        {
            spechit_overrun(ld as *const _ as *mut _);
        }
    }
    1
}

/// Blockmap thing iterator callback for `check_position`.
///
/// Tests whether `thing` blocks or interacts with the moving object
/// (`tmthing`). Handles skull-fly collision damage, missile detonation,
/// same-species missile pass-through, and special item pickup. Returns 1
/// to continue blockmap iteration; returns 0 to stop (collision confirmed
/// or skull charge resolved).
///
/// ## Technical Details
///
/// The two `P_Random` draws (skull slam damage, missile damage) are
/// demo-synchronization surface: the draw COUNT and ORDER are pinned by
/// the F9 goldens (p_inter Deterministic-Aspects pattern) -- the
/// `(P_Random() % 8) + 1` pairs must stay at exactly these two arms in
/// exactly this order. The knight/bruiser species pass-through and the
/// `DEH_DEFAULT_SPECIES_INFIGHTING` gate decide missile blocking,
/// which is collision-visible per tic.
///
/// ## On Calling
///
/// C ABI callback passed to `P_BlockThingsIterator` by `check_position`
/// (retained via the `PIT_CheckThing` export pin); never call it
/// directly. `thing` must be a valid, non-null pointer to an initialised
/// `mobj_t`; `tmthing`, `tmx`, `tmy`, and `tmflags` must have been set
/// by `check_position`.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and RNG draws throughout; see On Calling.
#[doc(alias = "PIT_CheckThing")]
#[export_name = "PIT_CheckThing"]
pub unsafe extern "C" fn pit_check_thing(thing: *mut mobj_t) -> c_uint
{
    let thing = &*thing;
    if thing.flags & (MF_SOLID | MF_SPECIAL | MF_SHOOTABLE) == 0
    {
        return 1;
    }
    let blockdist = thing.radius + (*tmthing).radius;
    if (thing.x - tmx).wrapping_abs() >= blockdist || (thing.y - tmy).wrapping_abs() >= blockdist
    {
        return 1;
    }
    if std::ptr::eq(thing, tmthing)
    {
        return 1;
    }

    // check for skulls slamming into things
    if (*tmthing).flags & MF_SKULLFLY != 0
    {
        let damage = ((P_Random() % 8) + 1) * (*((*tmthing).info as *mut MobjInfo)).damage;
        P_DamageMobj(
            thing as *const _ as *mut TeleptMobj,
            tmthing as *mut TeleptMobj,
            tmthing as *mut TeleptMobj,
            damage,
        );
        (*tmthing).flags &= !MF_SKULLFLY;
        (*tmthing).momx = 0;
        (*tmthing).momy = 0;
        (*tmthing).momz = 0;
        P_SetMobjState(
            tmthing as *mut TeleptMobj,
            (*((*tmthing).info as *mut MobjInfo)).spawnstate,
        );
        return 0;
    }

    // missiles can hit other things
    if (*tmthing).flags & MF_MISSILE != 0
    {
        if (*tmthing).z > thing.z + thing.height
        {
            return 1;
        }
        if (*tmthing).z + (*tmthing).height < thing.z
        {
            return 1;
        }
        if !(*tmthing).target.is_null()
        {
            let target = (*tmthing).target as *mut mobj_t;
            let target_type = (*target).type_;
            let thing_type = thing.type_;
            if target_type == thing_type
                || (target_type == MT_KNIGHT && thing_type == MT_BRUISER)
                || (target_type == MT_BRUISER && thing_type == MT_KNIGHT)
            {
                if std::ptr::eq(thing, target)
                {
                    return 1;
                }
                if thing_type != MT_PLAYER && DEH_DEFAULT_SPECIES_INFIGHTING == 0
                {
                    return 0;
                }
            }
        }
        if thing.flags & MF_SHOOTABLE == 0
        {
            return (thing.flags & MF_SOLID == 0) as c_uint;
        }
        let damage = ((P_Random() % 8) + 1) * (*((*tmthing).info as *mut MobjInfo)).damage;
        let target = (*tmthing).target as *mut TeleptMobj;
        P_DamageMobj(
            thing as *const _ as *mut TeleptMobj,
            tmthing as *mut TeleptMobj,
            target,
            damage,
        );
        return 0;
    }

    // check for special pickup
    if thing.flags & MF_SPECIAL != 0
    {
        let solid = thing.flags & MF_SOLID;
        if tmflags & MF_PICKUP != 0
        {
            P_TouchSpecialThing(
                thing as *const _ as *mut TeleptMobj,
                tmthing as *mut TeleptMobj,
            );
        }
        return (solid == 0) as c_uint;
    }

    (thing.flags & MF_SOLID == 0) as c_uint
}

/// Test whether `thing` can occupy position `(x, y)` without clipping.
///
/// This is a pure query — it does not move the thing. As a side effect it
/// sets `tmfloorz`, `tmceilingz`, `tmdropoffz`, `spechit`, and
/// `numspechit` for the tested position. Things with `MF_PICKUP` may
/// pick up items encountered during the sweep.
///
/// Returns 1 if the position is unobstructed, 0 if blocked.
///
/// ## Technical Details
///
/// The scratchpad initialisation order (tmbbox, sector heights, then the
/// `validcount` bump) and the two-pass blockmap sweep (things first with
/// `MAXRADIUS` expansion, then lines without) are statement-order
/// load-bearing: callback outcomes feed back through the scratchpad
/// globals between iterations. The `validcount` bump is part of the
/// cross-module stamp protocol shared with p_sight / p_maputl.
///
/// ## On Calling
///
/// `thing` must be a valid, non-null pointer to an initialised `mobj_t`;
/// blockmap and all sector/linedef structures must be fully initialised.
/// Raw fixed-point coordinates in; `c_uint` 0/1 out; single-threaded sim
/// only.
///
/// # Safety
///
/// Raw `mobj_t` and map-structure dereferences throughout; see On Calling.
#[doc(alias = "P_CheckPosition")]
#[export_name = "P_CheckPosition"]
pub unsafe extern "C" fn check_position(thing: *mut mobj_t, x: fixed_t, y: fixed_t) -> c_uint
{
    tmthing = thing;
    tmflags = (*thing).flags;
    tmx = x;
    tmy = y;

    tmbbox[BBox::TOP] = y + (*tmthing).radius;
    tmbbox[BBox::BOTTOM] = y - (*tmthing).radius;
    tmbbox[BBox::RIGHT] = x + (*tmthing).radius;
    tmbbox[BBox::LEFT] = x - (*tmthing).radius;

    let newsubsec = R_PointInSubsector(x, y) as *mut subsector_t;
    ceilingline = ptr::null_mut();
    let sec = (*newsubsec).sector as *mut sector_t;
    tmfloorz = (*sec).floorheight;
    tmdropoffz = tmfloorz;
    tmceilingz = (*sec).ceilingheight;

    validcount = validcount.wrapping_add(1);
    numspechit = 0;

    if tmflags & MF_NOCLIP != 0
    {
        return 1;
    }

    let xl = (tmbbox[BBox::LEFT] - bmaporgx - MAXRADIUS) >> MAPBLOCKSHIFT;
    let xh = (tmbbox[BBox::RIGHT] - bmaporgx + MAXRADIUS) >> MAPBLOCKSHIFT;
    let yl = (tmbbox[BBox::BOTTOM] - bmaporgy - MAXRADIUS) >> MAPBLOCKSHIFT;
    let yh = (tmbbox[BBox::TOP] - bmaporgy + MAXRADIUS) >> MAPBLOCKSHIFT;

    for bx in xl..=xh
    {
        for by in yl..=yh
        {
            if P_BlockThingsIterator(bx, by, Some(pit_check_thing)) == 0
            {
                return 0;
            }
        }
    }

    let xl = (tmbbox[BBox::LEFT] - bmaporgx) >> MAPBLOCKSHIFT;
    let xh = (tmbbox[BBox::RIGHT] - bmaporgx) >> MAPBLOCKSHIFT;
    let yl = (tmbbox[BBox::BOTTOM] - bmaporgy) >> MAPBLOCKSHIFT;
    let yh = (tmbbox[BBox::TOP] - bmaporgy) >> MAPBLOCKSHIFT;

    for bx in xl..=xh
    {
        for by in yl..=yh
        {
            if P_BlockLinesIterator(bx, by, Some(pit_check_line)) == 0
            {
                return 0;
            }
        }
    }

    1
}

/// Attempt to move `thing` to `(x, y)`, triggering crossed linedef specials.
///
/// Calls `check_position` internally. If the position is valid and all
/// height constraints pass, the thing is re-linked at the new position and
/// any special linedefs in `spechit` that were actually crossed are
/// activated. Things with `MF_TELEPORT` or `MF_NOCLIP` skip special-line
/// processing.
///
/// Returns 1 on success, 0 if the move is blocked.
///
/// ## Technical Details
///
/// The height/dropoff gate ladder (`tmceilingz - tmfloorz < height`, the
/// 24-unit step and dropoff checks) and the re-link order are the
/// demo-visible decision surface. The spechit DRAIN at the end is
/// catalog entry 1's read side: entries beyond the array were never
/// stored (see the guarded push in `pit_check_line`), so the
/// decrement-then-skip loop never reads out of bounds, and
/// `P_CrossSpecialLine` fires in reverse crossing order exactly when the
/// drain's side-flip predicate says so.
///
/// ## On Calling
///
/// `thing` must be a valid, non-null pointer to a live `mobj_t` already
/// linked into the map; map data must be fully initialised. Raw
/// fixed-point coordinates in; `c_uint` 0/1 out; single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` / `line_t` dereferences throughout; see On Calling.
#[doc(alias = "P_TryMove")]
#[export_name = "P_TryMove"]
pub unsafe extern "C" fn try_move(thing: *mut mobj_t, x: fixed_t, y: fixed_t) -> c_uint
{
    floatok = 0;
    if check_position(thing, x, y) == 0
    {
        return 0;
    }
    if (*thing).flags & MF_NOCLIP == 0
    {
        if tmceilingz - tmfloorz < (*thing).height
        {
            return 0;
        }
        floatok = 1;
        if (*thing).flags & MF_TELEPORT == 0 && tmceilingz - (*thing).z < (*thing).height
        {
            return 0;
        }
        if (*thing).flags & MF_TELEPORT == 0 && tmfloorz - (*thing).z > 24 * FRACUNIT
        {
            return 0;
        }
        if (*thing).flags & (MF_DROPOFF | MF_FLOAT) == 0 && tmfloorz - tmdropoffz > 24 * FRACUNIT
        {
            return 0;
        }
    }

    P_UnsetThingPosition(thing);
    let oldx = (*thing).x;
    let oldy = (*thing).y;
    (*thing).floorz = tmfloorz;
    (*thing).ceilingz = tmceilingz;
    (*thing).x = x;
    (*thing).y = y;
    P_SetThingPosition(thing);

    if (*thing).flags & (MF_TELEPORT | MF_NOCLIP) == 0
    {
        while numspechit > 0
        {
            numspechit -= 1;
            //? Entries beyond the array were never stored (see the guarded
            //? push in PIT_CheckLine); skip them instead of reading OOB.
            if numspechit as usize >= MAXSPECIALCROSS
            {
                continue;
            }
            let ld = spechit[numspechit as usize];
            let side = P_PointOnLineSide((*thing).x, (*thing).y, ld);
            let oldside = P_PointOnLineSide(oldx, oldy, ld);
            if side != oldside && (*ld).special != 0
            {
                let linenum = ld.offset_from(lines);
                P_CrossSpecialLine(linenum as c_int, oldside, thing);
            }
        }
    }

    1
}

/// Clip a thing's z-position after a nearby sector floor or ceiling has moved.
///
/// Re-runs `check_position` at the thing's current x/y to refresh
/// `tmfloorz` and `tmceilingz`, then adjusts `thing->z` if necessary.
/// Walking things rise and fall with the floor; floating things are only
/// pushed down if they would exceed the ceiling. Returns 0 if the thing no
/// longer fits in the vertical gap (caller should crush or block the move).
///
/// ## Technical Details
///
/// The crush-support predicate `ceilingz - floorz < height` is the
/// sector-crush decision fed back through `pit_change_sector`; the
/// onfloor sticky-z rule decides ride-along, which is movement-visible
/// the very next tic.
///
/// ## On Calling
///
/// `thing` must be a valid, non-null pointer to a live `mobj_t`; called
/// only from `pit_change_sector` during sector-height propagation.
/// Mutates the shared scratchpad via `check_position` -- not reentrant
/// with another position check in flight (single-threaded sim only).
///
/// # Safety
///
/// Raw `mobj_t` dereferences and scratchpad mutation; see On Calling.
#[doc(alias = "P_ThingHeightClip")]
#[export_name = "P_ThingHeightClip"]
pub unsafe extern "C" fn thing_height_clip(thing: *mut mobj_t) -> c_uint
{
    let onfloor = (*thing).z == (*thing).floorz;
    check_position(thing, (*thing).x, (*thing).y);
    (*thing).floorz = tmfloorz;
    (*thing).ceilingz = tmceilingz;
    if onfloor
    {
        (*thing).z = (*thing).floorz;
    }
    else if (*thing).z + (*thing).height > (*thing).ceilingz
    {
        (*thing).z = (*thing).ceilingz - (*thing).height;
    }
    if (*thing).ceilingz - (*thing).floorz < (*thing).height
    {
        return 0;
    }
    1
}
