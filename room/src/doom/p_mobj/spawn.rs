//! Map-object creation: the allocator/initialiser `spawn_mobj` every
//! other spawn path funnels through, the effect and projectile spawners
//! (puff, blood, missiles, player missiles), the launch-time validity
//! probe `check_missile_spawn`, and the vanilla null-pointer
//! substitution dummy `subst_null_mobj` -- bit-exact with the creation
//! half of `vendor/doomgeneric/p_mobj.c`.

use std::ffi::c_void;
use std::os::raw::c_int;
use std::ptr;

use crate::doom::d_player::MAXPLAYERS;
use crate::doom::g_game::gameskill;
use crate::doom::info::{self, *};
use crate::doom::m_fixed::{FixedMul, FRACUNIT};
use crate::doom::m_random::P_Random;
use crate::doom::p_map::{attackrange, linetarget, P_AimLineAttack, P_TryMove};
use crate::doom::p_maputl::{P_AproxDistance, P_SetThingPosition};
use crate::doom::p_tick::{thinker_t, P_AddThinker};
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};
use crate::doom::z_zone::{PU_LEVEL, Z_Malloc};

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
type CffiMobj = crate::doom::c_ffi::mobj_t;

use super::consts::{MELEERANGE, ONCEILINGZ, ONFLOORZ};
use super::dtmc::tics_jitter_clamp;
use super::lifecycle::mobj_thinker;
use super::state::{explode_missile, set_mobj_state};

/// Allocate, initialise, and link a new map object of type `type_` at world
/// position (`x`, `y`, `z`).
///
/// The initial state, sprite, and tic count are taken from `mobjinfo`.
/// `P_MobjThinker` is registered as the thinker.  Use `ONFLOORZ` / `ONCEILINGZ`
/// for `z` to snap to the sector floor or ceiling respectively.
///
/// Returns a pointer to the newly created `mobj_t`.
///
/// ## Technical Details
///
/// The initialisation order is the demo surface: `mobjtype`/info/radius/
/// height/flags/health first, the Nightmare reactiontime suppression
/// (`gameskill != 4`), the `lastlook` RNG draw (`P_Random() %
/// MAXPLAYERS` -- one draw per spawn, pinned), then the spawnstate
/// copy, the position link (which fills `subsector`), and only then
/// the floor/ceiling snap and the `P_MobjThinker` registration. The
/// thinker function stored into `acp1` is THE single
/// [`super::lifecycle::mobj_thinker`] item -- the savegame, precache,
/// and interp thinkers all identify mobjs by comparing against that
/// exact address, so this must never become a wrapper.
///
/// ## On Calling
///
/// Must be called only while a level is active (zone memory must be
/// initialised). `type_` must be a valid `mobjtype_t` index. Returns
/// the new `mobj_t` pointer; the thinker is already linked, so the
/// mobj acts from the next `P_RunThinkers` pass.
///
/// # Safety
///
/// Must be called only while a level is active (zone memory must be
/// initialised).  `type_` must be a valid `mobjtype_t` index.
#[doc(alias = "P_SpawnMobj")]
#[export_name = "P_SpawnMobj"]
pub unsafe extern "C" fn spawn_mobj(x: c_int, y: c_int, z: c_int, type_: c_int) -> *mut mobj_t
{
    let mobj = Z_Malloc(
        std::mem::size_of::<mobj_t>() as c_int,
        PU_LEVEL,
        ptr::null_mut(),
    ) as *mut mobj_t;
    // Z_Malloc already zeroes the allocation internally.
    let mobj_ref = &mut *mobj;
    let info = &mut info::mobjinfo[type_ as usize] as *mut MobjInfo;

    mobj_ref.mobjtype = type_;
    mobj_ref.info = info as *mut crate::doom::p_telept::mobjinfo_t;
    mobj_ref.x = x;
    mobj_ref.y = y;
    mobj_ref.radius = (*info).radius;
    mobj_ref.height = (*info).height;
    mobj_ref.flags = (*info).flags;
    mobj_ref.health = (*info).spawnhealth;

    if gameskill != 4
    {
        // sk_nightmare = 4
        mobj_ref.reactiontime = (*info).reactiontime;
    }

    mobj_ref.lastlook = P_Random() % MAXPLAYERS as c_int;

    let st = &mut info::states[(*info).spawnstate as usize] as *mut State;
    mobj_ref.state = st as *mut crate::doom::p_telept::state_t;
    mobj_ref.tics = (*st).tics;
    mobj_ref.sprite = (*st).sprite;
    mobj_ref.frame = (*st).frame;

    P_SetThingPosition(mobj as *mut crate::doom::c_ffi::mobj_t);

    mobj_ref.floorz = (*(*mobj_ref.subsector).sector).floorheight;
    mobj_ref.ceilingz = (*(*mobj_ref.subsector).sector).ceilingheight;

    if z == ONFLOORZ
    {
        mobj_ref.z = mobj_ref.floorz;
    }
    else if z == ONCEILINGZ
    {
        mobj_ref.z = mobj_ref.ceilingz - (*info).height;
    }
    else
    {
        mobj_ref.z = z;
    }

    mobj_ref.thinker.function.acp1 = Some(core::mem::transmute::<
        unsafe extern "C" fn(*mut mobj_t),
        unsafe extern "C" fn(*mut c_void),
    >(mobj_thinker));

    P_AddThinker(&mut mobj_ref.thinker as *mut thinker_t);

    mobj
}

