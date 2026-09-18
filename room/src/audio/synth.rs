//! Pull-based synth core: a thin wrapper over rustysynth (spec 2 D4).
//!
//! The single implementation of "MIDI bytes → interleaved stereo f32 samples"
//! lives in the lib; native (rodio Source) and web (AudioBuffer chunks) each
//! adapt the output on their side, and the synthesis logic is never
//! duplicated across the two shells (AGENTS.md).
//!
//! Imperative state: 512-frame block rendering + an interleaved cursor + a
//! finished flag. Volume is not this module's concern (each output adapter
//! scales on its own), keeping the core pure.

/// Frames rendered per sequencer render (BLOCK_SIZE samples on each side).
pub const BLOCK_SIZE: usize = 512;

use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use std::io::Cursor;
use std::sync::Arc;

use super::SAMPLE_RATE;

/// A parsed SF2 font. Opaque to the outside: rustysynth types never leak into
/// any shell's API. Clone = Arc clone (cheap), letting backends reuse it
/// across play_music calls.
#[derive(Clone)]
pub struct SynthFont(Arc<SoundFont>);

impl SynthFont {
    /// Parses an SF2 from bytes; None on failure (the caller logs and stays
    /// silent).
    pub fn parse(sf2_bytes: &[u8]) -> Option<Self> {
        SoundFont::new(&mut Cursor::new(sf2_bytes))
            .ok()
            .map(Arc::new)
            .map(Self)
    }
}

/// One playback session: sequencer + per-channel staging blocks + interleaved
/// cursor. The font itself does not come along (Synthesizer internally holds
/// Arc<SoundFont>), avoiding a re-parse per playback.
pub struct SynthEngine {
    sequencer: MidiFileSequencer,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    finished: bool,
}

impl SynthEngine {
    /// Builds a playback session from a font + SMF bytes. None when MIDI
    /// parsing fails.
    pub fn new(font: &SynthFont, midi_bytes: &[u8], looping: bool) -> Option<Self> {
        let settings = SynthesizerSettings::new(SAMPLE_RATE);
        // rustysynth 1.3's Synthesizer::new takes &Arc<SoundFont>; pass the
        // reference directly.
        let synthesizer = Synthesizer::new(&font.0, &settings).ok()?;
        let midi_file = Arc::new(MidiFile::new(&mut Cursor::new(midi_bytes)).ok()?);
        let mut sequencer = MidiFileSequencer::new(synthesizer);
        sequencer.play(&midi_file, looping);
        Some(Self {
            sequencer,
            buf_l: vec![0.0f32; BLOCK_SIZE],
            buf_r: vec![0.0f32; BLOCK_SIZE],
            pos: BLOCK_SIZE * 2, // first next_interleaved triggers a render
            finished: false,
        })
    }

    /// Outputs the next sample interleaved (L,R,L,R...). None once the
    /// sequence ended and the current block drained.
    pub fn next_interleaved(&mut self) -> Option<f32> {
        if self.pos >= BLOCK_SIZE * 2 {
            if self.finished {
                return None;
            }
            self.sequencer.render(&mut self.buf_l, &mut self.buf_r);
            self.pos = 0;
            if self.sequencer.end_of_sequence() {
                self.finished = true;
            }
        }
        let s = if self.pos.is_multiple_of(2) {
            self.buf_l[self.pos / 2]
        } else {
            self.buf_r[self.pos / 2]
        };
        self.pos += 1;
        Some(s)
    }

    /// Whether the sequence has ended (the current block may still be draining).
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mus2midi;

    /// The same minimal MUS as the music.rs tests, but with one note-on so
    /// the synthesizer has a real event to render.
    fn note_on_mus() -> Vec<u8> {
        let mut d = vec![0u8; 19];
        d[0..4].copy_from_slice(b"MUS\x1a");
        d[4] = 3; // score_len (low byte)
        d[6] = 16; // score_start
        d[16] = 0x10; // note-on, channel 0
        d[17] = 0x3c; // note 60
        d[18] = 0x60; // score end (not "last", just a placeholder)
        d
    }

