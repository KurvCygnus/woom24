//! Radius (explosion) splash damage: the explosion-state statics and the
//! blockmap sweep that damages everything shootable in line-of-sight --
//! bit-exact with the radius-attack half of
//! `vendor/doomgeneric/p_map.c`.

use std::ffi::{c_int, c_uint};
use std::ptr;

use crate::doom::c_ffi::{mobj_t, MAPBLOCKSHIFT};
use crate::doom::m_fixed::FRACBITS;
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_maputl::P_BlockThingsIterator;
use crate::doom::p_setup::{bmaporgx, bmaporgy};
use crate::doom::p_sight::P_CheckSight;

use super::consts::{MAXRADIUS, MF_SHOOTABLE, MT_CYBORG, MT_SPIDER};
use super::state::TeleptMobj;

/// The creature that triggered the explosion (may differ from `bombspot`).
///
/// Used as the `inflictor` when calling `P_DamageMobj` so that frag
/// credit goes to the right entity (e.g. the player who fired the rocket,
/// not the explosion object itself).
#[no_mangle]
pub static mut bombsource: *mut mobj_t = ptr::null_mut();

/// The map object at the centre of the current explosion.
///
/// Used by `pit_radius_attack` to measure distance and check line-of-sight.
#[no_mangle]
pub static mut bombspot: *mut mobj_t = ptr::null_mut();

/// Maximum damage (and effective radius in map units) of the current explosion.
///
/// Damage falls off linearly with Chebyshev distance from `bombspot`:
/// `damage = bombdamage - dist`.
#[no_mangle]
pub static mut bombdamage: c_int = 0;

/// Blockmap thing iterator callback for `radius_attack`.
///
/// Skips non-shootable things and the two boss types immune to splash
/// (Cyberdemon and Spider Mastermind). Computes Chebyshev distance from
/// `bombspot`, subtracts the thing's radius, and if in range and in
/// line-of-sight, deals `bombdamage - dist` damage.
///
/// Always returns 1 (continues blockmap iteration).
///
/// ## Technical Details
///
/// The Chebyshev `max(|dx|, |dy|)` with the radius subtraction (clamped
/// at 0) and the immunity list (`MT_CYBORG` / `MT_SPIDER`) are the
/// damage-decision surface; the `P_CheckSight` gate then decides whether
/// the splash penetrates walls. All three feed health/RNG-free state the
/// F9 goldens hash through behavior.
///
/// ## On Calling
///
/// C ABI callback passed to `P_BlockThingsIterator` by `radius_attack`
/// (retained via the `PIT_RadiusAttack` export pin); never call it
/// directly. `thing` must be a valid, non-null pointer to an initialised
/// `mobj_t`; `bombspot`, `bombsource`, and `bombdamage` must be set by
/// `radius_attack` before iteration.
///
/// # Safety
///
/// Raw `mobj_t` dereferences; see On Calling.
#[doc(alias = "PIT_RadiusAttack")]
#[export_name = "PIT_RadiusAttack"]
pub unsafe extern "C" fn pit_radius_attack(thing: *mut mobj_t) -> c_uint
{
    let thing = &*thing;
    if thing.flags & MF_SHOOTABLE == 0
    {
        return 1;
    }
    if thing.type_ == MT_CYBORG || thing.type_ == MT_SPIDER
    {
        return 1;
    }
    let dx = (thing.x - (*bombspot).x).wrapping_abs();
    let dy = (thing.y - (*bombspot).y).wrapping_abs();
    let mut dist = if dx > dy
    {
        dx
    }
    else
    {
        dy
    };
    dist = (dist - thing.radius) >> FRACBITS;
    if dist < 0
    {
        dist = 0;
    }
    if dist >= bombdamage
    {
        return 1;
    }
    let pt_mobj_t = thing as *const _ as *mut crate::doom::p_telept::mobj_t;
    let pt_bombspot = bombspot as *mut crate::doom::p_telept::mobj_t;
    if P_CheckSight(pt_mobj_t, pt_bombspot) != 0
    {
        P_DamageMobj(
            thing as *const _ as *mut TeleptMobj,
            bombspot as *mut TeleptMobj,
            bombsource as *mut TeleptMobj,
            bombdamage - dist,
        );
    }
    1
}

/// Apply splash damage from an explosion at `spot` to all nearby things.
///
/// Iterates over all blockmap cells within `damage + MAXRADIUS` of `spot`
/// and calls `pit_radius_attack` for each thing found. The Cyberdemon and
/// Spider Mastermind are immune; all other shootable things in line-of-sight
/// take linearly decreasing damage.
///
/// `source` is the creature credited with the kill (may differ from `spot`).
///
/// ## Technical Details
///
/// The block-range expansion (`(damage + MAXRADIUS) << FRACBITS`) and the
/// y-outer/x-inner iteration order decide which things are visited in
/// what order -- damage application order is observable through
/// infighting and death order, so it is pinned.
///
/// ## On Calling
///
/// `spot` and `source` must be valid, non-null pointers to live
/// `mobj_t`s; map and blockmap data must be fully initialised.
/// Mutates the three statics above; single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and global explosion-state mutation; see On
/// Calling.
#[doc(alias = "P_RadiusAttack")]
#[export_name = "P_RadiusAttack"]
pub unsafe extern "C" fn radius_attack(spot: *mut mobj_t, source: *mut mobj_t, damage: c_int)
{
    let dist = (damage + MAXRADIUS) << FRACBITS;
    let yh = ((*spot).y + dist - bmaporgy) >> MAPBLOCKSHIFT;
    let yl = ((*spot).y - dist - bmaporgy) >> MAPBLOCKSHIFT;
    let xh = ((*spot).x + dist - bmaporgx) >> MAPBLOCKSHIFT;
    let xl = ((*spot).x - dist - bmaporgx) >> MAPBLOCKSHIFT;
    bombspot = spot;
    bombsource = source;
    bombdamage = damage;
    for y in yl..=yh
    {
        for x in xl..=xh
        {
            P_BlockThingsIterator(x, y, Some(pit_radius_attack));
        }
    }
}
