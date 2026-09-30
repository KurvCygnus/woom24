//! The samplers: read-only interpolation queries called from the render
//! path. Every sampler returns the live simulation value bit-for-bit when
//! the gate is off, the board is inactive, a guard trips, or the pair is
//! missing (each fallback arm).

use crate::doom::c_ffi;
use crate::doom::d_player::{players, PlayerT, PspdefT, MAXPLAYERS};
use crate::doom::info::MF_MISSILE;
use crate::doom::m_fixed::fixed_t;
use crate::doom::p_setup::sectors;
use crate::doom::p_tick::leveltime;

use super::board::{
    board_active, enabled, mobj_lookup, teleported, CAM_SLOTS, FRACTION, MOBJ_SLOTS,
    OLD_LEVELTIME, PosSample, PSPR_SLOTS, SECTOR_CURR_CEIL, SECTOR_CURR_FLOOR, SECTOR_LEN,
    SECTOR_PREV_CEIL, SECTOR_PREV_FLOOR, SECTOR_PREV_VALID,
};
use super::math::{lerp_angle, lerp_fixed};

/// Interpolated player camera for `R_SetupFrame`. Falls back to the live
/// simulation values under the Woof! guard set: board inactive/disabled, no
/// pair yet, first level tic (`leveltime > 1`, `r_main.c:709-713`), paused
/// (`leveltime > oldleveltime`, `r_main.c:722`), or a teleport-sized jump.
///
/// # Safety
/// `player` must be a valid player pointer with a valid `mo` (same contract
/// as `R_SetupFrame` itself).
pub unsafe fn sample_camera(player: *mut PlayerT, mo: *mut c_ffi::mobj_t) -> PosSample
{
    let live = PosSample {
        x: (*mo).x,
        y: (*mo).y,
        z: (*player).viewz,
        angle: (*mo).angle,
    };

    let idx = (player as usize - std::ptr::addr_of!(players) as usize)
        / std::mem::size_of::<PlayerT>();
    if !enabled() || !board_active() || idx >= MAXPLAYERS { return live; }
    let cam = CAM_SLOTS[idx];
    if !cam.prev_valid || !cam.curr_valid || leveltime <= 1 || leveltime <= OLD_LEVELTIME { return live; }
    if teleported(cam.prev, cam.curr) { return live; }

    let frac = FRACTION;
    PosSample {
        x: lerp_fixed(cam.prev.x, cam.curr.x, frac),
        y: lerp_fixed(cam.prev.y, cam.curr.y, frac),
        z: lerp_fixed(cam.prev.z, cam.curr.z, frac),
        angle: lerp_angle(cam.prev.angle, cam.curr.angle, frac),
    }
}

/// Interpolated mobj position/angle for sprite placement (`R_ProjectSprite`).
/// Guard set: board miss (never captured — spawn guard), no prev pair,
/// paused, player-missile first pair (`p_mobj.c:752-756`), and the
/// render-side teleport snap (`p_map.c:319-320` replacement).
///
/// # Safety
/// `mo` must be a valid mobj pointer.
pub unsafe fn sample_mobj(mo: *mut c_ffi::mobj_t) -> PosSample
{
    let live = PosSample { x: (*mo).x, y: (*mo).y, z: (*mo).z, angle: (*mo).angle };

    if !enabled() || !board_active() { return live; }

    let slot = mobj_lookup(mo);
    let slot = match slot
    {
        Some(s) => s,
        None => return live,
    };
    let s = MOBJ_SLOTS[slot];
    if !s.prev_valid || !s.curr_valid || leveltime <= OLD_LEVELTIME { return live; }
    // Player missiles skip interpolation on their first pair (the render
    // right after their first full tic of travel). Woof! marks this with the
    // `interp == -1` sentinel in P_SpawnPlayerMissile; render-side we cannot
    // tell player missiles from enemy ones, so every missile snaps one render
    // — one frame of snap on enemy missiles, sim-neutral either way.
    if(*mo).flags & MF_MISSILE != 0 && s.age == 2 { return live; }
    if teleported(s.prev, s.curr) { return live; }

    let frac = FRACTION;
    PosSample {
        x: lerp_fixed(s.prev.x, s.curr.x, frac),
        y: lerp_fixed(s.prev.y, s.curr.y, frac),
        z: lerp_fixed(s.prev.z, s.curr.z, frac),
        angle: lerp_angle(s.prev.angle, s.curr.angle, frac),
    }
}

