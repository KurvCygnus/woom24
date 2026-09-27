//! Demo-synchronization surface extracted from `p_inter`: the four
//! pure computations behind the module's mid-tic `P_Random` consumers
//! and damage arithmetic -- the armor absorption split with its
//! exhaustion clamp, the four-arm fall-forward guard, the knockback
//! thrust formula, and the death-tic roll + clamp -- whose exact
//! integer results are pinned by the baseline vectors below.

use std::ffi::c_int;

use crate::doom::m_fixed::FRACUNIT;

/// The armor absorption step of `P_DamageMobj`: split `damage` into the
/// saved part (`damage / 3` for green armor, `damage / 2` otherwise),
/// clamp the saved amount to the remaining armor points (dropping
/// `armortype` to 0 on exhaustion), and return the surviving damage
/// together with the post-absorption armor state
/// (`vendor/doomgeneric/p_inter.c:858-873`; no named C function, hence
/// no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The integer division is load-bearing: `damage / 3` and `damage / 2`
/// truncate toward zero exactly as C division did, and the exhaustion
/// predicate reads `armorpoints <= saved` (not `<`), so an armor
/// score exactly equal to the saved amount is consumed AND the armor
/// type is lost. The helper returns `(damage_after, armorpoints_after,
/// armortype_after)`; `damage_after` feeds `player.health` and
/// `target.health` (i.e. whether the kill threshold is crossed), so any
/// change to the split or the clamp moves the demo state hash. God
/// mode and the sub-1000 gate live in `P_DamageMobj` BEFORE this
/// computation and must never fold into it.
///
/// ## On Calling
///
/// Pure integer computation: pass the incoming `damage`, the player's
/// current `armortype`, and the current `armorpoints`, and store the
/// returned tuple back in the same order the original body wrote it
/// (armorpoints, then armortype, then damage). Never debug-assert on
/// overflow -- real damage values are small, and the original body
/// used plain integer arithmetic. Single-threaded sim use.
pub fn armor_absorption(damage: c_int, armortype: c_int, armorpoints: c_int) -> (c_int, c_int, c_int)
{
    let saved = if armortype == 1
    {
        damage / 3
    }
    else
    {
        damage / 2
    };
    let mut saved_actual = saved;
    let mut armortype_after = armortype;
    if armorpoints <= saved_actual
    {
        saved_actual = armorpoints;
        armortype_after = 0;
    }
    (
        damage - saved_actual,
        armorpoints - saved_actual,
        armortype_after,
    )
}

/// The four-arm fall-forward guard of `P_DamageMobj`: `true` exactly
/// when a below-40 blow that will kill its target arrives from far
/// below (`z_delta > 64 * FRACUNIT`) and the drawn byte is odd -- the
/// condition under which vanilla flips the corpse's facing
/// (`ang += ANG180`) and quadruples its knockback thrust
/// (`p_inter.c:824-831`; no named C function, hence no
/// `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The arm ORDER is load-bearing, and so is where the draw happens:
/// the caller keeps the three cheap arms (`damage < 40`, `damage >
/// target.health`, `z_delta > 64 * FRACUNIT`) in its `&&` chain ahead
/// of this call, so the `P_Random()` argument is only evaluated when
/// the original body would draw. Drawing eagerly on a failed guard
/// would consume one extra `RNDTABLE` byte and shift every later draw
/// of the tic -- the `baseline_damage_fall_forward_flip_draw_pins`
/// vectors pin that no-draw property against the real body. The
/// helper re-verifies all four arms so its truth table is the
/// complete original predicate.
///
/// ## On Calling
///
/// Pure integer predicate: pass the same `damage` / `target.health` /
/// `z_delta` the chain arms used (never recomputed copies) and the
/// byte just drawn. Only call it from the short-circuited chain
/// position described above; calling it unconditionally with a fresh
/// `P_Random()` changes the demo RNG sequence.
pub fn fall_forward_flip(damage: c_int, health: c_int, z_delta: c_int, rand: c_int) -> bool
{
    damage < 40 && damage > health && z_delta > 64 * FRACUNIT && (rand & 1) != 0
}