/// Spawn a bullet-puff visual effect at (`x`, `y`, `z`), randomising the Z
/// slightly and the initial tic count.  Skips to the melee-contact frame
/// (`S_PUFF3`) when the attack was at melee range so punches do not spark.
///
/// ## Technical Details
///
/// Two RNG facts are pinned: the Z jitter is the `(P_Random() -
/// P_Random()) << 10` pair (drawn BEFORE the spawn, so the puff's own
/// `lastlook` draw follows it), and the tic jitter is one more draw
/// via [`super::dtmc::tics_jitter_clamp`]. The melee-range retarget to
/// `S_PUFF3` reads `attackrange` AFTER the spawn.
///
/// ## On Calling
///
/// Must be called during an active level tick with zone memory
/// available. Fires exactly two `P_Random` draws per call plus the
/// spawn's `lastlook` and tic-jitter draws.
///
/// # Safety
///
/// Must be called during an active level tick with zone memory available.
#[doc(alias = "P_SpawnPuff")]
#[export_name = "P_SpawnPuff"]
pub unsafe extern "C" fn spawn_puff(x: c_int, y: c_int, z: c_int)
{
    let z = z + ((P_Random() - P_Random()) << 10);
    let th = spawn_mobj(x, y, z, MT_PUFF);
    (*th).momz = FRACUNIT;
    (*th).tics = tics_jitter_clamp((*th).tics, P_Random());
    if attackrange == MELEERANGE
    {
        set_mobj_state(th, S_PUFF3);
    }
}

/// Spawn a blood-splat visual effect at (`x`, `y`, `z`).
///
/// The initial state is chosen based on `damage`: heavy hits use the default
/// `MT_BLOOD` spawn state, medium hits start at `S_BLOOD2`, and weak hits
/// start at `S_BLOOD3` (smaller splat).
///
/// ## Technical Details
///
/// Same draw ledger as the puff: the `(P_Random() - P_Random()) << 10`
/// Z-jitter pair before the spawn, one tic-jitter draw after. The
/// damage ladder (`9..=12` -> `S_BLOOD2`, below 9 -> `S_BLOOD3`) runs
/// after the jitter draw.
///
/// ## On Calling
///
/// Must be called during an active level tick with zone memory
/// available. `damage` is the hit's damage roll, already drawn by the
/// attacker.
///
/// # Safety
///
/// Must be called during an active level tick with zone memory available.
#[doc(alias = "P_SpawnBlood")]
#[export_name = "P_SpawnBlood"]
pub unsafe extern "C" fn spawn_blood(x: c_int, y: c_int, z: c_int, damage: c_int)
{
    let z = z + ((P_Random() - P_Random()) << 10);
    let th = spawn_mobj(x, y, z, MT_BLOOD);
    (*th).momz = FRACUNIT * 2;
    (*th).tics = tics_jitter_clamp((*th).tics, P_Random());
    if (9..=12).contains(&damage)
    {
        set_mobj_state(th, S_BLOOD2);
    }
    else if damage < 9
    {
        set_mobj_state(th, S_BLOOD3);
    }
}

