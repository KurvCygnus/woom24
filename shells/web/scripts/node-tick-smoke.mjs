// Headless Node harness: boots the wasm engine WITHOUT a browser and drives
// ticks, so boot freezes reproduce locally instead of waiting on a human
// browser pass. Prints the panic hook's file:line and the div-probe canary
// fingerprint directly on the Node console.
//
// Usage (repo root):  node shells/web/scripts/node-tick-smoke.mjs [ticks] [mode]
//   mode ""          - plain frozen-clock smoke (tics do not advance headless)
//   mode singletics  - force one-tic-per-tick advancement (probe hook)
//   mode clock       - arm the harness fake clock: the engine runs its real
//                      real-time paths (NetUpdate catch-up, TryRunTics wait
//                      spin, wipe pacing) just faster than wall time
//   mode diag        - sprite-invisibility classification run (Task 4 C):
//                      arms the fake clock, drives the REAL frame path
//                      (woom24_frame, the rAF shape the browser defect lives
//                      in - the legacy woom24_tick never arms the
//                      interpolation board), scripts menu keys into UV E1M1
//                      gameplay, and prints woom24_diag_frame() samples
//                      (frame hash, interp-OFF re-present hash, mobj
//                      sampled-vs-live census, 16x16 diff-density tile map)
//                      every 100 frames
// Prereq: bash/pwsh build-www script has produced shells/web/www/pkg/.

// Clock fix (div-probe): web_sys::window() resolves the `window` global, which
// Node lacks - so DG_GetTicksMs/the engine heartbeat froze at 0 and the
// title->demo window (which needs a running clock) was unreachable headless.
// Point `window` at globalThis: web-sys then finds Node's `performance`
// (global since v16) and the heartbeat runs. This MUST execute before the
// glue import: ESM imports hoist above this module's body, so the glue is
// brought in with a dynamic import below, after the shim is in place.
globalThis.window = globalThis;

import fs from "node:fs";

const TICKS = Number(process.argv[2] ?? 3000);
const MODE = process.argv[3] ?? "";

const { default: __wbg_init, woom24_attach_canvas, woom24_diag_frame, woom24_last_error, woom24_probe_gametic, woom24_probe_set_clock_armed, woom24_probe_set_singletics, woom24_probe_ticdup, woom24_push_key, woom24_register_file, woom24_standard_start, woom24_tick, woom24_frame, woom24_version } =
  await import("../www/pkg/room_shell_web.js");

await __wbg_init(fs.readFileSync(new URL("../www/pkg/room_shell_web_bg.wasm", import.meta.url)));
console.log("[harness] wasm online, version =", woom24_version());

const wad = fs.readFileSync("doom1.wad");
woom24_register_file("doom1.wad", wad);
console.log("[harness] doom1.wad registered,", wad.length, "bytes");

// Install the engine console logger via attach_canvas side effect (a stub
// canvas fails presenter selection AFTER logger install - expected: caught).
try { woom24_attach_canvas({ getContext: () => null, style: {} }); } catch (e) { console.log("[harness] attach_canvas expected failure:", String(e).slice(0, 80)); }

// Pacing hooks MUST be armed before standard_start: the boot's inline first
// doomgeneric_Tick runs TryRunTics, whose wait loop never escapes under the
// frozen Node clock (I_GetTime()==0 forever) - after that the tick call never
// returns and a probe hook would never be reached.
if (MODE === "singletics")
{
    woom24_probe_set_singletics(1);
    console.log("[harness] singletics forced via probe hook (pre-boot)");
}
else if (MODE === "clock" || MODE === "diag")
{
    // Seed with ~2 s of fake boot time: the browser boot takes 1-2 s, so tics
    // always run before the first D_Display; the harness needs the same head
    // start (otherwise the display draws with pagename==NULL and I_Errors).
    woom24_probe_set_clock_armed(1, 2000);
    console.log("[harness] fake clock armed via probe hook, seeded 2000 ms (pre-boot)");
}

// Standard entry: no DOM, no launcher - boots straight into the title/demo
// cycle, which is exactly the freeze window reported on the browser.
woom24_standard_start(JSON.stringify({ iwad: "doom1.wad" }));
console.log("[harness] standard start issued; ticking", TICKS, "times",
  MODE === "" ? "(frozen clock: tics will not advance)" : `(mode ${MODE})`,
  "gametic =", MODE === "" ? "?" : woom24_probe_gametic());

