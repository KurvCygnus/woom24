//! Hitscan attacks and auto-aim: the attack-state statics, the aim and
//! shoot traversal callbacks, the wall-puff spawn that decodes the C
//! `goto hitline`, and the `aim_line_attack` / `line_attack` entries --
//! bit-exact with the line-attack half of
//! `vendor/doomgeneric/p_map.c`. Aim narrows p_sight's
//! `topslope`/`bottomslope` (the dual-writer LOS protocol).

use std::ffi::{c_int, c_uint};
use std::ptr;

use crate::doom::c_ffi::{intercept_t, line_t, mobj_t, sector_t, LinedefFlag};
use crate::doom::m_fixed::{fixed_t, FixedDiv, FixedMul, FRACBITS, FRACUNIT};
use crate::doom::p_inter::P_DamageMobj;
use crate::doom::p_maputl::{openbottom, opentop, P_LineOpening, P_PathTraverse, PT_ADDLINES, PT_ADDTHINGS};
use crate::doom::p_mobj::{P_SpawnBlood, P_SpawnPuff, P_SubstNullMobj};
use crate::doom::p_sight::{bottomslope, topslope};
use crate::doom::p_spec::P_ShootSpecialLine;
use crate::doom::r_sky::skyflatnum;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};

use super::consts::{MF_NOBLOOD, MF_SHOOTABLE};
use super::state::TeleptMobj;

/// The map object struck by the most recent hitscan or auto-aim traversal.
///
/// Set to `null` before each `aim_line_attack` / `line_attack` call
/// and written by `ptr_aim_traverse` or `ptr_shoot_traverse` when a
/// target is hit. Callers check this to determine whether anything was
/// actually hit.
#[no_mangle]
pub static mut linetarget: *mut mobj_t = ptr::null_mut();

/// The map object that fired the current hitscan attack.
///
/// Set by `aim_line_attack` and `line_attack`; read by
/// `ptr_aim_traverse` and `ptr_shoot_traverse` to avoid self-hits.
#[no_mangle]
pub static mut shootthing: *mut mobj_t = ptr::null_mut();

/// Z-height from which the current hitscan ray originates.
///
/// Set to the shooter's mid-height plus 8 map units by both
/// `aim_line_attack` and `line_attack`.
#[no_mangle]
pub static mut shootz: fixed_t = 0;

/// Damage dealt by the current hitscan attack (0 for a pure aim/test trace).
#[no_mangle]
pub static mut la_damage: c_int = 0;

/// Maximum reach of the current hitscan ray, in fixed-point map units.
///
/// Used together with an intercept's fractional distance to compute the
/// actual world distance to a hit.
#[no_mangle]
pub static mut attackrange: fixed_t = 0;

/// Vertical slope (rise/run) of the winning auto-aim result.
///
/// Written by `ptr_aim_traverse` when a valid target is found, then read
/// by `aim_line_attack` (return value) and passed into `line_attack`
/// as the actual firing slope.
#[no_mangle]
pub static mut aimslope: fixed_t = 0;