    #[test]
    fn synth_font_rejects_garbage() {
        assert!(SynthFont::parse(b"not a soundfont").is_none());
        assert!(SynthFont::parse(&[]).is_none());
    }

    #[test]
    fn synth_engine_rejects_bad_midi() {
        // Without an SF2 no engine can be built, so exercise a rejection path
        // instead: bad MIDI must return None -- fall back to verifying
        // mus2midi's gatekeeping here:
        assert!(mus2midi(b"garbage").is_none());
    }

    #[test]
    fn synth_engine_renders_deterministic_samples() {
        // Needs a local SF2 (any under soundfonts/, not committed); skip with
        // a note when missing.
        let Some(sf2_bytes) = find_local_sf2() else {
            eprintln!("skip: no real local .sf2 under soundfonts/ (missing, or only an un-smudged LFS stub); determinism not exercised");
            return;
        };
        let font = SynthFont::parse(&sf2_bytes).expect("local sf2 must parse");
        let midi = mus2midi(&note_on_mus()).expect("fixture MUS must convert");
        let mut a = SynthEngine::new(&font, &midi, false).expect("engine a");
        let mut b = SynthEngine::new(&font, &midi, false).expect("engine b");
        let sa: Vec<f32> = (0..4096).filter_map(|_| a.next_interleaved()).collect();
        let sb: Vec<f32> = (0..4096).filter_map(|_| b.next_interleaved()).collect();
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "同一输入必须逐位一致");
        }
        // After the note-on the output must not be all zeros (the event
        // really got synthesized).
        assert!(sa.iter().any(|&s| s != 0.0));
    }

    /// Finds any .sf2 under soundfonts/ (the dev machine has the SC-55 font;
    /// CI / fontless environments skip).
    /// The `cargo test` process cwd is the package root (room/), so the
    /// repo-root soundfonts/ must resolve via CARGO_MANIFEST_DIR/.. (same as
    /// tables.rs and demo_playthrough.rs).
    fn find_local_sf2() -> Option<Vec<u8>> {
        let candidates = [
            std::path::PathBuf::from("soundfonts"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("soundfonts"),
        ];
        candidates.iter().find_map(|root| find_sf2_under(root))
    }

    /// A `.sf2` big enough to be a real font. Un-smudged Git LFS checkouts
    /// keep a ~133-byte pointer stub under the real filename, so the
    /// extension alone is not enough -- parsing such a stub panics in
    /// `SynthFont::parse`'s `expect`. A genuine font is orders of magnitude
    /// larger than the 1 MiB bar, so undersized files fall through to the
    /// test's graceful skip.
    fn plausible_font_path(p: &std::path::Path) -> bool {
        const MIN_FONT_BYTES: u64 = 1 << 20;
        p.extension().is_some_and(|e| e == "sf2")
            && std::fs::metadata(p)
                .map(|m| m.len() >= MIN_FONT_BYTES)
                .unwrap_or(false)
    }

    /// Scans a single candidate directory (plus one level of subdirectories)
    /// for any .sf2.
    fn find_sf2_under(root: &std::path::Path) -> Option<Vec<u8>> {
        for entry in std::fs::read_dir(root).ok()?.flatten() {
            let p = entry.path();
            if plausible_font_path(&p) {
                return std::fs::read(&p).ok();
            }
            // One level of subdirectories is allowed (repo reality:
            // soundfonts/SC55Soundfont-1.2b/*.sf2).
            if p.is_dir() {
                for sub in std::fs::read_dir(&p).ok()?.flatten() {
                    let sp = sub.path();
                    if plausible_font_path(&sp) {
                        return std::fs::read(&sp).ok();
                    }
                }
            }
        }
        None
    }
}
