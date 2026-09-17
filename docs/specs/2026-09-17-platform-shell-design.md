# Design: Platform shell carve-out (`shells/native`, wasm-clean `room` lib)

**Date:** 2026-09-17
**Status:** Approved (design; implementation plan pending)
**Series:** ① this spec (platform seam) → ② wasm shell (`wasm-bindgen`) → ③ c_tests LP64 strategy

## Problem

woom24 targets `wasm32-unknown-unknown`, but the engine library cannot even be type-checked for that
target today:

1. The `room` package's `[dependencies]` carry `winit`, `wgpu`, and `rodio` (used only by the binary
   side: `src/main.rs`, `src/platform/`, `src/gpu.rs`). None of these are wasm-clean in the shape the
   engine needs, and their mere presence blocks a wasm build of the crate.
2. The crate-private `audio` module (in the **lib**) is built directly on rodio primitives —
   `MixerDeviceSink`, `Mixer`, per-channel `Player`s, `PannedSource: rodio::Source` — so rodio
   reaches into the library itself.

At the same time, the platform layer is small and already well-shaped: the engine talks to the
outside world exclusively through the six doomgeneric callbacks `DG_Init`, `DG_DrawFrame`,
`DG_SetWindowTitle`, `DG_GetKey`, `DG_GetTicksMs`, `DG_SleepMs` (`#[no_mangle] extern "C"`,
resolved at final link).

## Goals

1. `cargo check -p room --target wasm32-unknown-unknown` passes (lib is wasm-clean at the type
   level; producing a wasm artifact is spec ②).
2. Native behavior is **byte-identical**: render/audio/input paths are *moved* code, not rewrites.
3. Engine modules under `room/src/doom/` stay untouched except one mechanical change: the audio
   call sites go through a backend trait instead of a concrete rodio-backed struct.
4. The full existing suite stays green: 372 unit tests + 2 `demo_playthrough` integration tests.
5. Upstream merge friction is bounded and acknowledged (file moves + workspace Cargo changes).

## Non-goals

