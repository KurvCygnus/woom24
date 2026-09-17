//! Aggregator for the audio backend submodules (sfx + music).
//!
//! The Rust audio backend replaces the SDL2 mixer that the original
//! `vendor/doomgeneric/i_sound.c` and `i_oplmusic.c` drive.  It is built on
//! top of `rodio`: each Doom logical channel becomes a rodio `Player`
//! connected to a single shared output device, and music decoding lives in
//! the `music` submodule.
//!
//! State is kept in a `thread_local!` `AUDIO` cell.  Doom's audio API is
//! synchronous and only ever called from the main thread, so a thread-local
//! is both safe and the simplest place to hold the `MixerDeviceSink` without
//! introducing any global lock.

pub(crate) mod music;
pub(crate) mod sfx;

use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

pub(crate) use sfx::{decode_doom_sfx, PannedSource};

/// Control-plane seam between the engine (`doom::i_sound`) and a platform
/// audio backend. The data plane (mixing graph) belongs to the backend.
pub trait AudioBackend {
    // SFX — mirrors the existing AudioState methods verbatim.
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool;
    fn stop_sound(&mut self, channel: usize);
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32);
    fn is_playing(&self, channel: usize) -> bool;
    // Music — mirrors MusicState; the backend owns its own mixer, so the
    // `&Mixer` parameter of MusicState::play disappears.
    fn load_sound_font(&mut self, path: &std::path::Path);
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool);
    fn stop_music(&mut self);
    fn set_music_volume(&self, vol: i32);
    fn pause_music(&self);
    fn resume_music(&self);
    fn is_music_playing(&self) -> bool;
}

thread_local! {
    /// Factory installed by the platform shell before the engine initialises
    /// audio. `None` in contexts without a shell (tests): audio stays silent.
    static BACKEND_FACTORY: Cell<Option<BackendFactory>> = const { Cell::new(None) };
}

/// Signature of a backend constructor installed by a platform shell.
pub type BackendFactory = fn() -> Result<Box<dyn AudioBackend>, String>;

/// Install the platform's backend constructor. Called once from the shell
/// (native: rodio; wasm: Web Audio) before `doomgeneric_Create`.
pub fn set_backend_factory(factory: BackendFactory) {
    BACKEND_FACTORY.with(|f| f.set(Some(factory)));
}

/// Invoke the installed factory. Returns `None` when no shell installed a
/// factory (headless/test contexts) or when the factory itself reports
/// failure (no audio device) — both mean "run silent", matching today's
/// `AUDIO == None` behavior.
pub(crate) fn create_backend() -> Option<Box<dyn AudioBackend>> {
    BACKEND_FACTORY.with(|f| f.get()).and_then(|factory| {
        let built = factory();
        if let Err(e) = &built {
            log::warn!("audio backend construction failed (running silent): {e}");
        }
        built.ok()
    })
}

/// The silent backend. Every control operation is a no-op and nothing ever
/// reports as playing. Formalises the engine's "no audio device" path.
pub struct NoopBackend;

impl AudioBackend for NoopBackend {
    fn start_sound(&mut self, _data: &[u8], _vol: i32, _sep: i32, _channel: usize) -> bool {
        true
    }
    fn stop_sound(&mut self, _channel: usize) {}
    fn update_sound_params(&self, _channel: usize, _vol: i32, _sep: i32) {}
    fn is_playing(&self, _channel: usize) -> bool {
        false
    }
    fn load_sound_font(&mut self, _path: &std::path::Path) {}
    fn play_music(&mut self, _midi_bytes: &[u8], _looping: bool) {}
    fn stop_music(&mut self) {}
    fn set_music_volume(&self, _vol: i32) {}
    fn pause_music(&self) {}
    fn resume_music(&self) {}
    fn is_music_playing(&self) -> bool {
        false
    }
}

thread_local! {
    /// Thread-local home for the singleton [`AudioState`].
    ///
    /// `None` until [`AudioState::new`] succeeds, after which callers in
    /// `doom::i_sound` and `doom::s_sound` borrow it mutably to start/stop
    /// sounds.  Keeping it thread-local avoids needing a `Mutex` because Doom
    /// only ever drives audio from the main loop thread.
    pub(crate) static AUDIO: RefCell<Option<Box<dyn AudioBackend>>> =
        const { RefCell::new(None) };
}

/// Singleton audio backend state held in the [`AUDIO`] thread-local.
///
/// Owns the output device sink, the shared mixer that all players feed into,
/// the eight Doom SFX channels, and the music state.
pub(crate) struct AudioState {
    /// The device-bound sink.  Held only to keep the audio device open for
    /// the lifetime of the engine; the mixer below is what we actually push
    /// samples through.
    pub(crate) _device_sink: MixerDeviceSink,
    /// Cloned handle to the device sink's mixer.  Cheap to clone (it is an
    /// `Arc` internally) and shared across all per-channel `Player`s and the
    /// music player.
    pub(crate) mixer: rodio::mixer::Mixer,
    /// Per-channel state for Doom's eight logical SFX channels.  `Box`ed so
    /// the `AudioState` itself stays small and movable.
    channels: Box<[sfx::ChannelState; 8]>,
    /// Music decoder/player state (MUS conversion, MIDI sequencing, volume).
    pub(crate) music: music::MusicState,
}

