# AGENTS.md — woom24

## Project Introduction

**woom24 = W(ASM)oom(ID)24**: a WASM port of Doom (1993), written in Rust, with **ID24** as the long-range
compatibility target. The name is the mission statement, deadpan on purpose:

| Fragment | Meaning |
|---|---|
| **W** | **WASM** — the only shipping target is a browser, via `wasm32-unknown-unknown`. |
| **oom** | **(D)oom** — engine work: demo-exact simulation, Boom/MBF-family compatibility. |
| **24** | **ID24** — the compatibility ceiling the project is named after; the roadmap ends there. |

Status: **spec ①+②+③ implemented (branch `feat/spec2-3`)** — platform layer lives in `shells/native`; audio goes through the `AudioBackend` control plane with native (rodio) and web (Web Audio) backends; `shells/web` ships the wasm shell (CRT/VFS shim, DG_* + rAF loop, Canvas2D/WebGL2 presenters, two-entry contract per "Web Entry Contract"; wasm artifact ≈925 KiB self-contained). c_tests/LP64 policy landed (spec ③): differential suite stays LP64-unix-gated (oracle = compiled C side, see docs/specs/2026-09-18-c-tests-lp64-audit.md), struct-size guards are model-aware. Browser human-pass pending on the user. GitHub-side fork: KurvCygnus/woom24 (origin); upstream: sunsided/room.
This file is the binding contract for any agent working in this
repository. When analyzing, reviewing, or generating code, strictly align with the constraints below.

### Decision Log (agents must not silently decide PENDING rows)

