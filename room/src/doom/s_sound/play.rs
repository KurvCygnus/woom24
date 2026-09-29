//! The SFX play path: per-level sound start, the `S_StartSound` entry the
//! whole sim fires sounds through, per-origin stops, and the per-tic
//! `update_sounds` housekeeping.

use std::ffi::{c_int, c_void};

use super::channels::{channels, get_channel, stop_channel};
use super::music::{change_music, snd_SfxVolume, snd_channels};
use super::params::{adjust_sound_params, MobjStub, NORM_SEP};
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::g_game::{consoleplayer, gameepisode, gamemap, players};
use crate::doom::i_sound::{
    I_GetSfxLumpNum, I_SoundIsPlaying, I_StartSound, I_UpdateSound, I_UpdateSoundParams,
};
use crate::doom::sounds::{Mus, SfxInfo, S_sfx, NUMSFX};

/// Per-level startup: stop every channel, unpause music, and start the level's
/// music track based on `gamemode`/`gameepisode`/`gamemap`.
///
/// For Doom 2 the track is `mus_runnin + gamemap - 1`. For Ultimate Doom
/// episode 4 a hard-coded `spmus[]` table picks Romero's chosen songs.
/// C origin: `S_Start` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game` imports the upstream name through the root shim.
#[doc(alias = "S_Start")]
#[export_name = "S_Start"]
pub extern "C" fn start_level_sounds() {
    unsafe {
        for cnum in 0..snd_channels {
            if !(*channels.offset(cnum as isize)).sfxinfo.is_null() {
                stop_channel(cnum);
            }
        }

        super::music::mus_paused = 0;

        let mnum: c_int;

        if gamemode == d_mode::commercial {
            mnum = Mus::Runnin as c_int + gamemap - 1;
        } else {
            let spmus: [c_int; 9] = [
                Mus::E3m4 as c_int,
                Mus::E3m2 as c_int,
                Mus::E3m3 as c_int,
                Mus::E1m5 as c_int,
                Mus::E2m7 as c_int,
                Mus::E2m4 as c_int,
                Mus::E2m6 as c_int,
                Mus::E2m5 as c_int,
                Mus::E1m9 as c_int,
            ];

            if gameepisode < 4 {
                mnum = Mus::E1m1 as c_int + (gameepisode - 1) * 9 + gamemap - 1;
            } else {
                mnum = spmus[(gamemap - 1) as usize];
            }
        }

        change_music(mnum, 1);
    }
}

/// Stop any sound whose `origin` matches the given mobj. Walks the channel
/// list and stops the first match. C origin: `S_StopSound` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_mobj` / `p_pspr` and the rest of the sim import the upstream name
/// through the root shim.
#[doc(alias = "S_StopSound")]
#[export_name = "S_StopSound"]
pub extern "C" fn stop_sound(origin: *mut MobjStub) {
    unsafe {
        for cnum in 0..snd_channels {
            let ch = *channels.offset(cnum as isize);
            if !ch.sfxinfo.is_null() && ch.origin == origin {
                stop_channel(cnum);
                break;
            }
        }
    }
}

/// Start an SFX from `origin_p` (an `mobj_t *`, may be null) with id `sfx_id`.
///
/// Bogus ids are silently dropped. If the sfx has a `link`, its `volume`
/// modifier is applied and clamped. Stereo/volume are computed from the
/// listener (the console player) unless `origin` is the listener itself, in
/// which case stereo is forced to centre. The chosen sfx replaces any sound
/// already playing on `origin`. C origin: `S_StartSound` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `hu_stuff.rs` extern-declares it (the one hard extern pin of this
/// module) and the rest of the sim imports the upstream name through the
/// root shim.
#[doc(alias = "S_StartSound")]
#[export_name = "S_StartSound"]
pub extern "C" fn start_sound(origin_p: *mut c_void, sfx_id: c_int) {
    unsafe {
        let origin = origin_p as *mut MobjStub;
        let mut volume = snd_SfxVolume;

        if sfx_id < 1 || sfx_id > NUMSFX as c_int {
            // Bogus sound id - skip
            return;
        }

        let sfx = &mut *std::ptr::addr_of_mut!(S_sfx[0]).offset(sfx_id as isize);

        if !sfx.link.is_null() {
            volume += sfx.volume;

            if volume < 1 {
                return;
            }

            if volume > snd_SfxVolume {
                volume = snd_SfxVolume;
            }
        }

        let mut sep: c_int = NORM_SEP;

        let player_mo =
            (*std::ptr::addr_of!(players[0]).offset(consoleplayer as isize)).mo as *mut MobjStub;
        if !origin.is_null() && origin != player_mo {
            let listener = player_mo;
            let rc = adjust_sound_params(listener, origin, &mut volume, &mut sep);

            if (*origin).x == (*listener).x && (*origin).y == (*listener).y {
                sep = NORM_SEP;
            }

            if rc == 0 {
                return;
            }
        }

        stop_sound(origin);

        let cnum = get_channel(origin, sfx);

        if cnum < 0 {
            return;
        }

        if sfx.usefulness < 0 {
            sfx.usefulness = 1;
        }
        sfx.usefulness += 1;

        if sfx.lumpnum < 0 {
            sfx.lumpnum = I_GetSfxLumpNum(sfx as *mut SfxInfo as *mut c_void);
        }

        (*channels.offset(cnum as isize)).handle =
            I_StartSound(sfx as *mut SfxInfo as *mut c_void, cnum, volume, sep);
    }
}

/// Per-tic sound housekeeping: drive the low-level mixer and re-attenuate
/// each active positional sound against the current `listener`. Stops
/// channels whose sound stopped playing or fell out of audible range.
///
/// C origin: `S_UpdateSounds` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` imports the upstream name through the root shim.
#[doc(alias = "S_UpdateSounds")]
#[export_name = "S_UpdateSounds"]
pub extern "C" fn update_sounds(listener: *mut MobjStub) {
    unsafe {
        I_UpdateSound();

        for cnum in 0..snd_channels {
            let c = &mut *channels.offset(cnum as isize);
            let sfx = c.sfxinfo;

            if !sfx.is_null() {
                if I_SoundIsPlaying(c.handle) != 0 {
                    let mut volume = snd_SfxVolume;
                    let mut sep = NORM_SEP;

                    if !(*sfx).link.is_null() {
                        volume += (*sfx).volume;
                        if volume < 1 {
                            stop_channel(cnum);
                            continue;
                        } else if volume > snd_SfxVolume {
                            volume = snd_SfxVolume;
                        }
                    }

                    if !c.origin.is_null() && listener != c.origin {
                        let audible =
                            adjust_sound_params(listener, c.origin, &mut volume, &mut sep);

                        if audible == 0 {
                            stop_channel(cnum);
                        } else {
                            I_UpdateSoundParams(c.handle, volume, sep);
                        }
                    }
                } else {
                    stop_channel(cnum);
                }
            }
        }
    }
}
