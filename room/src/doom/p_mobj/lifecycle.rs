//! Map-object lifecycle: the per-tic thinker, Nightmare-mode respawn,
//! removal with the deathmatch item-respawn queue (the queue statics
//! live here), and the periodic special-item respawner -- bit-exact
//! with the lifecycle half of `vendor/doomgeneric/p_mobj.c`.

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::c_ffi::ITEMQUESIZE;
use crate::doom::g_game::{deathmatch, respawnmonsters};
use crate::doom::i_timer::TICRATE;
use crate::doom::info::{self, *};
use crate::doom::m_fixed::FRACBITS;
use crate::doom::m_random::P_Random;
use crate::doom::p_map::P_CheckPosition;
use crate::doom::p_maputl::P_UnsetThingPosition;
use crate::doom::p_telept::{mapthing_t, mobj_t, subsector_t};
use crate::doom::p_tick::leveltime;
use crate::doom::p_tick::{thinker_t, P_RemoveThinker};
use crate::doom::r_main::R_PointInSubsector;
use crate::doom::s_sound::{MobjStub, S_StartSound, S_StopSound};
use crate::doom::sounds::Sfx;

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = crate::doom::c_ffi::mobj_t;

//* Sentinel single source (F10 wave B3a): the freeze-zone duplicate
//* that used to live beside these functions is retired in favor of
//* p_tick's extracted dtmc pair; `is_sentinel` is the only half this
//* module consumes (`sentinel_ac` has no direct caller here).
use crate::doom::p_tick::dtmc::is_sentinel;

use super::consts::{MTF_AMBUSH, ONCEILINGZ, ONFLOORZ};
use super::dtmc::{mapthing_angle_quantize, respawn_queue_step};
use super::movement::{xy_movement, z_movement};
use super::spawn::spawn_mobj;
use super::state::set_mobj_state;

/// Circular queue of map-thing spawn records for items that need to respawn.
///
/// The queue is indexed by `iquehead` (write) and `iquetail` (read), both
/// modulo `ITEMQUESIZE`.
#[no_mangle]
pub static mut itemrespawnque: [mapthing_t; ITEMQUESIZE] = [mapthing_t {
    x: 0,
    y: 0,
    angle: 0,
    r#type: 0,
    options: 0,
}; ITEMQUESIZE];

/// Level-time stamps recording when each item in `itemrespawnque` was removed.
///
/// An item is eligible to respawn once `leveltime - itemrespawntime[iquetail]`
/// exceeds 30 * `TICRATE`.
#[no_mangle]
pub static mut itemrespawntime: [c_int; ITEMQUESIZE] = [0; ITEMQUESIZE];

/// Write index (head) of the item-respawn circular queue.
#[no_mangle]
pub static mut iquehead: c_int = 0;

/// Read index (tail) of the item-respawn circular queue.
#[no_mangle]
pub static mut iquetail: c_int = 0;

/// Per-tic thinker for every active map object.
///
/// Runs XY and Z movement, advances the state machine, and initiates
/// Nightmare-mode respawn for eligible dead monsters.  Detects mid-tick
/// removal via the sentinel function pointer.
///
/// ## Technical Details
///
/// The gate ladder is the demo surface: XY movement runs only when
/// momentum or `MF_SKULLFLY` says so, Z movement only when airborne,
/// and after EACH the thinker re-checks `is_sentinel` -- a mid-tic
/// removal (missile explosion, sky hack) must not touch the freed
/// mobj. The state machine decrements `tics` and transitions at zero
/// (zero-tic chains inside [`super::state::set_mobj_state`]); the
/// Nightmare arm runs only for `MF_COUNTKILL` corpses when
/// `respawnmonsters` is set, after `movecount` reaches `12 * TICRATE`,
/// `leveltime & 31 == 0`, and `P_Random() > 4` fails -- draw count and
/// cadence pinned (one draw per eligible corpse-tic, drawn ONLY on
/// tics that pass every earlier gate).
///
/// ## On Calling
///
/// C ABI thinker callback registered by [`super::spawn::spawn_mobj`]
/// into each mobj's `acp1`; run by `P_RunThinkers`' dispatch. Never
/// call it directly. `mobj` must be a valid, non-null pointer to an
/// `mobj_t` currently linked into the thinker list.
///
/// # Safety
///
/// `mobj` must be a valid, non-null pointer to an `mobj_t` that is currently
/// linked into the thinker list.
#[doc(alias = "P_MobjThinker")]
#[export_name = "P_MobjThinker"]
pub unsafe extern "C" fn mobj_thinker(mobj: *mut mobj_t)
{
    let mobj = &mut *mobj;
    if mobj.momx != 0 || mobj.momy != 0 || mobj.flags & MF_SKULLFLY != 0
    {
        xy_movement(mobj as *mut mobj_t);
        if is_sentinel(mobj.thinker.function)
        {
            return;
        }
    }
    if mobj.z != mobj.floorz || mobj.momz != 0
    {
        z_movement(mobj as *mut mobj_t);
        if is_sentinel(mobj.thinker.function)
        {
            return;
        }
    }

    if mobj.tics != -1
    {
        mobj.tics -= 1;
        if mobj.tics == 0
        {
            let state_ptr = mobj.state as *mut State;
            set_mobj_state(mobj as *mut mobj_t, (*state_ptr).nextstate);
        }
    }
    else
    {
        if mobj.flags & MF_COUNTKILL == 0
        {
            return;
        }
        if respawnmonsters == 0
        {
            return;
        }
        mobj.movecount += 1;
        if mobj.movecount < 12 * TICRATE
        {
            return;
        }
        if leveltime & 31 != 0
        {
            return;
        }
        if P_Random() > 4
        {
            return;
        }
        nightmare_respawn(mobj as *mut mobj_t);
    }
}