| Decision | Status |
|---|---|
| Upstream strategy | **DECIDED (2026-09-17)** — fork `sunsided/room` as the implementation base; project inherits GPL-2.0. See "Fork Discipline" below. |
| WAD parsing | **DECIDED (2026-09-17)** — extend room's `w_wad` for compat tiers; borrow browser loading UX from `crustyview`, never a second parser. |
| WASM target | **DECIDED (2026-09-17)** — `wasm32-unknown-unknown` + wasm-bindgen. Doom's ecosystem only touches the platform at the system-I/O boundary, so the pure-Rust target is both the most compatible and a reasonable effort; Emscripten is out. |
| Render backend | **DECIDED (2026-09-17)** — WebGL2 default + Canvas2D baseline; WebGPU as an explicitly experimental backend behind a feature flag (still evolving, non-trivial — worth attempting, never required). All backends sit behind the render trait; core never knows which one runs. |
| Audio stack | **DECIDED (2026-09-17)** — MIDI-first SF2 synthesis with layered fallback: rustysynth + user-supplied SF2 (rustysynth is room's existing pure-Rust synth — it fills the FluidSynth role; the C FluidSynth library conflicts with the no-C-dependencies constraint) → built-in OPL2 emulation (asset-free) → silence. SF2 is loaded via the config UI; nothing is fetched at runtime. |
| Entry API | **DECIDED (2026-09-17)** — two explicitly separated wasm exports; see "Web Entry Contract". |
| Multiplayer | OPTIONAL — deferred; must never shape the core loop (see constraint 5) |

## Non-Negotiable Product Constraints

1. **Self-contained artifact.** The final `.wasm` + static assets must run standalone from any static host or
   `file://`. At runtime the engine fetches nothing: no CDN, no `fetch()` of remote resources, no telemetry, no
   server component, no analytics. The user supplies IWAD/PWAD files locally (file picker / IndexedDB).
   WAD files are copyrighted — never commit, bundle, or upload them.
2. **Low runtime requirements.** Pure Rust compiled to `wasm32-unknown-unknown` (wasm-bindgen). No Emscripten,
   no C dependencies unless one is explicitly approved. Browser APIs only; degrade gracefully where practical.
3. **Uncapped FPS.** Simulation is fixed at 35 tics/s; rendering runs at display rate with interpolation between
   the last two tics (prboom+ model). Render rate must never influence simulation state.
4. **Custom resolution & custom aspect ratio.** The renderer is resolution-independent; the vanilla 4:3 aspect
   correction (320×200 on non-square pixels) is the correctness baseline. Widescreen/custom aspect is
   render-side only — extra view frustum, never extra simulation.
5. **Demo playback is a hard requirement.** Record + playback across complevels: vanilla LMP (incl. longtics),
   Boom / MBF / MBF21 extended formats. Demos double as our regression vectors (see Testing Standards).
6. **Multiplayer is optional.** Core simulation stays single-player deterministic. Networking lives behind a
   feature boundary and must never leak into the core loop's structure or determinism.
7. **WAD compatibility levels.** REQUIRED: **Vanilla, Limit-Removing, Boom (2.02), MBF**. NAMED TARGET:
   **MBF21, ID24**. Behavior is selected by an explicit complevel value; never silently mix behaviors across
   tiers, and never let a higher tier change a lower tier's observable behavior.

## Compatibility Strategy (Spec-First, No ZDoom)

Resolution order for any compatibility question: **(1) published spec → (2) mature reference port → (3)
original source**. The ZDoom family (ZDoom/GZDoom/UZDoom) is excluded as a reference.

**Reference policy (decided 2026-09-17):** the implementation base is our fork of `sunsided/room`
(Vanilla-exact core); every tier above Vanilla is additive engine work validated against the references in the
table — reading material, never wholesale pasted code. Other Rust Doom projects (rust-doom, iron-doom, doome,
ferrum-doom, …) are **not** references. WASM-layer references: `crustyview` (browser WAD loading UX, automap
rendering ideas) and `GMH-Code/Dwasm` (uncapped FPS, custom resolution/aspect, single-file self-contained
deployment; GPL-2.0 — code-level borrowing is license-compatible).

| Tier | Spec? | Primary reference | Secondary |
|---|---|---|---|
| Vanilla | No (semantics = linuxdoom-1.10) | `id-Software/DOOM`; Chocolate Doom as the vanilla-accuracy checklist (`PHILOSOPHY`, `NOT-BUGS`) | Doom Wiki |
| Limit-Removing | No | Crispy Doom — enumerate exactly which limits are lifted (visplanes, SPECHITS, plats, openings, …) | Doom Wiki "Limit removing" |
| Boom | No | Boom 2.02 source (`doom-cross-port-collab/boom`) + its `BOOMREF.TXT` / `BOOMDEH.TXT` / `BOOMLUMP.TXT`; complevel table in prboom-plus docs | dsda-doom compat switches |
| MBF | No | MBF 2.03 source (`doom-cross-port-collab/mbf`, Boom-based) | Woof! (MBF continuation) |
| MBF21 | **Yes** — `kraflab/mbf21` `docs/spec.md` + `docs/developer_spec.md` (v1.4) | dsda-doom (first implementation, complevel 21) | Woof! |
| ID24 | **Yes** — `doom-cross-port-collab/id24` (v0.99.2 draft) | Woof! 16.x (most complete community implementation) | spec repo's `source_code_reference/` |

Notes:
- prboom-plus (`coelckers/prboom-plus`) was archived 2023 — treat it as a frozen snapshot; its successor is
  dsda-doom.
- ID24 is a pre-1.0 draft and **not** fully Boom/MBF21-compatible (demos, weapon behavior differ). Implement it
  **last**, as an additive layer; keep the DeHacked layer DSDHacked-ready so ID24 slots in without rework.
- Full research with URLs and the port×complevel matrix lives in `docs/research/references-and-specs.md`.

## Architecture

- Workspace: `core` (the single source of truth for all engine logic) + thin platform shells (a `web` shell on
  wasm-bindgen; a native dev shell is allowed). Shells own only rendering surfaces, input plumbing, and platform
  concerns — never logic. Logic is never duplicated between shells.
- `core` must not depend on `wasm-bindgen`, `web-sys`, `winit`, or any DOM/OS API. The shell↔core boundary is a
  small trait set modeled on doomgeneric's minimal interface (init / draw frame / get ticks / get keys).
- **Determinism is a feature, not an optimization detail.** Simulation is integer/fixed-point exactly as
  vanilla used it: no floats in sim, no hash-map iteration order reaching sim state, seeded deterministic RNG.
  When vanilla relied on accidental behavior, replicate that behavior explicitly and mark it with `//!`.
- One `Complevel` enum drives every compat switch. Per-tier behavior gets its own table in `core`, reviewed
  against that tier's primary reference.
- Wasm FFI rule: variadic `extern "C"` calls must never be declared directly in `room/src/doom/` — wasm-lld
  replaces arity-mismatched calls with trapping `signature_mismatch` stubs; they route through the per-arity
  `crt.rs` wrappers (decision record: `docs/specs/2026-09-18-wasm-per-arity-crt-decision.md`).

### Fork Discipline (`sunsided/room`)

- The upstream is **active** — keep an `upstream` remote, merge room master regularly, and never rewrite
  history in ways that block future merges.
- Preserve room's module layout, file names, and its C-vs-Rust differential-test culture; deviation needs a
  stated reason. Merge friction is a real cost.
- Keep changes upstreamable: bugfixes and general improvements (the WASM platform layer itself is a prime
  candidate) stay PR-shaped and go back to room. This is how the DOOM community works — participate.
- GPL-2.0 inheritance is accepted: keep `LICENSE`, preserve copyright/provenance headers, add ours on new
  files.
- First milestones on the fork: (1) run upstream's `cargo test --test demo_playthrough` to establish the
  demo-playback baseline (upstream already ships this regression — it is the evidence that demo playback
  works there); (2) carve the platform layer (winit/wgpu/rodio) into a shell trait so the native dev shell
  and the new wasm shell can coexist.

### Upstream Engineering References

- room's own engineering/agent doc is preserved in-tree at `docs/upstream/room-AGENTS.md` (moved from the
  repo root, tracked as a rename). It documents the C→Rust porting lessons that MUST be followed when
  touching ported modules — e.g. `angle_t` (`u32`) arithmetic must use `wrapping_*`, FFI strings must be
  null-terminated byte slices, `#[repr(C)]` padding/zeroing rules — plus the ASan workflow and the
  `c2rust-intermediate/` behavioral reference crate (excluded from default build; nightly-only). Read it
  before editing anything under `room/src/doom/`.
