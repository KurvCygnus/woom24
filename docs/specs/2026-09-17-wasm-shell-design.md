# Design: WASM shell (`shells/web`) — wasm-bindgen, VFS, Web Audio, two-entry contract

**Date:** 2026-09-17
**Status:** Approved — autonomously decided under the user's 2026-09-17 sleep mandate ("自主迭代");
every Decision below is marked [AUTO] and is open to revision on user review.
**Series:** ② this spec → ③ c_tests / LP64 strategy
**Inherits:** spec ① (`2026-09-17-platform-shell-design.md`), AGENTS.md "Web Entry Contract".

## Problem

After spec ① the engine library is platform-agnostic at the dependency level, but it cannot yet
run in a browser:

1. **CRT gap (from spec ① Goal 1 amendment):** the engine's `extern "C"` CRT layer
   (`fopen`/`fread`/`fseek`/`ftell`/`fclose`/`malloc`/`calloc`/`free`/`printf`/`snprintf`/
   `sscanf`/`exit`/`strlen`/`strcmp`/… — 73 errors across 13 `doom/` files) has no provider on
   `wasm32-unknown-unknown`; the `libc` crate is empty there.
2. **No platform shell for the web:** nothing implements the six `DG_*` callbacks, installs an
   `AudioBackend`, or drives the engine's loop in a browser.
3. **No entry contract implementation:** AGENTS.md mandates two explicitly separated wasm entries
   (minimal = device metadata + IWAD → config UI; standard = complete boot profile → direct
   start).

## Goals

1. A new workspace package `shells/web` (bin/cdylib `room-shell-web`) that builds with
   `wasm32-unknown-unknown` via `wasm-bindgen` + `web-sys` and produces static files only
   (wasm + JS glue + `index.html`) — no CDN, no runtime fetch, no server.
2. The engine runs to completion in the browser: IWAD supplied from the local file system via the
   config UI or host API, demo playback visible, SFX audible (music audible when a user-supplied
   SF2 is loaded).
3. The 76-error CRT gap is closed inside the web shell (engine code untouched except the one
   class of mechanical `cfg` changes called out below — none planned).
4. The two-entry contract is implemented as two separately exported functions sharing one init
   pipeline (AGENTS.md).

## Non-goals

- WebGPU presenter (stays an experimental follow-up; Canvas2D + WebGL2 ship here).
- Multiplayer, uncapped-FPS interpolation (engine-side, later).
- `wasm-bindgen-test` cross-target demo-hash infrastructure (recorded as follow-up; acceptance is
  browser-manual plus host-side unit tests for the pure modules).
- c_tests / LP64 policy (spec ③).

## Decisions (all [AUTO] — sleep-mandate)