/// Respawn a Nightmare-mode monster at its original spawn point.
///
/// Checks that the spawn point is unobstructed, spawns teleport fog at the
/// old and new positions, spawns a fresh copy of the monster, and removes the
/// old corpse.
///
/// ## Technical Details
///
/// The spawn order is the demo surface: old-position fog (with its
/// spawn's `lastlook` draw), new-position fog, then the monster copy
/// whose angle is quantized through
/// [`super::dtmc::mapthing_angle_quantize`], `MF_AMBUSH` is re-applied
/// from the spawnpoint options, `reactiontime` is forced to 18, and
/// the old corpse is removed last. The obstructed probe returns
/// WITHOUT spawning anything (retry next gate).
///
/// ## On Calling
///
/// `mobj` must be a valid, non-null pointer to an `mobj_t` that has
/// `MF_COUNTKILL` set and whose `spawnpoint` field is valid. Called
/// only from [`mobj_thinker`]'s Nightmare arm.
///
/// # Safety
///
/// `mobj` must be a valid, non-null pointer to an `mobj_t` that has
/// `MF_COUNTKILL` set and whose `spawnpoint` field is valid.
#[doc(alias = "P_NightmareRespawn")]
#[export_name = "P_NightmareRespawn"]
pub unsafe extern "C" fn nightmare_respawn(mobj: *mut mobj_t)
{
    let mobj = &mut *mobj;
    let x = (mobj.spawnpoint.x as c_int) << FRACBITS;
    let y = (mobj.spawnpoint.y as c_int) << FRACBITS;
    if P_CheckPosition(mobj as *mut _ as *mut CffiMobj, x, y) == 0
    {
        return;
    }

    let mut mo = spawn_mobj(
        mobj.x,
        mobj.y,
        (*(*mobj.subsector).sector).floorheight,
        MT_TFOG,
    );
    S_StartSound(mo as *mut c_void, Sfx::Telept as c_int);

    let ss = R_PointInSubsector(x, y) as *mut subsector_t;
    mo = spawn_mobj(x, y, (*(*ss).sector).floorheight, MT_TFOG);
    S_StartSound(mo as *mut c_void, Sfx::Telept as c_int);

    let mthing = &mobj.spawnpoint;
    let info = mobj.info as *mut MobjInfo;
    let z = if (*info).flags & MF_SPAWNCEILING != 0
    {
        ONCEILINGZ
    }
    else
    {
        ONFLOORZ
    };

    mo = spawn_mobj(x, y, z, mobj.mobjtype);
    (*mo).spawnpoint = mobj.spawnpoint;
    (*mo).angle = mapthing_angle_quantize(mthing.angle as u32);
    if mthing.options & MTF_AMBUSH as i16 != 0
    {
        (*mo).flags |= MF_AMBUSH;
    }
    (*mo).reactiontime = 18;
    remove_mobj(mobj as *mut mobj_t);
}

/// Unlink `mobj` from the sector/block lists, stop any playing sound, and
/// remove its thinker.
///
/// If the mobj is a collectable special item (not dropped and not an
/// invulnerability sphere / invisibility sphere), its spawn record is pushed
/// onto the item-respawn queue for potential deathmatch respawn.
///
/// ## Technical Details
///
/// The queue push runs FIRST, before the unlink: the spawnpoint copy,
/// the `leveltime` stamp, then the ring step through
/// [`super::dtmc::respawn_queue_step`] -- and when the head catches
/// the tail, the tail is forced forward (the overwrite order). The
/// eligibility gate (`MF_SPECIAL`, not dropped, neither sphere type)
/// decides whether a demo's deathmatch item respawn exists at all.
/// The p_saveg extern declarer links this symbol for the
/// `P_UnArchiveThinkers` first pass (`p_saveg.rs:96`), so the export
/// pin is load-bearing.
///
/// ## On Calling
///
/// `mobj` must be a valid, non-null pointer to an `mobj_t` currently
/// linked into the world. Marks the thinker for lazy removal (the
/// sentinel); the caller's subsequent sentinel checks must observe
/// that.
///
/// # Safety
///
/// `mobj` must be a valid, non-null pointer to an `mobj_t` that is currently
/// linked into the world.
#[doc(alias = "P_RemoveMobj")]
#[export_name = "P_RemoveMobj"]
pub unsafe extern "C" fn remove_mobj(mobj: *mut mobj_t)
{
    let mobj = &mut *mobj;
    if mobj.flags & MF_SPECIAL != 0
        && mobj.flags & MF_DROPPED == 0
        && mobj.mobjtype != MT_INV
        && mobj.mobjtype != MT_INS
    {
        itemrespawnque[iquehead as usize] = mobj.spawnpoint;
        itemrespawntime[iquehead as usize] = leveltime;
        iquehead = respawn_queue_step(iquehead);
        if iquehead == iquetail
        {
            iquetail = respawn_queue_step(iquetail);
        }
    }

    P_UnsetThingPosition(mobj as *mut mobj_t as *mut crate::doom::c_ffi::mobj_t);
    S_StopSound(mobj as *mut mobj_t as *mut MobjStub);
    P_RemoveThinker(&mut mobj.thinker as *mut thinker_t);
}

