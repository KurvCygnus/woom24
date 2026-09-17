//! Music playback data plane for the native shell: SoundFont synthesis.
//!
//! Standard MIDI File bytes (produced by `room::audio::mus2midi` or supplied
//! as raw MIDI) are synthesised by the shared pull-based core
//! `room::audio::synth::SynthEngine` (a thin wrapper around `rustysynth`)
//! and streamed into the rodio mixer owned by the shell's audio backend.
//!
//! ## Pipeline
//!
//! ```text
//!   SMF bytes ──SynthEngine──▶ interleaved stereo f32 PCM
//!                                    │
//!                    Arc<AtomicU32> ─┴── volume gain
//!                                    │
//!                              rodio::Player ──▶ mixer
//! ```
//!
//! ## Components
//!
//! - [`MusicSource`] — a `rodio::Source` adapter that pulls interleaved
//!   samples from the engine and applies a shared volume gain.
//! - [`MusicState`] — owns the parsed SF2 SoundFont, the active player, and
//!   the shared volume cell; exposes load/play/stop/pause/resume.

use rodio::{Player, Source};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use room::audio::{SynthEngine, SynthFont, SAMPLE_RATE};

// ---------------------------------------------------------------------------
// MusicSource — rodio Source adapter over the shared SynthEngine core
// ---------------------------------------------------------------------------

/// rodio adapter: `SynthEngine` output × shared volume.  All rendering logic
/// lives on the lib side (`room::audio::synth`).
pub(crate) struct MusicSource {
    /// The shared, shell-independent synthesis engine (pull-based).
    engine: SynthEngine,
    /// Shared output gain (`f32` bits) updated by [`MusicState::set_volume`].
    volume: Arc<AtomicU32>,
}

/// Construction helper for [`MusicSource`].
impl MusicSource {
    /// Build a new music source from a parsed SoundFont and a SMF byte slice.
    ///
    /// Returns `None` if the engine cannot be constructed (e.g. `midi_bytes`
    /// is not a parseable Standard MIDI File).  When `looping` is `true` the
    /// engine will restart the file on completion.
    pub(crate) fn new(
        sound_font: &SynthFont,
        midi_bytes: &[u8],
        looping: bool,
        volume: Arc<AtomicU32>,
    ) -> Option<Self> {
        Some(Self {
            engine: SynthEngine::new(sound_font, midi_bytes, looping)?,
            volume,
        })
    }
}

/// Iterator impl producing interleaved L,R,L,R,… samples scaled by `volume`.
impl Iterator for MusicSource {
    type Item = f32;

    /// Bit-identical to the pre-refactor output: `SynthEngine::next_interleaved`
    /// scaled by the shared volume (transported as `f32` bits).
    fn next(&mut self) -> Option<f32> {
        let vol = f32::from_bits(self.volume.load(Ordering::Relaxed));
        self.engine.next_interleaved().map(|s| s * vol)
    }
}

/// `rodio::Source` impl describing the stream as 2-channel `f32` PCM at
/// [`SAMPLE_RATE`] Hz with no fixed length (as before the refactor).
impl Source for MusicSource {
    /// No fixed-length span: the engine can emit indefinitely (looping
    /// playback) or end at any block boundary.
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    /// Always 2 (stereo).
    fn channels(&self) -> rodio::ChannelCount {
        std::num::NonZero::new(2u16).unwrap()
    }
    /// Output sample rate, matching the engine's synthesiser settings.
    fn sample_rate(&self) -> rodio::SampleRate {
        std::num::NonZero::new(SAMPLE_RATE as u32).unwrap()
    }
    /// Total duration is unknown in general; looping streams have none.
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// ---------------------------------------------------------------------------
// MusicState — SF2 loading and playback control
// ---------------------------------------------------------------------------

/// Owns the SoundFont, the optional active player, and the shared volume
/// cell.  Held inside the shell's [`crate::rodio_backend::RodioBackend`].
pub(crate) struct MusicState {
    /// Loaded font, reused for every subsequent play (no rustysynth types
    /// held directly any more).  `None` until [`MusicState::load_sound_font`]
    /// succeeds.
    pub(crate) sound_font: Option<SynthFont>,
    /// Active rodio player playing the current song, or `None` when stopped.
    /// Dropping the player halts playback.
    player: Option<Player>,
    /// Shared volume gain (`f32` bits) read by every active [`MusicSource`].
    volume: Arc<AtomicU32>,
}

/// Public lifecycle and control API for music playback.
impl MusicState {
    /// Construct an idle state with no SoundFont loaded, no active player,
    /// and volume initialised to full (`1.0`).
    pub(crate) fn new() -> Self {
        Self {
            sound_font: None,
            player: None,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
        }
    }

    /// Load an SF2 SoundFont from disk.
    ///
    /// On success the loaded font is stored in `sound_font` and used for all
    /// subsequent [`MusicState::play`] calls.  Errors (missing file, parse
    /// failure) are logged but not propagated — playback simply remains a
    /// no-op until a valid font is loaded.
    pub(crate) fn load_sound_font(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => match SynthFont::parse(&bytes) {
                Some(f) => {
                    log::info!("Soundfont loaded: {}", path.display());
                    self.sound_font = Some(f);
                }
                None => log::warn!("Soundfont parse error ({})", path.display()),
            },
            Err(e) => log::warn!("Soundfont not found ({}): {e}", path.display()),
        }
    }

    /// Start playing the given MIDI bytes on the supplied rodio mixer.
    ///
    /// If `looping` is `true`, the sequencer will repeat the song
    /// indefinitely.  Any previous song is replaced (its player is dropped,
    /// stopping playback before the new one is appended).  Returns silently
    /// if no SoundFont is loaded or if the MIDI bytes are invalid.
    pub(crate) fn play(&mut self, midi_bytes: &[u8], looping: bool, mixer: &rodio::mixer::Mixer) {
        let Some(sf) = self.sound_font.as_ref() else {
            return;
        };
        let Some(source) = MusicSource::new(sf, midi_bytes, looping, Arc::clone(&self.volume))
        else {
            log::warn!("I_PlaySong: failed to create MusicSource");
            return;
        };
        self.player = None;
        let player = Player::connect_new(mixer);
        player.append(source);
        self.player = Some(player);
    }

    /// Stop the currently playing song, if any, by dropping the player.
    pub(crate) fn stop(&mut self) {
        self.player = None;
    }

    /// Set the music gain from a Doom volume value (`0..=127`).
    ///
    /// The value is clamped, normalised to `[0.0, 1.0]`, and stored as the
    /// bit pattern of an `f32` in the shared atomic so the audio thread can
    /// pick it up on its next sample.
    pub(crate) fn set_volume(&self, vol: i32) {
        let gain = (vol.clamp(0, 127) as f32 / 127.0).to_bits();
        self.volume.store(gain, Ordering::Relaxed);
    }

    /// Pause playback without dropping the player or losing position.
    pub(crate) fn pause(&self) {
        if let Some(p) = &self.player {
            p.pause();
        }
    }

    /// Resume a paused player.  No-op if no player is active.
    pub(crate) fn resume(&self) {
        if let Some(p) = &self.player {
            p.play();
        }
    }

    /// Report whether a player is active and still has samples to play.
    pub(crate) fn is_playing(&self) -> bool {
        self.player.as_ref().is_some_and(|p| !p.empty())
    }
}
