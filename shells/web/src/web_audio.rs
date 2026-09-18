//! WebAudioBackend: the engine's AudioBackend control plane on the browser
//! (spec 2 D4).
//!
//! Key points:
//! - The SF2 is preloaded from the VFS at construction (i_sound's exists() is
//!   always false on wasm, so I_InitMusic never calls load_sound_font -- see
//!   the plan's Task 4 design notes).
//! - Music uses "chunked pull": woom24_tick calls pump_music() every frame,
//!   scheduling 0.2s ahead.
//! - SFX: one source+gain+panner pair per logical channel; bookkeeping clears
//!   the slot at end_time = is_playing.

use std::cell::{Cell, RefCell};

use room::audio::{decode_doom_sfx, AudioBackend, SynthEngine, SynthFont, BLOCK_SIZE};

use crate::wasm_vfs;

/// Doom volume (0..=127) → linear gain.
// Used by the trait method bodies and the unit tests.
fn vol_gain(vol: i32) -> f32 {
    vol.clamp(0, 127) as f32 / 127.0
}

/// sep (0..=255, centered at 128) → StereoPannerNode.pan in [-1, 1].
/// Reuses the engine's gains_from (left/right gains), folded into a single
/// pan value.
/// Note: the engine's NORM_SEP centers at 128 (0..=255, see s_sound.rs)
/// while gains_from centers at 127 (0..=254, the lib's own convention) --
/// both curves are linear and coincide point for point after shifting the
/// center by 1, hence (sep - 1) is fed in.
fn sep_to_pan(sep: i32) -> f32 {
    let (l, r) = room::audio::gains_from(127, (sep - 1).clamp(0, 254));
    let sum = l + r;
    if sum <= f32::EPSILON {
        0.0
    } else {
        (r - l) / sum
    }
}

thread_local! {
    /// Name of the SF2 to preload at construction (set by init_pipeline
    /// before creating the backend).
    static PENDING_SF2: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Shell-side handle to the built backend: the engine stores the factory's
    /// product in the lib-private AUDIO cell, unreachable from the shell -- so
    /// every successful factory construction leaves an Rc here, used solely by
    /// woom24_tick's music pump.
    static LAST_BUILT: RefCell<Option<std::rc::Rc<RefCell<WebAudioBackend>>>> =
        const { RefCell::new(None) };
}

/// Called by init_pipeline before doomgeneric_Create: remembers the SF2 name.
pub fn set_pending_sf2(name: Option<String>) {
    PENDING_SF2.with_borrow_mut(|s| *s = name);
}

/// Music pump called by woom24_tick every frame (the engine's AUDIO cell is
/// out of the shell's reach, so the pump goes through the handle kept here;
/// the two holders each mind their own role and share no mutable state).
pub fn pump_current_backend() {
    LAST_BUILT.with_borrow(|slot| {
        if let Some(b) = slot.as_ref() {
            b.borrow_mut().pump_music();
        }
    });
}

/// Active node group for one SFX logical channel.
/// `end_time` = the AudioContext time at which the source finishes
/// (AudioBufferSourceNode has no ended polling property; time bookkeeping
/// stands in for the onended callback, which has no global handle).
#[allow(dead_code)]
struct SfxChannel {
    source: web_sys::AudioBufferSourceNode,
    gain: web_sys::GainNode,
    panner: web_sys::StereoPannerNode,
    end_time: f64,
}