/// Validate a newly spawned missile: jitter its tic count, nudge it half a
/// step forward along its trajectory, and explode it immediately if that
/// initial position is blocked.
///
/// ## Technical Details
///
/// The tic-jitter draw is the FIRST statement (before the half-step
/// probe), so a missile that explodes on spawn consumes its jitter
/// draw before the death-state draws; the half-step nudge
/// (`mom >> 1` on each axis) is the position vanilla demos observed
/// missiles at one tic after launch. The probe reuses `P_TryMove`,
/// whose blocked-path side effects (touch specials, cross lines) run
/// here exactly as in normal movement.
///
/// ## On Calling
///
/// `th` must be a valid, non-null pointer to a freshly spawned
/// `mobj_t` with `MF_MISSILE` set and non-zero momentum. Consumes one
/// `P_Random` draw plus whatever the probe path draws.
///
/// # Safety
///
/// `th` must be a valid, non-null pointer to an `mobj_t` with `MF_MISSILE`
/// set and non-zero momentum.
#[doc(alias = "P_CheckMissileSpawn")]
#[export_name = "P_CheckMissileSpawn"]
pub unsafe extern "C" fn check_missile_spawn(th: *mut mobj_t)
{
    let th = &mut *th;
    th.tics = tics_jitter_clamp(th.tics, P_Random());
    th.x += th.momx >> 1;
    th.y += th.momy >> 1;
    th.z += th.momz >> 1;
    if P_TryMove(th as *mut _ as *mut CffiMobj, th.x, th.y) == 0
    {
        explode_missile(th as *mut mobj_t);
    }
}

/// Return `mobj` unchanged, or a pointer to a zeroed dummy `mobj_t` if
/// `mobj` is null.
///
/// This substitution avoids null-pointer crashes in code that assumes the
/// pointer is always valid, emulating the original Vanilla Doom behaviour
/// where unchecked null dereferences "worked" due to the absence of memory
/// protection.
///
/// ## Technical Details
///
/// Vanilla comment (kept from `p_mobj.c:925`): "Doom did not crash
/// because of the lack of proper memory protection." The dummy is a
/// function-local `static mut` whose x/y/z/flags fields are re-zeroed
/// on every substitution -- the fields the call sites read, nothing
/// more (catalog entry 12,
/// `docs/vanilla-workarounds.md`). Glue tier: a pure pointer swap
/// with no sim sequence and no RNG draws.
///
/// ## On Calling
///
/// The returned dummy pointer is valid only until the next call to this
/// function from the same thread; callers must not store it across ticks.
///
/// # Safety
///
/// The returned dummy pointer is valid only until the next call to this
/// function from the same thread; callers must not store it across ticks.
#[doc(alias = "P_SubstNullMobj")]
#[export_name = "P_SubstNullMobj"]
pub unsafe extern "C" fn subst_null_mobj(mobj: *mut mobj_t) -> *mut mobj_t
{
    if mobj.is_null()
    {
        static mut DUMMY_MOBJ: mobj_t = unsafe { std::mem::zeroed() };
        DUMMY_MOBJ.x = 0;
        DUMMY_MOBJ.y = 0;
        DUMMY_MOBJ.z = 0;
        DUMMY_MOBJ.flags = 0;
        return std::ptr::addr_of_mut!(DUMMY_MOBJ);
    }
    mobj
}