/// Construction and channel-control entry points used by the engine's
/// `S_sound` shims.
impl AudioState {
    /// Open the default audio device and build a fresh [`AudioState`].
    ///
    /// Returns an error if the platform refuses to open a default output
    /// device (e.g. no audio hardware, audio daemon not running).  On success
    /// the eight channel slots are empty (`player == None`) and music
    /// playback is idle.
    pub(crate) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let device_sink = DeviceSinkBuilder::open_default_sink()?;
        let mixer = device_sink.mixer().clone();
        let channels = Box::new(std::array::from_fn(|_| sfx::ChannelState::new()));
        Ok(Self {
            _device_sink: device_sink,
            mixer,
            channels,
            music: music::MusicState::new(),
        })
    }

    /// Start playing a Doom DMX SFX blob on the given logical channel.
    ///
    /// `data` is the raw `DSxxxxx` lump bytes; `vol` is the Doom 0-127 volume
    /// scale; `sep` is the 0-255 stereo separation (`128` is centred);
    /// `channel` must be in `0..8`.
    ///
    /// Returns `true` on success, `false` if the channel index is out of
    /// range or if the SFX lump fails to decode.  Any previously playing
    /// sound on the same channel is replaced (its `Player` is dropped, which
    /// stops playback immediately).
    pub(crate) fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        let Some((sample_rate, samples)) = decode_doom_sfx(data) else {
            return false;
        };
        let pan = Arc::clone(&self.channels[channel].pan);
        pan.update(vol, sep);
        let buf = SamplesBuffer::new(
            std::num::NonZero::new(1u16).unwrap(),
            std::num::NonZero::new(sample_rate).unwrap(),
            samples,
        );
        let source = PannedSource::new(buf, pan);
        let player = Player::connect_new(&self.mixer);
        player.append(source);
        self.channels[channel].player = Some(player);
        true
    }

    /// Immediately stop whatever is playing on the given channel.
    ///
    /// Out-of-range `channel` values are silently ignored, matching the
    /// permissive behaviour of the SDL backend.
    pub(crate) fn stop_sound(&mut self, channel: usize) {
        if channel >= 8 {
            return;
        }
        self.channels[channel].player = None;
    }

    /// Update the volume and stereo separation of an in-flight sound without
    /// restarting it.
    ///
    /// Doom calls this every tic while a sound is active so the apparent
    /// position tracks the emitting `mobj`.  Out-of-range `channel` values
    /// are silently ignored.
    pub(crate) fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        if channel >= 8 {
            return;
        }
        self.channels[channel].pan.update(vol, sep);
    }

    /// Return `true` if the channel still has unconsumed samples.
    ///
    /// Out-of-range `channel` values report `false`.
    pub(crate) fn is_playing(&self, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        self.channels[channel]
            .player
            .as_ref()
            .is_some_and(|p| !p.empty())
    }
}

impl AudioBackend for AudioState {
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        AudioState::start_sound(self, data, vol, sep, channel)
    }
    fn stop_sound(&mut self, channel: usize) {
        AudioState::stop_sound(self, channel)
    }
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        AudioState::update_sound_params(self, channel, vol, sep)
    }
    fn is_playing(&self, channel: usize) -> bool {
        AudioState::is_playing(self, channel)
    }
    fn load_sound_font(&mut self, path: &std::path::Path) {
        self.music.load_sound_font(path)
    }
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        let mixer = self.mixer.clone();
        self.music.play(midi_bytes, looping, &mixer);
    }
    fn stop_music(&mut self) {
        self.music.stop()
    }
    fn set_music_volume(&self, vol: i32) {
        self.music.set_volume(vol)
    }
    fn pause_music(&self) {
        self.music.pause()
    }
    fn resume_music(&self) {
        self.music.resume()
    }
    fn is_music_playing(&self) -> bool {
        self.music.is_playing()
    }
}

#[cfg(test)]
mod noop_tests {
    use super::*;

    #[test]
    fn noop_backend_reports_silence() {
        let mut b = NoopBackend;
        assert!(b.start_sound(&[], 100, 128, 0), "Noop accepts sounds (silent success)");
        assert!(!b.is_playing(0), "Noop never reports playback");
        assert!(!b.is_music_playing(), "Noop never reports music");
        b.stop_sound(0);
        b.update_sound_params(0, 50, 128);
        b.load_sound_font(std::path::Path::new("none.sf2"));
        b.play_music(&[], false);
        b.stop_music();
        b.set_music_volume(50);
        b.pause_music();
        b.resume_music();
    }

    #[test]
    fn no_factory_means_silent() {
        // Backends are created only via a shell-installed factory; in tests
        // none is installed, mirroring headless runs.
        assert!(create_backend().is_none());
    }
}
