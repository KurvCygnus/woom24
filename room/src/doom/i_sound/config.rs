//! The `snd_*` config statics (plus the `EMPTY_CSTR` backing store and
//! the `c_bytes` literal helper) and `bind_sound_variables`, the
//! `m_config` binding surface.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};

use crate::doom::m_config::M_BindVariable;

/// Music device selector. C default is `SNDDEVICE_SB` (1); this port
/// uses `3` (`SNDDEVICE_SB_PRO`) to match the original chocolate-doom
/// behaviour observed at runtime. Configurable via `m_config`.
#[no_mangle]
pub static mut snd_musicdevice: c_int = 3;

/// SFX device selector. Same notes as `snd_musicdevice`.
#[no_mangle]
pub static mut snd_sfxdevice: c_int = 3;

/// Output sample rate for digital sound, in Hz. Mirrors
/// `snd_samplerate` from `i_sound.c`. Bound via `m_config`.
#[no_mangle]
pub static mut snd_samplerate: c_int = 44100;

/// Maximum number of bytes the SFX cache may hold. Default 64 MiB,
/// matching the C source. Bound via `m_config`.
#[no_mangle]
pub static mut snd_cachesize: c_int = 64 * 1024 * 1024;

/// Mixer slice length in milliseconds. Default 28 ms (~1 buffer per
/// 35 Hz tic). Mirrors `snd_maxslicetime_ms` in `i_sound.c`.
#[no_mangle]
pub static mut snd_maxslicetime_ms: c_int = 28;

/// Legacy SoundBlaster I/O port. Retained only so the config file
/// stays cross-compatible with chocolate-doom; not used at runtime.
#[no_mangle]
pub static mut snd_sbport: c_int = 0;

/// Legacy SoundBlaster IRQ. Same note as `snd_sbport`.
#[no_mangle]
pub static mut snd_sbirq: c_int = 0;

/// Legacy SoundBlaster DMA channel. Same note as `snd_sbport`.
#[no_mangle]
pub static mut snd_sbdma: c_int = 0;

/// Legacy MPU-401 MIDI port. Same note as `snd_sbport`.
#[no_mangle]
pub static mut snd_mport: c_int = 0;

/// Pitch-shift toggle. Not consumed by the rustysynth music backend;
/// preserved for config-file compatibility.
#[no_mangle]
pub static mut snd_pitchshift: c_int = 0;

/// Backing storage for the empty default `snd_musiccmd` C string.
/// `m_config` may rewrite `snd_musiccmd` to point at a different
/// buffer; this static merely supplies the initial NUL byte.
static mut EMPTY_CSTR: [c_char; 1] = [0];

/// External shell command used by the original chocolate-doom to play
/// back music via an external process. Unused by this port (the
/// `crate::audio::music` backend handles playback directly) but kept
/// for config-file compatibility. Starts pointing at `EMPTY_CSTR`.
#[no_mangle]
pub static mut snd_musiccmd: *mut c_char = unsafe { std::ptr::addr_of_mut!(EMPTY_CSTR[0]) };

/// Reinterpret a `&[u8]` as a `&[c_char]` without copying.
///
/// `c_char` is `i8` on most targets; this helper lets `M_BindVariable`
/// calls below use byte-string literals while passing the same memory
/// as a C-char slice. The signedness reinterpretation is sound
/// because the byte values are pure ASCII.
const fn c_bytes(s: &[u8]) -> &'static [c_char]
{
    unsafe { &*(s as *const [u8] as *const [c_char]) }
}

/// Bind the `snd_*` globals to `m_config` so they survive across
/// runs via the config file. Mirrors `I_BindSoundVariables` from
/// `i_sound.c`. The C source also binds `use_libsamplerate` and
/// `libsamplerate_scale` under `FEATURE_SOUND`; those are omitted
/// here because this port doesn't use libsamplerate.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/bind.rs` imports the upstream name through the root shim.
#[doc(alias = "I_BindSoundVariables")]
#[export_name = "I_BindSoundVariables"]
pub extern "C" fn bind_sound_variables()
{
    M_BindVariable(
        c_bytes(b"snd_musicdevice\0").as_ptr() as *mut c_char,
        &raw mut snd_musicdevice as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_sfxdevice\0").as_ptr() as *mut c_char,
        &raw mut snd_sfxdevice as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_sbport\0").as_ptr() as *mut c_char,
        &raw mut snd_sbport as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_sbirq\0").as_ptr() as *mut c_char,
        &raw mut snd_sbirq as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_sbdma\0").as_ptr() as *mut c_char,
        &raw mut snd_sbdma as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_mport\0").as_ptr() as *mut c_char,
        &raw mut snd_mport as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_maxslicetime_ms\0").as_ptr() as *mut c_char,
        &raw mut snd_maxslicetime_ms as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_musiccmd\0").as_ptr() as *mut c_char,
        &raw mut snd_musiccmd as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_samplerate\0").as_ptr() as *mut c_char,
        &raw mut snd_samplerate as *mut c_int as *mut c_void,
    );
    M_BindVariable(
        c_bytes(b"snd_cachesize\0").as_ptr() as *mut c_char,
        &raw mut snd_cachesize as *mut c_int as *mut c_void,
    );
}