- The GitNexus sections inside that doc apply only when the GitNexus MCP server is connected; it is not part
  of this environment's toolset — ignore those instructions and keep the rest.

### Web Entry Contract (two explicit exports)

The wasm surface exposes two entry functions that MUST stay explicitly separated — their behaviors differ by
design, and one must never degrade into the other via optional parameters or flags:

- **Minimal entry** — arguments: device metadata (at minimum the maximum render resolution the host allows,
  so the engine caps its framebuffer to the device) + the loaded IWAD. It boots the engine into **launcher
  mode**: the config UI is shown first, where the user supplies PWADs, SF2/MIDI assets, and options; the game
  starts only after that.
- **Standard entry** — arguments: a complete boot profile (IWAD, PWADs and their order, options, assets). It
  starts the game directly with no config UI. Intended for hosts that bring their own UI.

Separation lives at the contract surface only: internally both entries converge on the same init pipeline
with different boot profiles — never two divergent init code paths. The PWAD/asset set is fixed at boot;
mid-game asset swaps are out of scope until explicitly designed (they are determinism hazards).

## Code Formatting & Style Guidelines

- `rustfmt` is authoritative; run `cargo fmt` on touched files. **Zero `cargo clippy` warnings is the goal.**
  For unavoidable findings, suppress with an inline reason (`#![allow(...)] // ! Reason…`). Deprecated APIs are
  never used.
- Comment prefix system:
  - `//*` — explains something important.
  - `//!` — explains something edgy, counterintuitive, or footgunny.
  - `//?` — confusion, TODO, FIX.
- The `//*` / `//!` / `//?` markers live **only in `//` comments**. Doc comments (`///`, `//!`) are
  rendered by rustdoc — these markers have no effect there and render literally (decided
  2026-09-18: never prefix doc comments with them). Doc comments use standard Markdown and
  Rustdoc conventions instead (headings, backticks, `# Panics` / `# Safety` sections).
