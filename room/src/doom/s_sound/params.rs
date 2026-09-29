//! Positional attenuation: the layout-critical `MobjStub` mobj prefix, the
//! distance / stereo constants, and the pure integer math of
//! `adjust_sound_params`.

use std::ffi::{c_int, c_void};

use super::music::snd_SfxVolume;
use crate::doom::g_game::gamemap;
use crate::doom::m_fixed::{FixedMul, FRACBITS, FRACUNIT};
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::tables::{finesine, ANGLETOFINESHIFT};

/// Distance, in fixed-point units, beyond which a positional sound is clipped
/// out entirely (`S_CLIPPING_DIST` in `s_sound.c`).
const S_CLIPPING_DIST: c_int = 1200 * FRACUNIT;
/// Distance, in fixed-point units, within which a sound plays at full SFX
/// volume (`S_CLOSE_DIST` in `s_sound.c`).
const S_CLOSE_DIST: c_int = 200 * FRACUNIT;
/// Integer attenuation range between `S_CLOSE_DIST` and `S_CLIPPING_DIST`,
/// shifted out of fixed-point. Used as the divisor in the linear volume falloff.
const S_ATTENUATOR: c_int = (S_CLIPPING_DIST - S_CLOSE_DIST) >> FRACBITS;
/// Maximum stereo swing applied to `sep` based on listener-to-source angle, in
/// fixed-point units.
const S_STEREO_SWING: c_int = 96 * FRACUNIT;
/// Pitch value treated as "no pitch shift" by the driver.
const NORM_PITCH: c_int = 128;
/// Default channel priority when none is supplied.
const NORM_PRIORITY: c_int = 64;
/// Stereo separation value meaning "centred" (0 = full left, 255 = full right).
pub(super) const NORM_SEP: c_int = 128;

/// Layout-compatible prefix of `mobj_t` (`p_mobj.h`).
///
/// The sound subsystem only needs the positional/orientation fields of an
/// mobj, so we expose just those and keep the rest as opaque padding. The
/// 24-byte `thinker_t` prefix MUST come first or sound effects whose `origin`
/// is not the player will be positioned at garbage coordinates (see
/// `MEMORY.md`'s `MobjStub layout bug` note).
#[repr(C)]
pub struct MobjStub {
    // thinker_t prefix (3 pointers * 8 bytes = 24 bytes on 64-bit)
    /// Padding: `thinker_t.prev` pointer.
    _thinker_prev: *mut c_void,
    /// Padding: `thinker_t.next` pointer.
    _thinker_next: *mut c_void,
    /// Padding: `thinker_t.function` pointer.
    _thinker_fn: *mut c_void,
    // mobj_t positional fields
    /// World X coordinate, fixed-point.
    pub x: c_int,
    /// World Y coordinate, fixed-point.
    pub y: c_int,
    /// Padding: `mobj_t.z`.
    _z: c_int,
    /// Padding: alignment slot before the sector list pointers.
    _pad0: u32,
    /// Padding: `mobj_t.snext`.
    _snext: *mut c_void,
    /// Padding: `mobj_t.sprev`.
    _sprev: *mut c_void,
    /// Facing angle of the mobj, BAM (binary angle) units.
    pub angle: u32,
}

