//! Rust port of `vendor/doomgeneric/s_sound.c`.
//!
//! High-level sound subsystem: owns the pool of mixing channels, distance and
//! stereo attenuation, music playback control and the SFX lookup table. The
//! low-level driver work (lump fetching, mixing, output) lives in `i_sound`.
//!
//! Channel allocation mirrors the C version: a fixed-size `channel_t` array is
//! allocated from zone memory at startup, then `get_channel` picks a free
//! channel or evicts one of equal-or-lower priority. Stereo and volume are
//! derived from listener-to-source distance and angle.
//!
//! Notable Rust-vs-C differences:
//! - `MobjStub` is a layout-compatible prefix of `mobj_t` (see the struct doc).
//!   Only the fields actually touched by attenuation are named; the rest are
//!   opaque padding so we do not need to mirror `r_defs::mobj_t` here.
//! - C linkage is preserved via `#[no_mangle]` on every exported tunable
//!   global and via `#[export_name]` on every renamed function, so other
//!   ported modules and the test harness keep working.
//!
//! ## Submodule Responsibility
//!
//! - `channels.rs` -- the `channel_t` slot descriptor, the zone-allocated
//!   `channels` array, and the `stop_channel` / `get_channel` pool
//!   primitives (private in C, so no C symbols to pin)
//! - `params.rs` -- the `MobjStub` mobj prefix, the attenuation / stereo
//!   constants, and `adjust_sound_params`
//! - `play.rs` -- `start_level_sounds`, `start_sound`, `stop_sound`, and
//!   `update_sounds`
//! - `music.rs` -- the volume/config statics, `mus_playing` state,
//!   `init_sound` / `shutdown_sound`, and the music control family
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `S_Init` | `music::init_sound` | glue | precache + volume apply + channel allocation, registers shutdown via `I_AtExit`; also runs `S_InitSfxLinks` (Rust-side data-table init); C symbol pinned via `#[export_name]`; upstream `s_sound.c:114` |
//! | `S_Shutdown` | `music::shutdown_sound` | glue | driver teardown wired through `I_AtExit`; C symbol pinned; upstream `s_sound.c:146` |
//! | `S_StopChannel` | `channels::stop_channel` | glue | private in C too; the dead middle loop is a faithful port and stays (see the fn doc) |
//! | `S_Start` | `play::start_level_sounds` | glue | per-level music pick incl. the Ultimate-Doom `spmus` table; C symbol pinned; upstream `s_sound.c:191` |
//! | `S_StopSound` | `play::stop_sound` | glue | first matching-origin channel; C symbol pinned; upstream `s_sound.c:243` |
//! | `S_GetChannel` | `channels::get_channel` | glue | private in C too; higher numeric priority = less important (eviction rule) |
//! | `S_AdjustSoundParams` | `params::adjust_sound_params` | glue | private in C too; pure wrapping fixed-point math whose outputs reach only the audio driver (see Deterministic Aspects) |
//! | `S_StartSound` | `play::start_sound` | glue | the sim-wide entry; C symbol pinned via `#[export_name]` (`hu_stuff/mod.rs` extern-declares it -- the one hard extern pin of this module); upstream `s_sound.c:391` |
//! | `S_PauseSound` | `music::pause_sound` | glue | C symbol pinned; upstream `s_sound.c:482` |
//! | `S_ResumeSound` | `music::resume_sound` | glue | C symbol pinned; upstream `s_sound.c:491` |
//! | `S_UpdateSounds` | `play::update_sounds` | glue | per-tic from `g_game` (path call); C symbol pinned; upstream `s_sound.c:504` |
//! | `S_SetMusicVolume` | `music::set_music_volume` | glue | 0-127 range guard; C symbol pinned; upstream `s_sound.c:571` |
//! | `S_SetSfxVolume` | `music::set_sfx_volume` | glue | 0-127 range guard (baseline-tested); C symbol pinned; upstream `s_sound.c:582` |
//! | `S_StartMusic` | `music::start_music` | glue | non-looping `change_music` wrapper; C symbol pinned; upstream `s_sound.c:596` |
//! | `S_ChangeMusic` | `music::change_music` | glue | AdLib/SB intro reroute, lump load + register + play; C symbol pinned; upstream `s_sound.c:601` |
//! | `S_MusicPlaying` | `music::music_playing` | glue | thin `I_MusicIsPlaying` wrapper; C symbol pinned (dead-but-exported); upstream `s_sound.c:649` |
//! | `S_StopMusic` | `music::stop_music` | glue | stop + unregister + lump release; C symbol pinned; upstream `s_sound.c:654` |
//! | `channel_t` (C type) | `channels::channel_t` | data | name kept; `repr(C)` slot descriptor |
//! | `MobjStub` (Rust-only) | `params::MobjStub` | data | name kept; layout-critical `mobj_t` prefix (`MEMORY.md` layout-bug note) |
//! | volume / config statics | `music` | data | `sfxVolume`, `musicVolume`, `snd_channels`, `snd_SfxVolume`, `snd_MusicVolume`, `mus_paused` keep names + `#[no_mangle]` (config-bound via `d_main/bind.rs`) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: vanilla demos carry input only; sound
//! channel / attenuation state never feeds the simulation's observable
//! state. The one nuance worth recording: `adjust_sound_params`
//! (`params.rs`) is *pure integer math* (wrapping fixed-point) and looks
//! dtmc-shaped, but its outputs reach only the audio driver -- verdict
//! glue. If a future feature ever fed sound state back into the
//! simulation (it must not), this adjudication would need revisiting.

pub mod channels;
pub mod music;
pub mod params;
pub mod play;

//* path-stability re-export: the config-bound statics keep their
//* module-root paths (`d_main/bind.rs` binds them; `d_main/boot.rs` and
//* `m_menu` read them).
pub use music::{musicVolume, mus_paused, sfxVolume, snd_MusicVolume, snd_SfxVolume, snd_channels};

//* path-stability re-export: the module's public types keep their
//* module-root paths (`p_mobj/lifecycle.rs` names `MobjStub`).
pub use channels::channel_t;
pub use params::MobjStub;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use music::{
    change_music as S_ChangeMusic, init_sound as S_Init, music_playing as S_MusicPlaying,
    pause_sound as S_PauseSound, resume_sound as S_ResumeSound, set_music_volume as S_SetMusicVolume,
    set_sfx_volume as S_SetSfxVolume, shutdown_sound as S_Shutdown, start_music as S_StartMusic,
    stop_music as S_StopMusic,
};
pub use play::{
    start_level_sounds as S_Start, start_sound as S_StartSound, stop_sound as S_StopSound,
    update_sounds as S_UpdateSounds,
};
