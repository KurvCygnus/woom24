//! The music driver family: init/shutdown, volume, pause/resume, the
//! register/play/stop song lifecycle over
//! `crate::audio::music::MusicHandle`, and the SoundFont resolution
//! (`find_soundfont_path`).

use std::ffi::{c_int, c_void};

use crate::audio::music::MusicHandle;
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};

/// Locate a `.sf2` SoundFont file for music playback.
///
/// Resolution order:
///  1. wasm only: the audio backend's SoundFont hint -- the
///     launcher-selected SF2 name registered under the VFS (there is no
///     filesystem to probe; see `AudioBackend::soundfont_hint`).
///  2. `-sf2 <path>` command-line argument (printed warning if missing).
///  3. Bundled SC-55 SoundFont under `soundfonts/...` relative to CWD.
///  4. Same path relative to the directory holding the executable.
///
/// Returns `None` and logs a warning if no font is found - music will
/// then play silently. Not present in the C source: chocolate-doom
/// uses an external timidity config instead.
fn find_soundfont_path() -> Option<std::path::PathBuf>
{
    //* wasm: no filesystem exists to probe; the backend holds the
    //* launcher-selected font's VFS name (spec 4 defect B - the old probe
    //* always failed and music went silent with a selected SF2).
    #[cfg(target_arch = "wasm32")]
    {
        let hinted = crate::audio::AUDIO.with_borrow(|audio| {
            audio.as_ref().and_then(|a| a.soundfont_hint())
        });
        if let Some(name) = hinted {
            return Some(std::path::PathBuf::from(name));
        }
    }

    unsafe {
        let p = M_CheckParmWithArgs(c"-sf2".as_ptr().cast_mut(), 1);
        if p != 0 {
            let arg = *myargv.add((p + 1) as usize);
            if !arg.is_null() {
                let s = std::ffi::CStr::from_ptr(arg).to_string_lossy();
                let path = std::path::PathBuf::from(s.as_ref());
                if path.exists() {
                    return Some(path);
                }
                log::warn!("-sf2 path not found: {}", path.display());
            }
        }
    }

    let bundled = std::path::Path::new("soundfonts/SC55Soundfont-1.2b/SC-55 SoundFont v1.2b.sf2");
    if bundled.exists() {
        return Some(bundled.to_path_buf());
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(bundled);
            if p.exists() {
                return Some(p);
            }
        }
    }

    log::warn!("No soundfont found. Use -sf2 <path> to specify one. Music will be silent.");
    None
}

/// Initialise the music backend by locating a SoundFont and loading
/// it into the audio module. No-op if no SoundFont can be found.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "I_InitMusic")]
#[export_name = "I_InitMusic"]
pub extern "C" fn init_music()
{
    if let Some(path) = find_soundfont_path() {
        crate::audio::AUDIO.with_borrow_mut(|audio| {
            if let Some(a) = audio.as_mut() {
                a.load_sound_font(&path);
            }
        });
    }
}

/// Stop any music playback and tear down music-specific state.
/// Mirrors `I_ShutdownMusic` from `i_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_ShutdownMusic")]
#[export_name = "I_ShutdownMusic"]
pub extern "C" fn shutdown_music()
{
    crate::audio::AUDIO.with_borrow_mut(|audio| {
        if let Some(a) = audio.as_mut() {
            a.stop_music();
        }
    });
}

/// Set the music output volume. The accepted range matches the
/// engine convention (0..=127); the music backend clamps internally.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_SetMusicVolume")]
#[export_name = "I_SetMusicVolume"]
pub extern "C" fn set_device_music_volume(volume: c_int)
{
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            a.set_music_volume(volume);
        }
    });
}

/// Pause the currently playing song without unloading it.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_PauseSong")]
#[export_name = "I_PauseSong"]
pub extern "C" fn pause_song()
{
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            a.pause_music();
        }
    });
}

/// Resume a song that was paused via `pause_song`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_ResumeSong")]
#[export_name = "I_ResumeSong"]
pub extern "C" fn resume_song()
{
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            a.resume_music();
        }
    });
}

/// Register a song for later playback by converting its MUS payload
/// to MIDI and returning an opaque `*mut MusicHandle` cast to
/// `*mut c_void`.
///
/// Returns null on null input, zero/negative length, or if MUS-to-MIDI
/// conversion fails. The caller must release the returned handle via
/// `unregister_song`; the boxed handle owns the MIDI bytes.
///
/// In the C source the song data is the raw MUS lump, which the SDL
/// backend hands to libtimidity. This port performs the MUS->MIDI
/// conversion up front so the rustysynth player can consume it.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_RegisterSong")]
#[export_name = "I_RegisterSong"]
pub extern "C" fn register_song(data: *mut c_void, len: c_int) -> *mut c_void
{
    if data.is_null() || len <= 0 {
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(data as *const u8, len as usize) };
    match crate::audio::music::mus2midi(slice) {
        Some(midi_bytes) => Box::into_raw(Box::new(MusicHandle { midi_bytes })) as *mut c_void,
        None => {
            log::warn!("I_RegisterSong: MUS-to-MIDI conversion failed");
            std::ptr::null_mut()
        }
    }
}

/// Free a music handle previously returned by `register_song`.
/// Null pointers are ignored. After this call the handle is invalid.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_UnRegisterSong")]
#[export_name = "I_UnRegisterSong"]
pub extern "C" fn unregister_song(handle: *mut c_void)
{
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle as *mut MusicHandle));
        }
    }
}

/// Start playing the song referenced by `handle`. If `looping` is
/// non-zero the song is looped indefinitely. No-op on null handle.
///
/// The MIDI bytes inside the handle are borrowed for the duration of
/// the call; the music backend internally clones them into its own
/// playback thread.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_PlaySong")]
#[export_name = "I_PlaySong"]
pub extern "C" fn play_song(handle: *mut c_void, looping: c_int)
{
    if handle.is_null() { return; }
    let music_handle = unsafe { &*(handle as *const MusicHandle) };
    crate::audio::AUDIO.with_borrow_mut(|audio| {
        if let Some(a) = audio.as_mut() {
            a.play_music(&music_handle.midi_bytes, looping != 0);
        }
    });
}

/// Stop the currently playing song. The handle itself remains valid
/// and can be replayed later. Mirrors `I_StopSong` from `i_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_StopSong")]
#[export_name = "I_StopSong"]
pub extern "C" fn stop_song()
{
    crate::audio::AUDIO.with_borrow_mut(|audio| {
        if let Some(a) = audio.as_mut() {
            a.stop_music();
        }
    });
}

/// Return 1 if a song is currently audible, 0 otherwise.
/// Mirrors `I_MusicIsPlaying` from `i_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_MusicIsPlaying")]
#[export_name = "I_MusicIsPlaying"]
pub extern "C" fn music_is_playing() -> c_int
{
    let mut playing = 0;
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            playing = a.is_music_playing() as c_int;
        }
    });
    playing
}