/// Path-traversal callback for `aim_line_attack` auto-aim.
///
/// For each line intercept, narrows the vertical aim window (`topslope` /
/// `bottomslope`) based on the opening. For each thing intercept, checks
/// whether the thing falls within the aim window and, if so, records it in
/// `linetarget` and computes `aimslope`.
///
/// Returns 1 to continue traversal, 0 to stop (target locked or window
/// closed).
///
/// ## Technical Details
///
/// This callback is one of the two writers of p_sight's
/// `topslope`/`bottomslope` (the other is `P_CheckSight` itself): the
/// narrowing order (floor slope first, then ceiling slope, then the
/// `topslope <= bottomslope` window-closed check) is a cross-module
/// protocol and must not be reordered. `aimslope = (top + bottom) / 2`
/// is the exact mid-window pick that decides auto-aim targeting.
///
/// ## On Calling
///
/// C ABI callback passed to `P_PathTraverse` by `aim_line_attack`
/// (retained via the `PTR_AimTraverse` export pin); never call it
/// directly. `in_` must be a valid, non-null pointer to an initialised
/// `intercept_t`; `shootthing`, `shootz`, `attackrange`, `topslope`, and
/// `bottomslope` must be set before the traversal begins.
///
/// # Safety
///
/// Raw `intercept_t` / `line_t` / `sector_t` dereferences and global
/// aim-state mutation; see On Calling.
#[doc(alias = "PTR_AimTraverse")]
#[export_name = "PTR_AimTraverse"]
pub unsafe extern "C" fn ptr_aim_traverse(in_: *mut intercept_t) -> c_uint
{
    let in_ = &*in_;
    if in_.isaline != 0
    {
        let li = in_.d.line;
        if(*li).flags as c_int & LinedefFlag::TWOSIDED as c_int == 0 { return 0; }
        P_LineOpening(li);
        if openbottom >= opentop { return 0; }
        let dist = FixedMul(attackrange, in_.frac);
        let front = (*li).frontsector as *mut sector_t;
        let back = (*li).backsector as *mut sector_t;
        if back.is_null() || (*front).floorheight != (*back).floorheight
        {
            let slope = FixedDiv(openbottom - shootz, dist);
            if slope > bottomslope { bottomslope = slope; }
        }
        if back.is_null() || (*front).ceilingheight != (*back).ceilingheight
        {
            let slope = FixedDiv(opentop - shootz, dist);
            if slope < topslope { topslope = slope; }
        }
        if topslope <= bottomslope { return 0; }
        return 1;
    }

    let th = in_.d.thing as *mut mobj_t;
    if th == shootthing { return 1; }
    if(*th).flags & MF_SHOOTABLE == 0 { return 1; }
    let dist = FixedMul(attackrange, in_.frac);
    let thingtopslope = FixedDiv((*th).z + (*th).height - shootz, dist);
    if thingtopslope < bottomslope { return 1; }
    let thingbottomslope = FixedDiv((*th).z - shootz, dist);
    if thingbottomslope > topslope { return 1; }
    let mut thingtopslope = thingtopslope;
    let mut thingbottomslope = thingbottomslope;
    if thingtopslope > topslope { thingtopslope = topslope; }
    if thingbottomslope < bottomslope { thingbottomslope = bottomslope; }
    aimslope = (thingtopslope + thingbottomslope) / 2;
    linetarget = th;
    0
}

