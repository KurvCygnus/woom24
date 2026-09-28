//! Demo-synchronization surface extracted from `p_mobj`: the four pure
//! computations the mobj paths lean on -- the mapthing BAM-angle
//! quantization, the item-respawn ring step, the THINGS-lump skill
//! filter bit ladder, and the spawn-tic jitter clamp. Their exact
//! integer outputs feed spawn angles, deathmatch item respawn order,
//! map-load filtering, and every spawn/explode tic count; the stateful
//! side (the spawns themselves, the queue writes, the RNG draws) stays
//! at the call sites.

use std::ffi::c_int;

use crate::doom::c_ffi::ITEMQUESIZE;
use crate::doom::tables::ANG45;

/// Quantize a mapthing's degree angle to Doom's binary angle measure --
/// the pure half of the `ANG45 * (angle / 45)` assignment that
/// `P_SpawnMapThing` / `P_SpawnPlayer` / `P_NightmareRespawn` /
/// `P_RespawnSpecials` apply to every spawned mobj
/// (`vendor/doomgeneric/p_mobj.c:823`, `:696`, `:383`-adjacent,
/// `:651`-adjacent).
///
/// ## Technical Details
///
/// The formula is `ANG45.wrapping_mul((angle as u32) / 45)` exactly as
/// the four pre-split bodies wrote it: the `i16 -> u32` conversion
/// sign-extends and the division is unsigned, so the wrap in the
/// multiply is the only overflow path (360 degrees wraps the `u32`
/// angle space to zero). A negative `mapthing_t::angle` never occurs in
/// WAD data, but the conversion order is pinned by the baseline vectors
/// nonetheless -- C's signed-division-then-cast would give a different
/// bit pattern there, and the shipped expression is the contract.
///
/// ## On Calling
///
/// Pass `(mapthing.angle as u32)` -- the cast stays at the call site
/// exactly as in the pre-split bodies. Pure computation over no state;
/// never debug-assert on the multiply (the wrap is the behavior demo
/// playback depends on).
pub fn mapthing_angle_quantize(angle: u32) -> u32 { ANG45.wrapping_mul(angle / 45) }

/// Advance one step around the item-respawn ring buffer -- the pure
/// half of the `(i + 1) & (ITEMQUESIZE - 1)` index updates in
/// `P_RemoveMobj` and `P_RespawnSpecials`
/// (`vendor/doomgeneric/p_mobj.c:589`-adjacent, `:647`).
///
/// ## Technical Details
///
/// `ITEMQUESIZE` is 128, a power of two, so the mask IS the modulo:
/// index 127 steps back to 0. The wrap is the deathmatch
/// item-respawn demo surface -- `iquehead` chases `iquetail` around
/// the ring, and when the head catches the tail the tail is forced
/// forward (the overwrite order at the call site), which decides which
/// queued item a long deathmatch demo respawns.
///
/// ## On Calling
///
/// Pass a current queue index (`0..ITEMQUESIZE`); out-of-range inputs
/// are not modeled and never occur (both writers mask through this
/// step). Pure computation over no state.
pub fn respawn_queue_step(index: c_int) -> c_int { (index + 1) & (ITEMQUESIZE as c_int - 1) }

/// Map a skill level to the THINGS-lump skill-bit filter -- the pure
/// half of the ladder in `P_SpawnMapThing`
/// (`vendor/doomgeneric/p_mobj.c:812`-adjacent): skill 0 (baby) plays
/// the easy-family things, skill 4 (nightmare) shares the hard bit
/// with skill 3, and the middle rungs use `1 << (gameskill - 1)`.
///
/// ## Technical Details
///
/// The bit selects which `options` bits a mapthing must carry to spawn:
/// bits 1/2/4 are the easy/medium/hard skill flags, and both extreme
/// skills alias onto the ends of that ladder (baby -> bit 1, nightmare
/// -> bit 4). A wrong ladder changes which things a level loads, which
/// desyncs every demo the moment the map loads.
///
/// ## On Calling
///
/// Pass the `gameskill` global's value (`0..=4`; other inputs are not
/// modeled -- the `1 << (gameskill - 1)` arm shifts by whatever it is
/// given, verbatim from the body). Pure computation over no state.
pub fn spawn_skill_bit(skill_level: c_int) -> c_int
{
    if skill_level == 0
    {
        // sk_baby
        1
    }
    else if skill_level == 4
    {
        // sk_nightmare
        4
    }
    else { 1 << (skill_level - 1) }
}

