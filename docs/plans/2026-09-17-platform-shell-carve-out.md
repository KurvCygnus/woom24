# Platform Shell Carve-Out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the engine library (`room`) from the platform layer (`shells/native`) so the lib type-checks for `wasm32-unknown-unknown` while native behavior stays byte-identical.

**Architecture:** Keep the `DG_*` C ABI as the engine↔platform seam (approach A of `docs/specs/2026-09-17-platform-shell-design.md`). Move the binary side (winit/wgpu platform) into a new `shells/native` workspace package. Replace the lib's concrete rodio audio state with a control-plane `AudioBackend` trait + backend factory; move the rodio mixer graph into the native shell.

**Tech Stack:** Rust 2021 workspace, winit 0.30, wgpu 29, rodio 0.22, rustysynth 1, cargo.

**Spec:** `docs/specs/2026-09-17-platform-shell-design.md` (approved 2026-09-17)

## Global Constraints

- Engine modules under `room/src/doom/` must not change except the audio call-site deref in `i_sound.rs` (spec Goals 3).
- `cargo test` must end every task green: baseline is 372 unit tests + 2 `demo_playthrough` tests (374 unit after Task 2 adds the NoopBackend tests).
- Simulation/render/audio must remain byte-identical on native: moved code, not rewritten code.
- Comments in touched files follow the file-local convention (English, `///` doc comments); new woom24-authored code defaults to Chinese comments with half-width ASCII punctuation (AGENTS.md).
- Never commit WAD/soundfont files; `soundfonts/` stays as-is.
- Every potentially long-running command carries a bounded timeout; never run two builds concurrently (AGENTS.md Tooling).
- Conventional Commits, English, no CI-skip markers.
- Zero clippy warnings for **new** code; pre-existing warnings (e.g. `static_mut_refs` in tests) are upstream debt and stay untouched.

---

### Task 1: Create `shells/native` and move the binary platform

**Files:**
- Create: `shells/native/Cargo.toml`
- Move: `room/src/main.rs` → `shells/native/src/main.rs`
- Move: `room/src/gpu.rs` → `shells/native/src/gpu.rs`
- Move: `room/src/platform/mod.rs` → `shells/native/src/platform.rs`
- Move: `room/src/platform/keys.rs` → `shells/native/src/platform/keys.rs`
- Modify: `Cargo.toml` (workspace root)
- Modify: `room/Cargo.toml`
- Delete: `room/src/platform/mod.rs` (moved), empty `room/src/platform/` dir

**Interfaces:**
- Consumes: `room` lib as an external dependency (`room::doom::doomgeneric`, `room::doom::d_main`).
- Produces: workspace package `room-shell-native` with bin `room`; the six `DG_*` symbols now exported from this bin; `room` package without a bin.

- [ ] **Step 1: Create the package directory and move files**

```bash
mkdir -p shells/native/src
git mv room/src/main.rs shells/native/src/main.rs
git mv room/src/gpu.rs shells/native/src/gpu.rs
git mv room/src/platform/mod.rs shells/native/src/platform.rs
git mv room/src/platform/keys.rs shells/native/src/platform/keys.rs
```

Module resolution note: `platform.rs` + `platform/keys.rs` is exactly the file layout that `pub mod keys;` inside `platform.rs` expects, so `main.rs`'s `mod gpu; mod platform;` and `use platform::{GPU, KEY_QUEUE, QUIT_REQUESTED, WINDOW};` keep working unmodified.

- [ ] **Step 2: Write `shells/native/Cargo.toml`**

```toml
[package]
name = "room-shell-native"
version = "0.1.0"
edition = "2021"
description = "Native winit/wgpu/rodio platform shell for the room engine"
repository = "https://github.com/KurvCygnus/woom24"
license = "GPL-2.0"

[[bin]]
name = "room"
path = "src/main.rs"

[features]
dhat-heap = ["dep:dhat"]

[dependencies]
room = { path = "../../room" }
winit = "0.30"
wgpu = { version = "29", features = ["wgsl"] }
pollster = "0.4"
rodio = "0.22"
env_logger = "0.11"
log = "0.4"
dhat = { version = "0.3", optional = true }
```

(`rodio` is listed now because Task 3 moves the audio backend here; it is unused in this task — remove the `#[warn(unused)]` noise by leaving it, an unused dependency does not warn.)

- [ ] **Step 3: Edit `room/Cargo.toml`**