/// Launch a projectile of type `type_` from `source` toward `dest`.
///
/// The missile is spawned 32 units above `source`'s origin, aimed directly at
/// `dest`'s centre (with random spread if `dest` has `MF_SHADOW`).  Vertical
/// momentum is computed from the Z distance divided by travel time.
///
/// Returns a pointer to the spawned missile `mobj_t`.
///
/// ## Technical Details
///
/// The shadow spread is the demo surface: when `dest` has `MF_SHADOW`,
/// ONE extra `(P_Random() - P_Random()) << 20` pair adjusts the aim
/// angle before the fine-table lookup. The `seesound` fires before the
/// aim computation, `momz` derives from `dist / speed` (floored at 1),
/// and [`check_missile_spawn`] runs last -- all in the pinned order.
///
/// ## On Calling
///
/// `source` and `dest` must be valid, non-null pointers to `mobj_t`
/// instances; `type_` must be a valid `mobjtype_t` index with
/// `MF_MISSILE` in its flags. Draw count: `lastlook` (inside spawn),
/// the shadow pair when applicable, the tic jitter, plus any probe-
/// path draws.
///
/// # Safety
///
/// `source` and `dest` must be valid, non-null pointers to `mobj_t` instances.
/// `type_` must be a valid `mobjtype_t` index with `MF_MISSILE` in its flags.
#[doc(alias = "P_SpawnMissile")]
#[export_name = "P_SpawnMissile"]
pub unsafe extern "C" fn spawn_missile(
    source: *mut mobj_t,
    dest: *mut mobj_t,
    type_: c_int,
) -> *mut mobj_t
{
    let source = &*source;
    let dest = &*dest;
    let th = spawn_mobj(source.x, source.y, source.z + 4 * 8 * FRACUNIT, type_);
    let info = (*th).info as *mut MobjInfo;
    if (*info).seesound != Sfx::None
    {
        S_StartSound(th as *mut c_void, (*info).seesound as c_int);
    }
    (*th).target = source as *const mobj_t as *mut mobj_t;
    let mut an = R_PointToAngle2(source.x, source.y, dest.x, dest.y);
    if dest.flags & MF_SHADOW != 0
    {
        an = an.wrapping_add(((P_Random() - P_Random()) << 20) as u32);
    }
    (*th).angle = an;
    an >>= ANGLETOFINESHIFT;
    (*th).momx = FixedMul((*info).speed, *finecosine.0.add(an as usize));
    (*th).momy = FixedMul((*info).speed, finesine[an as usize]);

    let mut dist = P_AproxDistance(dest.x - source.x, dest.y - source.y);
    dist /= (*info).speed;
    if dist < 1
    {
        dist = 1;
    }
    (*th).momz = (dest.z - source.z) / dist;
    check_missile_spawn(th);
    th
}

/// Launch a projectile of type `type_` from the player's `source` mobj,
/// auto-aiming within a 90-degree cone and then two additional angle offsets
/// before giving up and firing straight ahead.
///
/// ## Technical Details
///
/// The three-cone aim ladder is the demo surface (no RNG here):
/// straight aim first, then `+ 1 << 26`, then `- 2 << 26`, falling
/// back to the player's own angle with zero slope. Each
/// `P_AimLineAttack` probe (reading p_map's `linetarget` scratch
/// static) runs the blockmap traversal whose side effects are
/// position-pinned. The spawn itself and the [`check_missile_spawn`]
/// probe follow.
///
/// ## On Calling
///
/// `source` must be a valid, non-null pointer to a player `mobj_t`;
/// `type_` must be a valid `mobjtype_t` index with `MF_MISSILE` in
/// its flags.
///
/// # Safety
///
/// `source` must be a valid, non-null pointer to a player `mobj_t`.
/// `type_` must be a valid `mobjtype_t` index with `MF_MISSILE` in its flags.
#[doc(alias = "P_SpawnPlayerMissile")]
#[export_name = "P_SpawnPlayerMissile"]
pub unsafe extern "C" fn spawn_player_missile(source: *mut mobj_t, type_: c_int)
{
    let source = &mut *source;
    let mut an = source.angle;
    let mut slope = P_AimLineAttack(source as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);

    if linetarget.is_null()
    {
        an = an.wrapping_add(1 << 26);
        slope = P_AimLineAttack(source as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);
        if linetarget.is_null()
        {
            an = an.wrapping_sub(2 << 26);
            slope = P_AimLineAttack(source as *mut _ as *mut CffiMobj, an, 16 * 64 * FRACUNIT);
            if linetarget.is_null()
            {
                an = source.angle;
                slope = 0;
            }
        }
    }

    let x = source.x;
    let y = source.y;
    let z = source.z + 4 * 8 * FRACUNIT;

    let th = spawn_mobj(x, y, z, type_);
    let info = (*th).info as *mut MobjInfo;
    if (*info).seesound != Sfx::None
    {
        S_StartSound(th as *mut c_void, (*info).seesound as c_int);
    }
    (*th).target = source as *const mobj_t as *mut mobj_t;
    (*th).angle = an;
    let fine_idx = (an >> ANGLETOFINESHIFT) as usize;
    (*th).momx = FixedMul((*info).speed, *finecosine.0.add(fine_idx));
    (*th).momy = FixedMul((*info).speed, finesine[fine_idx]);
    (*th).momz = FixedMul((*info).speed, slope);

    check_missile_spawn(th);
}
