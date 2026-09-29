//! Rust port of `vendor/doomgeneric/i_sound.c`.
//!
//! Low-level sound and music interface. The C source delegates every
//! call to a `sound_module_t` / `music_module_t` vtable (set up by
//! `InitSfxModule` / `InitMusicModule`). This port collapses that
//! indirection into a single concrete backend living in the
//! `crate::audio` module: SFX are mixed via the platform audio state,
//! and MIDI/MUS playback is routed through rustysynth using a bundled
//! Sound Canvas SC-55 SoundFont.
//!
//! Each exported symbol keeps the original Doom C symbol via
//! `#[export_name]` so the rest of the engine and the wasm export
//! surface link unchanged. Public `static mut` `snd_*` globals retain
//! C linkage because `m_config.c` binds them by address as
//! user-configurable variables.
//!
//! ## Submodule Responsibility
//!
//! - `config.rs` -- the `snd_*` config statics (plus the `EMPTY_CSTR`
//!   backing store and the `c_bytes` literal helper) and
//!   `bind_sound_variables`, the `m_config` binding surface
//! - `sfx.rs` -- the SFX driver family: subsystem init/shutdown, lump
//!   resolution, the per-tic mixer hooks, start/stop/query on the eight
//!   mixing channels, and the no-op precache stub
//! - `music.rs` -- the music driver family: init/shutdown, volume,
//!   pause/resume, the register/play/stop song lifecycle over
//!   `crate::audio::music::MusicHandle`, and the SoundFont resolution
//!   (`find_soundfont_path`)
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the path-stability re-exports of the config statics,
//! and the upstream-name shims below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `I_InitSound` | `sfx::init_audio` | glue | creates the `crate::audio` backend (tolerant of absence, matching C's tolerance of missing devices); drops C's `-nosound`/`-nosfx`/`-nomusic`/`screensaver_mode` gating (upstream `i_sound.c:144-198` -- no `-nosound` handling exists anywhere in this port); C symbol pinned (`d_main/boot.rs` imports the upstream name through the root shim); upstream `i_sound.c:144` |
//! | `I_ShutdownSound` | `sfx::shutdown_audio` | glue | drops the global `AUDIO` state (C shut both module vtables down, `i_sound.c:200`); C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:200` |
//! | `I_GetSfxLumpNum` | `sfx::get_sfx_lump_num` | glue | inlines the `DS` + 6-char lump-name build and the 64-deep `link` walk the C SDL backend owned; returns -1 on null where C returns 0 on no-module; C symbol pinned (`s_sound/play.rs` imports it); upstream `i_sound.c:213` |
//! | `I_UpdateSound` | `sfx::update_sound` | glue | no-op per-tic mixer hook (the platform backend mixes on its own thread); C symbol pinned (`s_sound/play.rs` imports it); upstream `i_sound.c:225` |
//! | `I_UpdateSoundParams` | `sfx::update_sound_params` | glue | out-of-range channel guard; C's `CheckVolumeSeparation` clamping (upstream `i_sound.c:238`) is delegated to the mixer, which clamps equivalently; C symbol pinned (`s_sound/play.rs` imports it); upstream `i_sound.c:259` |
//! | `I_StartSound` | `sfx::start_sfx` | glue | caches the lump (`PU_CACHE`), hands a borrowed byte slice to the backend, returns the channel or -1; C symbol pinned (`s_sound/play.rs` imports it); upstream `i_sound.c:268` |
//! | `I_StopSound` | `sfx::stop_sfx` | glue | channel-range guard + backend stop; C symbol pinned (`s_sound/channels.rs` imports it); upstream `i_sound.c:281` |
//! | `I_SoundIsPlaying` | `sfx::sfx_is_playing` | glue | channel-range guard + backend query; C symbol pinned (`s_sound/channels.rs` / `play.rs` import it); upstream `i_sound.c:289` |
//! | `I_PrecacheSounds` | `sfx::precache_sounds` | glue | no-op -- the backend caches lazily in `start_sfx` (C's `CacheSounds` vtable slot, `i_sound.c:301`); C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:301` |
//! | `I_InitMusic` | `music::init_music` | glue | locates a SoundFont (`find_soundfont_path`) and loads it into the backend; no-op when none is found; C symbol pinned (`d_main/boot.rs` imports it); upstream `i_sound.c:309` |
//! | `I_ShutdownMusic` | `music::shutdown_music` | glue | actually stops music -- the C body is empty (`i_sound.c:317-320`); C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:317` |
//! | `I_SetMusicVolume` | `music::set_device_music_volume` | glue | device-level volume push (the `s_sound` slider static of the same meaning is `s_sound`'s); C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:322` |
//! | `I_PauseSong` | `music::pause_song` | glue | C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:330` |
//! | `I_ResumeSong` | `music::resume_song` | glue | C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:338` |
//! | `I_RegisterSong` | `music::register_song` | glue | performs MUS-to-MIDI up front (C hands the raw MUS lump to libtimidity, `i_sound.c:346`), boxes a `MusicHandle` as the opaque song id; C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:346` |
//! | `I_UnRegisterSong` | `music::unregister_song` | glue | frees the boxed handle, null ignored; C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:358` |
//! | `I_PlaySong` | `music::play_song` | glue | borrows the handle's MIDI bytes into the backend's playback thread; C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:366` |
//! | `I_StopSong` | `music::stop_song` | glue | handle stays valid for replay; C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:374` |
//! | `I_MusicIsPlaying` | `music::music_is_playing` | glue | C symbol pinned (`s_sound/music.rs` imports it); upstream `i_sound.c:382` |
//! | `I_BindSoundVariables` | `config::bind_sound_variables` | glue | binds the ten `snd_*` config vars through `m_config`'s root shim; the C `FEATURE_SOUND` libsamplerate bindings (`i_sound.c:411-414`) are omitted (no libsamplerate in this port); binding order preserved verbatim; C symbol pinned (`d_main/bind.rs` imports it); upstream `i_sound.c:395` |
//! | `snd_*` statics (11) | `config` | data | names + `#[no_mangle]` kept (config surface; bound by address via `bind_sound_variables`, no extern declarers anywhere); `snd_musicdevice`/`snd_sfxdevice` default 3 (`SNDDEVICE_SB_PRO`) vs C `SNDDEVICE_SB` (1), deliberately, to match the observed chocolate-doom runtime behaviour; `snd_pitchshift` has no vendor `i_sound.c` counterpart (newer-chocolate config var, unbound) |
//! | `CheckVolumeSeparation`, `InitSfxModule`, `InitMusicModule`, `SndDeviceInList` (C file-statics) | -- (never ported) | -- | vtable-selection plumbing and clamping, collapsed into the `crate::audio` backend; nothing to split |
//! | `find_soundfont_path` (Rust-only) | `music::find_soundfont_path` | glue | private; SF2 resolution: wasm VFS hint -> `-sf2` argv -> bundled path -> exe-relative; the wasm branch fixes "spec 4 defect B" |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: vanilla demo files carry input only;
//! the sound driver consumes sim-derived values (channel ids, volume /
//! separation attenuations, lump-backed sample bytes) and never produces
//! state the simulation reads back. The closest-looking candidates,
//! checked individually: `get_sfx_lump_num` reads the WAD directory and
//! walks `SfxInfo::link`, but its output feeds playback selection only;
//! `start_sfx`'s returned channel number lands in `s_sound`'s
//! `channel_t::handle`, which is audio bookkeeping (and `s_sound` itself
//! adjudicated glue wholesale); `update_sound` / `update_sound_params`
//! run per-tic from `s_sound::update_sounds` but only push to the
//! driver. Init/shutdown and config binding are boot glue. If a future
//! feature ever fed audio state back into the simulation (it must not),
//! this adjudication would need revisiting.

