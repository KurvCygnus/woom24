# shells/web — woom24's wasm shell

`room-shell-web` is the platform shell that plugs the room engine into the browser (cdylib, `wasm32-unknown-unknown`): wasm-bindgen exports + an in-memory VFS CRT shim (the `woom24-libc` declaration layer under `crt/`) + a Web Audio backend + Canvas2D/WebGL2 presentation.

## Build the static artifact (self-contained)

Prerequisites: the `wasm32-unknown-unknown` rustup target, and the `wasm-bindgen` CLI pinned to **0.2.121** (it must stay in lockstep with the crate version in `Cargo.lock`; the script verifies both and fails on mismatch).

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File shells/web/scripts/build-www.ps1
```

The script builds the release wasm, runs `wasm-bindgen --target web`, and writes the generated glue into `shells/web/www/pkg/` (git-ignored; always reproducible from the script). The served tree is `shells/web/www/`: `index.html` + `woom24.js` (loader) + `pkg/` (generated).

## Serve

```bash
python -m http.server 8000 --directory shells/web/www
# open http://localhost:8000/
```

Any static host serving `www/` works; the engine fetches nothing at runtime.

## Browser smoke checklist (manual)

Run in Chromium and Firefox; record pass/fail per line.

1. Page loads, status shows `wasm ok (v1)` — no console errors.
2. Pick a local `doom1.wad` (shareware, user-supplied) → launcher panel appears (PWAD/SF2 inputs + Start).
3. Click Start (no PWADs) → demo playback visible on canvas, 35 Hz feel, no console panics.
4. SFX audible (pistol/door during demo). Music silent without SF2 (expected degradation), audible with a user-picked `.sf2`.
5. Keys work: arrows move, Ctrl fires, Esc responds.
6. Reload, then in DevTools console: `woom24_demo_standard(<File object of doom1.wad>)` → boots directly, no launcher.
7. Forced fallback: with WebGL2 unavailable (e.g. a Chromium `--disable-webgl2` test profile), re-run items 3–6 → the game still renders, via the Canvas2D fallback. This exercises the **probe-time** GL2→Canvas2D selection (the clean Canvas2D path). The **GL-init-fail** fallback (webgl2 context exists, shader/program setup fails) is covered by the same smoke: the failed attempt poisons the canvas for `2d` (a canvas that has seen a webgl2 context can never yield a `2d` context), so the shell swaps in a fresh deep clone before the Canvas2D attach — the console shows `webgl2 init failed; canvas was context-poisoned — swapped in a fresh canvas for Canvas2D fallback`. Both fallback paths must render.
   Known gap while playing: in-menu save attempts currently trap (fopen miss → `I_Error` → wasm trap) until the DOM-surfacing `I_Error` hook lands.
8. Double-start guard: invoke a start entry a second time (e.g. call `woom24_demo_standard(file)` again, or click Start after a standard start) → the second call is refused with a console error (`woom24: engine already started…`) and the running game stays intact (no state corruption, no double tick). Frame-entry gate (BUG A fix): the rAF loop runs from the IWAD pick onward but only calls `woom24_frame` once a start has succeeded — the loader's `engineStarted` flag (set directly by the standard entry, or by the launcher's `woom24-started` document event) and the core-side `DG_CREATED` latch (a pre-create frame entry no-ops in `room/src/doom/d_main.rs`) guard both layers, so no frame can enter the engine while it is un-initialised (the historical symptom was a `ticdup == 0` division panic during the launcher phase).
9. Reload discipline: reloading mid-game re-enters via the launcher (minimal entry); the PWAD/asset set is frozen per boot — there is no mid-game asset swap.
10. `file://` open of `www/index.html` may fail on module/wasm fetch — record the exact browser error text; single-file embedding is a follow-up, static-host serving is the contract.
11. F1 M1 uncapped rendering: motion during demo playback is smooth at the display refresh rate (check on a 144 Hz display if available) — no 35 Hz stepping of the player, mobs, weapon bob, or a moving door/lift; the picture must not show doubled or frozen frames.
12. F1 M1 tab-suspend catch-up cap: switch away from the tab for ~10 s mid-demo, then return → the game resumes without a multi-second freeze or a runaway fast-forward burst (the engine catches up at most 4 tics per frame) and motion stays smooth afterwards.
13. F1 M2 mid-game resolution switch: with the game running, call `woom24_probe_set_video_resolution(1366, 768)` in the console → the next frame renders at the new internal raster (canvas dims follow; the picture stays correct, no garbage rows); `woom24_probe_set_video_resolution(320, 200)` switches back. Out-of-range values (e.g. `…(100, 100)`) are refused with a console error and the game keeps running. The user-facing switch ships with the F7 settings surface.
