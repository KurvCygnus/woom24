//! SFX data plane and the [`RodioBackend`] control plane for the native
//! shell.
//!
//! Moved verbatim from the engine library's former `room::audio` modules:
//! the per-channel mixer plumbing ([`PannedSource`], [`ChannelState`]) from
//! `sfx`, and the backend that used to be `AudioState` — renamed, with the
//! shared mixer handle inlined into the struct — implementing
//! `room::audio::AudioBackend`.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::{Player, Source};

use room::audio::{decode_doom_sfx, AudioBackend, PanState};

use crate::audio_music::MusicState;

/// Stereo adapter that wraps a mono `rodio::Source` and emits interleaved
/// left/right samples, each scaled by the gains held in [`PanState`].
///
/// `S` is the underlying mono source; `buffered` holds the input sample
/// between the left output and the right output so each input frame produces
/// exactly two output frames.
pub(crate) struct PannedSource<S> {
    /// Mono input source.  Read once per emitted stereo frame.
    inner: S,
    /// Shared gain state, read every output sample.
    pan: Arc<PanState>,
    /// `None` after a left sample is emitted; `Some(s)` after we've stashed
    /// the input sample so the next call can emit the right channel.
    buffered: Option<f32>,
}

/// Construction helper for [`PannedSource`].
impl<S: Source> PannedSource<S> {
    /// Wrap `inner` and pair it with a shared [`PanState`] for live gain
    /// updates.  The wrapped source must be mono; multi-channel input is
    /// unsupported (only one input sample is read per emitted stereo frame).
    pub(crate) fn new(inner: S, pan: Arc<PanState>) -> Self {
        Self {
            inner,
            pan,
            buffered: None,
        }
    }
}

/// Iterator impl producing alternating left/right samples.
impl<S: Source> Iterator for PannedSource<S> {
    type Item = f32;

    /// Pull one stereo sample. State machine on `buffered`: when empty,
    /// fetch a fresh mono sample from `inner`, cache it, and emit it
    /// scaled by the left gain; when full, emit the cached sample
    /// scaled by the right gain and clear the cache.
    fn next(&mut self) -> Option<f32> {
        match self.buffered.take() {
            None => {
                let s = self.inner.next()?;
                let l = s * f32::from_bits(self.pan.l_gain.load(Ordering::Relaxed));
                self.buffered = Some(s);
                Some(l)
            }
            Some(s) => Some(s * f32::from_bits(self.pan.r_gain.load(Ordering::Relaxed))),
        }
    }
}

/// `rodio::Source` impl that advertises the wrapped source as stereo.
impl<S: Source> Source for PannedSource<S> {
    /// Length of the current contiguous span in samples; doubled because each
    /// mono input sample yields two output samples.
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len().map(|n| n * 2)
    }
    /// Always 2 (stereo).
    fn channels(&self) -> rodio::ChannelCount {
        std::num::NonZero::new(2).unwrap()
    }
    /// Pass through the underlying mono source's sample rate.
    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }
    /// Pass through the underlying source's total duration (if any).
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

/// State for one of Doom's eight logical SFX channels.
///
/// Held in an array by [`RodioBackend`].  The optional `player` is
/// the live `rodio::Player` currently playing on this channel (dropping it
/// stops playback immediately); the shared `pan` outlives individual players
/// so volume/separation updates between sounds remain coherent.
pub(crate) struct ChannelState {
    /// Active rodio player for this channel, or `None` when idle.  Dropping
    /// the player halts playback.
    pub(crate) player: Option<Player>,
    /// Shared pan state cloned into every [`PannedSource`] on this channel
    /// so [`RodioBackend::update_sound_params`] can adjust
    /// gains without restarting playback.
    pub(crate) pan: Arc<PanState>,
}

/// Default-construction helper for [`ChannelState`].
impl ChannelState {
    /// Build an idle channel: no active player and a centred-volume pan state
    /// (`vol = 127`, `sep = 127`).
    pub(crate) fn new() -> Self {
        Self {
            player: None,
            pan: Arc::new(PanState::new(127, 127)),
        }
    }
}

// ---------------------------------------------------------------------------
// RodioBackend — the former AudioState, renamed
// ---------------------------------------------------------------------------