//? The plan's Task 4 Step 3 new() used Rc::try_unwrap to hand out both the
//? body and the LAST_BUILT pump handle, but under Rust ownership one value
//? cannot be Box-exclusive and Rc-aliased at once (try_unwrap always fails at
//? strong count 2, and its Ok side is the RefCell, not the body -- the plan's
//? code does not compile as written). Minimal fix: sink the body state into
//? BackendCore and reduce WebAudioBackend to a cheap Rc handle (Clone); the
//? engine Box and the shell pump handle share the same core -- signatures and
//? the LAST_BUILT type stay as in the plan.
/// Backend body: actually owns the AudioContext and all node/engine state.
// State is only ever reached through WebAudioBackend handles.
struct BackendCore {
    ctx: web_sys::AudioContext,
    /// 8 logical channels (matching the engine's I_UpdateSoundParams 0..8).
    channels: [Option<SfxChannel>; 8],
    music_gain: web_sys::GainNode,
    /// Preloaded font (read from the VFS at construction; Clone = Arc clone,
    /// ready on demand).
    font: Option<SynthFont>,
    /// Current music engine; None = no font / not playing / finished.
    music: Option<SynthEngine>,
    /// Absolute time when the next music block should be scheduled
    /// (AudioContext clock, seconds).
    music_next_time: f64,
    /// Scheduled but unfinished source nodes (stopped en masse by stop_music).
    scheduled: Vec<web_sys::AudioBufferSourceNode>,
    /// Whether music is explicitly paused. The trait's pause/resume take
    /// &self, hence Cell interior mutability (fixes the plan code's E0594
    /// assignment through &self).
    paused: Cell<bool>,
}

/// The engine AudioBackend's wasm implementation: an Rc handle sharing the
/// core (Clone = refcount +1).
#[derive(Clone)]
pub struct WebAudioBackend {
    core: std::rc::Rc<RefCell<BackendCore>>,
}

impl BackendCore {
    // deprecated: web-sys 0.3.98 marks AudioBufferSourceNode::stop (the whole
    // overload family) #[deprecated] -- a WebIDL-overload generation artifact
    // with no replacement API; allow it explicitly.
    #![allow(dead_code, deprecated)]
    /// Preloads the SF2 from the VFS (name provided by init_pipeline via
    /// set_pending_sf2). Failure only degrades to "music silent", never
    /// panics (AGENTS constraint 2).
    fn preload_sound_font(&mut self) {
        let Some(name) = PENDING_SF2.with_borrow(|s| s.clone()) else {
            return;
        };
        let Some(bytes) = wasm_vfs::vfs_get(&name) else {
            log::warn!("SF2 '{name}' 未注册，音乐将静音");
            return;
        };
        match SynthFont::parse(&bytes) {
            Some(f) => self.font = Some(f),
            None => log::warn!("SF2 '{name}' 解析失败，音乐将静音"),
        }
    }

    /// pump: SFX slot cleanup + music block scheduling. Called by woom24_tick
    /// every frame (D4 pull model).
    fn pump_music(&mut self) {
        if self.paused.get() {
            return;
        }
        let now = self.ctx.current_time();
        // SFX slot cleanup: a source is idle once end_time is reached
        // (mirrors native !player.empty()).
        for slot in self.channels.iter_mut() {
            if let Some(ch) = slot {
                if now >= ch.end_time {
                    *slot = None;
                }
            }
        }
        // Music scheduling: keep 0.2s ahead on the timeline.
        while self.music.is_some() && self.music_next_time < now + 0.2 {
            let Some(buf) = self.render_block_buffer() else {
                // Sequence drained: settle the session (fix round 2). Without
                // clearing to None the engine session never ends -- scheduled
                // accumulates one finished node wrapper per block (tens of
                // thousands for long tracks) and is_music_playing stays true
                // forever, wedging the engine's session polling via i_sound.
                self.music = None;
                break;
            };
            let Ok(src) = self.ctx.create_buffer_source() else {
                break;
            };
            src.set_buffer(Some(&buf));
            let _ = src.connect_with_audio_node(&self.music_gain);
            let _ = src.start_with_when(self.music_next_time.max(now));
            self.scheduled.push(src);
            self.music_next_time += BLOCK_SIZE as f64 / 44100.0;
        }
        if self.music.is_none() && !self.scheduled.is_empty() {
            // Reached in the very frame the sequence drains (only truly
            // reachable after fix round 2): drop the queued node wrappers.
            // Tail blocks already start()ed on the browser side play out
            // normally (the audio graph holds nodes until finished); the Rust
            // side only drops wrappers, nothing leaks.
            self.scheduled.clear();
        }
    }

