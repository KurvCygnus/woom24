//! Demo-synchronization surface extracted from `p_pspr`: the four
//! pure roll expressions behind the module's dense `P_Random`
//! consumers -- the melee damage roll shared by `A_Punch` and `A_Saw`,
//! the gunshot damage roll shared by `P_GunShot` and
//! `A_FireShotgun2`, the u32-wrap spread pair, and the BFG spray
//! damage accumulator -- whose exact integer results are pinned by the
//! baseline vectors below.

use std::ffi::c_int;

/// The melee damage roll of `A_Punch` and `A_Saw`:
/// `((rand % 10) + 1) << 1` -- two to twenty points in steps of two
/// (`p_pspr.c:467` / `p_pspr.c:501`; no named C function, hence no
/// `#[doc(alias)]`). The berserk ×10 multiplier stays at the call
/// site, applied to the returned value.
///
/// ## Technical Details
///
/// `% 10` on a byte gives 0..=9, so the roll is strictly positive and
/// even; the `<< 1` shape is vanilla's and is value-identical to the
/// `2 * (...)` spelling `A_Saw` used. Each call consumes exactly one
/// `RNDTABLE` byte at the caller's draw position -- the fist and the
/// saw each draw this byte FIRST, before their spread pair, and every
/// later draw of the tic keys off that cursor position.
///
/// ## On Calling
///
/// Pure integer computation: pass the byte just drawn from
/// `P_Random()` and store the result (after the berserk multiply, for
/// the fist) into the damage argument of the attack. Never draw
/// speculatively or batch the melee and spread draws -- the 3-byte
/// order is pinned by demo goldens.
pub fn melee_roll(rand: c_int) -> c_int
{
    ((rand % 10) + 1) << 1
}

/// The hitscan damage roll of `P_GunShot` and the `A_FireShotgun2`
/// pellet loop: `5 * ((rand % 3) + 1)` -- five, ten, or fifteen
/// points per bullet (`p_pspr.c:642` / `p_pspr.c:727`; no named C
/// function, hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// `% 3` on a byte gives 0..=2, so each pellet lands in {5, 10, 15}.
/// The SSG consumes five bytes per pellet in the exact order damage,
/// spread pair, slope pair (100 per fire) -- the densest RNG consumer
/// in the engine -- so any change to this formula shifts every later
/// draw of the tic.
///
/// ## On Calling
///
/// Pure integer computation: pass the byte just drawn from
/// `P_Random()` at the original statement position and feed the
/// result to `P_LineAttack` unchanged.
pub fn gunshot_roll(rand: c_int) -> c_int
{
    5 * (rand % 3 + 1)
}

/// The random horizontal spread pair of the gun attacks, pre-folded:
/// `((r1 - r2) as u32) << shift`, where `r1` is the byte drawn FIRST
/// and `r2` the one drawn second (`A_Punch` / `A_Saw` / `P_GunShot`
/// shift 18, `p_pspr.c:473/503/646`; `A_FireShotgun2` shifts 19,
/// `p_pspr.c:729`; no named C function, hence no `#[doc(alias)]`).
///
/// ## Technical Details
///
/// The subtraction happens in `c_int` and the difference is then
/// reinterpreted as `u32` BEFORE the shift -- that u32 wrap is
/// load-bearing: a negative difference becomes a huge unsigned value
/// whose shifted low bits are the angle perturbation `wrapping_add`
/// applies to the firing angle. Swapping the operand order, signing
/// the shift, or shifting the signed value all produce different
/// angles for the same bytes. The SSG's slope offset keeps its
/// separate `c_int` (signed-shift) shape inline at its call site --
/// do not route it through this helper. The two draws stay at the
/// call site in first/second order (Rust evaluates arguments
/// left-to-right, so `spread_angle(P_Random(), P_Random(), shift)`
/// preserves it).
///
/// ## On Calling
///
/// Pure integer computation: pass the two bytes in draw order
/// (first, second) and the site's shift (18 or 19), and
/// `wrapping_add` the returned `u32` into the firing angle exactly
/// where the original expression stood.
pub fn spread_angle(r1: c_int, r2: c_int, shift: u32) -> u32
{
    ((r1 - r2) as u32) << shift
}

