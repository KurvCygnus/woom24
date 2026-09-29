//! Music playback and subsystem lifecycle: the volume/config statics, the
//! active-track state, `init_sound` / `shutdown_sound`, and the
//! pause / resume / start / change / stop music family.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void};

use super::channels::{channel_t, channels};
use crate::c_write;
use crate::doom::i_sound::{
    I_MusicIsPlaying, I_PauseSong, I_PlaySong, I_PrecacheSounds, I_RegisterSong, I_ResumeSong,
    I_SetMusicVolume, I_ShutdownMusic, I_ShutdownSound, I_StopSong, I_UnRegisterSong,
    snd_musicdevice,
};
use crate::doom::i_system::I_AtExit;
use crate::doom::sounds::{
    Mus, MusicInfo, S_InitSfxLinks, S_music, S_sfx, NUMMUSIC, NUMSFX,
};
use crate::doom::w_wad::{W_CacheLumpNum, W_GetNumForName, W_LumpLength, W_ReleaseLumpNum};
use crate::doom::z_zone::{PU_STATIC, Z_Malloc};
use crate::types::Boolean;

/// Music device id for AdLib, as recognised by `snd_musicdevice` config.
const SNDDEVICE_ADLIB: c_int = 2;
/// Music device id for SoundBlaster, as recognised by `snd_musicdevice`.
const SNDDEVICE_SB: c_int = 3;

/// User-facing SFX volume slider value (0-15). Mirrors `sfxVolume` in
/// `s_sound.c`; `#[no_mangle]` so the menu and config code share the symbol.
#[no_mangle]
pub static mut sfxVolume: c_int = 8;

/// User-facing music volume slider value (0-15). Mirrors `musicVolume` in
/// `s_sound.c`; exposed via `#[no_mangle]` for the menu and config code.
#[no_mangle]
pub static mut musicVolume: c_int = 8;

/// Number of mixing channels to allocate (default 8). Read by `init_sound`.
#[no_mangle]
pub static mut snd_channels: c_int = 8;

/// Internal SFX volume on the 0-127 scale used by the driver. Set by
/// `set_sfx_volume`. C name: `snd_SfxVolume`.
#[no_mangle]
pub static mut snd_SfxVolume: c_int = 8;

/// Internal music volume on the 0-127 scale used by the driver. Set by
/// `set_music_volume`. C name: `snd_MusicVolume`.
#[no_mangle]
pub static mut snd_MusicVolume: c_int = 8;

/// Non-zero while music is paused, used by `pause_sound`/`resume_sound`.
#[no_mangle]
pub static mut mus_paused: c_int = 0;

/// Currently playing music entry, or null if no song is active.
static mut mus_playing: *mut MusicInfo = std::ptr::null_mut();

/// Initialise the sound subsystem.
///
/// Pre-caches all SFX lumps, applies initial volumes, allocates the channel
/// array from zone memory, resets every channel to free, marks every sfx
/// lump as not-yet-loaded, and registers `shutdown_sound` with `I_AtExit`.
///
/// `sfx_volume` and `music_volume` are the initial 0-15 slider values.
///
/// C origin: `S_Init` in `s_sound.c`. The Rust port additionally calls
/// `S_InitSfxLinks` here because that work is data-table-side in Rust.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "S_Init")]
#[export_name = "S_Init"]
pub extern "C" fn init_sound(sfx_volume: c_int, music_volume: c_int) {
    unsafe {
        S_InitSfxLinks();

        I_PrecacheSounds(
            std::ptr::addr_of_mut!(S_sfx[0]) as *mut c_void,
            NUMSFX as c_int,
        );

        set_sfx_volume(sfx_volume);
        set_music_volume(music_volume);

        channels = Z_Malloc(
            (snd_channels as usize * std::mem::size_of::<channel_t>()) as c_int,
            PU_STATIC,
            std::ptr::null_mut(),
        ) as *mut channel_t;

        for i in 0..snd_channels {
            (*channels.offset(i as isize)).sfxinfo = std::ptr::null_mut();
        }

        mus_paused = 0;

        for i in 1..NUMSFX {
            (*std::ptr::addr_of_mut!(S_sfx[0]).add(i)).lumpnum = -1;
            (*std::ptr::addr_of_mut!(S_sfx[0]).add(i)).usefulness = -1;
        }

        I_AtExit(shutdown_sound, Boolean::TRUE);
    }
}

/// Tear down the SFX and music drivers. Wired in at process exit via
/// `I_AtExit` from `init_sound`. C origin: `S_Shutdown` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "S_Shutdown")]
#[export_name = "S_Shutdown"]
pub extern "C" fn shutdown_sound() {
    I_ShutdownSound();
    I_ShutdownMusic();
}

/// Pause the currently playing music if any. No-op if nothing is playing or
/// the song is already paused. C origin: `S_PauseSound` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` imports the upstream name through the root shim.
#[doc(alias = "S_PauseSound")]
#[export_name = "S_PauseSound"]
pub extern "C" fn pause_sound() {
    unsafe {
        if !mus_playing.is_null() && mus_paused == 0 {
            I_PauseSong();
            mus_paused = 1;
        }
    }
}

/// Resume music previously paused by `pause_sound`. No-op if nothing was
/// paused. C origin: `S_ResumeSound` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` imports the upstream name through the root shim.
#[doc(alias = "S_ResumeSound")]
#[export_name = "S_ResumeSound"]
pub extern "C" fn resume_sound() {
    unsafe {
        if !mus_playing.is_null() && mus_paused != 0 {
            I_ResumeSong();
            mus_paused = 0;
        }
    }
}