    /// Renders one BLOCK_SIZE-frame block (interleaved L/R) into a fresh
    /// AudioBuffer. Returns None when the sequence is already drained (not
    /// even the first sample); pads the tail with zeros on early end.
    fn render_block_buffer(&mut self) -> Option<web_sys::AudioBuffer> {
        let engine = self.music.as_mut()?;
        let first = engine.next_interleaved()?;
        let buf = self.ctx.create_buffer(2, BLOCK_SIZE as u32, 44100.0).ok()?;
        // get_channel_data returns a Vec copy (writing it throws the data
        // away) -- stage locally first, then write into the AudioBuffer with
        // copy_to_channel (fix round 1 ruling).
        let mut l = vec![0f32; BLOCK_SIZE];
        let mut r = vec![0f32; BLOCK_SIZE];
        // Consumption order: L0, R0, L1, R1, ..., R(BLOCK-1) -- exactly
        // 2*BLOCK samples.
        l[0] = first;
        for i in 0..BLOCK_SIZE {
            let Some(s) = engine.next_interleaved() else {
                break;
            };
            r[i] = s;
            if i + 1 < BLOCK_SIZE {
                let Some(s2) = engine.next_interleaved() else {
                    break;
                };
                l[i + 1] = s2;
            }
        }
        buf.copy_to_channel(&l, 0).ok()?;
        buf.copy_to_channel(&r, 1).ok()?;
        Some(buf)
    }

    /// Engine-driven SFX start (the body behind the trait's start_sound).
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        let Some((sample_rate, samples)) = decode_doom_sfx(data) else {
            return false;
        };
        let (Ok(buf), Ok(gain), Ok(panner)) = (
            self.ctx
                .create_buffer(1, samples.len() as u32, sample_rate as f32),
            self.ctx.create_gain(),
            self.ctx.create_stereo_panner(),
        ) else {
            return false;
        };
        // get_channel_data returns a Vec copy; writing it throws the data
        // away -- copy_to_channel is what actually writes the AudioBuffer
        // (fix round 1 ruling).
        if buf.copy_to_channel(&samples, 0).is_err() {
            return false;
        }
        gain.gain().set_value(vol_gain(vol));
        panner.pan().set_value(sep_to_pan(sep));
        let Ok(src) = self.ctx.create_buffer_source() else {
            return false;
        };
        src.set_buffer(Some(&buf));
        let _ = src.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&panner);
        let _ = panner.connect_with_audio_node(&self.ctx.destination());
        let _ = src.start();
        // End-time bookkeeping: pump_music clears the slot when due (stands in
        // for the onended callback, because AudioBufferSourceNode has no ended
        // polling property and the callback could not get &mut self).
        let end_time = self.ctx.current_time() + samples.len() as f64 / sample_rate as f64;
        self.channels[channel] = Some(SfxChannel {
            source: src,
            gain,
            panner,
            end_time,
        });
        true
    }

    /// Stops one logical channel (the body behind the trait's stop_sound).
    fn stop_sound(&mut self, channel: usize) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = self.channels[channel].take() {
            let _ = ch.source.stop();
        }
    }

    /// Updates channel volume/pan (the body behind the trait's
    /// update_sound_params).
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = &self.channels[channel] {
            ch.gain.gain().set_value(vol_gain(vol));
            ch.panner.pan().set_value(sep_to_pan(sep));
        }
    }

    /// Non-empty slot = source not yet at end_time (pump_music clears slots
    /// every frame). Semantics align with native `!player.empty()`; channels
    /// beyond 8 are always false.
    fn is_playing(&self, channel: usize) -> bool {
        channel < 8 && self.channels[channel].is_some()
    }

    /// Never called by the engine on wasm (the Path::exists() that
    /// find_soundfont_path relies on is always false); kept for trait
    /// completeness -- the font is preloaded from the VFS at construction.
    fn load_sound_font(&mut self, _path: &std::path::Path) {
        log::info!("load_sound_font 在 wasm 上为 no-op：字体已在构造时从 VFS 预载");
    }

    /// Starts MIDI playback (the body behind the trait's play_music).
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        let Some(font) = self.font.clone() else {
            log::warn!("无已预载 SF2，play_music 忽略");
            return;
        };
        match SynthEngine::new(&font, midi_bytes, looping) {
            Some(engine) => {
                for s in self.scheduled.drain(..) {
                    let _ = s.stop();
                }
                self.music = Some(engine);
                self.music_next_time = self.ctx.current_time() + 0.05;
            }
            None => log::warn!("I_PlaySong: SynthEngine 构建失败"),
        }
    }

    /// Stops music and drains scheduled sources (the body behind the trait's
    /// stop_music).
    fn stop_music(&mut self) {
        for s in self.scheduled.drain(..) {
            let _ = s.stop();
        }
        self.music = None;
    }

    /// Music bus volume (the body behind the trait's set_music_volume).
    fn set_music_volume(&self, vol: i32) {
        self.music_gain.gain().set_value(vol_gain(vol));
    }
}

