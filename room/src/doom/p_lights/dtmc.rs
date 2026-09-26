//! Demo-synchronization surface extracted from `p_lights`: the two
//! pure light computations behind the module's mid-tic `P_Random`
//! consumers -- the masked duration draw of the flash/strobe thinkers
//! and the fire-flicker amount + clamp -- whose exact integer results
//! are pinned by the baseline vectors below.

use std::ffi::c_int;

/// One masked random duration draw of the light specials: compute
/// `(rand & mask) + 1`, the exact inline expression behind the
/// phase-duration and initial-count draws of `T_LightFlash`,
/// `P_SpawnLightFlash`, and `P_SpawnStrobeFlash`
/// (`vendor/doomgeneric/p_lights.c` -- no named C function, hence no
/// `#[doc(alias)]`). The draw itself stays at the call site
/// (`P_Random()` is passed in), preserving the demo-visible draw
/// order.
///
/// ## Technical Details
///
/// The masks are bit masks (`mintime = 7`, `maxtime = 64`, the strobe
/// sync mask `7`), so `(rand & mask) + 1` produces durations in
/// `1..=mask + 1`. Every drawn value consumes one entry of the
/// demo-pinned `RNDTABLE` sequence: any change to the formula shifts
/// every later draw of the same tic and moves the demo goldens. The
/// `+ 1` guarantees a nonzero duration -- a zero count would make the
/// thinker act again on its very next `P_RunThinkers` visit.
///
/// ## On Calling
///
/// Pure integer computation. Call it with the byte just drawn from
/// `P_Random()` and the thinker's stored mask, and never batch or
/// reorder the draws: each call site draws exactly once, at the point
/// the original body drew, interleaved with the sector `lightlevel`
/// writes.
pub fn flash_duration(rand: c_int, mask: c_int) -> c_int
{
    (rand & mask) + 1
}