Remove the whole `[[bin]]` table, and remove these dependency lines (they are bin-only):
`winit`, `wgpu`, `pollster`, `env_logger`, `dhat` optional dep, and the `dhat-heap` feature entry.
Keep: `bytemuck`, `log`, `libc`, `rustysynth`, `rodio` (still used by the lib audio module until Task 3),
`[dev-dependencies] doomgeneric-sys`, and the `[lib]` table.

- [ ] **Step 4: Edit workspace root `Cargo.toml`**

```toml
[workspace]
default-members = ["doomgeneric-sys", "room", "shells/native"]
members = [
    "c2rust-intermediate",
    "doomgeneric-sys",
    "room",
    "shells/native",
]
resolver = "2"
```

- [ ] **Step 5: Fix `dhat` references in `shells/native/src/main.rs`**

`main.rs` lines 42-45 and 262-263 use `#[cfg(feature = "dhat-heap")]` — the feature now lives in
`room-shell-native` (Step 2), and `dhat::Alloc`/`dhat::Profiler` resolve through its own optional
dependency. No source edit needed; verify by building with and without `--features dhat-heap`.

- [ ] **Step 6: Build and test**

```bash
cargo build
cargo test
```

Expected: build succeeds; 372 unit + 2 `demo_playthrough` pass. If `room` lib fails with
`unresolved import crate::gpu` style errors, check no lib file imported the moved modules
(`room/src/lib.rs` never declared `gpu`/`platform` — they were bin-only).

- [ ] **Step 7: Smoke-run the binary**

```bash
cargo run -p room-shell-native --bin room -- -iwad doom1.wad
```