// Wiring: init_pipeline's factory closure calls new(); woom24_tick pumps via
// pump_current_backend().
impl WebAudioBackend {
    /// Factory entry: returns Err when Web Audio is unavailable (room logs it
    /// and falls back to NoopBackend -- the spec's "degrade to silence"
    /// contract). On success a pump handle is left in LAST_BUILT: the returned
    /// body and the handle in LAST_BUILT are two Rcs over the same core (see
    /// BackendCore's note).
    pub fn new() -> Result<Self, String> {
        let ctx = web_sys::AudioContext::new().map_err(|e| format!("AudioContext: {e:?}"))?;
        let music_gain = ctx
            .create_gain()
            .map_err(|e| format!("create_gain: {e:?}"))?;
        music_gain
            .connect_with_audio_node(&ctx.destination())
            .map_err(|e| format!("connect: {e:?}"))?;
        let mut core = BackendCore {
            ctx,
            channels: std::array::from_fn(|_| None),
            music_gain,
            font: None,
            music: None,
            music_next_time: 0.0,
            scheduled: Vec::new(),
            paused: Cell::new(false),
        };
        core.preload_sound_font();
        let backend = Self {
            core: std::rc::Rc::new(RefCell::new(core)),
        };
        LAST_BUILT.with_borrow_mut(|slot| {
            *slot = Some(std::rc::Rc::new(RefCell::new(backend.clone())));
        });
        Ok(backend)
    }

    /// Pump entry: forwards to the shared core (SFX slot cleanup + music
    /// block scheduling).
    fn pump_music(&mut self) {
        self.core.borrow_mut().pump_music();
    }
}

impl AudioBackend for WebAudioBackend {
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        self.core.borrow_mut().start_sound(data, vol, sep, channel)
    }

    fn stop_sound(&mut self, channel: usize) {
        self.core.borrow_mut().stop_sound(channel);
    }

    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        self.core.borrow().update_sound_params(channel, vol, sep);
    }

    fn is_playing(&self, channel: usize) -> bool {
        self.core.borrow().is_playing(channel)
    }

    fn load_sound_font(&mut self, path: &std::path::Path) {
        self.core.borrow_mut().load_sound_font(path);
    }

    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        self.core.borrow_mut().play_music(midi_bytes, looping);
    }

    fn stop_music(&mut self) {
        self.core.borrow_mut().stop_music();
    }

    fn set_music_volume(&self, vol: i32) {
        self.core.borrow().set_music_volume(vol);
    }

    fn pause_music(&self) {
        let core = self.core.borrow();
        core.paused.set(true);
        let _ = core.ctx.suspend();
    }

    fn resume_music(&self) {
        let core = self.core.borrow();
        core.paused.set(false);
        let _ = core.ctx.resume();
    }

    fn is_music_playing(&self) -> bool {
        let core = self.core.borrow();
        core.music.is_some() || !core.scheduled.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vol_gain_matches_doom_scale() {
        assert_eq!(vol_gain(0), 0.0);
        assert!((vol_gain(127) - 1.0).abs() < 1e-6);
        assert!((vol_gain(64) - 64.0 / 127.0).abs() < 1e-6);
        assert_eq!(vol_gain(200), 1.0, "超范围钳制");
    }

    #[test]
    fn sep_to_pan_is_centred_at_128() {
        assert!(sep_to_pan(128).abs() < 1e-6);
        assert!(sep_to_pan(254) > 0.0, "全右为正");
        assert!(sep_to_pan(0) < 0.0, "全左为负");
        assert!(sep_to_pan(-5).abs() <= 1.0, "越界钳制后仍在 [-1,1]");
    }
}