| # | Decision | Rationale / rejected alternatives |
|---|---|---|
| D1 | **A wasm CRT shim + in-memory VFS lives in the web shell** (`shells/web/src/wasm_vfs.rs`): `#[no_mangle] extern "C"` definitions for exactly the symbols the engine references (authoritative list = regenerate `cargo check -p room --target wasm32-unknown-unknown` and capture; baseline: task-4 report of spec ①, 73 errors/13 files). Files are registered from JS as `(name → bytes)` into an in-memory table; `fopen/fread/fseek/ftell/fclose` operate on that table; `malloc/calloc/free` forward to the Rust global allocator (`std::alloc` with layout tracking); `printf`-family (`printf`/`snprintf`/`vsnprintf`/`sscanf`) is a minimal formatter covering the specifier set actually used by the engine (audited during implementation; expected `%s %d %u %x %c %%` + widths — anything outside the audited set logs and degrades, never traps); `exit` traps (`unreachable!`) — the engine only exits fatally. | Alternative (a) a fake `libc`-shaped crate making `check` pass with unreachable bodies was rejected: throwaway, and spec ② needs real VFS anyway. Alternative (b) porting all CRT call sites now: large engine diff, upstream-hostile. Shim-in-shell keeps `doom/` untouched and is the module spec ② genuinely needs at runtime. |
| D2 | **The six `DG_*` callbacks are `#[wasm_bindgen]`-adjacent `#[no_mangle] extern "C"` exports** (same shape as `shells/native`): JS drives the loop via `requestAnimationFrame` → exported `woom24_tick()` → `doomgeneric_Tick()`; `DG_DrawFrame` paints the 640×400 BGRA frame; `DG_GetKey` pops a queue that JS pushes via an exported `woom24_push_key(pressed: bool, doom_key: u8)`; `DG_GetTicksMs` reads `performance.now()` (web-sys) against shell start time; `DG_SleepMs` is a no-op (the JS cadence governs; engine throttling is driven by its tick clock). | doomgeneric's Emscripten precedent (spec ① Decision); avoids any engine change. |
| D3 | **Presenter: Canvas2D baseline first, WebGL2 default at runtime when available.** Canvas2D = `ImageData` + `putImageData` (trivially correct, debuggable); WebGL2 = one texture upload + blit. Runtime selection, Canvas2D always compiled in as fallback. WebGPU: out of this spec. | AGENTS.md Decision Log already fixed the backend policy; smallest correct first slice. |
| D4 | **Audio: `WebAudioBackend` implements `room::audio::AudioBackend`.** SFX: `decode_doom_sfx` (lib, shared) → `AudioBuffer` + `StereoPannerNode` per channel; channel bookkeeping mirrors `ChannelState` semantics (`is_playing` = source active). Music: extract a pull-based `SynthEngine` (rustysynth `MidiFileSequencer` + `Synthesizer` rendering into an interleaved f32 buffer) into the **lib** (`room/src/audio/synth.rs`) and refactor the native `MusicSource` to consume it — one synth core, two output adapters (rodio Source / Web Audio worklet-less ScriptProcessor-free pull via `AudioBuffer` chunking). SF2 bytes come from the entry assets. | Extracting the synth core avoids duplicating MUS→MIDI + sequencing logic in a second shell (AGENTS.md: logic never duplicated between shells). Rejected: leaving music native-only. |
| D5 | **Two entries, one pipeline** (AGENTS.md): `#[wasm_bindgen] pub fn woom24_minimal_start(max_render_res: u32, iwad_name: &str, iwad: &[u8])` registers the IWAD in the VFS, sets the framebuffer cap, and enters **launcher mode** — a DOM config UI (built with web-sys DOM APIs: file inputs for PWADs/SF2, start button) rendered into a container element; game starts only after the user confirms. `#[wasm_bindgen] pub fn woom24_standard_start(profile_json: &str)` takes a complete boot profile (IWAD name, PWAD list + order, SF2 name, options — blobs registered beforehand via `woom24_register_file(name, bytes)`) and starts directly. Both converge on one `init_pipeline(profile)`; the exports differ only in how the profile is produced. `Config`/`set` of `myargv`/`myargv` happens in Rust (profile → argv vector), no JS argv. | AGENTS.md entry contract; separation at the export surface only. |
| D6 | **Determinism guard:** nothing in the shell may feed wall-clock nondeterminism into the simulation — the shell touches the engine only through `DG_*`, argv, and the audio control plane (all out-of-band). Render-side interpolation stays out (engine work, later). | Spec ① testing philosophy. |
| D7 | **Build/deploy:** `wasm-pack build shells/web --target web` (or plain `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen` CLI — decided at plan time by what the repo's tooling supports cleanly); output copied to `shells/web/www/` with a minimal local `index.html` + loader JS. `wasm-opt` pass optional; binary-size budget noted in acceptance (report the .wasm size; no hard gate this spec). | Self-contained constraint (AGENTS.md constraint 1). |

## Architecture

```
shells/web/
├── Cargo.toml            # cdylib + bin? cdylib only; deps: room, wasm-bindgen, web-sys, js-sys, rustysynth
├── src/
│   ├── lib.rs            # wasm-bindgen exports: entries, tick, push_key, register_file, canvas wiring
│   ├── init_pipeline.rs  # profile → argv → doomgeneric_Create; shared by both entries
│   ├── wasm_vfs.rs       # CRT shim + in-memory VFS (D1)
│   ├── present_c2d.rs    # Canvas2D presenter (D3)
│   ├── present_gl2.rs    # WebGL2 presenter (D3)
│   ├── dg.rs             # DG_* impls over web state (D2)
│   ├── web_audio.rs      # WebAudioBackend (D4)
│   ├── clock.rs          # performance.now-based ticks (D2)
│   └── launcher_ui.rs    # DOM config UI for minimal entry (D5)
└── www/                  # index.html + loader (static, no CDN)
```

Engine-facing surface is unchanged: `room/src/doom/` gets **zero** edits (the CRT shim satisfies
the existing extern declarations at link time).

## Data flow

JS `rAF` → `woom24_tick()` → `doomgeneric_Tick()` → (`DG_GetKey` pops JS-fed queue) →
`DG_DrawFrame` → presenter → canvas. Keys: DOM `keydown/keyup` → `woom24_push_key`. Files: JS
`File.arrayBuffer()` → `woom24_register_file(name, bytes)` → VFS table → engine `fopen`.
Audio: engine `i_sound` → `AudioBackend` → Web Audio graph → device.

## Error handling

- Missing IWAD/failed engine init → launcher UI error banner (minimal) or `Err` return to host
  (standard); never a bare panic into the console.
- VFS: open of an unregistered name → NULL (matches CRT), engine's existing error path handles it.
- Web Audio unavailable/denied → `NoopBackend` (silent), same degradation contract as native.
- WebGL2 context creation failure → Canvas2D presenter.

## Testing & acceptance

1. Host-side unit tests for pure modules: VFS table semantics (register/open/read/seek/close),
   printf-formatting golden cases for the audited specifier set, `SynthEngine` renders
   deterministic samples for a fixed MIDI input.
2. `cargo check -p room --target wasm32-unknown-unknown` passes **after** the shim exists in the
   workspace (the shim lives in `shells/web`, so the room-only check needs `-p room-shell-web`
   included or the check re-pointed at the workspace: acceptance = `cargo check --workspace
   --target wasm32-unknown-unknown` passes; native packages keep compiling for the host via
   existing cfg).
   *Note:* `shells/native` does not build for wasm — workspace-level wasm check must exclude it
   (`--exclude room-shell-native`), mirroring how `c2rust-intermediate` is excluded. Exact
   command pinned at plan time.
3. Browser smoke (manual, documented in README): serve `shells/web/www/` statically, minimal
   entry → pick `doom1.wad` → start → demo visible + SFX audible; standard entry via console call
   boots directly.
4. `.wasm` size reported in the implementation summary (informational).
5. Full host suite stays green (376 unit + 2 demo).

## Follow-ups

- WebGPU experimental presenter; `wasm-bindgen-test` demo-hash harness; `wasm-opt` size pass;
  `BackendFactory` may need to become `Arc<dyn Fn…>` if the web backend needs captured config
  (currently non-capturing works).
- From final review: decide `rustysynth` keep/reword/drop in `room` (synth-core extraction in D4
  supersedes this — resolve when touching `room/src/audio`).