/// Compute distance-attenuated volume and stereo separation for `source`
/// relative to `listener`.
///
/// Writes the result into `*vol` (0-127) and `*sep` (0-255, 128 = centred)
/// and returns non-zero if the sound is still audible. Distance is the
/// `|dx|+|dy| - min(|dx|,|dy|)/2` approximation Doom uses; volume falls off
/// linearly between `S_CLOSE_DIST` and `S_CLIPPING_DIST`. On map 8 (boss
/// levels) the falloff bottoms out at 15 instead of 0 so the boss is always
/// faintly audible.
///
/// # Safety
/// `listener`, `source`, `vol`, `sep` must all point at valid memory.
pub(super) unsafe fn adjust_sound_params(
    listener: *mut MobjStub,
    source: *mut MobjStub,
    vol: *mut c_int,
    sep: *mut c_int,
) -> c_int {
    let adx = (*listener).x.wrapping_sub((*source).x).wrapping_abs();
    let ady = (*listener).y.wrapping_sub((*source).y).wrapping_abs();

    let approx_dist = adx
        .wrapping_add(ady)
        .wrapping_sub((if adx < ady { adx } else { ady }) >> 1);

    if gamemap != 8 && approx_dist > S_CLIPPING_DIST {
        return 0;
    }

    let mut angle = R_PointToAngle2((*listener).x, (*listener).y, (*source).x, (*source).y);

    if angle > (*listener).angle {
        angle -= (*listener).angle;
    } else {
        angle = angle.wrapping_add(0xffffffff_u32 - (*listener).angle);
    }

    angle >>= ANGLETOFINESHIFT;

    *sep = 128 - (FixedMul(S_STEREO_SWING, finesine[angle as usize]) >> FRACBITS);

    if approx_dist < S_CLOSE_DIST {
        *vol = snd_SfxVolume;
    } else if gamemap == 8 {
        let mut clipped_dist = approx_dist;
        if clipped_dist > S_CLIPPING_DIST {
            clipped_dist = S_CLIPPING_DIST;
        }

        *vol = 15
            + ((snd_SfxVolume - 15) * ((S_CLIPPING_DIST - clipped_dist) >> FRACBITS))
                / S_ATTENUATOR;
    } else {
        *vol = (snd_SfxVolume * ((S_CLIPPING_DIST - approx_dist) >> FRACBITS)) / S_ATTENUATOR;
    }

    (*vol > 0) as c_int
}

/// Baseline vectors written before the graduation move (F10 wave F2-b):
/// the pure integer attenuation math must be identical before and after the
/// split. No driver, no fixtures.
#[cfg(test)]
mod tests {
    use super::*;

    /// A `MobjStub` at `(x, y)` with the default (zero) facing.
    fn stub(x: c_int, y: c_int) -> MobjStub {
        MobjStub {
            _thinker_prev: std::ptr::null_mut(),
            _thinker_next: std::ptr::null_mut(),
            _thinker_fn: std::ptr::null_mut(),
            x,
            y,
            _z: 0,
            _pad0: 0,
            _snext: std::ptr::null_mut(),
            _sprev: std::ptr::null_mut(),
            angle: 0,
        }
    }

    /// Listener == source: full SFX volume, still audible. The stereo sep
    /// is NOT forced to `NORM_SEP` here (that forcing lives in
    /// `S_StartSound`); the raw function wraps the zero angle through the
    /// else branch (`0xffffffff - 0`) and lands on `finesine[8191]`, which
    /// yields exactly 129. Pinned as captured pre-move.
    /// (Precondition: a non-boss map -- `gamemap != 8` -- so the boss-level
    /// floor does not apply.)
    #[test]
    fn adjust_sound_params_listener_is_source() {
        unsafe {
            assert_ne!(std::ptr::addr_of!(gamemap).read(), 8);
            let saved = snd_SfxVolume;
            snd_SfxVolume = 100;
            let mut listener = stub(0, 0);
            let mut vol: c_int = 0;
            let mut sep: c_int = 0;
            let rc = adjust_sound_params(
                &mut listener as *mut MobjStub,
                &mut listener as *mut MobjStub,
                &mut vol,
                &mut sep,
            );
            snd_SfxVolume = saved;
            assert_eq!(rc, 1);
            assert_eq!(vol, 100);
            assert_eq!(sep, 129);
        }
    }

    /// A source beyond `S_CLIPPING_DIST` on a non-boss map is clipped out
    /// entirely (return 0).
    #[test]
    fn adjust_sound_params_far_source_clipped() {
        unsafe {
            assert_ne!(std::ptr::addr_of!(gamemap).read(), 8);
            let mut listener = stub(0, 0);
            let mut source = stub(2000 * FRACUNIT, 0);
            let mut vol: c_int = 0;
            let mut sep: c_int = 0;
            let rc = adjust_sound_params(
                &mut listener as *mut MobjStub,
                &mut source as *mut MobjStub,
                &mut vol,
                &mut sep,
            );
            assert_eq!(rc, 0);
        }
    }
}