// Task 4 (spec-4 C) diagnosis scenario: the real frame path + scripted
// UV E1M1 gameplay + periodic diag samples. The classification rule is the
// brief's three-way rule applied to the A/B tile map series plus the mobj
// sampled-vs-live census (mo=<max>/<moved>/<total> in each diag line):
//   - moved==0 and shimmer-like maps            -> interpolation healthy
//   - mo max/moved nonzero (a sprite displaced) -> stale-slot aliasing (H1)
//   - dense blob where a sprite should be, OFF  -> C-i (not drawn) /
//     re-present shows it                       -> C-ii (displaced)
function runDiagScenario()
{
    const KEY = { ESC: 27, ENTER: 13, DOWN: 0xaf, UP: 0xad, RIGHT: 0xae, FIRE: 0xa3, USE: 0xa2 };
    const pump = (n) => { for (let i = 0; i < n; i++) { woom24_frame(0); } };
    const press = (key) => { woom24_push_key(true, key); woom24_push_key(false, key); };

    // Warmup: title screen -> demo 1 running (gameplay sprites on screen).
    pump(400);
    console.log("[diag] warmup done, gametic =", woom24_probe_gametic());

    // Menu script (shareware flow): ESC -> New Game -> Episode 1 ->
    // skill cursor (enters at index 0) DOWN x3 = Ultra-Violence -> start.
    press(KEY.ESC);   pump(30);
    press(KEY.ENTER); pump(30);   // New Game -> "Which episode?"
    press(KEY.ENTER); pump(30);   // Episode 1 -> skill menu at index 0
    for (let i = 0; i < 3; i++) { press(KEY.DOWN); pump(10); }
    press(KEY.ENTER); pump(60);   // Ultra-Violence -> deferred InitNew runs

    // Movement: hold UPARROW (press without release, re-asserted below) and
    // sweep RIGHTARROW so the player cannot stay pinned facing a wall at
    // spawn - a static scene degenerates every prev/curr pair to identity
    // and the board never interpolates anything (first run proved that).
    press(KEY.UP);
    for (let f = 0; f < 4000; f++)
    {
        woom24_frame(0);
        if (f % 300 === 0) { woom24_push_key(true, KEY.UP); }   // re-assert hold
        if (f % 100 < 25) { woom24_push_key(true, KEY.RIGHT); } // turn sweep
        else { woom24_push_key(false, KEY.RIGHT); }
        if (f % 200 === 100) { press(KEY.FIRE); }
        if (f % 250 === 125) { press(KEY.USE); }
        if (f % 100 === 0)
        {
            const diag = woom24_diag_frame();
            const header = diag.split("tiles=")[0];
            const tiles = diag.split("tiles=")[1] ?? "";
            const field = (name) => header.split(`${name}=`)[1]?.split(";")[0] ?? "?";
            const [maxD2, movedD2] = field("mo2").split("/").map(Number);
            const camD2 = Number(field("cam2"));
            console.log(`[diag] frame ${f}: ${header}`);
            // Forced-half-fraction census: MAXMOVE caps each AXIS of a
            // legitimate per-tic move at 30 map units, so at half fraction a
            // legitimate EUCLIDEAN diagonal still reaches sqrt(15^2+15^2) ~
            // 21.2 u (~1.39 M fixed) -- a scalar 15-u bound would flag
            // healthy diagonals (final whole-branch review). The detection
            // threshold is therefore the teleport-guard bound: r_interp
            // falls back to live values past 2 x MAXMOVE = 60 u = 3932160
            // fixed (the same bound sprite_apply_regression.rs pins as
            // ALIASING_BOUND_FIXED) -- beyond it only a stale-slot alias can
            // be the source.
            if (maxD2 > 3932160 || camD2 > 3932160)
            {
                console.log(`[diag] BOARD-DISPLACEMENT DETECTED (forced-half-fraction: mo2 max ${maxD2} fixed = ${(maxD2 / 65536).toFixed(2)}u, moved ${movedD2}, cam2 ${camD2} = ${(camD2 / 65536).toFixed(2)}u):`);
                console.log(tiles);
            }
            else if (f % 1000 === 0)
            {
                console.log(`[diag] baseline tile map at frame ${f}:`);
                console.log(tiles);
            }
        }
    }
    console.log("[diag] scenario complete; gametic =", woom24_probe_gametic());
}

let ticks = 0;
if (MODE === "diag")
{
    // Wrap the scenario so a mid-scenario wasm trap still reaches the
    // last-error verdict below before the non-zero exit (same shape as the
    // plain loop's catch).
    try
    {
        runDiagScenario();
    }
    catch (e)
    {
        console.log("[diag] frame threw:", String(e));
        console.log("[diag] gametic at throw =", woom24_probe_gametic?.() ?? "?");
        process.exitCode = 1;
    }
}
else
try
{
    for (; ticks < TICKS; ticks++)
    {
        woom24_tick();
        if (MODE !== "" && ticks % 100 === 0)
        {
            console.log("[harness] progress tick", ticks, "gametic =", woom24_probe_gametic(),
              "ticdup =", woom24_probe_ticdup());
        }
    }
    console.log("[harness] SURVIVED", ticks, "ticks - freeze NOT reproduced headless",
      MODE === "" ? "(note: frozen clock keeps TryRunTics parked)" : "");
}
catch (e)
{
    console.log("[harness] tick", ticks, "threw:", String(e));
    console.log("[harness] gametic at throw =", woom24_probe_gametic?.() ?? "?");
    console.log("[harness] ^ the [woom24 panic] line above carries the exact file:line;");
    console.log("[harness] ^ the last [div-probe] canary change carries the clobber fingerprint.");
}

// Fix E harness hook: the engine's last I_Error message. A healthy run ends
// with null; a crash run prints the engine's own fatal text instead of just
// the wasm trap string.
const lastError = woom24_last_error?.() ?? null;
if (lastError === null)
{
    console.log("[harness] woom24_last_error = null (healthy: no fatal I_Error fired)");
}
else
{
    console.log("[harness] woom24_last_error =", lastError);
    console.log("[harness] FAILED: last-error channel must be null on a healthy smoke run");
    process.exitCode = 1;
}