/// Path-traversal callback for `line_attack` hitscan shooting.
///
/// For line intercepts, activates any special on the line, then checks
/// whether the shot passes through the opening or hits the wall; if it
/// hits, `spawn_wall_puff` spawns a bullet puff and the traversal stops.
/// For thing intercepts, checks z-overlap with `aimslope`, spawns puff
/// or blood, and calls `P_DamageMobj` if `la_damage` is non-zero.
///
/// Returns 1 to continue traversal, 0 to stop (shot consumed).
///
/// ## Technical Details
///
/// Side-effect ORDER is the demo surface: the `P_ShootSpecialLine`
/// activation happens before the opening checks, and the impact point is
/// backed off the wall by `10 * FRACUNIT` (things) via the fully
/// qualified `crate::doom::p_maputl::trace` ray read -- that trace read
/// stays fully qualified by design so the provenance of the cross-module
/// global stays obvious. The puff-vs-blood branch keys on `MF_NOBLOOD`.
///
/// ## On Calling
///
/// C ABI callback passed to `P_PathTraverse` by `line_attack` (retained
/// via the `PTR_ShootTraverse` export pin); never call it directly.
/// `in_` must be a valid, non-null pointer to an initialised
/// `intercept_t`; `shootthing`, `shootz`, `attackrange`, `aimslope`, and
/// `la_damage` must be set before the traversal.
///
/// # Safety
///
/// Raw `intercept_t` / `line_t` / `sector_t` / `mobj_t` dereferences and
/// global attack-state mutation; see On Calling.
#[doc(alias = "PTR_ShootTraverse")]
#[export_name = "PTR_ShootTraverse"]
pub unsafe extern "C" fn ptr_shoot_traverse(in_: *mut intercept_t) -> c_uint
{
    let in_ = &*in_;
    if in_.isaline != 0
    {
        let li = in_.d.line;
        if(*li).special != 0 { P_ShootSpecialLine(shootthing, li); }
        if (*li).flags as c_int & LinedefFlag::TWOSIDED as c_int != 0
        {
            P_LineOpening(li);
            let dist = FixedMul(attackrange, in_.frac);
            let back = (*li).backsector as *mut sector_t;
            if back.is_null()
            {
                let slope = FixedDiv(openbottom - shootz, dist);
                if slope > aimslope
                {
                    spawn_wall_puff(li, in_);
                    return 0;
                }
                let slope = FixedDiv(opentop - shootz, dist);
                if slope < aimslope
                {
                    spawn_wall_puff(li, in_);
                    return 0;
                }
            }
            else
            {
                let front = (*li).frontsector as *mut sector_t;
                if (*front).floorheight != (*back).floorheight
                {
                    let slope = FixedDiv(openbottom - shootz, dist);
                    if slope > aimslope
                    {
                        spawn_wall_puff(li, in_);
                        return 0;
                    }
                }
                if (*front).ceilingheight != (*back).ceilingheight
                {
                    let slope = FixedDiv(opentop - shootz, dist);
                    if slope < aimslope
                    {
                        spawn_wall_puff(li, in_);
                        return 0;
                    }
                }
            }
            return 1;
        }
        spawn_wall_puff(li, in_);
        return 0;
    }

    let th = in_.d.thing as *mut mobj_t;
    if th == shootthing { return 1; }
    if(*th).flags & MF_SHOOTABLE == 0 { return 1; }
    let dist = FixedMul(attackrange, in_.frac);
    let thingtopslope = FixedDiv((*th).z + (*th).height - shootz, dist);
    if thingtopslope < aimslope { return 1; }
    let thingbottomslope = FixedDiv((*th).z - shootz, dist);
    if thingbottomslope > aimslope { return 1; }

    let frac = in_.frac - FixedDiv(10 * FRACUNIT, attackrange);
    let x = crate::doom::p_maputl::trace.x + FixedMul(crate::doom::p_maputl::trace.dx, frac);
    let y = crate::doom::p_maputl::trace.y + FixedMul(crate::doom::p_maputl::trace.dy, frac);
    let z = shootz + FixedMul(aimslope, FixedMul(frac, attackrange));
    if(*th).flags & MF_NOBLOOD != 0 { P_SpawnPuff(x, y, z); }
    else { P_SpawnBlood(x, y, z, la_damage); }
    if la_damage != 0
    {
        P_DamageMobj(
            th as *mut TeleptMobj,
            shootthing as *mut TeleptMobj,
            shootthing as *mut TeleptMobj,
            la_damage,
        );
    }
    0
}

/// Spawn a bullet puff at the point where a hitscan shot struck linedef `li`.
///
/// Computes the impact position slightly in front of the intercept (to avoid
/// z-fighting), skips spawning if the shot hit sky on the front sector or a
/// sky-hack wall on the back sector. This function decodes the C
/// `goto hitline` jumps in upstream `PTR_ShootTraverse`.
///
/// ## Technical Details
///
/// The sky-hack suppression chain (front `ceilingpic == skyflatnum`, then
/// `z > ceilingheight`, then the back-sector `ceilingpic` check) is what
/// makes shots vanish against sky walls instead of puffing -- a
/// demo-visible side-effect gate. The `4 * FRACUNIT` back-off differs
/// from the thing-impact `10 * FRACUNIT` on purpose (upstream values).
///
/// ## On Calling
///
/// Private helper, only called from `ptr_shoot_traverse` at the exact
/// spots upstream jumped to `hitline`. `li` must be a valid, non-null
/// pointer to an initialised `line_t`; `in_` must refer to the intercept
/// that triggered the call; `shootz`, `aimslope`, and `attackrange` must
/// be current.
///
/// # Safety
///
/// Raw `line_t` / `sector_t` dereferences; see On Calling.
#[doc(alias = "goto_hitline")]
unsafe fn spawn_wall_puff(li: *mut line_t, in_: &intercept_t)
{
    let frac = in_.frac - FixedDiv(4 * FRACUNIT, attackrange);
    let x = crate::doom::p_maputl::trace.x + FixedMul(crate::doom::p_maputl::trace.dx, frac);
    let y = crate::doom::p_maputl::trace.y + FixedMul(crate::doom::p_maputl::trace.dy, frac);
    let z = shootz + FixedMul(aimslope, FixedMul(frac, attackrange));
    let front = (*li).frontsector as *mut sector_t;
    if (*front).ceilingpic as c_int == skyflatnum
    {
        if z > (*front).ceilingheight { return; }
        let back = (*li).backsector as *mut sector_t;
        if !back.is_null() && (*back).ceilingpic as c_int == skyflatnum { return; }
    }
    P_SpawnPuff(x, y, z);
}

