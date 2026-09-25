//! WebAudioBackend: the engine's AudioBackend control plane on the browser
//! (spec 2 D4).
//!
//! Key points:
//! - The SF2 is preloaded from the VFS at construction; the engine's
//!   load_sound_font call (fed by i_sound's wasm hint bridge, which replaces
//!   the always-false exists() probe -- spec 4 defect B) passes the same VFS
//!   name, so it short-circuits on the loaded-name check instead of parsing
//!   the identical bytes a second time (a different name still resolves
//!   through resolve_sound_font).
//! - Music uses "chunked pull": woom24_tick calls pump_music() every frame,
//!   scheduling 0.2s ahead.
//! - SFX: one source+gain+panner pair per logical channel; bookkeeping clears
//!   the slot at end_time = is_playing.

use std::cell::{Cell, RefCell};

use room::audio::{decode_doom_sfx, AudioBackend, SynthEngine, SynthFont, BLOCK_SIZE};

use crate::wasm_vfs;

/// Doom volume (0..=127) → linear gain.
// Used by the trait method bodies and the unit tests.
fn vol_gain(vol: i32) -> f32 { vol.clamp(0, 127) as f32 / 127.0 }

/// F9 audio-health counters (spec F9 §4). Pure counts with no web-sys
/// reach, so the movement logic is unit-testable on host; `BackendCore`
/// embeds one and the pump / schedule / session-retirement sites drive it.
///
/// Semantics (documented approximations):
/// - `underruns`: one increment per pump call that finds the music timeline
///   behind the AudioContext clock while a session is active -- the queue
///   ran dry before this pump refilled it. Not counted per block, so a
///   catch-up burst after a stall adds a single event.
/// - `scheduled_seconds`: cumulative seconds handed to the audio graph by
///   the current music session (sum of `BLOCK_SIZE / 44100` at schedule
///   time). This is NOT instantaneous queue depth: consumption is not
///   observable (`AudioBufferSourceNode` has no ended polling handle and
///   the onended callback cannot reach `&mut self`), and a wall-clock decay
///   was deliberately avoided to keep the counter deterministic under the
///   harness fake clock. The accumulator resets to 0 at every session
///   boundary (successful `play_music`, drain settle, `stop_music`), which
///   is the sanctioned approximation.
struct AudioCounters
{
    underruns: Cell<u32>,
    scheduled_seconds: Cell<f64>,
}

impl AudioCounters
{
    fn new() -> Self { Self { underruns: Cell::new(0), scheduled_seconds: Cell::new(0.0), } }

    fn record_underrun(&self) { self.underruns.set(self.underruns.get().wrapping_add(1)); }

    fn add_scheduled_seconds(&self, seconds: f64) { self.scheduled_seconds.set(self.scheduled_seconds.get() + seconds); }

    /// Session-retirement decay: the retired session's scheduled volume is
    /// no longer attributable to any live session.
    fn reset_scheduled_seconds(&self) { self.scheduled_seconds.set(0.0); }
}

/// The audio-stats JSON contract (spec F9 §4): one line, no serde -- field
/// order and shape are the runner's (`assert_stats`) cross-target contract.
/// `scheduled_seconds` renders via Rust's f64 Display (JSON-valid for every
/// finite value these sums can produce); bools render as `true`/`false`.
/// Compiled for the wasm harness build and the host unit tests only, so a
/// production non-harness build carries no dead diagnostics surface.
#[cfg(any(feature = "harness", test))]
fn audio_stats_json_impl(underruns: u32, scheduled_seconds: f64, music_active: bool, voices: u32) -> String
{
    format!(
        "{{\"underruns\":{},\"scheduled_seconds\":{},\"music_active\":{},\"voices\":{}}}",
        underruns,
        scheduled_seconds,
        music_active,
        voices
    )
}

