//! The mixing channel pool: the `channel_t` slot descriptor, the
//! zone-allocated `channels` array, and the two pool primitives
//! (`stop_channel`, `get_channel`) that the play path drives.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

use super::music::snd_channels;
use super::params::MobjStub;
use crate::doom::i_sound::{I_SoundIsPlaying, I_StopSound};
use crate::doom::sounds::SfxInfo;

/// Mixing channel slot. C counterpart: `channel_t` defined inside `s_sound.c`.
///
/// A null `sfxinfo` marks the slot as free. `origin` points at the mobj the
/// sound is attached to (used to keep stereo/volume in sync while it moves);
/// it is `null` for player-local sounds. `handle` is the driver's opaque id.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct channel_t {
    /// Currently playing sound's metadata, or null if the channel is free.
    pub sfxinfo: *mut SfxInfo,
    /// Map object the sound emanates from, or null for non-positional sounds.
    pub origin: *mut MobjStub,
    /// Driver-side handle returned by `I_StartSound` for this channel.
    pub handle: c_int,
}

/// Heap-allocated array of `snd_channels` mixing channels. Allocated by
/// `init_sound` from zone memory, never freed (PU_STATIC).
pub(super) static mut channels: *mut channel_t = std::ptr::null_mut();

/// Stop the sound playing on channel `cnum` and free the slot.
///
/// Decrements the sfx's `usefulness` and clears `sfxinfo`, making the channel
/// available for [`get_channel`]. The middle loop scanning other channels is a
/// faithful port of the C version and is currently a no-op (its `break` does
/// not gate the usefulness update).
///
/// # Safety
/// `cnum` must be a valid index into the `channels` array (`0..snd_channels`).
pub(super) unsafe fn stop_channel(cnum: c_int) {
    let c = &mut *channels.offset(cnum as isize);

    if !c.sfxinfo.is_null() {
        if I_SoundIsPlaying(c.handle) != 0 {
            I_StopSound(c.handle);
        }

        for i in 0..snd_channels {
            if cnum != i
                && !channels.offset(i as isize).is_null()
                && (*channels.offset(i as isize)).sfxinfo == c.sfxinfo
            {
                break;
            }
        }

        (*c.sfxinfo).usefulness -= 1;
        c.sfxinfo = std::ptr::null_mut();
    }
}

/// Reserve a channel for a new sound.
///
/// Returns the chosen channel index, or `-1` if no eligible slot exists.
/// The function first tries to reuse a free channel or a channel already
/// owned by the same `origin` (stopping it first), then falls back to
/// evicting the first channel whose `priority` is greater than or equal
/// to `sfxinfo->priority`. In Doom's convention higher numeric `priority`
/// means less-important, so this evicts the first equally- or
/// less-important channel; if every active channel is more important
/// (numerically lower), the new sound is dropped and `-1` is returned.
///
/// # Safety
/// `sfxinfo` must point to a valid `SfxInfo`; `origin` may be null. `channels`
/// must already be allocated (call `init_sound` first).
pub(super) unsafe fn get_channel(origin: *mut MobjStub, sfxinfo: *mut SfxInfo) -> c_int {
    let mut cnum: c_int = 0;

    for cnum_search in 0..snd_channels {
        let ch = *channels.offset(cnum_search as isize);
        if ch.sfxinfo.is_null() {
            cnum = cnum_search;
            break;
        } else if !origin.is_null() && ch.origin == origin {
            stop_channel(cnum_search);
            cnum = cnum_search;
            break;
        }
    }

    if cnum == snd_channels {
        for cnum_search in 0..snd_channels {
            let ch = *channels.offset(cnum_search as isize);
            if !ch.sfxinfo.is_null() && (*ch.sfxinfo).priority >= (*sfxinfo).priority {
                cnum = cnum_search;
                break;
            }
        }

        if cnum == snd_channels {
            return -1;
        } else {
            stop_channel(cnum);
        }
    }

    let c = &mut *channels.offset(cnum as isize);
    c.sfxinfo = sfxinfo;
    c.origin = origin;

    cnum
}