/// Auto-aim a hitscan ray from `t1` along `angle` up to `distance` away.
///
/// Runs `ptr_aim_traverse` to find the best shootable target within the
/// vertical aim window (approximately ±35 degrees). Returns the vertical
/// slope to the centre of that target, or 0 if nothing was found.
/// Sets `linetarget` as a side effect.
///
/// ## Technical Details
///
/// The ray endpoint math (`(distance >> FRACBITS) * finecosine/finesine`
/// index) and the `shootz = mid-height + 8*FRACUNIT` origin are exact
/// integer formulas; the `topslope`/`bottomslope` window initialisation
/// here is the other half of the p_sight dual-writer protocol. The
/// returned slope is what the weapon fires with -- a one-unit difference
/// is a missed shot in a demo.
///
/// ## On Calling
///
/// `t1` must be a valid pointer to a live `mobj_t` (null is substituted
/// via `P_SubstNullMobj` before use); map data must be initialised.
/// Returns raw fixed-point slope; single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and global attack-state mutation; see On
/// Calling.
#[doc(alias = "P_AimLineAttack")]
#[export_name = "P_AimLineAttack"]
pub unsafe extern "C" fn aim_line_attack(
    t1: *mut mobj_t,
    angle: c_uint,
    distance: fixed_t,
) -> fixed_t
{
    let t1 = P_SubstNullMobj(t1 as *mut TeleptMobj) as *mut mobj_t;
    let angle = (angle >> ANGLETOFINESHIFT) as usize;
    shootthing = t1;
    let x2 = (*t1).x + ((distance >> FRACBITS) as c_int) * *finecosine.0.add(angle);
    let y2 = (*t1).y + ((distance >> FRACBITS) as c_int) * finesine[angle];
    shootz = (*t1).z + ((*t1).height >> 1) + 8 * FRACUNIT;
    topslope = 100 * FRACUNIT / 160;
    bottomslope = -(100 * FRACUNIT / 160);
    attackrange = distance;
    linetarget = ptr::null_mut();
    P_PathTraverse(
        (*t1).x,
        (*t1).y,
        x2,
        y2,
        PT_ADDLINES | PT_ADDTHINGS,
        Some(ptr_aim_traverse),
    );
    if !linetarget.is_null() { return aimslope; }
    0
}

/// Fire a hitscan attack from `t1` with the given `angle`, `distance`,
/// `slope`, and `damage`.
///
/// Runs `ptr_shoot_traverse` along the ray. If `damage` is 0 the call is
/// a pure trace that sets `linetarget` without dealing damage. Spawns
/// puffs or blood at the impact point as a side effect.
///
/// ## Technical Details
///
/// The scratchpad setup order (shootthing, la_damage, ray endpoint,
/// shootz, attackrange, aimslope) must precede the traversal exactly --
/// the callbacks read every one of them per intercept. The endpoint math
/// matches `aim_line_attack` so aim and shoot agree on where the ray
/// lands.
///
/// ## On Calling
///
/// `t1` must be a valid pointer to a live `mobj_t`; map data must be
/// fully initialised. `damage == 0` means trace-only (no damage, still
/// puffs/blood-sprays visuals). Single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and global attack-state mutation; see On
/// Calling.
#[doc(alias = "P_LineAttack")]
#[export_name = "P_LineAttack"]
pub unsafe extern "C" fn line_attack(
    t1: *mut mobj_t,
    angle: c_uint,
    distance: fixed_t,
    slope: fixed_t,
    damage: c_int,
)
{
    let angle = (angle >> ANGLETOFINESHIFT) as usize;
    shootthing = t1;
    la_damage = damage;
    let x2 = (*t1).x + ((distance >> FRACBITS) as c_int) * *finecosine.0.add(angle);
    let y2 = (*t1).y + ((distance >> FRACBITS) as c_int) * finesine[angle];
    shootz = (*t1).z + ((*t1).height >> 1) + 8 * FRACUNIT;
    attackrange = distance;
    aimslope = slope;
    P_PathTraverse(
        (*t1).x,
        (*t1).y,
        x2,
        y2,
        PT_ADDLINES | PT_ADDTHINGS,
        Some(ptr_shoot_traverse),
    );
}
