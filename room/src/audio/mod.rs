//! Aggregator for the audio subsystem: the backend-agnostic control plane
//! plus the pure, platform-shared DSP helpers in the `sfx` and `music`
//! submodules.
//!
//! The Rust audio backend replaces the SDL2 mixer that the original
//! `vendor/doomgeneric/i_sound.c` and `i_oplmusic.c` drive.  The control
//! plane is the [`AudioBackend`] trait: platform shells implement it (the
//! native shell builds a mixer graph on top of the pure helpers re-exported
//! here) and install a constructor via [`set_backend_factory`] before the
//! engine initialises audio.
//!
//! State is kept in a `thread_local!` `AUDIO` cell.  Doom's audio API is
//! synchronous and only ever called from the main thread, so a thread-local
//! is both safe and the simplest place to hold the backend handle without
//! introducing any global lock.

pub mod sfx;
pub(crate) mod music;

pub use music::{mus2midi, SAMPLE_RATE};
pub use sfx::{decode_doom_sfx, gains_from, PanState};

use std::cell::Cell;
use std::cell::RefCell;

/// Control-plane seam between the engine (`doom::i_sound`) and a platform
/// audio backend. The data plane (mixing graph) belongs to the backend.
pub trait AudioBackend {
    // SFX control operations.
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool;
    fn stop_sound(&mut self, channel: usize);
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32);
    fn is_playing(&self, channel: usize) -> bool;
    // Music control operations. A backend that owns its own mixer passes
    // that mixer's handle to the music player internally.
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
/// (native: the shell's mixer-graph backend; wasm: Web Audio) before
/// `doomgeneric_Create`.
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
    /// Thread-local home for the platform's audio backend.
    ///
    /// Holds the `Box<dyn AudioBackend>` produced by the factory installed
    /// via [`set_backend_factory`] (and built by [`create_backend`]).
    /// `None` until a shell-installed factory succeeds, after which callers
    /// in `doom::i_sound` and `doom::s_sound` borrow it mutably to
    /// start/stop sounds.  Keeping it thread-local avoids needing a `Mutex`
    /// because Doom only ever drives audio from the main loop thread.
    pub(crate) static AUDIO: RefCell<Option<Box<dyn AudioBackend>>> =
        const { RefCell::new(None) };
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
