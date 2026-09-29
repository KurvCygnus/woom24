//! The SFX driver family: subsystem init/shutdown, lump resolution, the
//! per-tic mixer hooks, start/stop/query on the eight mixing channels,
//! and the no-op precache stub.

use std::ffi::{c_char, c_int, c_uint, c_void};

use crate::doom::sounds::SfxInfo;
use crate::doom::w_wad::{W_CacheLumpNum, W_CheckNumForName, W_LumpLength};
use crate::doom::z_zone::PU_CACHE;

use crate::types::Boolean;

/// Initialise the audio subsystem.
///
/// Allocates the global `AUDIO` state if it has not been created yet.
/// On failure the game continues silently (with a `log::warn!`),
/// matching the C source's tolerance of missing sound devices.
///
/// The `use_sfx_prefix` flag is accepted for API compatibility but
/// ignored: the bundled SFX module always uses the `DS` lump-name
/// prefix encoded in `get_sfx_lump_num`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "I_InitSound")]
#[export_name = "I_InitSound"]
pub extern "C" fn init_audio(_use_sfx_prefix: Boolean)
{
    crate::audio::AUDIO.with_borrow_mut(|audio| {
        if audio.is_none() {
            match crate::audio::create_backend() {
                Some(backend) => {
                    log::info!("Audio initialised");
                    *audio = Some(backend);
                }
                None => log::warn!("No audio backend installed (running silent)"),
            }
        }
    });
}

/// Tear down the audio subsystem by dropping the global `AUDIO`
/// state. Safe to call even if `init_audio` was never run.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_ShutdownSound")]
#[export_name = "I_ShutdownSound"]
pub extern "C" fn shutdown_audio()
{
    crate::audio::AUDIO.with_borrow_mut(|audio| *audio = None);
}

/// Resolve the WAD lump number for a sound effect's sample data.
///
/// Walks the `link` chain so aliased SFX (`SfxInfo::link` non-null)
/// inherit the lump of their target, up to a depth limit of 64 to
/// avoid cycles. The lump name is `DS` plus the first six characters
/// of `SfxInfo::name`. Returns `-1` if `sfxinfo` is null or the lump
/// is not present.
///
/// In the C source this is delegated to `sound_module->GetSfxLumpNum`
/// and the lump-name construction lives in the SDL backend. This port
/// inlines the construction here.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/play.rs` imports the upstream name through the root shim.
#[doc(alias = "I_GetSfxLumpNum")]
#[export_name = "I_GetSfxLumpNum"]
pub extern "C" fn get_sfx_lump_num(sfxinfo: *mut c_void) -> c_int
{
    if sfxinfo.is_null() { return -1; }
    unsafe
    {
        let mut sfx = sfxinfo as *const SfxInfo;
        let mut depth = 0usize;
        while !(*sfx).link.is_null() && depth < 64
        {
            sfx = (*sfx).link;
            depth += 1;
        }
        let mut lump_name = [0 as c_char; 9];
        lump_name[0] = b'D' as c_char;
        lump_name[1] = b'S' as c_char;
        for (i, &c) in (*sfx).name.iter().take(6).enumerate()
        {
            if c == 0 { break; }
            lump_name[2 + i] = c;
        }
        W_CheckNumForName(lump_name.as_ptr())
    }
}

/// Per-frame sound update hook. The platform audio backend mixes
/// audio on its own thread so this is a no-op; the symbol exists
/// because the engine main loop calls it every tic.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/play.rs` imports the upstream name through the root shim.
#[doc(alias = "I_UpdateSound")]
#[export_name = "I_UpdateSound"]
pub extern "C" fn update_sound() {}