/// One accumulator step of the `A_BFGSpray` damage ladder:
/// `acc + (rand & 7) + 1` -- per hit target, fifteen draws build
/// 15..=120 damage (`p_pspr.c:814-816`; no named C function, hence
/// no `#[doc(alias)]`). The 15-draw loop stays at the call site;
/// only the addition lives here.
///
/// ## Technical Details
///
/// `(rand & 7) + 1` contributes 1..=8 per byte, so a full ladder is
/// 15..=120. A hit target consumes EXACTLY fifteen bytes in a row
/// before the damage call; a miss consumes none -- the
/// `baseline_bfg_spray_miss_draw_pin` vector pins that zero-draw
/// property against the real body. The accumulation order is the
/// demo-visible byte order of the spray.
///
/// ## On Calling
///
/// Pure integer computation: fold each drawn byte in draw order
/// (`damage = spray_step(damage, P_Random())`) exactly where the
/// original `+=` stood, then pass the total to `P_DamageMobj`.
pub fn spray_step(acc: c_int, rand: c_int) -> c_int
{
    acc + (rand & 7) + 1
}

#[cfg(test)]
mod tests
{
    use super::{gunshot_roll, melee_roll, spread_angle, spray_step};
    use crate::doom::c_ffi::WEAPONTOP;
    use std::ffi::c_int;
    use crate::doom::d_player::{players, PlayerT, PspdefT};
    use crate::doom::info::{S_CHAIN1, S_NULL};
    use crate::doom::p_pspr::state::{
        am_clip, am_shell, wp_chaingun, wp_nochange, wp_pistol, wp_shotgun, wp_supershotgun,
        ANG90,
    };
    use crate::doom::m_fixed::{FixedMul, FRACUNIT};
    use crate::doom::m_random::prndindex;
    use crate::doom::p_pspr::{
        A_BFGSpray, A_FireCGun, A_FirePistol, A_FireShotgun, A_FireShotgun2, A_Punch, A_Saw,
        A_WeaponReady, P_BulletSlope, P_CheckAmmo, P_GunShot, P_CalcSwing, P_SetPsprite,
    };
    use crate::doom::p_telept::mobj_t;
    use crate::doom::p_tick::leveltime;
    use crate::doom::s_sound::snd_channels;
    use crate::doom::tables::{finecosine, finesine, FINEANGLES, FINEMASK};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Read the `prndindex` cursor for assertions without creating a
    /// shared reference to the mutable static (the `static_mut_refs`
    /// hazard a direct `assert_eq!(prndindex, ...)` would trigger).
    fn prnd_index() -> c_int
    {
        unsafe { std::ptr::addr_of!(prndindex).read() }
    }

    /// Restores the shared statics captured at rig setup.
    struct RigGuard
    {
        saved_channels: c_int,
        saved_console_mo: *mut crate::doom::d_player::mobj_t,
    }

    impl RigGuard
    {
        /// # Safety
        /// Writes back the `players[0].mo` / `snd_channels` values
        /// captured when the rig was built; must be called before the
        /// rig is dropped so sibling tests see pristine statics.
        unsafe fn restore(&mut self)
        {
            std::ptr::addr_of_mut!(players[0].mo).write(self.saved_console_mo);
            std::ptr::addr_of_mut!(snd_channels).write(self.saved_channels);
        }
    }

    /// Wire a bare weapon-test rig: `snd_channels = 0` (the mixer
    /// never allocates channels, so `S_GetChannel` declines every
    /// sound) and the console player's `mo` pointed at the acting
    /// mobj (so `S_StartSound` short-circuits on
    /// `origin == player_mo`). The local player gets 100 health and
    /// the `mo` pointer. Restores via [`RigGuard::restore`].
    ///
    /// # Safety
    /// `mem::zeroed` on these `repr(C)` structs yields null pointers
    /// and zeroed integers, a valid bit pattern; the weapon actions
    /// touch only the fields each test assigns plus the shared
    /// statics wired here. The empty world (`bmapwidth == 0`) makes
    /// every `P_AimLineAttack` / `P_LineAttack` a no-op, isolating
    /// the draw sites under test.
    ///
    /// Both pointers must alias locals of the calling test, which
    /// must keep them alive for the whole test.
    unsafe fn weapon_rig(mo: *mut mobj_t, player: *mut PlayerT) -> (RigGuard, *mut PlayerT)
    {
        let saved_channels = snd_channels;
        snd_channels = 0;
        let saved_console_mo = std::ptr::addr_of_mut!(players[0].mo).read();
        std::ptr::addr_of_mut!(players[0].mo).write(mo as *mut _);
        (*player).health = 100;
        (*player).mo = mo as *mut _;
        (
            RigGuard {
                saved_channels,
                saved_console_mo,
            },
            player,
        )
    }

