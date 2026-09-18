# shells/web — woom24's wasm shell

`room-shell-web` is the platform shell that plugs the room engine into the browser (cdylib, `wasm32-unknown-unknown`): wasm-bindgen exports + an in-memory VFS CRT shim (the `woom24-libc` declaration layer under `crt/`) + a Web Audio backend + Canvas2D/WebGL2 presentation.

## Build the static artifact (self-contained)

Prerequisites: the `wasm32-unknown-unknown` rustup target, and the `wasm-bindgen` CLI pinned to **0.2.121** (it must stay in lockstep with the crate version in `Cargo.lock`; the script verifies both and fails on mismatch).

```bash
bash shells/web/scripts/build-www.sh
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
7. Forced fallback: with WebGL2 unavailable (e.g. a Chromium `--disable-webgl2` test profile), re-run items 3–6 → the game still renders, via the Canvas2D fallback. Scope: this exercises the **probe-time** GL2→Canvas2D selection (the clean Canvas2D path) only — the GL-init-fail fallback is **not** covered here (the ledgered canvas-context conflict: a webgl2-then-fail canvas is poisoned for `2d`).
   Known gap while playing: in-menu save attempts currently trap (fopen miss → `I_Error` → wasm trap) until the DOM-surfacing `I_Error` hook lands.
8. Double-start guard: invoke a start entry a second time (e.g. call `woom24_demo_standard(file)` again, or click Start after a standard start) → the second call is refused with a console error (`woom24: engine already started…`) and the running game stays intact (no state corruption, no double tick).
9. Reload discipline: reloading mid-game re-enters via the launcher (minimal entry); the PWAD/asset set is frozen per boot — there is no mid-game asset swap.
10. `file://` open of `www/index.html` may fail on module/wasm fetch — record the exact browser error text; single-file embedding is a follow-up, static-host serving is the contract.