/// Update volume (`vol`, 0..=127) and stereo separation (`sep`,
/// 0..=254) for the SFX currently playing on `channel`.
///
/// Channel numbers outside `0..8` are ignored (the engine uses a
/// fixed 8-channel mixer). The C source clamps `vol`/`sep` via
/// `CheckVolumeSeparation`; the underlying Rust mixer in
/// `crate::audio` performs equivalent clamping internally.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/play.rs` imports the upstream name through the root shim.
#[doc(alias = "I_UpdateSoundParams")]
#[export_name = "I_UpdateSoundParams"]
pub extern "C" fn update_sound_params(channel: c_int, vol: c_int, sep: c_int)
{
    if !(0..8).contains(&channel) { return; }
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            a.update_sound_params(channel as usize, vol, sep);
        }
    });
}

/// Begin playing an SFX on `channel` with the given volume and
/// separation. Returns the channel number on success or `-1` on any
/// failure (null `sfxinfo`, invalid channel, missing lump, mixer
/// rejection).
///
/// Loads the sample data via `W_CacheLumpNum` with `PU_CACHE`, then
/// hands a borrowed byte slice to the audio backend's `start_sound`.
/// The cache tag means the data may be evicted once the SFX finishes.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/play.rs` imports the upstream name through the root shim.
#[doc(alias = "I_StartSound")]
#[export_name = "I_StartSound"]
pub extern "C" fn start_sfx(sfxinfo: *mut c_void, channel: c_int, vol: c_int, sep: c_int) -> c_int
{
    if sfxinfo.is_null() || !(0..8).contains(&channel) { return -1; }
    unsafe
    {
        let sfx = sfxinfo as *const SfxInfo;
        let lumpnum = (*sfx).lumpnum;
        if lumpnum < 0 { return -1; }
        let lump_len = W_LumpLength(lumpnum as c_uint);
        if lump_len <= 0 { return -1; }
        let ptr = W_CacheLumpNum(lumpnum, PU_CACHE);
        if ptr.is_null() { return -1; }
        let data = std::slice::from_raw_parts(ptr as *const u8, lump_len as usize);
        let mut result = -1;
        crate::audio::AUDIO.with_borrow_mut(|audio| {
            if let Some(a) = audio.as_mut() {
                if a.start_sound(data, vol, sep, channel as usize) {
                    result = channel;
                }
            }
        });
        result
    }
}

/// Stop the SFX currently playing on `channel`. Out-of-range channel
/// numbers are silently ignored.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/channels.rs` imports the upstream name through the root
/// shim.
#[doc(alias = "I_StopSound")]
#[export_name = "I_StopSound"]
pub extern "C" fn stop_sfx(channel: c_int)
{
    if !(0..8).contains(&channel) { return; }
    crate::audio::AUDIO.with_borrow_mut(|audio| {
        if let Some(a) = audio.as_mut() {
            a.stop_sound(channel as usize);
        }
    });
}

/// Return 1 if an SFX is currently audible on `channel`, 0 otherwise
/// (including for out-of-range channels). Mirrors `I_SoundIsPlaying`
/// from `i_sound.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/channels.rs` / `s_sound/play.rs` import the upstream name
/// through the root shim.
#[doc(alias = "I_SoundIsPlaying")]
#[export_name = "I_SoundIsPlaying"]
pub extern "C" fn sfx_is_playing(channel: c_int) -> c_int
{
    if !(0..8).contains(&channel) { return 0; }
    let mut playing = 0;
    crate::audio::AUDIO.with_borrow(|audio| {
        if let Some(a) = audio.as_ref() {
            playing = a.is_playing(channel as usize) as c_int;
        }
    });
    playing
}

/// Hook for the SDL backend to pre-cache a batch of sounds. The
/// platform audio backend caches lazily on `start_sfx`, so this
/// is a no-op. Kept for API compatibility with the engine.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound/music.rs` imports the upstream name through the root shim.
#[doc(alias = "I_PrecacheSounds")]
#[export_name = "I_PrecacheSounds"]
pub extern "C" fn precache_sounds(_sounds: *mut c_void, _num_sounds: c_int) {}