/// sep (0..=255, centered at 128) → StereoPannerNode.pan in [-1, 1].
/// Reuses the engine's gains_from (left/right gains), folded into a single
/// pan value.
/// Note: the engine's NORM_SEP centers at 128 (0..=255, see s_sound.rs)
/// while gains_from centers at 127 (0..=254, the lib's own convention) --
/// both curves are linear and coincide point for point after shifting the
/// center by 1, hence (sep - 1) is fed in.
fn sep_to_pan(sep: i32) -> f32
{
    let (l, r) = room::audio::gains_from(127, (sep - 1).clamp(0, 254));
    let sum = l + r;
    if sum <= f32::EPSILON { 0.0 }
    else { (r - l) / sum }
}

thread_local!
{
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
pub fn set_pending_sf2(name: Option<String>) { PENDING_SF2.with_borrow_mut(|s| *s = name); }

/// The pending SF2 name, as the engine sees it through
/// `AudioBackend::soundfont_hint`. Free function so host tests can exercise
/// the hint logic without a DOM AudioContext (the backend cannot be
/// constructed there).
fn pending_sf2_hint() -> Option<String> { PENDING_SF2.with_borrow(|s| s.clone()) }

/// Resolves an SF2 name against the VFS and parses it. The single lookup
/// path for fonts: the construction-time preload (keyed on the pending
/// hint) and load_sound_font (keyed on the engine-provided path, which on
/// wasm IS the hint name) both resolve through here.
fn resolve_sound_font(name: &str) -> Result<SynthFont, String>
{
    let Some(bytes) = wasm_vfs::vfs_get(name) else
    {
        return Err(format!("'{name}' not registered"));
    };
    SynthFont::parse(&bytes).ok_or_else(|| format!("'{name}' failed to parse"))
}

/// Music pump called by woom24_tick every frame (the engine's AUDIO cell is
/// out of the shell's reach, so the pump goes through the handle kept here;
/// the two holders each mind their own role and share no mutable state).
pub fn pump_current_backend()
{
    LAST_BUILT.with_borrow(
        |slot|
        { if let Some(b) = slot.as_ref() { b.borrow_mut().pump_music(); } }
    );
}

/// Active node group for one SFX logical channel.
/// `end_time` = the AudioContext time at which the source finishes
/// (AudioBufferSourceNode has no ended polling property; time bookkeeping
/// stands in for the onended callback, which has no global handle).
#[allow(dead_code)]
struct SfxChannel
{
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
///
/// State is only ever reached through WebAudioBackend handles.
struct BackendCore
{
    ctx: web_sys::AudioContext,
    /// 8 logical channels (matching the engine's I_UpdateSoundParams 0..8).
    channels: [Option<SfxChannel>; 8],
    music_gain: web_sys::GainNode,
    /// Preloaded font (read from the VFS at construction; Clone = Arc clone,
    /// ready on demand).
    font: Option<SynthFont>,
    /// VFS name the loaded `font` was parsed from (loaded-name short-circuit:
    /// the engine's boot-time load_sound_font passes the same hint name the
    /// construction preload resolved, so without this it would re-parse the
    /// identical bytes; a different name still parses).
    font_name: Option<String>,
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
    /// F9 audio-health counters (spec F9 §4), driven at the pump / schedule
    /// / session-retirement sites below.
    counters: AudioCounters,
}

/// The engine AudioBackend's wasm implementation: an Rc handle sharing the
/// core (Clone = refcount +1).
#[derive(Clone)]
pub struct WebAudioBackend { core: std::rc::Rc<RefCell<BackendCore>>, }

impl BackendCore
{
    // deprecated: web-sys 0.3.98 marks AudioBufferSourceNode::stop (the whole
    // overload family) #[deprecated] -- a WebIDL-overload generation artifact
    // with no replacement API; allow it explicitly.
    #![allow(dead_code, deprecated)]
    /// Preloads the SF2 from the VFS (name provided by init_pipeline via
    /// set_pending_sf2). Failure only degrades to "music silent", never
    /// panics (AGENTS constraint 2).
    fn preload_sound_font(&mut self)
    {
        let Some(name) = pending_sf2_hint() else { return; };
        match resolve_sound_font(&name)
        {
            Ok(f) =>
            {
                self.font = Some(f);
                self.font_name = Some(name);
            }
            Err(e) => log::warn!("SF2 {e}; music will be silent"),
        }
    }

    /// pump: SFX slot cleanup + music block scheduling. Called by woom24_tick
    /// every frame (D4 pull model).
    fn pump_music(&mut self)
    {
        if self.paused.get() { return; }
        let now = self.ctx.current_time();
        // SFX slot cleanup: a source is idle once end_time is reached
        // (mirrors native !player.empty()).
        for slot in self.channels.iter_mut() { if let Some(ch) = slot { if now >= ch.end_time { *slot = None; } } }
        // Underrun site (F9 §4): the music timeline has already passed the
        // AudioContext clock while a session is active -- the consumer ran
        // dry before this pump refilled it. Counted once per pump call, not
        // per block, so a catch-up burst after a stall adds one event.
        if self.music.is_some() && self.music_next_time < now
        {
            self.counters.record_underrun();
        }
        // Music scheduling: keep 0.2s ahead on the timeline.
        while self.music.is_some() && self.music_next_time < now + 0.2
        {
            let Some(buf) = self.render_block_buffer() else
            {
                // Sequence drained: settle the session (fix round 2). Without
                // clearing to None the engine session never ends -- scheduled
                // accumulates one finished node wrapper per block (tens of
                // thousands for long tracks) and is_music_playing stays true
                // forever, wedging the engine's session polling via i_sound.
                self.music = None;
                // Session retirement (drain settle): decay the scheduled
                // volume with the session (AudioCounters semantics).
                self.counters.reset_scheduled_seconds();
                break;
            };
            let Ok(src) = self.ctx.create_buffer_source() else { break; };
            src.set_buffer(Some(&buf));
            let _ = src.connect_with_audio_node(&self.music_gain);
            let _ = src.start_with_when(self.music_next_time.max(now));
            self.scheduled.push(src);
            self.counters.add_scheduled_seconds(BLOCK_SIZE as f64 / 44100.0);
            self.music_next_time += BLOCK_SIZE as f64 / 44100.0;
        }
        if self.music.is_none() && !self.scheduled.is_empty()
        {
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
    fn render_block_buffer(&mut self) -> Option<web_sys::AudioBuffer>
    {
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
        for i in 0..BLOCK_SIZE
        {
            let Some(s) = engine.next_interleaved() else { break; };
            r[i] = s;
            if i + 1 < BLOCK_SIZE
            {
                let Some(s2) = engine.next_interleaved() else { break; };
                l[i + 1] = s2;
            }
        }
        buf.copy_to_channel(&l, 0).ok()?;
        buf.copy_to_channel(&r, 1).ok()?;
        Some(buf)
    }

    /// Engine-driven SFX start (the body behind the trait's start_sound).
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool
    {
        if channel >= 8 { return false; }
        let Some((sample_rate, samples)) = decode_doom_sfx(data) else { return false; };
        let (Ok(buf), Ok(gain), Ok(panner)) = (
            self.ctx.create_buffer(1, samples.len() as u32, sample_rate as f32),
            self.ctx.create_gain(),
            self.ctx.create_stereo_panner(),
        )
        else { return false; };
        // get_channel_data returns a Vec copy; writing it throws the data
        // away -- copy_to_channel is what actually writes the AudioBuffer
        // (fix round 1 ruling).
        if buf.copy_to_channel(&samples, 0).is_err() { return false; }
        gain.gain().set_value(vol_gain(vol));
        panner.pan().set_value(sep_to_pan(sep));
        let Ok(src) = self.ctx.create_buffer_source() else { return false; };
        src.set_buffer(Some(&buf));
        let _ = src.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&panner);
        let _ = panner.connect_with_audio_node(&self.ctx.destination());
        let _ = src.start();
        // End-time bookkeeping: pump_music clears the slot when due (stands in
        // for the onended callback, because AudioBufferSourceNode has no ended
        // polling property and the callback could not get &mut self).
        let end_time = self.ctx.current_time() + samples.len() as f64 / sample_rate as f64;
        self.channels[channel] = Some(
            SfxChannel
            {
                source: src,
                gain,
                panner,
                end_time,
            }
        );
        true
    }

    /// Stops one logical channel (the body behind the trait's stop_sound).
    fn stop_sound(&mut self, channel: usize)
    {
        if channel >= 8 { return; }
        if let Some(ch) = self.channels[channel].take() { let _ = ch.source.stop(); }
    }

    /// Updates channel volume/pan (the body behind the trait's
    /// update_sound_params).
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32)
    {
        if channel >= 8 { return; }
        if let Some(ch) = &self.channels[channel]
        {
            ch.gain.gain().set_value(vol_gain(vol));
            ch.panner.pan().set_value(sep_to_pan(sep));
        }
    }

    /// Non-empty slot = source not yet at end_time (pump_music clears slots
    /// every frame). Semantics align with native `!player.empty()`; channels
    /// beyond 8 are always false.
    fn is_playing(&self, channel: usize) -> bool { channel < 8 && self.channels[channel].is_some() }

    /// Engine-driven font load (spec 4 defect B fix): resolves the engine's
    /// path against the VFS. On wasm the engine passes the backend hint
    /// (i_sound's wasm branch of find_soundfont_path), i.e. the same VFS
    /// name the construction-time preload keys on -- that name is already
    /// loaded, so the identical bytes are not parsed a second time; a
    /// different name still resolves and parses here. Failure only degrades
    /// to "music silent", never panics (AGENTS constraint 2).
    fn load_sound_font(&mut self, path: &std::path::Path)
    {
        let name = path.display().to_string();
        if self.font_name.as_deref() == Some(name.as_str())
        {
            log::debug!("SF2 '{name}' already loaded; skipping re-parse");
            return;
        }
        match resolve_sound_font(&name)
        {
            Ok(f) =>
            {
                self.font = Some(f);
                self.font_name = Some(name);
            }
            Err(e) => log::warn!("SF2 {e}; music will be silent"),
        }
    }

    /// Starts MIDI playback (the body behind the trait's play_music).
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool)
    {
        let Some(font) = self.font.clone() else
        {
            log::warn!("no preloaded SF2, ignoring play_music");
            return;
        };
        match SynthEngine::new(&font, midi_bytes, looping)
        {
            Some(engine) =>
            {
                for s in self.scheduled.drain(..) { let _ = s.stop(); }
                // Session replacement: the replaced session's scheduled
                // volume decays with it; the new session starts from zero.
                self.counters.reset_scheduled_seconds();
                self.music = Some(engine);
                self.music_next_time = self.ctx.current_time() + 0.05;
            }
            None => log::warn!("I_PlaySong: SynthEngine build failed"),
        }
    }

    /// Stops music and drains scheduled sources (the body behind the trait's
    /// stop_music).
    fn stop_music(&mut self)
    {
        for s in self.scheduled.drain(..) { let _ = s.stop(); }
        // Session retirement: decay the scheduled volume with the session.
        self.counters.reset_scheduled_seconds();
        self.music = None;
    }

    /// Music bus volume (the body behind the trait's set_music_volume).
    fn set_music_volume(&self, vol: i32) { self.music_gain.gain().set_value(vol_gain(vol)); }
}

// Wiring: init_pipeline's factory closure calls new(); woom24_tick pumps via
// pump_current_backend().
impl WebAudioBackend
{
    /// Factory entry: returns Err when Web Audio is unavailable (room logs it
    /// and falls back to NoopBackend -- the spec's "degrade to silence"
    /// contract). On success a pump handle is left in LAST_BUILT: the returned
    /// body and the handle in LAST_BUILT are two Rcs over the same core (see
    /// BackendCore's note).
    pub fn new() -> Result<Self, String>
    {
        let ctx = web_sys::AudioContext::new().map_err(|e| format!("AudioContext: {e:?}"))?;
        let music_gain = ctx.
            create_gain().
            map_err(|e| format!("create_gain: {e:?}"))?;
        music_gain.
            connect_with_audio_node(&ctx.destination()).
            map_err(|e| format!("connect: {e:?}"))?;
        let mut core = BackendCore
        {
            ctx,
            channels: std::array::from_fn(|_| None),
            music_gain,
            font: None,
            font_name: None,
            music: None,
            music_next_time: 0.0,
            scheduled: Vec::new(),
            paused: Cell::new(false),
            counters: AudioCounters::new(),
        };
        core.preload_sound_font();
        let backend = Self { core: std::rc::Rc::new(RefCell::new(core)), };
        LAST_BUILT.with_borrow_mut(|slot| { *slot = Some(std::rc::Rc::new(RefCell::new(backend.clone()))); });
        Ok(backend)
    }

    /// Pump entry: forwards to the shared core (SFX slot cleanup + music
    /// block scheduling).
    fn pump_music(&mut self) { self.core.borrow_mut().pump_music(); }
}

/// F9 audio-health snapshot (spec F9 §4), consumed by the harness export
/// `harness_audio_stats`. Harness-only surface: a production wasm build
/// (feature off) compiles none of this. Reads the last successfully built
/// backend's counters; when no backend was ever built (Node harness, host,
/// factory failure) the all-zero shape is reported -- by design: the
/// counters only exist while the WebAudio backend lives, and "no audio
/// subsystem" is indistinguishable from "silent session" at this contract's
/// granularity. `music_active` mirrors `is_music_playing`'s session
/// lifecycle exactly (live engine session OR blocks still queued on the
/// audio graph); `voices` is the live SFX slot occupancy (the backend does
/// not track MIDI polyphony -- SynthEngine exposes no voice count), so it
/// reads 0 whenever no SFX source is inside its end-time window.
#[cfg(feature = "harness")]
pub fn audio_stats_json() -> String
{
    LAST_BUILT.with_borrow(
        |slot| match slot.as_ref()
        {
            Some(b) =>
            {
                // LAST_BUILT holds the cheap Rc handle (WebAudioBackend);
                // the counters live on the shared BackendCore body. Two
                // bindings: the handle borrow must outlive the core borrow.
                let handle = b.borrow();
                let core = handle.core.borrow();
                audio_stats_json_impl(
                    core.counters.underruns.get(),
                    core.counters.scheduled_seconds.get(),
                    core.music.is_some() || !core.scheduled.is_empty(),
                    core.channels.iter().filter(|s| s.is_some()).count() as u32,
                )
            }
            None => audio_stats_json_impl(0, 0.0, false, 0),
        }
    )
}

impl AudioBackend for WebAudioBackend
{
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool { self.core.borrow_mut().start_sound(data, vol, sep, channel) }

    fn stop_sound(&mut self, channel: usize) { self.core.borrow_mut().stop_sound(channel); }

    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) { self.core.borrow().update_sound_params(channel, vol, sep); }

    fn is_playing(&self, channel: usize) -> bool { self.core.borrow().is_playing(channel) }

    fn load_sound_font(&mut self, path: &std::path::Path) { self.core.borrow_mut().load_sound_font(path); }

    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) { self.core.borrow_mut().play_music(midi_bytes, looping); }

    fn stop_music(&mut self) { self.core.borrow_mut().stop_music(); }

    fn set_music_volume(&self, vol: i32) { self.core.borrow().set_music_volume(vol); }

    fn pause_music(&self)
    {
        let core = self.core.borrow();
        core.paused.set(true);
        let _ = core.ctx.suspend();
    }

    fn resume_music(&self)
    {
        let core = self.core.borrow();
        core.paused.set(false);
        let _ = core.ctx.resume();
    }

    fn is_music_playing(&self) -> bool
    {
        let core = self.core.borrow();
        core.music.is_some() || !core.scheduled.is_empty()
    }

    fn soundfont_hint(&self) -> Option<String> { pending_sf2_hint() }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn vol_gain_matches_doom_scale()
    {
        assert_eq!(vol_gain(0), 0.0);
        assert!((vol_gain(127) - 1.0).abs() < 1e-6);
        assert!((vol_gain(64) - 64.0 / 127.0).abs() < 1e-6);
        assert_eq!(vol_gain(200), 1.0, "out-of-range value clamped");
    }

    #[test]
    fn sep_to_pan_is_centred_at_128()
    {
        assert!(sep_to_pan(128).abs() < 1e-6);
        assert!(sep_to_pan(254) > 0.0, "full right is positive");
        assert!(sep_to_pan(0) < 0.0, "full left is negative");
        assert!(sep_to_pan(-5).abs() <= 1.0, "still within [-1,1] after clamping an out-of-range value");
    }

    // The hint seam is tested through the free function, not through a
    // constructed backend: WebAudioBackend::new() needs a DOM AudioContext,
    // which host tests cannot provide (same constraint as the other tests
    // in this module). PENDING_SF2 is thread-local state, no DOM involved.
    #[test]
    fn soundfont_hint_returns_pending_sf2()
    {
        set_pending_sf2(Some("uzdoom.sf2".to_string()));
        assert_eq!(pending_sf2_hint().as_deref(), Some("uzdoom.sf2"));
        set_pending_sf2(None);
        assert_eq!(pending_sf2_hint(), None, "cleared pending must not hint");
    }

    // ---- F9 audio-health counters (pure logic; BackendCore needs a real
    // AudioContext, so the "fake core" here is the AudioCounters the pump
    // sites drive) ----

    #[test]
    fn counters_move_with_pump_sites()
    {
        let c = AudioCounters::new();
        assert_eq!(c.underruns.get(), 0, "fresh counters start clean");
        // Underrun site: one record per pump call that finds the queue dry.
        c.record_underrun();
        c.record_underrun();
        assert_eq!(c.underruns.get(), 2);
        // Schedule site: each block adds its duration (BLOCK_SIZE / 44100).
        c.add_scheduled_seconds(BLOCK_SIZE as f64 / 44100.0);
        c.add_scheduled_seconds(BLOCK_SIZE as f64 / 44100.0);
        assert!((c.scheduled_seconds.get() - 2.0 * BLOCK_SIZE as f64 / 44100.0).abs() < 1e-12);
        // Retirement decay: a session boundary zeroes the accumulator, never
        // the underrun count (underruns are lifetime health).
        c.reset_scheduled_seconds();
        assert_eq!(c.scheduled_seconds.get(), 0.0);
        assert_eq!(c.underruns.get(), 2);
    }

    #[test]
    fn underrun_counter_wraps_not_panics()
    {
        let c = AudioCounters::new();
        c.underruns.set(u32::MAX);
        c.record_underrun();
        assert_eq!(c.underruns.get(), 0, "wrapping_add keeps the pump panic-free");
    }

    #[test]
    fn stats_json_contract_shape()
    {
        // The exact JSON shape the runner's assert_stats parses: field order
        // fixed, bools unquoted, numbers bare (JSON.parseable without serde
        // -- this crate adds no dependencies). String-checked on the seams
        // that matter: keys, separators, bool literal, empty f64 rendering.
        let s = audio_stats_json_impl(3, 0.0, false, 1);
        assert_eq!(s, "{\"underruns\":3,\"scheduled_seconds\":0,\"music_active\":false,\"voices\":1}");
        assert!(audio_stats_json_impl(0, 0.0, true, 0).contains("\"music_active\":true"));
        // Fractional f64 renders as a JSON number.
        let s = audio_stats_json_impl(0, 0.5, false, 0);
        assert!(s.contains("\"scheduled_seconds\":0.5,"), "unexpected: {s}");
        // All-zero shape: the no-backend contract the harness reports.
        assert_eq!(audio_stats_json_impl(0, 0.0, false, 0), "{\"underruns\":0,\"scheduled_seconds\":0,\"music_active\":false,\"voices\":0}");

    }
}