- Comments explain **why, not what**; no doc block on the self-evident. Comments are written in
  **English** (decided 2026-09-18: the project is global-facing); ASCII punctuation only; follow
  file-local convention when a file already uses another language.
- Always use guard clauses; prefer expression bodies for one-liners. Self-descriptive names; readability over
  brevity — long but meaningful names are acceptable.
- Temporary mess is acceptable inside module boundaries; anything crossing a declared boundary must be clean.
- Model WAD/lump data as typed structs — no `serde_json::Value` or weakly-typed maps as DTO substitutes.
- `unsafe` only with local justification and a `// SAFETY:` comment. Code reachable from WAD input must not
  panic — parse with `Result` and report errors.

## Tooling & Workflow Protocols

1. Any file referenced here or in `docs/` must be read via file-reading tools before writing code that depends
   on it.
2. **Verification before done:** `cargo build` + `cargo clippy` + `cargo test` on host, plus
   `cargo build --target wasm32-unknown-unknown` whenever the web layer is touched. Never claim success without
   running them; report failures with real output.
3. **Timeout discipline:** every build/test/run command carries an explicit bounded timeout. Exit 124 = timed
   out = FAILED; record partial evidence, report the stall — never retry blindly, and never raise the limit.
   A command needing >300 s is a signal to **decompose** the work.
4. **No concurrent heavy commands.** Never run two builds/tests in parallel (not as parallel tool-call batches,
   not across subagents) — this machine freezes. Use `--no-<phase>` flags to keep re-runs cheap.
5. **Dependency freeze.** Introducing any new crate requires explicit user approval. Prefer zero-dependency,
   wasm-clean pure-Rust crates. The approved list starts empty and grows only via the Open Decisions table.
6. **Reference clones are read-only.** Vendored upstream sources (Chocolate, Crispy, Boom/MBF sources,
   dsda-doom, Woof!, Dwasm, crustyview, …) live under `reference/` and are never built into our targets,
   never modified, never committed inside. Cite their files as evidence; write shipped code fresh in Rust.
   The exception is the fork itself — room code in our tree is ours to change (see Fork Discipline).
7. Never commit unless the user explicitly asks; inspect `git status` / `git diff` first, stage only intended
   files, never commit secrets or WAD files.
8. PowerShell invocations use `-NoProfile`. Long-running scripts the user waits on end with
   `[console]::beep(880, 250)`.

## Testing Standards

- Tests never hit the network and never require commercial IWADs. Use tiny synthetic WAD fixtures built
  in-test or committed fixtures we generate ourselves.
- **Golden demo tests are mandatory:** a fixed demo input must produce byte-identical simulation state hashes
  across runs **and across host/wasm targets**. This is the project's core regression mechanism — demo-exact
  determinism is both a product requirement (constraint 5) and the test oracle.
- Where a GPL reference's behavior must be matched but its code cannot be reused, use differential testing:
  run reference and ours side-by-side on the same inputs and compare observable outputs (the
  `sunsided/room` C-vs-Rust harness pattern), rather than copying code.
- When a shared test helper exists for a fixture type, using it is mandatory; no ad-hoc inline duplicates.

## Documentation Standards

- After a task changes design or usage, synchronize `docs/DESIGN.md` and `README.md`.
- Compatibility decisions are recorded per complevel in a decision log under `docs/`, each entry citing the
  spec section or reference-port file it was derived from. No uncited compat behavior.
- Changelogs (once shipping) are user-facing: no implementation details, no `[Unreleased]` header churn;
  feature → minor bump, fix → patch bump.

## CRITICAL PROTOCOL: SKILL ENFORCEMENT & HARD STOP

Before starting any task, check the harness's available-skills list. If a skill matches the task (creative
work, debugging, planning, verification, parallel dispatch, …), you are expressly forbidden from solving the
task directly: invoke the matching skill and follow it. If no skill applies, proceed. If the skill system is
inaccessible, state exactly that once, then proceed with the discipline this file already prescribes.