pub mod config;
pub mod music;
pub mod sfx;

//* path-stability re-export: the config-bound statics keep their
//* module-root paths (`s_sound/music.rs` reads `snd_musicdevice`; the
//* others are wasm export surface only).
pub use config::{
    snd_cachesize, snd_maxslicetime_ms, snd_mport, snd_musiccmd, snd_musicdevice, snd_pitchshift,
    snd_sbdma, snd_sbirq, snd_sbport, snd_sfxdevice, snd_samplerate,
};
//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use config::bind_sound_variables as I_BindSoundVariables;
pub use music::{
    init_music as I_InitMusic, music_is_playing as I_MusicIsPlaying, pause_song as I_PauseSong,
    play_song as I_PlaySong, register_song as I_RegisterSong, resume_song as I_ResumeSong,
    set_device_music_volume as I_SetMusicVolume, shutdown_music as I_ShutdownMusic,
    stop_song as I_StopSong, unregister_song as I_UnRegisterSong,
};
pub use sfx::{
    get_sfx_lump_num as I_GetSfxLumpNum, init_audio as I_InitSound,
    precache_sounds as I_PrecacheSounds, shutdown_audio as I_ShutdownSound,
    sfx_is_playing as I_SoundIsPlaying, start_sfx as I_StartSound, stop_sfx as I_StopSound,
    update_sound as I_UpdateSound, update_sound_params as I_UpdateSoundParams,
};