/// The fire-flicker light step: compute the sector `lightlevel` a
/// `T_FireFlicker` thinker writes on a fired tic -- the amount
/// `(rand & 3) * 16` subtracted from the light, with the exact clamp
/// of the upstream `T_FireFlicker` (`p_lights.c:39`) -- from the
/// current level, the thinker's bounds, and the drawn byte. The
/// `P_Random()` draw stays at the call site, preserving the
/// demo-visible draw order.
///
/// ## Technical Details
///
/// The clamp predicate reads the CURRENT sector lightlevel
/// (`cur_level - amount < minlight`), not `maxlight`: after a clamped
/// tic the sector sits at `minlight`, and a `maxlight`-based
/// predicate would answer differently -- the `(150, 200, 160, 8)`
/// baseline vector below pins exactly this arm. The result is
/// truncated to `i16` at the call site, exactly as the original body
/// assigned it. `(rand & 3) * 16` yields amounts in `0..=48`; each
/// draw consumes one `RNDTABLE` entry, so formula or order changes
/// shift the demo sequence.
///
/// ## On Calling
///
/// Pure integer computation over the four inputs: pass the sector's
/// lightlevel BEFORE the write, the thinker's `maxlight` / `minlight`,
/// and the byte just drawn. Do not switch the arithmetic to
/// `wrapping_*` -- real map lightlevels are `0..=255` and cannot
/// overflow, and the original body used plain integer arithmetic.
pub fn fire_flicker_level(cur_level: c_int, maxlight: c_int, minlight: c_int, rand: c_int) -> c_int
{
    let amount = (rand & 3) * 16;
    if cur_level - amount < minlight
    {
        minlight
    }
    else
    {
        maxlight - amount
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::m_random::{prndindex, RNDTABLE};
    use crate::doom::p_lights::dtmc::{fire_flicker_level, flash_duration};
    use crate::doom::p_lights::{
        fireflicker_t, glow_t, lightflash_t, sector_t, strobe_t, T_FireFlicker, T_Glow,
        T_LightFlash, T_StrobeFlash,
    };
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;
    use std::ffi::c_int;

    /// Read the `prndindex` cursor for assertions without creating a
    /// shared reference to the mutable static (the `static_mut_refs`
    /// hazard a direct `assert_eq!(prndindex, ...)` would trigger).
    fn prnd_index() -> c_int
    {
        unsafe { std::ptr::addr_of!(prndindex).read() }
    }

    /// Baseline contract (F10 wave A2): these vectors were written
    /// and run against the original in-file draw sites BEFORE the
    /// formula extracted into `dtmc::flash_duration`, then retargeted
    /// -- same vectors, same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the masked duration -> dtmc -- the exact
    /// `(rand & mask) + 1` expression is what makes the
    /// `T_LightFlash` / `P_SpawnLightFlash` / `P_SpawnStrobeFlash`
    /// draws consume the demo-pinned `RNDTABLE` sequence; the draws
    /// themselves stay at the call sites.
    #[test]
    fn baseline_flash_duration_vectors()
    {
        assert_eq!(flash_duration(8, 7), 1);
        assert_eq!(flash_duration(109, 7), 6);
        assert_eq!(flash_duration(8, 64), 1);
        assert_eq!(flash_duration(109, 64), 65);
        assert_eq!(flash_duration(200, 64), 65);
        assert_eq!(flash_duration(63, 64), 1);
        assert_eq!(flash_duration(7, 7), 8);
        assert_eq!(flash_duration(0, 7), 1);
    }

    /// Baseline contract (F10 wave A2): these vectors were written
    /// and run against the original in-file body BEFORE the amount +
    /// clamp extracted into `dtmc::fire_flicker_level`, then
    /// retargeted -- same vectors, same results.
    ///
    /// Adjudication: the flicker step -> dtmc -- `T_FireFlicker`
    /// writes `lightlevel` per tic in the fixed thinker order from
    /// exactly this computation. The `(150, 200, 160, 8)` vector
    /// discriminates the clamp predicate: it reads the current level
    /// (150), so a maxlight-based predicate would wrongly answer 200
    /// instead of 160.
    #[test]
    fn baseline_fire_flicker_level_vectors()
    {
        assert_eq!(fire_flicker_level(200, 200, 160, 8), 200);
        assert_eq!(fire_flicker_level(150, 200, 160, 8), 160);
        assert_eq!(fire_flicker_level(200, 200, 160, 109), 184);
        assert_eq!(fire_flicker_level(160, 255, 160, 255), 160);
        assert_eq!(fire_flicker_level(207, 207, 160, 7), 160);
        assert_eq!(fire_flicker_level(255, 255, 157, 109), 239);
    }

    /// Baseline contract (F10 wave A2): `T_FireFlicker` draws exactly
    /// one `P_Random` byte per fired tic, applies
    /// `maxlight - (rand & 3) * 16`, clamps against `minlight` using
    /// the CURRENT sector level, and resets `count = 4`. Captured
    /// green against the pre-extraction body; re-run green after the
    /// `dtmc::fire_flicker_level` re-route.
    #[test]
    fn baseline_t_fire_flicker_draw_and_clamp()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            prndindex = 0;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // `T_FireFlicker` touches only `count`, `maxlight`,
            // `minlight`, `sector`, and the sector's `lightlevel`.
            let mut sec: sector_t = std::mem::zeroed();
            let mut flick: fireflicker_t = std::mem::zeroed();
            flick.sector = &mut sec;
            flick.maxlight = 200;
            flick.minlight = 160;

            // Draw byte RNDTABLE[1] = 8 -> amount = (8 & 3) * 16 = 0;
            // 200 - 0 >= 160, so the level keeps maxlight - amount.
            sec.lightlevel = 200;
            flick.count = 1;
            T_FireFlicker(&mut flick);
            assert_eq!(sec.lightlevel as c_int, 200);
            assert_eq!(flick.count, 4);
            assert_eq!(prnd_index(), 1);

            // Clamp arm predicated on the CURRENT level (150), not
            // maxlight: a maxlight-based predicate would answer 200.
            sec.lightlevel = 150;
            flick.count = 1;
            T_FireFlicker(&mut flick);
            assert_eq!(sec.lightlevel as c_int, 160);
            assert_eq!(prnd_index(), 2);

            // Nonzero amount: reseed so the next byte is
            // RNDTABLE[2] = 109 -> (109 & 3) * 16 = 16;
            // 200 - 16 = 184 >= 160.
            prndindex = 1;
            sec.lightlevel = 200;
            flick.count = 1;
            T_FireFlicker(&mut flick);
            assert_eq!(sec.lightlevel as c_int, 184);
            assert_eq!(prnd_index(), 2);
            assert_eq!(RNDTABLE[1], 8);
            assert_eq!(RNDTABLE[2], 109);
        }
    }

    /// Baseline contract (F10 wave A2): `T_LightFlash` draws exactly
    /// one byte per fired tic, on the bright->dark edge masked with
    /// `mintime` and on the dark->bright edge masked with `maxtime`,
    /// writing the light level BEFORE drawing the next duration.
    /// Captured green against the pre-extraction body; re-run green
    /// after the `dtmc::flash_duration` re-route.
    #[test]
    fn baseline_t_light_flash_branch_masks()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            prndindex = 0;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // `T_LightFlash` touches only the fields assigned below
            // and the sector's `lightlevel`.
            let mut sec: sector_t = std::mem::zeroed();
            let mut flash: lightflash_t = std::mem::zeroed();
            flash.sector = &mut sec;
            flash.maxlight = 200;
            flash.minlight = 100;
            flash.maxtime = 64;
            flash.mintime = 7;

            // Bright -> dark: level == maxlight, duration drawn with
            // the MINTIME mask: (RNDTABLE[1] & 7) + 1 = 1.
            sec.lightlevel = 200;
            flash.count = 1;
            T_LightFlash(&mut flash);
            assert_eq!(sec.lightlevel as c_int, 100);
            assert_eq!(flash.count, 1);
            assert_eq!(prnd_index(), 1);

            // Dark -> bright: level != maxlight, duration drawn with
            // the MAXTIME mask on the NEXT byte RNDTABLE[2] = 109:
            // (109 & 64) + 1 = 65.
            flash.count = 1;
            T_LightFlash(&mut flash);
            assert_eq!(sec.lightlevel as c_int, 200);
            assert_eq!(flash.count, 65);
            assert_eq!(prnd_index(), 2);

            // Distinguishable masks: reseed so the drawn byte is
            // RNDTABLE[15] = 21. The bright->dark edge masks with
            // mintime: (21 & 7) + 1 = 6 -- a maxtime-masked answer
            // here would be 1.
            prndindex = 14;
            sec.lightlevel = 200;
            flash.count = 1;
            T_LightFlash(&mut flash);
            assert_eq!(sec.lightlevel as c_int, 100);
            assert_eq!(flash.count, 6);
            assert_eq!(prnd_index(), 15);

            // ...and the dark->bright edge masks with maxtime on the
            // next byte RNDTABLE[16] = 211: (211 & 64) + 1 = 65.
            flash.count = 1;
            T_LightFlash(&mut flash);
            assert_eq!(sec.lightlevel as c_int, 200);
            assert_eq!(flash.count, 65);
            assert_eq!(prnd_index(), 16);
            assert_eq!(RNDTABLE[15], 21);
            assert_eq!(RNDTABLE[16], 211);
        }
    }

    /// Baseline contract (F10 wave A2): `T_StrobeFlash` and `T_Glow`
    /// are fully deterministic phase machines that draw NO
    /// `P_Random` bytes -- pinning the zero-draw property the
    /// adjudication records for them (both stay whole in
    /// `effects.rs`; nothing extracts).
    #[test]
    fn baseline_strobe_and_glow_draw_no_random()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            prndindex = 0;
            // SAFETY: `mem::zeroed` on these `repr(C)` structs yields
            // null pointers and zeroed integers, a valid bit pattern;
            // the tick fns touch only the fields assigned below and
            // the sector's `lightlevel`.
            let mut sec: sector_t = std::mem::zeroed();
            let mut flash: strobe_t = std::mem::zeroed();
            flash.sector = &mut sec;
            flash.maxlight = 200;
            flash.minlight = 100;
            flash.brighttime = 5;
            flash.darktime = 15;

            // Dark -> bright phase switch: deterministic, no draw.
            sec.lightlevel = 100;
            flash.count = 1;
            T_StrobeFlash(&mut flash);
            assert_eq!(sec.lightlevel as c_int, 200);
            assert_eq!(flash.count, 5);
            assert_eq!(prnd_index(), 0);

            // Glow: one downward GLOWSPEED step, no draw.
            let mut glow: glow_t = std::mem::zeroed();
            glow.sector = &mut sec;
            glow.maxlight = 200;
            glow.minlight = 100;
            glow.direction = -1;
            sec.lightlevel = 200;
            T_Glow(&mut glow);
            assert_eq!(sec.lightlevel as c_int, 192);
            assert_eq!(glow.direction, -1);
            assert_eq!(prnd_index(), 0);

            // Glow bounce: a step landing at/below minlight steps
            // back up and reverses direction, still without drawing.
            glow.minlight = 190;
            sec.lightlevel = 195;
            T_Glow(&mut glow);
            assert_eq!(sec.lightlevel as c_int, 195);
            assert_eq!(glow.direction, 1);
            assert_eq!(prnd_index(), 0);
        }
    }
}