/// Interpolated floor height of a sector (`R_FindPlane` / wall-span args).
/// Falls back to the live height when the board does not cover the sector.
///
/// # Safety
/// `sector` must point into the engine's sector array.
pub unsafe fn sector_floor(sector: *mut c_ffi::sector_t) -> fixed_t
{
    let idx = sector_index(sector);
    if !enabled() || !board_active() || idx >= SECTOR_LEN || !SECTOR_PREV_VALID[idx] { return (*sector).floorheight; }
    lerp_fixed(SECTOR_PREV_FLOOR[idx], SECTOR_CURR_FLOOR[idx], FRACTION)
}

/// Interpolated ceiling height of a sector.
///
/// # Safety
/// `sector` must point into the engine's sector array.
pub unsafe fn sector_ceiling(sector: *mut c_ffi::sector_t) -> fixed_t
{
    let idx = sector_index(sector);
    if !enabled() || !board_active() || idx >= SECTOR_LEN || !SECTOR_PREV_VALID[idx] { return (*sector).ceilingheight; }
    lerp_fixed(SECTOR_PREV_CEIL[idx], SECTOR_CURR_CEIL[idx], FRACTION)
}

/// Sector array index from a sector pointer (all `sector_t` mirrors are
/// 128-byte `#[repr(C)]` images of the same C struct).
unsafe fn sector_index(sector: *mut c_ffi::sector_t) -> usize
{
    let base = *std::ptr::addr_of!(sectors) as usize;
    (sector as usize - base) / std::mem::size_of::<c_ffi::sector_t>()
}

/// Interpolated weapon-sprite screen position for `R_DrawPSprite`. The flash
/// slot (1) samples the weapon slot's pair — vanilla copies `sx`/`sy` from
/// the weapon slot every tic (`P_MovePsprites`), so the two must stay glued
/// together mid-frame. A state change between the pair snaps (Woof!
/// `p_pspr.c:1221`), so the weapon never slides between two different frames.
///
/// # Safety
/// `psp` must point into `viewplayer`'s psprite array.
pub unsafe fn sample_psp(psp: *mut PspdefT) -> (fixed_t, fixed_t)
{
    let live = ((*psp).sx, (*psp).sy);
    if !enabled() || !board_active() { return live; }

    let vp = crate::doom::r_main::viewplayer;
    if vp.is_null() { return live; }

    let player_idx = (vp as usize - std::ptr::addr_of!(players) as usize)
        / std::mem::size_of::<PlayerT>();
    if player_idx >= MAXPLAYERS { return live; }

    // Slot 1 mirrors slot 0; both draw from the weapon pair.
    let ps = PSPR_SLOTS[player_idx];
    if !ps.prev_valid || !ps.curr_valid || leveltime <= OLD_LEVELTIME { return live; }
    if ps.prev_state != ps.curr_state { return live; }

    let frac = FRACTION;
    (
        lerp_fixed(ps.prev_sx, ps.curr_sx, frac),
        lerp_fixed(ps.prev_sy, ps.curr_sy, frac),
    )
}

#[cfg(test)]
mod tests
{
    use crate::doom::c_ffi;

    /// The `sector_index` pointer arithmetic assumes all `sector_t` mirrors
    /// share one layout; this parity check rides along with the samplers.
    #[test]
    fn struct_layout_parity_for_mirrors()
    {
        // All sector_t mirrors are 128-byte repr(C) images of the same C
        // struct; the board indexes by pointer arithmetic against that size.
        assert_eq!(
            std::mem::size_of::<c_ffi::sector_t>(),
            std::mem::size_of::<crate::doom::r_bsp::sector_t>()
        );
        assert_eq!(
            std::mem::offset_of!(c_ffi::sector_t, floorheight),
            std::mem::offset_of!(crate::doom::r_bsp::sector_t, floorheight)
        );
        assert_eq!(
            std::mem::offset_of!(c_ffi::sector_t, ceilingheight),
            std::mem::offset_of!(crate::doom::r_bsp::sector_t, ceilingheight)
        );
    }
}