/// The native shell's audio backend: the engine's former `AudioState`,
/// renamed, with the mixer handle inlined into the struct.
pub struct RodioBackend {
    /// The device-bound sink.  Held only to keep the audio device open for
    /// the lifetime of the engine; the mixer below is what we actually push
    /// samples through.
    _device_sink: rodio::MixerDeviceSink,
    /// Cloned handle to the device sink's mixer.  Cheap to clone (it is an
    /// `Arc` internally) and shared across all per-channel `Player`s and the
    /// music player.
    mixer: rodio::mixer::Mixer,
    /// Per-channel state for Doom's eight logical SFX channels.  `Box`ed so
    /// the backend itself stays small and movable.
    channels: Box<[ChannelState; 8]>,
    /// Music decoder/player state (MIDI sequencing, volume).
    music: MusicState,
}

/// Construction helper for [`RodioBackend`].
impl RodioBackend {
    /// Open the default audio device and build a fresh [`RodioBackend`].
    ///
    /// Returns an error if the platform refuses to open a default output
    /// device (e.g. no audio hardware, audio daemon not running).  On success
    /// the eight channel slots are empty (`player == None`) and music
    /// playback is idle.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let device_sink = rodio::DeviceSinkBuilder::open_default_sink()?;
        let mixer = device_sink.mixer().clone();
        let channels = Box::new(std::array::from_fn(|_| ChannelState::new()));
        Ok(Self {
            _device_sink: device_sink,
            mixer,
            channels,
            music: MusicState::new(),
        })
    }
}

/// Engine-facing control operations. The bodies are identical to the
/// `impl AudioBackend for AudioState` (and its inherent helpers) that lived
/// in `room/src/audio/mod.rs` before the move; the only delta is the struct
/// name.
impl AudioBackend for RodioBackend {
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
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
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
    /// Out-of-range `channel` values are silently ignored, matching the
    /// permissive behaviour of the SDL backend.
    fn stop_sound(&mut self, channel: usize) {
        if channel >= 8 {
            return;
        }
        self.channels[channel].player = None;
    }

    /// Update the volume and stereo separation of an in-flight sound without
    /// restarting it.  Out-of-range `channel` values are silently ignored.
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        if channel >= 8 {
            return;
        }
        self.channels[channel].pan.update(vol, sep);
    }

    /// Return `true` if the channel still has unconsumed samples.
    /// Out-of-range `channel` values report `false`.
    fn is_playing(&self, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        self.channels[channel]
            .player
            .as_ref()
            .is_some_and(|p| !p.empty())
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
mod tests {
    use super::*;
    use std::num::NonZero;

    #[test]
    fn panned_source_center_two_samples() {
        let buf = SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(11025).unwrap(),
            vec![1.0_f32, 0.5_f32],
        );
        let pan = Arc::new(PanState::new(127, 127));
        let mut src = PannedSource::new(buf, Arc::clone(&pan));
        let l1 = src.next().unwrap();
        let r1 = src.next().unwrap();
        assert!((l1 - 0.5_f32).abs() < 1e-4, "l1={l1}");
        assert!((r1 - 0.5_f32).abs() < 1e-4, "r1={r1}");
        let l2 = src.next().unwrap();
        let r2 = src.next().unwrap();
        assert!((l2 - 0.25_f32).abs() < 1e-4, "l2={l2}");
        assert!((r2 - 0.25_f32).abs() < 1e-4, "r2={r2}");
        assert!(src.next().is_none());
    }

    #[test]
    fn panned_source_hard_left() {
        let buf = SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(11025).unwrap(),
            vec![0.5_f32],
        );
        let pan = Arc::new(PanState::new(127, 0));
        let mut src = PannedSource::new(buf, pan);
        let l = src.next().unwrap();
        let r = src.next().unwrap();
        assert!((l - 0.5_f32).abs() < 1e-4, "l={l}");
        assert!((r - 0.0_f32).abs() < 1e-4, "r={r}");
    }

    #[test]
    fn panned_source_channels_is_2() {
        use rodio::Source;
        let buf = SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(11025).unwrap(),
            vec![0.0_f32],
        );
        let pan = Arc::new(PanState::new(127, 127));
        let src = PannedSource::new(buf, pan);
        assert_eq!(src.channels().get(), 2);
    }

    #[test]
    fn panned_source_live_pan_update() {
        let buf = SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(11025).unwrap(),
            vec![1.0_f32, 1.0_f32],
        );
        let pan = Arc::new(PanState::new(127, 127));
        let mut src = PannedSource::new(buf, Arc::clone(&pan));
        let _ = src.next();
        let _ = src.next();
        pan.update(127, 0);
        let l2 = src.next().unwrap();
        let r2 = src.next().unwrap();
        assert!((l2 - 1.0_f32).abs() < 1e-4, "l2={l2}");
        assert!((r2 - 0.0_f32).abs() < 1e-4, "r2={r2}");
    }
}