    /// Baseline contract (F10 wave B1): these vectors were written and
    /// run against the original in-file roll expressions of `A_Punch`
    /// and `A_Saw` BEFORE they extracted into `dtmc::melee_roll` /
    /// `dtmc::spread_angle`, then retargeted -- same vectors, same
    /// results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the melee rolls -> dtmc -- each action consumes
    /// exactly 3 bytes per swing in the order damage, then the
    /// `(P_Random() - P_Random())` spread pair, and every later draw
    /// of the tic keys off that cursor position. The empty world makes
    /// the aim/attack traversals no-ops, so the prndindex delta is
    /// EXACTLY the action's own draws.
    #[test]
    fn baseline_melee_draw_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            let mut psp: PspdefT = std::mem::zeroed();

            // A_Punch: 1 damage draw + 2 spread draws = exactly 3.
            prndindex = 0;
            A_Punch(player, &mut psp);
            assert_eq!(prnd_index(), 3);

            // A_Saw (miss path): same 3-draw shape, then the empty
            // world leaves linetarget null (Sawful sound only).
            prndindex = 0;
            A_Saw(player, &mut psp);
            assert_eq!(prnd_index(), 3);

            rig.restore();
        }
    }

    /// Baseline contract (F10 wave B1): written and run against the
    /// original in-file gunshot rolls BEFORE they extracted into
    /// `dtmc::gunshot_roll` / `dtmc::spread_angle`, then retargeted --
    /// same vectors, same results. An accurate shot consumes exactly
    /// 1 byte (damage); a refired shot adds the 2-byte spread pair.
    #[test]
    fn baseline_gunshot_draw_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut target_mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let mo: *mut mobj_t = &mut target_mo;
            let (mut rig, _player) = weapon_rig(mo, &mut p);

            prndindex = 0;
            P_GunShot(mo, 1);
            assert_eq!(prnd_index(), 1); // accurate: damage roll only

            prndindex = 0;
            P_GunShot(mo, 0);
            assert_eq!(prnd_index(), 3); // refire: damage + spread pair

            rig.restore();
        }
    }

    /// Baseline contract (F10 wave B1): written and run against the
    /// original in-file firing loops BEFORE the rolls extracted into
    /// the `dtmc` helpers, then retargeted -- same vectors, same
    /// results. Pins the dense RNG consumers end to end: the SSG eats
    /// exactly 5 bytes per pellet (damage, spread pair, slope pair)
    /// for 20 pellets, the shotgun 3 per pellet for 7 pellets via
    /// `P_GunShot`, the pistol and chaingun 1 byte each on an accurate
    /// shot, and the chaingun's empty-ammo early return draws 0. The
    /// flash states walked by `P_SetPsprite` carry `A_Light*` actions
    /// (extralight writes, no draws), so every byte below comes from
    /// the fire logic under test.
    #[test]
    fn baseline_firing_draw_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut psp: PspdefT = std::mem::zeroed();

            // Super shotgun: 20 pellets x (1 + 2 + 2) = 100 draws.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            (*player).readyweapon = wp_supershotgun;
            (*player).ammo[am_shell as usize] = 10;
            prndindex = 0;
            A_FireShotgun2(player, &mut psp);
            assert_eq!(prnd_index(), 100);
            assert_eq!(
                (*player).ammo[am_shell as usize],
                8
            ); // 2 shells
            rig.restore();

            // Shotgun: 7 pellets x 3 (via P_GunShot inaccurate) = 21.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            (*player).readyweapon = wp_shotgun;
            (*player).ammo[am_shell as usize] = 10;
            prndindex = 0;
            A_FireShotgun(player, &mut psp);
            assert_eq!(prnd_index(), 21);
            rig.restore();

            // Pistol: accurate shot = 1 draw.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            (*player).readyweapon = wp_pistol;
            (*player).ammo[am_clip as usize] = 10;
            prndindex = 0;
            A_FirePistol(player, &mut psp);
            assert_eq!(prnd_index(), 1);
            rig.restore();

            // Chaingun with ammo: 1 draw (all inside P_GunShot; the
            // flash-state walk itself is draw-free).
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            (*player).readyweapon = wp_chaingun;
            (*player).ammo[am_clip as usize] = 1;
            psp.state = std::ptr::addr_of!(crate::doom::info::states[S_CHAIN1 as usize]) as *mut _;
            prndindex = 0;
            A_FireCGun(player, &mut psp);
            assert_eq!(prnd_index(), 1);
            rig.restore();

            // Chaingun with an empty clip: the early return draws 0.
            let mut mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, player) = weapon_rig(&mut mo, &mut p);
            (*player).readyweapon = wp_chaingun;
            (*player).ammo[am_clip as usize] = 0;
            prndindex = 0;
            A_FireCGun(player, &mut psp);
            assert_eq!(prnd_index(), 0);
            rig.restore();
        }
    }

    /// Baseline contract (F10 wave B1): the state-machine core draws
    /// ZERO bytes -- `P_CheckAmmo` (ammo-sufficient path),
    /// `P_SetPsprite` (S_NULL transition), `P_BulletSlope` (three aim
    /// probes on the empty world leave `bulletslope` untouched), and
    /// `A_WeaponReady` (idle bob) -- plus the dead-but-kept
    /// `P_CalcSwing` phase math, whose exact finetable values are
    /// pinned against the body. Written and run green BEFORE the
    /// extractions; re-run green after.
    #[test]
    fn baseline_zero_draw_and_swing_pins()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut target_mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let mo: *mut mobj_t = &mut target_mo;
            let (mut rig, player) = weapon_rig(mo, &mut p);
            let mut psp: PspdefT = std::mem::zeroed();

            // P_CheckAmmo, sufficient: 0 draws, returns 1.
            (*player).readyweapon = wp_pistol;
            (*player).ammo[am_clip as usize] = 10;
            prndindex = 0;
            assert_eq!(P_CheckAmmo(player), 1);
            assert_eq!(prnd_index(), 0);

            // P_SetPsprite to S_NULL: 0 draws, slot parked.
            prndindex = 0;
            P_SetPsprite(player, 0, S_NULL);
            assert_eq!(prnd_index(), 0);
            assert!((*player).psprites[0].state.is_null());

            // P_BulletSlope: 3 aim probes, 0 draws, slope untouched.
            prndindex = 0;
            P_BulletSlope(mo);
            assert_eq!(prnd_index(), 0);
            assert_eq!(
                std::ptr::addr_of!(crate::doom::p_pspr::bulletslope).read(),
                0
            );

            // A_WeaponReady idle: 0 draws; the bob math writes
            // psp.sx/sy from the finetables (read back via the same
            // table entries the body used).
            (*player).readyweapon = wp_pistol;
            (*player).pendingweapon = wp_nochange;
            (*player).cmd.buttons = 0;
            (*player).bob = FRACUNIT;
            psp.state = std::ptr::null_mut();
            (*player).psprites[0].state = std::ptr::null_mut();
            leveltime = 0;
            prndindex = 0;
            A_WeaponReady(player, &mut psp);
            assert_eq!(prnd_index(), 0);
            let bob_angle = (128 * leveltime) as u32 & FINEMASK as u32;
            assert_eq!(bob_angle, 0);
            assert_eq!(psp.sx, FRACUNIT + FixedMul(FRACUNIT, *finecosine.0.add(0)));
            assert_eq!(
                psp.sy,
                WEAPONTOP
                    + FixedMul(
                        FRACUNIT,
                        finesine[(bob_angle & (FINEANGLES / 2 - 1) as u32) as usize]
                    )
            );
            (*player).bob = 0;

            // P_CalcSwing (dead in both trees, anchor-kept): the
            // leveltime-phase math, pinned against the body.
            (*player).bob = FRACUNIT;
            prndindex = 0;
            P_CalcSwing(player);
            assert_eq!(prnd_index(), 0);
            let swing_phase = (FINEANGLES as c_int / 70 * leveltime) & FINEMASK;
            let expected_swingx = FixedMul(FRACUNIT, finesine[swing_phase as usize]);
            assert_eq!(
                std::ptr::addr_of!(crate::doom::p_pspr::swingx).read(),
                expected_swingx
            );
            let swing_phase2 =
                (FINEANGLES as c_int / 70 * leveltime + FINEANGLES as c_int / 2) & FINEMASK;
            let swingx_now = std::ptr::addr_of!(crate::doom::p_pspr::swingx).read();
            assert_eq!(
                std::ptr::addr_of!(crate::doom::p_pspr::swingy).read(),
                -FixedMul(swingx_now, finesine[swing_phase2 as usize])
            );
            crate::doom::p_pspr::swingx = 0;
            crate::doom::p_pspr::swingy = 0;
            (*player).bob = 0;
            leveltime = 0;

            rig.restore();
        }
    }

    /// Baseline contract (F10 wave B1): `A_BFGSpray` on the empty
    /// world is a genuine zero-draw call -- all 40 rays miss, no
    /// target survives to roll its 15-damage byte ladder. The
    /// per-hit accumulator math is pinned at helper level by the
    /// `baseline_spray_and_roll_vectors` test; the 15-draw loop
    /// itself stays at the call site verbatim.
    #[test]
    fn baseline_bfg_spray_miss_draw_pin()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let mut spray_mo: mobj_t = std::mem::zeroed();
            let mut p: PlayerT = std::mem::zeroed();
            let (mut rig, _player) = weapon_rig(&mut spray_mo, &mut p);
            let mut target: mobj_t = std::mem::zeroed();
            spray_mo.angle = ANG90; // keep the arc arithmetic in-range
            spray_mo.target = &mut target;
            let mo: *mut mobj_t = &mut spray_mo;

            prndindex = 0;
            A_BFGSpray(mo);
            assert_eq!(prnd_index(), 0); // miss: no draws at all

            rig.restore();
        }
    }

    /// Baseline contract (F10 wave B1): the extracted helpers
    /// reproduce the exact values the body-level tests above captured
    /// against the pre-extraction bodies -- same vectors, same
    /// results. The spread vectors pin the u32 wrap (a negative
    /// `c_int` difference becomes a huge unsigned, then shifts); the
    /// spray vectors pin the 15..=120 ladder bounds.
    #[test]
    fn baseline_spray_and_roll_vectors()
    {
        // melee_roll: known % 10 boundary table.
        assert_eq!(melee_roll(0), 2);
        assert_eq!(melee_roll(5), 12);
        assert_eq!(melee_roll(9), 20);
        assert_eq!(melee_roll(10), 2);
        assert_eq!(melee_roll(19), 20);
        assert_eq!(melee_roll(255), 12);

        // gunshot_roll: known % 3 boundary table.
        assert_eq!(gunshot_roll(0), 5);
        assert_eq!(gunshot_roll(1), 10);
        assert_eq!(gunshot_roll(2), 15);
        assert_eq!(gunshot_roll(3), 5);
        assert_eq!(gunshot_roll(254), 15);
        assert_eq!(gunshot_roll(255), 5);

        // spread_angle: u32 wrap pinned with literals.
        assert_eq!(spread_angle(0, 255, 18), 0xfc040000);
        assert_eq!(spread_angle(255, 0, 18), 0x03fc0000);
        assert_eq!(spread_angle(128, 128, 19), 0);
        assert_eq!(spread_angle(1, 0, 19), 0x00080000);
        assert_eq!(spread_angle(-8, 8, 18), 0xffc00000);

        // spray_step: the ladder bounds and a mixed byte.
        assert_eq!(spray_step(0, 0), 1); // min per byte
        assert_eq!(spray_step(0, 255), 8); // max per byte
        assert_eq!(spray_step(105, 255), 113);
        // Full-ladder bounds: 15 draws of 1 and 15 draws of 8.
        let mut acc = 0;
        for _ in 0..15
        {
            acc = spray_step(acc, 0);
        }
        assert_eq!(acc, 15);
        acc = 0;
        for _ in 0..15
        {
            acc = spray_step(acc, 255);
        }
        assert_eq!(acc, 120);
    }
}