/// Apply the spawn-tic jitter to a freshly transitioned or spawned
/// mobj's tic count -- the pure half of the
/// `t -= P_Random() & 3; if t < 1 { t = 1 }` pair at the four call
/// sites (`explode_missile`, `spawn_puff`, `spawn_blood`,
/// `check_missile_spawn`; `vendor/doomgeneric/p_mobj.c:90`-adjacent,
/// `:851`, `:878`, `:907`).
///
/// ## Technical Details
///
/// The mask, not the raw byte, is what subtracts: `draw & 3` yields
/// `0..=3`, then the result floors at 1 (a zero-tic state would
/// immediately advance again through the caller's chain and change the
/// per-tic action sequence). The RNG draw itself STAYS at the call
/// site -- only the mask-subtract + floor pair extracts -- so the
/// demo-visible draw count and order are untouched.
///
/// ## On Calling
///
/// Pass the mobj's current `tics` and the raw `P_Random()` byte in
/// that order; the caller assigns the result back. Pure computation
/// over no state; the subtraction is the body's plain `-` (values in
/// range never overflow).
pub fn tics_jitter_clamp(tics: c_int, draw: c_int) -> c_int
{
    let tics = tics - (draw & 3);
    if tics < 1 { 1 }
    else { tics }
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_int;

    use crate::doom::c_ffi::ITEMQUESIZE;
    use crate::doom::tables::ANG45;

    use super::{mapthing_angle_quantize, respawn_queue_step, spawn_skill_bit, tics_jitter_clamp};

    /// Baseline contract (F10 wave B3a): the angle vectors were written
    /// and run against the pre-move in-file expression (test-local
    /// transcription, commit `377ca48`) BEFORE the extraction, then
    /// retargeted here -- same vectors, same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): wholly qualifying -- the quantized angle is what
    /// every spawned mobj's facing is, and the `u32` wrap at a full
    /// turn is part of that.
    #[test]
    fn baseline_mapthing_angle_quantize_vectors()
    {
        assert_eq!(mapthing_angle_quantize(0), 0);
        assert_eq!(mapthing_angle_quantize(45), ANG45);
        assert_eq!(mapthing_angle_quantize(90), ANG45 * 2);
        assert_eq!(mapthing_angle_quantize(135), ANG45 * 3);
        assert_eq!(mapthing_angle_quantize(180), ANG45 * 4);
        assert_eq!(mapthing_angle_quantize(225), ANG45 * 5);
        assert_eq!(mapthing_angle_quantize(270), ANG45 * 6);
        assert_eq!(mapthing_angle_quantize(315), ANG45 * 7);
        // 360 degrees == one full turn: the multiply wraps the u32.
        assert_eq!(mapthing_angle_quantize(360), 0);
        // Negative mapthing angle (never produced by WAD data): pins
        // the sign-extend-then-unsigned-divide the shipped body uses.
        assert_eq!(mapthing_angle_quantize((-45i16) as u32), 0x8000_0000);
    }

    /// Baseline contract (F10 wave B3a): pre-move transcription
    /// baseline (commit `377ca48`), retargeted -- same vectors, same
    /// results. The wrap at 127 IS the deathmatch respawn surface.
    #[test]
    fn baseline_respawn_queue_step_wraps_at_127()
    {
        assert_eq!(ITEMQUESIZE, 128);
        assert_eq!(respawn_queue_step(0), 1);
        assert_eq!(respawn_queue_step(1), 2);
        assert_eq!(respawn_queue_step(63), 64);
        assert_eq!(respawn_queue_step(126), 127);
        // The wrap the report names: the last slot steps back to 0.
        assert_eq!(respawn_queue_step(127), 0);
    }

    /// Baseline contract (F10 wave B3a): pre-move transcription
    /// baseline (commit `377ca48`), retargeted -- same vectors, same
    /// results. Skills 0..=4 select bits 1, 1, 2, 4, 4.
    #[test]
    fn baseline_spawn_skill_bit_ladder()
    {
        assert_eq!(spawn_skill_bit(0), 1);
        assert_eq!(spawn_skill_bit(1), 1);
        assert_eq!(spawn_skill_bit(2), 2);
        assert_eq!(spawn_skill_bit(3), 4);
        assert_eq!(spawn_skill_bit(4), 4);
    }

    /// Baseline contract (F10 wave B3a): pre-move transcription
    /// baseline (commit `377ca48`), retargeted -- same vectors, same
    /// results. Masked draws 0..=3 subtract with a floor at 1; the
    /// mask, not the raw byte, is what subtracts.
    #[test]
    fn baseline_tics_jitter_clamp_floors_at_one()
    {
        assert_eq!(tics_jitter_clamp(5, 0), 5);
        assert_eq!(tics_jitter_clamp(5, 1), 4);
        assert_eq!(tics_jitter_clamp(5, 2), 3);
        assert_eq!(tics_jitter_clamp(5, 3), 2);
        assert_eq!(tics_jitter_clamp(4, 3), 1);
        assert_eq!(tics_jitter_clamp(2, 1), 1);
        assert_eq!(tics_jitter_clamp(1, 0), 1);
        assert_eq!(tics_jitter_clamp(1, 3), 1);
        assert_eq!(tics_jitter_clamp(0, 0), 1);
        assert_eq!(tics_jitter_clamp(-2, 0), 1);
        assert_eq!(tics_jitter_clamp(10, 3), 7);
        // The & 3 mask, not the raw byte, is what subtracts.
        assert_eq!(tics_jitter_clamp(10, 4), 10);
        assert_eq!(tics_jitter_clamp(10, 255), 7);
    }

    /// The extraction takes the raw `P_Random()` byte at the call
    /// site; a zero draw must be the identity (the `& 3` arm's no-jitter
    /// case), pinning that no extra clamp branch sneaks in.
    #[test]
    fn zero_draw_is_identity_for_positive_tics()
    {
        assert_eq!(tics_jitter_clamp(1, 0), 1);
        assert_eq!(tics_jitter_clamp(2, 0), 2);
        assert_eq!(tics_jitter_clamp(30, 0), 30);
        assert_eq!(tics_jitter_clamp(c_int::MAX, 0), c_int::MAX);
    }
}