Expected: window opens at 640×400, title `room`, demo plays. Close window to exit.
(The shell accepts any argv; `-iwad doom1.wad` suffices from the repo root.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(shell): move winit/wgpu platform layer to shells/native

The room package becomes a pure engine library; the binary platform
(winit window, wgpu presenter, DG_* callbacks, key mapping) moves to a
new room-shell-native package. Behavior is unchanged: this is a file
move plus Cargo wiring (room drops its bin and winit/wgpu/pollster/
env_logger deps)."
```

---

### Task 2: `AudioBackend` trait, factory, and engine call-site switch

**Files:**
- Modify: `room/src/audio/mod.rs`
- Modify: `room/src/doom/i_sound.rs` (I_InitSound + ~7 `a.music.X()` sites)
- Test: `room/src/audio/mod.rs` (add `#[cfg(test)] mod tests` inside)

**Interfaces:**
- Consumes: `AudioState` (existing, temporarily named as-is; renamed `RodioBackend` in Task 3).
- Produces (Task 3 depends on these exact items):
  - `pub trait AudioBackend` with the 12 methods listed in the spec;
  - `pub fn set_backend_factory(factory: fn() -> Result<Box<dyn AudioBackend>, String>)`;
  - `pub(crate) fn create_backend() -> Option<Box<dyn AudioBackend>>`;
  - `pub struct NoopBackend` implementing `AudioBackend`;
  - `AUDIO: RefCell<Option<Box<dyn AudioBackend>>>` (visibility unchanged: `pub(crate)`).

- [ ] **Step 1: Add the trait, factory, and NoopBackend to `room/src/audio/mod.rs`**

Insert after the existing `use` items (top of file), replacing nothing yet:

```rust
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
```

Add `use std::cell::Cell;` to the imports. Then change the `AUDIO` cell type:

```rust
pub(crate) static AUDIO: RefCell<Option<Box<dyn AudioBackend>>> = const { RefCell::new(None) };
```

- [ ] **Step 2: Implement the trait for `AudioState` (in place, temporary)**

Append to `room/src/audio/mod.rs` (Task 3 moves this impl into `shells/native`):

```rust
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
```

- [ ] **Step 3: Switch `I_InitSound` to the factory (`room/src/doom/i_sound.rs:105-119`)**

Replace the body of the `AUDIO.with_borrow_mut` closure:

```rust
#[no_mangle]
pub extern "C" fn I_InitSound(_use_sfx_prefix: Boolean) {
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
```

(The `Box<dyn AudioBackend>` error-string change: construction failures are logged inside
`create_backend`; the engine only reports the silent fallback.)

- [ ] **Step 4: Rewrite the music call sites to trait methods (same file)**

Each `a.music.X(...)` becomes `a.X(...)` — the sites are `I_InitMusic` (`a.music.load_sound_font(&path)`),
`I_ShutdownMusic` (`a.music.stop()`), `I_SetMusicVolume` (`a.music.set_volume(volume)`),
`I_PauseSong` (`a.music.pause()`), the `I_ResumeSong` site (`a.music.resume()`), and the two sites
in `I_PlaySong`/`I_SoundIsMusicPlaying`-equivalent functions further down
(`a.music.play(midi, looping)` → `a.play_music(midi, looping)`, `a.music.is_playing()` →
`a.is_music_playing()`). Run `grep -n "a\.music\." room/src/doom/i_sound.rs` after editing;
expected output: empty.

- [ ] **Step 5: Verify no other audio-path breakage**

```bash
grep -rn "AudioState" room/src/doom/ | grep -v i_sound
grep -n "a\.music\." room/src/doom/i_sound.rs
cargo build
```

Expected: first grep empty (engine only touches audio through `AUDIO`), second grep empty, build OK.

- [ ] **Step 6: Add NoopBackend unit tests (in `room/src/audio/mod.rs`)**

```rust
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
```

Note: `create_backend` reads a thread-local — the test thread has no factory, so this holds
regardless of other tests. If `BACKEND_FACTORY` were process-global this test would be flaky;
the `thread_local!` in Step 1 is what makes it sound.

- [ ] **Step 7: Run the test suite**

```bash
cargo test
```

Expected: 374 unit tests (372 + 2 new) + 2 `demo_playthrough` pass. `demo_playthrough` now runs
silent (no factory installed) instead of opening the real audio device — strictly less
environment-dependent, and simulation is unaffected (audio is out-of-band).

- [ ] **Step 8: Commit**

```bash
git add room/src/audio/mod.rs room/src/doom/i_sound.rs
git commit -m "refactor(audio): introduce AudioBackend control-plane trait

The engine now drives audio through a Box<dyn AudioBackend> behind the
existing AUDIO thread-local; backends are constructed via a factory
installed by the platform shell. NoopBackend formalises the silent
path. Native rodio wiring still lives in the lib at this point."
```

---

### Task 3: Move the rodio backend to `shells/native`, drop rodio from the lib

**Files:**
- Create: `shells/native/src/rodio_backend.rs`
- Create: `shells/native/src/audio_music.rs` (music backend parts of `room/src/audio/music.rs`)
- Modify: `room/src/audio/mod.rs` (remove `AudioState` + `impl AudioBackend`; expose pure items)
- Modify: `room/src/audio/sfx.rs` (keep pure parts; move `PannedSource`/`ChannelState` out)
- Modify: `room/src/audio/music.rs` (keep `mus2midi` + sample-rate constant; move the rest)
- Modify: `room/Cargo.toml` (remove `rodio`)
- Modify: `shells/native/Cargo.toml` (add `rustysynth = "1"`)
- Modify: `shells/native/src/main.rs` (install the factory)
- Delete: nothing else

**Interfaces:**
- Consumes: `room::audio::{AudioBackend, set_backend_factory}` (Task 2), `room::audio::{decode_doom_sfx, gains_from, PanState, mus2midi, SAMPLE_RATE}` (made `pub` in this task).
- Produces: `RodioBackend::new() -> Result<RodioBackend, Box<dyn std::error::Error>>` in the native shell; `room` lib with no rodio dependency.

- [ ] **Step 1: Open visibilities in the lib's pure audio pieces**

In `room/src/audio/sfx.rs`: `pub fn decode_doom_sfx`, `pub fn gains_from`, `pub struct PanState`
(+ `pub fn new/update`, `pub(crate) fn left_gain/right_gain` → `pub`), `pub struct PannedSource`
moves out (Step 2).
In `room/src/audio/music.rs`: keep `pub fn mus2midi` and `pub const SAMPLE_RATE: i32 = 44100;`
(lift the `const` visibility when moving it to the lib side); everything else moves out (Step 2).
Also keep the pure MUS/MIDI helper consts (`MUS_MAGIC`, `MIDI_TEMPO`, `MIDI_PPQ`, `BLOCK_SIZE`)
next to whichever side uses them after the move.
In `room/src/audio/mod.rs`: change `pub(crate) mod music; pub(crate) mod sfx;` to `pub mod sfx;`
and `pub(crate) use sfx::{decode_doom_sfx, ...}` → `pub use sfx::{decode_doom_sfx, gains_from, PanState};`,
`pub use music::mus2midi;`. The module `audio` itself becomes `pub mod audio;` in `room/src/lib.rs`.

- [ ] **Step 2: Create `shells/native/src/rodio_backend.rs`**

`git mv`-style port of the rodio-coupled code — move these items verbatim (only `crate::`/visibility
fixes listed):

From `room/src/audio/sfx.rs`: `PannedSource` (struct + `new` + `Iterator` impl + `rodio::Source`
impl) and `ChannelState` (struct + impl). Imports: `use room::audio::{decode_doom_sfx, gains_from, PanState};`
replacing the `crate::audio::` paths; `use rodio::{Player, Source, SamplesBuffer};`.

From `room/src/audio/music.rs`: `MusicSource` (struct + render core + `rodio::Source` impl),
`MusicState` (struct + all methods). One signature change: `MusicState::play` drops the
`mixer: &rodio::mixer::Mixer` parameter — `RodioBackend` owns the `MixerDeviceSink`/`Mixer` and
passes its own handle (this is the spec's "the backend owns its mixer"). Imports:
`use room::audio::mus2midi;` and `use room::audio::SAMPLE_RATE;` (or the const's new path).

Then add the backend struct (this is the former `AudioState`, renamed, with the mixer parameter
inlined):

```rust
pub struct RodioBackend {
    _device_sink: rodio::MixerDeviceSink,
    channels: Box<[ChannelState; 8]>,
    music: MusicState,
    mixer: rodio::mixer::Mixer,
}

impl RodioBackend {
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

impl AudioBackend for RodioBackend {
    // Identical bodies to the temporary `impl AudioBackend for AudioState`
    // removed from room/src/audio/mod.rs in this task (see Task 2 Step 2).
    // `play_music` keeps passing a mixer handle: `let mixer = self.mixer.clone();
    // self.music.play(midi_bytes, looping, &mixer);` — `MusicState` and its
    // `play(&mut self, midi_bytes, looping, mixer: &Mixer)` signature move
    // over verbatim, so no method-signature changes are needed anywhere.
}
```

Implementation note (exact): mirror the bodies of the removed `impl AudioBackend for AudioState`
from `room/src/audio/mod.rs`; the only delta is the struct name `RodioBackend`. Channel plumbing
(`start_sound` body) moves with `ChannelState`/`PannedSource` unchanged.

Register the module in `shells/native/src/main.rs`: `mod rodio_backend;` and
`use rodio_backend::RodioBackend;`.

- [ ] **Step 3: Install the factory in `shells/native/src/main.rs`**

In `fn main()`, after `env_logger` init and **before** `EventLoop::new()`:

```rust
room::audio::set_backend_factory(|| {
    RodioBackend::new().map(|b| Box::new(b) as Box<dyn room::audio::AudioBackend>)
        .map_err(|e| e.to_string())
});
```

- [ ] **Step 4: Remove the moved code and the rodio dependency from the lib**

Delete from `room/src/audio/mod.rs`: `AudioState` struct, its inherent impl, the temporary
`impl AudioBackend for AudioState`, the `rodio` imports, and `NoopBackend` stays.
Delete from `sfx.rs`: `PannedSource`, `ChannelState` (both moved), and their rodio imports.
Delete from `music.rs`: `MusicSource`, `MusicState`, and rodio imports; keep `mus2midi`, the
sample-rate constant, and any pure helpers the moved code now imports from the lib.
Remove `rodio = "0.22"` from `room/Cargo.toml`. Add `rustysynth = "1"` to
`shells/native/Cargo.toml` (it moves with the music code) — keep it in `room` too if
`room/src/audio/music.rs` still references it after the split; if the split leaves no
rustysynth reference in the lib, remove it there instead. Verify with `grep -rn "rustysynth" room/src/`.

- [ ] **Step 5: Build, test, and smoke**

```bash
cargo build
cargo test
cargo run -p room-shell-native --bin room -- -iwad doom1.wad
```

Expected: build + 374 unit + 2 demo tests green (unit count may shift by the Noop tests only);
the binary plays with sound (SFX audible; music audible when a soundfont is found at
`soundfonts/SC55Soundfont-1.2b/SC-55 SoundFont v1.2b.sf2` — present in the repo via LFS; if the
LFS object is not pulled, music stays silent with the familiar warning).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(audio)!: move the rodio backend into the native shell

room/src/audio keeps only the pure, platform-shared pieces (DMX SFX
decode, pan gains, mus2midi, sample-rate) plus the AudioBackend trait;
the rodio mixer graph (RodioBackend, ChannelState, PannedSource,
MusicSource, MusicState) moves to room-shell-native, which installs it
via set_backend_factory. The room lib no longer depends on rodio."
```

---

### Task 4: wasm32 acceptance check for the `room` lib

**Files:**
- Modify: possibly none — this task verifies and fixes type-level fallout only.

**Interfaces:**
- Consumes: Tasks 1-3 (lib without winit/wgpu/rodio).
- Produces: a passing `cargo check -p room --target wasm32-unknown-unknown` (spec acceptance 2).

- [ ] **Step 1: Ensure the wasm target is installed**

```bash
rustup target list --installed | grep wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
```

- [ ] **Step 2: Run the check**

```bash
cargo check -p room --target wasm32-unknown-unknown
```

Expected: passes. The lib's remaining platform contact points are `extern "C"` CRT declarations
(declarations only — they do not link in `cargo check`) and `room/src/doom/crt.rs` (per-target
dispatch, already windows/unix-gated via `link_name` — on wasm none of those cfg branches compile,
so add nothing; if the check fails on `crt.rs` because neither `cfg(unix)` nor `cfg(windows)`
applies, gate the two shim extern items additionally with `#[cfg(not(target_arch = "wasm32"))]`
and make `strdup`/`mkdir`/`errno_location` panic with a clear "no CRT on wasm" message in that
case).

- [ ] **Step 3: If the check surfaces other errors, resolve by class**

- Missing-item errors from rodio/winit/wgpu paths → a Task 3 leak; move the offending item to
  `shells/native` the same way.
- `Send`/`Sync` bound errors from `Box<dyn AudioBackend>` → relax nothing; store the backend in
  the existing thread-local (main-thread-only) exactly as today.
- Anything else → it is a portability type issue; fix per the `crt.rs` pattern (pure-Rust
  reimplementation or per-target dispatch) and add a unit test alongside.

- [ ] **Step 4: Record the acceptance**

```bash
cargo check -p room --target wasm32-unknown-unknown 2>&1 | tail -2
```

Expected output ends with `Finished`. Capture it in the commit message body.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore(wasm): verify room lib type-checks for wasm32-unknown-unknown

cargo check -p room --target wasm32-unknown-unknown passes after the
shell carve-out; fixes any residual type-level platform leaks."
```

(If Step 2 passed with no edits, commit only if something changed; otherwise note the pass in the
Task 5 docs commit.)

---

### Task 5: Docs sync and final verification

**Files:**
- Modify: `README.md` (usage commands)
- Modify: `AGENTS.md` (status line)
- Modify: `docs/specs/2026-09-17-platform-shell-design.md` (Status: → Implemented)

**Interfaces:**
- Consumes: package/bin names `room-shell-native` / `room` (Tasks 1-3).

- [ ] **Step 1: Update README run/build instructions**

Replace `cargo run --bin room` style commands with
`cargo run -p room-shell-native --bin room -- -iwad doom1.wad`, and note the workspace layout
(`room` = engine lib, `shells/native` = native platform shell). Keep all other README content
untouched.

- [ ] **Step 2: Update `AGENTS.md` status line**

Change `Status: **forked, baseline green** …` to note that spec ① is implemented: lib is
wasm-clean, native shell moved to `shells/native`, audio control plane in place. Reference
`docs/specs/2026-09-17-platform-shell-design.md`.

- [ ] **Step 3: Update the spec status**

`**Status:** Approved (design; implementation plan pending)` → `**Status:** Implemented (2026-09-17)`.

- [ ] **Step 4: Final verification**

```bash
cargo fmt
cargo build
cargo test
cargo check -p room --target wasm32-unknown-unknown
cargo clippy -p room-shell-native 2>&1 | tail -5
```

Expected: fmt clean on moved/touched files; tests green; wasm check passes; clippy warnings only
in pre-existing code (fix any that point at files this plan created, i.e. `shells/native/*`).

- [ ] **Step 5: Commit and push**

```bash
git add -A
git commit -m "docs: sync README/AGENTS for the shell carve-out (spec 1 implemented)"
git push origin main
```