/// Set the music volume (0-127). Values out of range are silently ignored to
/// match the C behaviour. C origin: `S_SetMusicVolume` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_menu` imports the upstream name through the root shim.
#[doc(alias = "S_SetMusicVolume")]
#[export_name = "S_SetMusicVolume"]
pub extern "C" fn set_music_volume(volume: c_int) {
    if !(0..=127).contains(&volume) {
        return;
    }

    I_SetMusicVolume(volume);
}

/// Set the SFX volume (0-127), stored in `snd_SfxVolume` and applied to
/// subsequent `S_StartSound` calls. Values out of range are ignored.
/// C origin: `S_SetSfxVolume` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_menu` imports the upstream name through the root shim.
#[doc(alias = "S_SetSfxVolume")]
#[export_name = "S_SetSfxVolume"]
pub extern "C" fn set_sfx_volume(volume: c_int) {
    unsafe {
        if !(0..=127).contains(&volume) {
            return;
        }

        snd_SfxVolume = volume;
    }
}

/// Start playing music `m_id` once (non-looping). Convenience wrapper around
/// `change_music`. C origin: `S_StartMusic` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `f_finale` imports the upstream name through the root shim.
#[doc(alias = "S_StartMusic")]
#[export_name = "S_StartMusic"]
pub extern "C" fn start_music(m_id: c_int) {
    change_music(m_id, 0);
}

/// Switch the active music track.
///
/// If the device is AdLib or SoundBlaster, the intro is rerouted from
/// `Mus::Intro` to `Mus::Introa` (a shorter variant). Out-of-range ids are
/// ignored. The lump for the chosen track is loaded on first use, registered
/// with the music driver, played, and tracked in `mus_playing`. `looping`
/// non-zero requests an infinite loop.
///
/// C origin: `S_ChangeMusic` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `f_finale` / `s_sound` itself import the upstream name through the root
/// shim.
#[doc(alias = "S_ChangeMusic")]
#[export_name = "S_ChangeMusic"]
pub extern "C" fn change_music(musicnum: c_int, looping: c_int) {
    unsafe {
        let mut musicnum = musicnum;

        if musicnum == Mus::Intro as c_int
            && (snd_musicdevice == SNDDEVICE_ADLIB || snd_musicdevice == SNDDEVICE_SB)
        {
            musicnum = Mus::Introa as c_int;
        }

        if musicnum <= Mus::None as c_int || musicnum >= NUMMUSIC as c_int {
            return;
        }

        let music = &mut *std::ptr::addr_of_mut!(S_music[0]).offset(musicnum as isize);

        if mus_playing == music {
            return;
        }

        stop_music();

        if music.lumpnum == 0 {
            let mut namebuf: [c_char; 9] = [0; 9];
            let name_str = std::ffi::CStr::from_ptr(music.name).to_string_lossy();
            c_write!(namebuf, "d_{}", name_str);
            music.lumpnum = W_GetNumForName(namebuf.as_ptr() as *const c_char);
        }

        music.data = W_CacheLumpNum(music.lumpnum, PU_STATIC);

        let len = W_LumpLength(music.lumpnum as c_uint);
        let handle = I_RegisterSong(music.data, len);
        music.handle = handle;
        I_PlaySong(handle, looping);

        mus_playing = music;
    }
}

/// Return non-zero if the music driver reports an active song. Thin wrapper
/// around `I_MusicIsPlaying`. C origin: `S_MusicPlaying` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "S_MusicPlaying")]
#[export_name = "S_MusicPlaying"]
pub extern "C" fn music_playing() -> c_int {
    I_MusicIsPlaying()
}

/// Stop the currently playing music, unregister the song, and release its
/// WAD lump back to zone memory. No-op if nothing is playing.
/// C origin: `S_StopMusic` in `s_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_menu` / `d_main` import the upstream name through the root shim.
#[doc(alias = "S_StopMusic")]
#[export_name = "S_StopMusic"]
pub extern "C" fn stop_music() {
    unsafe {
        if !mus_playing.is_null() {
            let m = &mut *mus_playing;

            if mus_paused != 0 {
                I_ResumeSong();
            }

            I_StopSong();
            I_UnRegisterSong(m.handle);
            W_ReleaseLumpNum(m.lumpnum);
            m.data = std::ptr::null_mut();
            mus_playing = std::ptr::null_mut();
        }
    }
}

/// Baseline vector written before the graduation move (F10 wave F2-b): the
/// volume range guard must be identical before and after the split.
#[cfg(test)]
mod tests {
    use super::*;

    /// Volume setters ignore out-of-range values (C behaviour) and accept
    /// the 0-127 range ends. State is read through `addr_of!` (no
    /// mutable-static references).
    #[test]
    fn set_volume_range_guard() {
        unsafe {
            let saved = std::ptr::addr_of!(snd_SfxVolume).read();
            set_sfx_volume(128);
            assert_eq!(std::ptr::addr_of!(snd_SfxVolume).read(), saved);
            set_sfx_volume(64);
            assert_eq!(std::ptr::addr_of!(snd_SfxVolume).read(), 64);
            set_sfx_volume(saved);

            set_sfx_volume(0);
            assert_eq!(std::ptr::addr_of!(snd_SfxVolume).read(), 0);
            set_sfx_volume(saved);

            set_music_volume(-1);
            set_music_volume(128);
            set_music_volume(127);
        }
    }
}