/// The knockback thrust formula of `P_DamageMobj`:
/// `damage * (FRACUNIT >> 3) * 100 / mass` -- the horizontal impulse
/// magnitude later scaled by the impact cosine and, on a
/// [`fall_forward_flip`] hit, by four (`p_inter.c:821`; no named C
/// function, hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// Plain truncating integer division, exactly as C: a mass that does
/// not divide the product evenly drops the fraction (pinned by the
/// mass-7 vector below). Valid WADs never carry `mass == 0`, and the
/// original body had no guard -- do not add an assert or a default;
/// the division-by-zero panic on a corrupt WAD is the honest
/// translation of the C behavior. The `FRACUNIT >> 3` scale (1/8 in
/// fixed-point, i.e. `<< 13`) is vanilla's tuning constant.
///
/// ## On Calling
///
/// Pure integer computation: pass the (post-skill-halving) `damage`
/// and the target's `info.mass` raw. Do not widen the arithmetic --
/// the C original computed the product in `int` and overflow wrapped;
/// keeping plain `*` reproduces the release-build behavior for every
/// input a demo can produce.
pub fn thrust_for(damage: c_int, mass: c_int) -> c_int
{
    damage * (FRACUNIT >> 3) * 100 / mass
}

/// The death-tic roll of `P_KillMobj`: subtract up to three tics from
/// the freshly-entered death state's duration (`rand & 3`) and clamp
/// the result to a minimum of one tic, so every corpse animates on
/// its next thinker visit (`p_inter.c:724-727`; no named C function,
/// hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The draw order is demo-visible: `P_KillMobj` consumes exactly one
/// `P_Random` byte per kill, and the byte is the SUBTRAHEND, not the
/// new duration. The `< 1` clamp is what keeps a state whose tics the
/// roll would drive below one from being skipped by `P_MovePsprites` /
/// the thinker loop on the following tic. The caller draws
/// (`P_Random()`) at the exact original statement position and stores
/// the returned value back into `target.tics`.
///
/// ## On Calling
///
/// Pure integer computation: pass the tic count just written by
/// `P_SetMobjState` and the byte just drawn, and store the result
/// unmodified. Do not reorder the draw relative to the kill
/// accounting -- the byte position in the tic's sequence is pinned by
/// demo goldens.
pub fn death_tic_roll(tics: c_int, rand: c_int) -> c_int
{
    let tics = tics - (rand & 3);
    if tics < 1
    {
        1
    }
    else
    {
        tics
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::d_player::{players, PlayerT, CF_GODMODE};
    use crate::doom::g_game::gameskill;
    use crate::doom::info::{MobjInfo, MF_SHOOTABLE, S_PLAY_ATK1};
    use crate::doom::m_fixed::{FixedMul, FRACUNIT};
    use crate::doom::m_random::{prndindex, RNDTABLE};
    use crate::doom::p_telept::{mobj_t, sector_t, subsector_t};
    use crate::doom::tables::{finecosine, finesine};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Read the `prndindex` cursor for assertions without creating a
    /// shared reference to the mutable static (the `static_mut_refs`
    /// hazard a direct `assert_eq!(prndindex, ...)` would trigger).
    fn prnd_index() -> c_int
    {
        unsafe { std::ptr::addr_of!(prndindex).read() }
    }

    /// Set up a bare shootable target around a zeroed `mobj_t` with a
    /// private zeroed `MobjInfo` and a zeroed `subsector -> sector`
    /// chain wired in, so the `P_DamageMobj` / `P_KillMobj` bodies can
    /// run without map data. Zeroed `Sfx` is `Sfx::None`
    /// (`#[repr(C)]`, discriminant 0), so `mem::zeroed` on `MobjInfo`
    /// is a valid bit pattern.
    ///
    /// # Safety
    /// The returned pointer aliases the locals; the caller must keep
    /// them alive for the duration of the call.
    unsafe fn bare_target(
        mobj: &mut mobj_t,
        info: &mut MobjInfo,
        subsector: &mut subsector_t,
        sector: &mut sector_t,
    ) -> *mut mobj_t
    {
        info.mass = 100;
        info.painchance = 0;
        info.spawnhealth = 100;
        info.xdeathstate = 0;
        info.deathstate = S_PLAY_ATK1;
        subsector.sector = sector as *mut _;
        mobj.flags = MF_SHOOTABLE;
        mobj.health = 100;
        mobj.info = info as *mut MobjInfo as *mut _;
        mobj.subsector = subsector as *mut _;
        mobj.mobjtype = 0; // MT_PLAYER: not in P_KillMobj's drop table
        mobj as *mut _
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file armor-absorption block of
    /// `P_DamageMobj` BEFORE the computation extracted into
    /// `dtmc::armor_absorption`, then retargeted -- same vectors, same
    /// results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the armor block -> dtmc -- the `damage / 3 |
    /// damage / 2` split and the exhaustion clamp feed `player.health`,
    /// `player.armorpoints` and the surviving `damage` that lands on
    /// `target.health` (i.e. whether the kill threshold is crossed).
    /// God mode / the sub-1000 gate are handled OUTSIDE the block and
    /// stay outside the helper (vectors 5 and 6 pin both sides of the
    /// gate).
    #[test]
    fn baseline_damage_armor_absorption_vectors()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let saved_skill = gameskill;
            gameskill = 2; // not sk_baby: no damage halving
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // `P_DamageMobj` touches only the fields assigned below
            // plus the `gameskill` static.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut mi: MobjInfo = std::mem::zeroed();
            let mut sub: subsector_t = std::mem::zeroed();
            let mut sec: sector_t = std::mem::zeroed();
            let target = bare_target(&mut mo, &mut mi, &mut sub, &mut sec);

            // (1) Green armor: damage 60 -> saved 20, damage_after 40.
            let mut p: PlayerT = std::mem::zeroed();
            (*target).player = &mut p as *mut PlayerT as *mut _;
            p.armortype = 1;
            p.armorpoints = 100;
            p.health = 100;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target, std::ptr::null_mut(), std::ptr::null_mut(), 60);
            assert_eq!(p.armorpoints, 80);
            assert_eq!(p.armortype, 1);
            assert_eq!(p.health, 60);
            assert_eq!(p.damagecount, 40);
            assert_eq!(p.attacker, std::ptr::null_mut());
            // Survived: exactly the painchance probe (1 byte) consumed.
            assert_eq!(prnd_index(), 1);

            // (2) Blue armor: damage 60 -> saved 30, damage_after 30.
            let mut mo2: mobj_t = std::mem::zeroed();
            let target2 = bare_target(&mut mo2, &mut mi, &mut sub, &mut sec);
            let mut p2: PlayerT = std::mem::zeroed();
            (*target2).player = &mut p2 as *mut PlayerT as *mut _;
            p2.armortype = 2;
            p2.armorpoints = 100;
            p2.health = 100;
            crate::doom::p_inter::P_DamageMobj(target2, std::ptr::null_mut(), std::ptr::null_mut(), 60);
            assert_eq!(p2.armorpoints, 70);
            assert_eq!(p2.armortype, 2);
            assert_eq!(p2.health, 70);

            // (3) Exhaustion: (60, type 1, ap 15) -> saved clamps to 15,
            // armortype drops to 0, damage_after 45.
            let mut mo3: mobj_t = std::mem::zeroed();
            let target3 = bare_target(&mut mo3, &mut mi, &mut sub, &mut sec);
            let mut p3: PlayerT = std::mem::zeroed();
            (*target3).player = &mut p3 as *mut PlayerT as *mut _;
            p3.armortype = 1;
            p3.armorpoints = 15;
            p3.health = 100;
            crate::doom::p_inter::P_DamageMobj(target3, std::ptr::null_mut(), std::ptr::null_mut(), 60);
            assert_eq!(p3.armorpoints, 0);
            assert_eq!(p3.armortype, 0);
            assert_eq!(p3.health, 55);

            // (4) Zero-damage boundary (0, type 1, ap 0): the
            // exhaustion branch fires (0 <= 0) even on zero damage.
            let mut mo4: mobj_t = std::mem::zeroed();
            let target4 = bare_target(&mut mo4, &mut mi, &mut sub, &mut sec);
            let mut p4: PlayerT = std::mem::zeroed();
            (*target4).player = &mut p4 as *mut PlayerT as *mut _;
            p4.armortype = 1;
            p4.armorpoints = 0;
            p4.health = 100;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target4, std::ptr::null_mut(), std::ptr::null_mut(), 0);
            assert_eq!(p4.armortype, 0);
            assert_eq!(p4.armorpoints, 0);
            assert_eq!(p4.health, 100); // damage_after 0: health untouched
            assert_eq!(prnd_index(), 1); // survive: pain probe only

            // (5) God mode below 1000 is a genuine zero-draw early
            // return: NOTHING is touched, not even the armor block.
            let mut mo5: mobj_t = std::mem::zeroed();
            let target5 = bare_target(&mut mo5, &mut mi, &mut sub, &mut sec);
            let mut p5: PlayerT = std::mem::zeroed();
            (*target5).player = &mut p5 as *mut PlayerT as *mut _;
            p5.armortype = 1;
            p5.armorpoints = 100;
            p5.health = 100;
            p5.cheats = CF_GODMODE;
            let before5 = prnd_index();
            crate::doom::p_inter::P_DamageMobj(target5, std::ptr::null_mut(), std::ptr::null_mut(), 60);
            assert_eq!(p5.armorpoints, 100);
            assert_eq!(p5.armortype, 1);
            assert_eq!(p5.health, 100);
            assert_eq!(prnd_index(), before5); // NO draw

            // (6) God mode does NOT stop damage >= 1000: armor still
            // absorbs (saved 1000/3 = 333 clamps to the 100 armor
            // points -> exhaustion), the mobj dies, and the only draw
            // is P_KillMobj's death-tic roll (pain path unreached).
            // The dying player must BE players[0]: P_KillMobj's frag
            // accounting does `target_player.offset_from(players[0])`,
            // which is only valid for a slot of the real array.
            let mut mo6: mobj_t = std::mem::zeroed();
            let target6 = bare_target(&mut mo6, &mut mi, &mut sub, &mut sec);
            let p6 = std::ptr::addr_of_mut!(players[0]);
            (*target6).player = p6 as *mut _;
            (*p6).armortype = 1;
            (*p6).armorpoints = 100;
            (*p6).health = 100;
            (*p6).cheats = CF_GODMODE;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target6, std::ptr::null_mut(), std::ptr::null_mut(), 1000);
            assert_eq!((*p6).armorpoints, 0);
            assert_eq!((*p6).armortype, 0); // 100 <= 333: exhaustion
            assert_eq!((*p6).health, 0); // 100 - 900 clamped
            assert_eq!((*p6).playerstate, 1); // PST_DEAD via P_KillMobj
            assert_eq!((*p6).frags[0], 1); // frag credited to players[0]
            assert_eq!(prnd_index(), 1); // kill roll only
            // Restore the shared players[0] slot (the statics start
            // zeroed; leave every touched field zeroed again -- incl.
            // health and the psprite sy the P_DropWeapon -> A_Lower
            // walk moved).
            (*p6).health = 0;
            (*p6).armortype = 0;
            (*p6).cheats = 0;
            (*p6).playerstate = 0;
            (*p6).damagecount = 0;
            (*p6).frags[0] = 0;
            (*p6).attacker = std::ptr::null_mut();
            (*p6).psprites[0].state = std::ptr::null_mut();
            (*p6).psprites[0].tics = 0;
            (*p6).psprites[0].sy = 0;

            gameskill = saved_skill;
        }
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file knockback + fall-forward block
    /// of `P_DamageMobj` BEFORE the guard extracted into
    /// `dtmc::fall_forward_flip`, then retargeted -- same vectors,
    /// same results.
    ///
    /// Adjudication: the four-arm guard -> dtmc -- its short-circuit
    /// ORDER is load-bearing: the `P_Random()` draw fires ONLY when
    /// `damage < 40 && damage > target.health && z_delta >
    /// 64*FRACUNIT` all pass, so a helper that draws eagerly would
    /// shift every later byte of the tic. The prndindex deltas below
    /// pin the no-draw-when-guard-fails property against the real
    /// body; the momx vectors pin the `ang += ANG180; thrust *= 4`
    /// consequence riding the guard.
    #[test]
    fn baseline_damage_fall_forward_flip_draw_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let saved_skill = gameskill;
            gameskill = 2;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // the thrust path touches only the fields assigned below
            // plus the `gameskill` static. The empty world
            // (`bmapwidth == 0`) makes every `P_Random`-bearing arm
            // observable in isolation.
            let mut mi: MobjInfo = std::mem::zeroed();
            let mut sub: subsector_t = std::mem::zeroed();
            let mut sec: sector_t = std::mem::zeroed();
            let mut inflictor: mobj_t = std::mem::zeroed();
            inflictor.x = -FRACUNIT;
            inflictor.y = 0;
            inflictor.z = 0;

            // (a) Survive, guard fails at arm 2 (damage < health): NO
            // flip draw; thrust still applied eastward; exactly the
            // painchance probe consumed (with painchance 0 -- the draw
            // happens BEFORE the compare). finecosine[0] = 65535 and
            // finesine[0] = 25 (this table carries slop), so the
            // assertions read the table entries themselves.
            let mut mo: mobj_t = std::mem::zeroed();
            let target = bare_target(&mut mo, &mut mi, &mut sub, &mut sec);
            (*target).health = 100;
            (*target).z = 0;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target, &mut inflictor, std::ptr::null_mut(), 20);
            let thrust_a = 20 * (FRACUNIT >> 3) * 100 / 100;
            assert_eq!((*target).momx, FixedMul(thrust_a, *finecosine.0.add(0)));
            assert_eq!((*target).momy, FixedMul(thrust_a, finesine[0]));
            assert_eq!(prnd_index(), 1); // pain probe only, no flip draw

            // (b) Kill, guard fails at arm 3 (z_delta 10F <= 64F): no
            // flip draw; death-tic roll is the only byte.
            let mut mo_b: mobj_t = std::mem::zeroed();
            let target_b = bare_target(&mut mo_b, &mut mi, &mut sub, &mut sec);
            (*target_b).health = 10;
            (*target_b).z = 10 * FRACUNIT;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target_b, &mut inflictor, std::ptr::null_mut(), 20);
            assert_eq!(prnd_index(), 1); // kill roll only, no flip draw
            // The kill roll is the call's FIRST and only draw: cursor
            // 0 -> 1 reads RNDTABLE[1] = 8, and 8 & 3 == 0.
            assert_eq!((*target_b).tics, 12 - (RNDTABLE[1] as c_int & 3));

            // (c) Kill, guard passes, drawn byte even (RNDTABLE[1] =
            // 8): flip draw fires but the predicate is false -- momx
            // is the plain eastward thrust, NOT quadrupled.
            let mut mo_c: mobj_t = std::mem::zeroed();
            let target_c = bare_target(&mut mo_c, &mut mi, &mut sub, &mut sec);
            (*target_c).health = 10;
            (*target_c).z = 100 * FRACUNIT;
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target_c, &mut inflictor, std::ptr::null_mut(), 20);
            let thrust_c = 20 * (FRACUNIT >> 3) * 100 / 100;
            assert_eq!((*target_c).momx, FixedMul(thrust_c, *finecosine.0.add(0)));
            assert_eq!(prnd_index(), 2); // flip draw + kill roll

            // (d) Kill, guard passes, drawn byte odd (RNDTABLE[16] =
            // 211): the flip fires -- ang += ANG180 (fine-cos index
            // 4096) and thrust *= 4.
            let mut mo_d: mobj_t = std::mem::zeroed();
            let target_d = bare_target(&mut mo_d, &mut mi, &mut sub, &mut sec);
            (*target_d).health = 10;
            (*target_d).z = 100 * FRACUNIT;
            prndindex = 15;
            crate::doom::p_inter::P_DamageMobj(target_d, &mut inflictor, std::ptr::null_mut(), 20);
            assert_eq!((*target_d).momx, FixedMul(thrust_c * 4, *finecosine.0.add(4096)));
            assert_eq!((*target_d).momy, FixedMul(thrust_c * 4, finesine[4096]));
            assert_eq!(prnd_index(), 17); // flip draw + kill roll
            assert_eq!(RNDTABLE[16], 211);
            assert_eq!(RNDTABLE[1], 8);

            gameskill = saved_skill;
        }
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file death-tic roll of `P_KillMobj`
    /// BEFORE the roll + clamp extracted into `dtmc::death_tic_roll`,
    /// then retargeted -- same vectors, same results.
    ///
    /// Adjudication: the roll -> dtmc -- `P_KillMobj` consumes exactly
    /// one `P_Random` byte per kill (the `(rand & 3)` subtract with
    /// its `< 1` clamp), and that byte position in the tic's sequence
    /// is demo-visible.
    #[test]
    fn baseline_kill_death_tic_roll_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // `P_KillMobj` touches only the fields assigned below.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut mi: MobjInfo = std::mem::zeroed();
            let mut sub: subsector_t = std::mem::zeroed();
            let mut sec: sector_t = std::mem::zeroed();
            let target = bare_target(&mut mo, &mut mi, &mut sub, &mut sec);

            // Death state S_PLAY_ATK1 sets tics = 12 (action None).
            // Byte RNDTABLE[1] = 8 -> 8 & 3 == 0: tics unchanged.
            prndindex = 0;
            (*target).tics = 0;
            crate::doom::p_inter::P_KillMobj(std::ptr::null_mut(), target);
            assert_eq!((*target).tics, 12);
            assert_eq!(prnd_index(), 1); // exactly one draw

            // Byte RNDTABLE[2] = 109 -> 109 & 3 == 1: 12 - 1 = 11.
            (*target).tics = 0;
            crate::doom::p_inter::P_KillMobj(std::ptr::null_mut(), target);
            assert_eq!((*target).tics, 12 - (RNDTABLE[2] as c_int & 3));
            assert_eq!(prnd_index(), 2); // exactly one draw again

            // Byte RNDTABLE[16] = 211 -> 211 & 3 == 3: 12 - 3 = 9.
            // (P_SetMobjState resets tics to 12 before every roll, so
            // the < 1 clamp can never fire through this body with a
            // valid death state; the clamp arm is pinned at helper
            // level by the death_tic_roll vectors.)
            prndindex = 15;
            (*target).tics = 0;
            crate::doom::p_inter::P_KillMobj(std::ptr::null_mut(), target);
            assert_eq!((*target).tics, 12 - (RNDTABLE[16] as c_int & 3));
            assert_eq!(prnd_index(), 16);
        }
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file knockback formula of
    /// `P_DamageMobj` BEFORE the computation extracted into
    /// `dtmc::thrust_for`, then retargeted -- same vectors, same
    /// results. The mass=7 case pins truncating division.
    #[test]
    fn baseline_thrust_for_body_vectors()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let saved_skill = gameskill;
            gameskill = 2;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // the thrust path touches only the fields assigned below
            // plus the `gameskill` static.
            let mut sub: subsector_t = std::mem::zeroed();
            let mut sec: sector_t = std::mem::zeroed();
            let mut inflictor: mobj_t = std::mem::zeroed();
            inflictor.x = -FRACUNIT;

            // (15, mass 100): 15 * 8192 * 100 / 100 = 122880.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut mi: MobjInfo = std::mem::zeroed();
            let target = bare_target(&mut mo, &mut mi, &mut sub, &mut sec);
            (*target).health = 100; // survive: pain probe closes the call
            prndindex = 0;
            crate::doom::p_inter::P_DamageMobj(target, &mut inflictor, std::ptr::null_mut(), 15);
            // The body routes the thrust through finecosine[0] = 65535
            // (this table carries slop), so the momx delta is the
            // FixedMul product, not the raw 122880 the helper returns.
            assert_eq!((*target).momx, FixedMul(122880, *finecosine.0.add(0)));
            assert_eq!(prnd_index(), 1);

            // (15, mass 7): 12288000 / 7 = 1755428.57... -> 1755428
            // (plain truncating division, no rounding).
            let mut mo2: mobj_t = std::mem::zeroed();
            let mut mi2: MobjInfo = std::mem::zeroed();
            let target2 = bare_target(&mut mo2, &mut mi2, &mut sub, &mut sec);
            // bare_target pins mass = 100; this vector needs the
            // truncating-division mass AFTER that reset. Write through
            // the mobj's info pointer (the alias the compiler cannot
            // see through `&mut mi2` alone).
            (*((*target2).info as *mut MobjInfo)).mass = 7;
            (*target2).health = 100;
            crate::doom::p_inter::P_DamageMobj(target2, &mut inflictor, std::ptr::null_mut(), 15);
            assert_eq!(
                (*target2).momx,
                FixedMul(12288000 / 7, *finecosine.0.add(0))
            );

            gameskill = saved_skill;
        }
    }

    /// Baseline contract (F10 wave B1): the extracted helpers
    /// reproduce the exact values the body-level tests above captured
    /// against the pre-extraction bodies -- same vectors, same
    /// results. These known vectors are pure arithmetic (no finetable
    /// slop involved on the helper level).
    #[test]
    fn baseline_helper_known_vectors()
    {
        // armor_absorption: (damage, armortype, armorpoints) ->
        // (damage_after, armorpoints_after, armortype_after).
        assert_eq!(armor_absorption(60, 1, 100), (40, 80, 1));
        assert_eq!(armor_absorption(60, 2, 100), (30, 70, 2));
        assert_eq!(armor_absorption(60, 1, 15), (45, 0, 0)); // exhaustion
        assert_eq!(armor_absorption(0, 1, 0), (0, 0, 0)); // boundary
        // 1000/3 = 333 truncates; exhaustion clamps saved to ap.
        assert_eq!(armor_absorption(1000, 1, 100), (900, 0, 0));
        // A non-1 armortype uses the /2 split and is PRESERVED when
        // not exhausted (only exhaustion drops the type to 0).
        assert_eq!(armor_absorption(60, 3, 100), (30, 70, 3));
        // Equal armorpoints == saved: exhaustion still fires (<=).
        assert_eq!(armor_absorption(30, 2, 15), (15, 0, 0));

        // fall_forward_flip: the full four-arm truth table, including
        // every boundary the body-level pins exercise.
        assert!(fall_forward_flip(39, 10, 64 * FRACUNIT + 1, 1));
        assert!(!fall_forward_flip(40, 10, 64 * FRACUNIT + 1, 1)); // arm 1: 40 < 40 fails
        assert!(!fall_forward_flip(39, 39, 64 * FRACUNIT + 1, 1)); // arm 2: 39 > 39 fails
        assert!(!fall_forward_flip(39, 10, 64 * FRACUNIT, 1)); // arm 3: not >
        assert!(!fall_forward_flip(39, 10, 64 * FRACUNIT - 1, 3));
        assert!(!fall_forward_flip(39, 10, 64 * FRACUNIT + 1, 0)); // even byte
        assert!(fall_forward_flip(39, 10, 64 * FRACUNIT + 1, 3)); // odd byte
        assert!(!fall_forward_flip(20, 100, 65 * FRACUNIT, 1)); // damage < health

        // thrust_for: known vectors incl. truncating division.
        assert_eq!(thrust_for(15, 100), 122880);
        assert_eq!(thrust_for(15, 7), 12288000 / 7);
        assert_eq!(thrust_for(39, 100), 319488);
        assert_eq!(thrust_for(0, 100), 0);

        // death_tic_roll: subtract + clamp.
        assert_eq!(death_tic_roll(12, 8), 12); // 8 & 3 == 0
        assert_eq!(death_tic_roll(12, 211), 9); // 211 & 3 == 3
        assert_eq!(death_tic_roll(5, 0), 5); // no draw coupling
        assert_eq!(death_tic_roll(2, 211), 1); // clamp arm
        assert_eq!(death_tic_roll(1, 211), 1); // clamp arm at the floor
    }
}