- Uncapped-FPS interpolation (engine-side future work; rendering stays at tic rate).
- The wasm artifact itself: wasm-bindgen exports, the two-entry contract (AGENTS.md "Web Entry
  Contract"), WebGL2/Canvas2D/WebGPU presenters, Web Audio backend — all spec ②.
- The c_tests LP64 audit — spec ③.
- Upstream pull requests — explicitly deferred (upstream main is mid-movement; we re-PR later).

## Decision: keep `DG_*` as the engine↔platform ABI

Chosen over (B) converting the engine to call a Rust `Platform` trait object — that would touch
ported call sites across `i_video`/`i_input`/`i_system`/`doomgeneric`, maximise divergence from
upstream `room`, and invalidate the demo-exact regression story for zero behavioral gain — and over
(C) adding a speculative `PlatformHost` lifecycle trait now (YAGNI until spec ② proves the need).

Precedent: doomgeneric's Emscripten backend drives the *same* `DG_*` ABI from JavaScript
(`requestAnimationFrame` → `DG_DrawFrame`; JS pushes keys into a queue that `DG_GetKey` pops; a
virtual clock backs `DG_GetTicksMs`). The seam therefore already has a proven wasm shape; spec ②
inherits it unchanged.

## Workspace layout (after)

```
woom24/ (workspace)
├── room/                  # pure engine lib: doom/ + audio/ (pure logic) + headless/ + types/
│                          # deps: − winit, − wgpu, − rodio, − env_logger, − pollster; keeps rustysynth
├── shells/
│   └── native/            # NEW package; bin name `room`; owns winit + wgpu + rodio + env_logger + pollster
├── doomgeneric-sys/       # unchanged
├── c2rust-intermediate/   # unchanged (nightly-only reference, default-members excluded)
└── shells/web/            # spec ② landing spot; intentionally absent in this spec
```

`default-members` gains `shells/native`; CI-equivalent local flow stays `cargo build` /
`cargo test`.

## Components (`shells/native`, all moved code)

| Component | Moved from | Responsibility |
|---|---|---|
| `main.rs` | `room/src/main.rs` | startup: inject `myargc`/`myargv` → winit window → wgpu surface → pick audio backend → `doomgeneric_Create` → event loop drives redraw |
| `dg.rs` | `room/src/platform/mod.rs` | the six `DG_*` `#[no_mangle] extern "C"` impls, reading shell state |
| `state.rs` | statics in `main.rs` / `platform/mod.rs` | `WINDOW`, `GPU`, `KEY_QUEUE`, `QUIT_REQUESTED`, start clock — consolidated as shell-crate-private state (they already existed only on the bin side) |
| `present.rs` | `room/src/gpu.rs` | wgpu presentation: 320×200 paletted frame → scaled surface (resize/reconfigure logic unchanged) |
| `keys.rs` | `room/src/platform/keys.rs` | winit key → Doom key mapping |
| `rodio_backend.rs` | rodio parts of `room/src/audio/` | `RodioBackend`: today's mixer graph (Mixer/Player/PannedSource + music rodio Source) relocated verbatim |

Data flow (unchanged): winit events → `KEY_QUEUE` → `DG_GetKey` → engine; engine tic →
`DG_DrawFrame` → `present.rs`; `DG_GetTicksMs`/`DG_SleepMs` ← start clock; `i_sound` →
`AudioBackend` → mixer → device.

## Audio seam (control plane / data plane split)

Reading `audio/mod.rs` shows rodio is not merely the output device — the whole 8-channel graph is
rodio-native. The seam is therefore the *control plane* (the engine-facing API), not "a frame
pusher":

```rust
// room/src/audio/ — exported (`pub`): implemented by shells/native.
pub trait AudioBackend {
    // SFX — mirrors AudioState verbatim (audio/mod.rs)
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool;
    fn stop_sound(&mut self, channel: usize);
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32);
    fn is_playing(&self, channel: usize) -> bool;
    // Music — mirrors MusicState minus the mixer parameter (the backend owns its mixer)
    fn load_sound_font(&mut self, path: &std::path::Path);
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool);
    fn stop_music(&mut self);
    fn set_music_volume(&self, vol: i32);
    fn pause_music(&self);
    fn resume_music(&self);
    fn is_music_playing(&self) -> bool;
}
```

- **Stays in the lib (pure, shared by all backends):** `decode_doom_sfx` (DMX decode), `mus2midi`,
  the rustysynth song registration/sequencing core of `music.rs`. `rustysynth` is pure Rust and
  remains a `room` dependency.
- **Moves to `shells/native`:** the rodio graph. `RodioBackend` implements the trait with today's
  code, relocated verbatim; its `play_music` re-attaches the music player to its own mixer (today
  `MusicState::play` receives `&Mixer` — the parameter becomes internal state).
- **Engine-side change (the one mechanical edit):** the `AUDIO` thread-local becomes
  `RefCell<Option<Box<dyn AudioBackend>>>`; `i_sound.rs` / `s_sound.rs` call sites keep their
  shape, dereferencing through the trait object. MUS→MIDI conversion stays at the engine call site,
  exactly as today.
- **Backend installation:** the `AUDIO` cell stays crate-private; the lib exposes one public
  installer, `room::audio::set_backend(Option<Box<dyn AudioBackend>>)`. `shells/native` calls it
  during startup with `RodioBackend` (or `NoopBackend` on device failure); tests may install
  `NoopBackend` explicitly.
- **`NoopBackend`:** every control op is a no-op; `is_playing`/`is_music_playing` return `false`.
  It formalises today's silent path (no audio device / no soundfont) so the engine always has a
  backend and "audio degrades, never fails".

## Error handling (parity with current behavior)

- GPU adapter/surface creation failure → fatal (`I_Error`-style exit), as today.
- Audio device open failure → WARN log, fall back to `NoopBackend` (today: `AUDIO` stays `None`
  and the engine runs silent — equivalent semantics, one code path instead of scattered `None`
  checks).
- Runtime surface loss (minimize/resize) → unchanged reconfigure logic from `gpu.rs`.

## Testing & acceptance

1. `cargo test` — 372 unit + 2 `demo_playthrough` stay green (integration tests keep their own
   `DG_*` stubs in `tests/demo_playthrough.rs`; lib unit tests keep `dg_test_stubs`; both were
   introduced in commit `ed64934` and are untouched by this spec).
2. **New hard acceptance:** `cargo check -p room --target wasm32-unknown-unknown` passes.
3. Manual smoke (native): `room` with the shareware `doom1.wad` — window renders, menu navigable,
   `DEMO1` plays, audio audible.
4. Byte-parity claim is by construction for audio: the mixer graph is moved, not rewritten;
   `demo_playthrough` pins simulation determinism independently of audio.

## Upstream divergence accounting

- Diff shape: one new package (`shells/native`), file moves (`git mv`) for main/platform/gpu and
  the rodio audio parts, workspace `Cargo.toml` edits, and the `AUDIO` trait-object change in
  `i_sound.rs`/`s_sound.rs`.
- Moves are rename-detectable; the engine (`doom/`) diff is confined to the audio call sites.
- Per AGENTS.md Fork Discipline: room's module layout inside `room/src/doom/` is untouched, and
  the upstream remote keeps tracking `sunsided/room`.

## Follow-ups

- Spec ② (wasm shell): wasm-bindgen exports, Web Entry Contract (minimal/standard entries),
  presenter backends (WebGL2 default, Canvas2D baseline, WebGPU experimental), Web Audio
  `AudioBackend` implementation.
- Spec ③ (c_tests): decide the LP64/LLP64 strategy for the C-vs-Rust differential suite.