/// Respawn the oldest queued special item if deathmatch mode 2 is active and
/// the item has been gone for at least 30 seconds.
///
/// Spawns an item-fog effect at the respawn location, then spawns the item
/// itself and advances the queue tail.
///
/// ## Technical Details
///
/// The gate ladder runs without drawing: `deathmatch == 2`, a
/// non-empty ring, and the `30 * TICRATE` dwell check against the
/// stored stamp. The spawn sequence (item fog, then the item via the
/// doomednum walk) and the tail step through
/// [`super::dtmc::respawn_queue_step`] decide WHICH slot a long
/// deathmatch demo respawns next; the respawned mobj's angle is
/// quantized through [`super::dtmc::mapthing_angle_quantize`].
///
/// ## On Calling
///
/// Must be called only during an active level tick, from the tic
/// dispatcher's pinned slot (`P_UpdateSpecials` ->
/// `P_RespawnSpecials`, p_tick/mod.rs). Draws: the two spawns'
/// `lastlook` draws only.
///
/// # Safety
///
/// Must be called only during an active level tick.
#[doc(alias = "P_RespawnSpecials")]
#[export_name = "P_RespawnSpecials"]
pub unsafe extern "C" fn respawn_specials()
{
    if deathmatch != 2
    {
        return;
    }
    if iquehead == iquetail
    {
        return;
    }
    if leveltime - itemrespawntime[iquetail as usize] < 30 * TICRATE
    {
        return;
    }

    let mthing = &itemrespawnque[iquetail as usize];
    let x = (mthing.x as c_int) << FRACBITS;
    let y = (mthing.y as c_int) << FRACBITS;

    let ss = R_PointInSubsector(x, y) as *mut subsector_t;
    let mut mo = spawn_mobj(x, y, (*(*ss).sector).floorheight, MT_IFOG);
    S_StartSound(mo as *mut c_void, Sfx::Itmbk as c_int);

    let mut i = 0;
    while i < NUMMOBJTYPES
    {
        if mthing.r#type as c_int == info::mobjinfo[i].doomednum
        {
            break;
        }
        i += 1;
    }

    let z = if info::mobjinfo[i].flags & MF_SPAWNCEILING != 0
    {
        ONCEILINGZ
    }
    else
    {
        ONFLOORZ
    };

    mo = spawn_mobj(x, y, z, i as c_int);
    (*mo).spawnpoint = *mthing;
    (*mo).angle = mapthing_angle_quantize(mthing.angle as u32);

    iquetail = respawn_queue_step(iquetail);
}

#[cfg(test)]
mod tests
{
    use std::os::raw::c_int;

    use crate::doom::c_ffi::ITEMQUESIZE;
    use crate::doom::p_mobj::{itemrespawnque, itemrespawntime, iquehead, iquetail};

    /// Queue statics keep their `#[no_mangle]` data-tier names and
    /// their module-root paths (`p_setup.rs`, `c_tests/p_mobj_c.rs`).
    #[test]
    fn itemrespawntime_length_is_itemquesize()
    {
        unsafe
        {
            assert_eq!(itemrespawntime.len(), ITEMQUESIZE);
        }
    }

    #[test]
    fn itemrespawnque_length_is_itemquesize()
    {
        unsafe
        {
            assert_eq!(itemrespawnque.len(), ITEMQUESIZE);
        }
    }

    /// The queue starts empty (head == tail) and the indices are
    /// `c_int`-wide, exactly as the pre-split suite pinned.
    #[test]
    fn itemque_starts_empty()
    {
        unsafe
        {
            assert_eq!(
                iquehead, iquetail,
                "item queue must be empty (head == tail) at startup"
            );
        }
    }

    #[test]
    fn itemrespawntime_starts_zeroed()
    {
        unsafe
        {
            for (i, &t) in itemrespawntime.iter().enumerate()
            {
                assert_eq!(t, 0, "itemrespawntime[{i}] should be 0 before level load");
            }
        }
    }

    #[test]
    fn queue_indices_are_c_int_width()
    {
        const _: () = assert!(std::mem::size_of::<c_int>() == 4);
        unsafe
        {
            let _: c_int = iquehead;
            let _: c_int = iquetail;
        }
    }
}
