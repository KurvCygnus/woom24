// run-scenarios.mjs - F9 Layer-A scenario driver (wasm side).
//
// Usage (repo root):
//   node shells/web/scripts/run-scenarios.mjs <scenario.json> [--bless] [--pkg <dir>]
//     [--verbose] [--dump-frame <anchor>] [--dump-sequence <anchor>[:count[:stride]]]
//     [--allow-fail <scenario>]
//
// Consumes a scenario manifest and drives the wasm engine headless through
// its steps, producing the ledger (one JSON object per anchor) at
// shells/web/scripts/scenarios/out/wasm-<name>.jsonl. With --bless the ledger
// becomes the golden at shells/web/scripts/scenarios/goldens/<name>.json;
// otherwise the run is compared against that golden and exits 1 on any
// mismatch or on any console error-level output (gate 3).
//
// Task-4 debug/exit-code aids (all out-of-band: they change no step
// semantics, so the schema below stays the host twin's contract):
// - --dump-frame <anchor> writes the present buffer of that anchor as a PPM
//   (P6, BGRA->RGB) to scenarios/out/<scenario>-<anchor>.ppm. A debugging aid
//   for human eyes (defect-C sprite evidence), never part of any gate.
// - --dump-sequence <anchor>[:count[:stride]] (F9 spec §4, approved
//   2026-09-25) extends --dump-frame into a temporal strip: starting at the
//   named anchor, writes `count` PPM frames (default 8) with `stride`
//   simulation tics between renders (default 4) into
//   scenarios/out/seq-<anchor>/. Renders under the anchor's interp-off
//   discipline (one woom24_frame per stride tick), filenames carry the live
//   gametic. Model-readable menu/cursor/wipe debugging - never a gate.
// - --allow-fail <scenario> (repeatable; env SCENARIO_ALLOW_FAIL=csv also
//   honored): if the named scenario fails at run time (engine exception,
//   failed assert, console-error gate, golden mismatch) exit 0 with an
//   [expected-fail] record instead of exit 1. Never combines with --bless
//   semantics: a failed run writes no golden. Setup errors (bad manifest,
//   missing pkg) still exit 1.
//
// The manifest schema and step semantics are NORMATIVE (F9 task 3 brief):
// the browser-side host twin parses exactly this schema, so any change here
// is a cross-target contract change. M2 adds three step kinds and one anchor
// flag (all additive, cross-target-relevant, keep the twin in lockstep):
// - {"assert_stats": {field: value, ...}} - subset-exact match against the
//   harness_audio_stats() JSON object.
// - {"console_contains": {"channel": "warn"|"error", "text": ...}} -
//   substring search over that channel's captured lines (so far).
// - {"expect_state_at_load": "<anchor>"} - current harness_state_hash_load()
//   must equal the named anchor's recorded state_hash_load.
// - anchor flag "state_hash_load": true - record the load-comparable digest
//   (gametic + rndindex + prndindex zeroed; see harness_hash::state_hash_load)
//   on the anchor line.
// - manifest "name" must equal the manifest file's stem: ledger and golden
//   keys derive from the name (wasm-<name>.jsonl / <name>.json), so a
//   misnamed manifest must fail loudly instead of blessing under the wrong
//   key - validateManifest gates the stem, the host twin gates the same
//   identity in run_selected_scenario.
//
// Requires the pkg built with --features harness:
//   cargo build -p room-shell-web --target wasm32-unknown-unknown --release --features harness
//   wasm-bindgen --target web --out-dir target/pkg-harness --no-typescript \
//     target/wasm32-unknown-unknown/release/room_shell_web.wasm
// Default pkg dir is <repo root>/target/pkg-harness (--pkg overrides). This is
// deliberately NOT shells/web/www/pkg: that is the production pkg and must
// never be overwritten with a harness build.
//
// Determinism strategy (mirrored by the host twin - keep in lockstep):
// - Pacing hooks arm BEFORE standard_start: the boot's inline first
//   doomgeneric_Tick runs TryRunTics, which never escapes a frozen clock
//   (same doctrine as node-tick-smoke.mjs). The fake clock arms with the
//   manifest's boot_ms seed (boot.clock_armed), feeding real-time paths
//   (wipe pacing, blinks); it never reaches simulation state.
// - All engine pumping runs under the singletics probe hook (exactly one tic
//   per woom24_tick call). Under plain clock_armed pumping TryRunTics catches
//   up a wall-time-dependent number of tics per call, so a wait_gametic
//   target could be overshot and anchors would land on wall-sensitive tics.
//   Singletics pins wait-loop exits to gametic == N exactly, and pins the
//   anchor frame's catch-up pump to exactly MAX_TICS_PER_FRAME (4) tics.
// - Anchor order is state first, frame second: state_hash must digest the
//   state at the waited gametic; the frame call advances the sim by its
//   4-tic pump before presenting. Ledger `gametic` is the anchor-start value.
// - Hash normalization: wasm u64 exports surface as JS BigInt; every hash is
//   normalized to lowercase "0x" + 16 zero-padded hex digits, everywhere
//   (ledger, goldens, comparison).
//
// Console channel (gate 3): the engine's log facade routes through the
// shell's ConsoleLogger (web_sys::console::error_1/warn_1/info_1/log_1).
// In Node (with the window shim below) those resolve against globalThis.console
// at call time, so patching console.error before the glue import intercepts
// engine errors; boot's info-level lines landing on the patched console.info
// empirically confirm the routing (node-tick-smoke.mjs saw I_Error text on
// the same path). console.error is the only gated channel; panics surface
// additionally as JS exceptions from the export calls and fail the run too.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
// Script lives at <repo root>/shells/web/scripts/ - three levels up is the root.
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_PKG = path.join(REPO_ROOT, "target", "pkg-harness");
const OUT_DIR = path.join(SCRIPT_DIR, "scenarios", "out");
const GOLDEN_DIR = path.join(SCRIPT_DIR, "scenarios", "goldens");

// Safety cap per wait step: a healthy boot reaches 980 tics in ~1-2k calls;
// 100k means the engine stalled (I_Error/panic land as exceptions before this).
const MAX_TICKS_PER_WAIT = 100000;

// now_ms for anchor frames: the fake clock dominates (clock.rs engine_ms
// ignores the rAF timestamp while armed), so any value works.
const ANCHOR_FRAME_NOW_MS = 1000;

// ---------------------------------------------------------------- CLI + util

const args = process.argv.slice(2);
const bless = args.includes("--bless");
const verbose = args.includes("--verbose");
const pkgFlagIdx = args.indexOf("--pkg");
const dumpFlagIdx = args.indexOf("--dump-frame");
const seqFlagIdx = args.indexOf("--dump-sequence");
const allowFailFlagIdxs = args.reduce((acc, a, i) => { if (a === "--allow-fail") { acc.push(i); } return acc; }, []);
// Value positions of every value-taking flag, so the bare scenario path is
// never confused with a flag argument.
const flagValueIdxs = new Set();
if (pkgFlagIdx >= 0) { flagValueIdxs.add(pkgFlagIdx + 1); }
if (dumpFlagIdx >= 0) { flagValueIdxs.add(dumpFlagIdx + 1); }
if (seqFlagIdx >= 0) { flagValueIdxs.add(seqFlagIdx + 1); }
for (const i of allowFailFlagIdxs) { flagValueIdxs.add(i + 1); }
const pkgDir = path.resolve(pkgFlagIdx >= 0 ? args[pkgFlagIdx + 1] : DEFAULT_PKG);
const dumpAnchor = dumpFlagIdx >= 0 ? args[dumpFlagIdx + 1] : null;
// --dump-sequence <anchor>[:count[:stride]] (spec F9 §4): defaults mirror the
// spec - 8 frames, one render every 4 tics. Parsed here, validated just after
// fail() exists (setup error, not a run failure: a bad flag says nothing
// about the engine).
const seqSpec = (() =>
{
    if (seqFlagIdx < 0) { return null; }
    const raw = args[seqFlagIdx + 1] ?? "";
    const [anchor, countRaw, strideRaw] = raw.split(":");
    const count = countRaw === undefined ? 8 : Number(countRaw);
    const stride = strideRaw === undefined ? 4 : Number(strideRaw);
    return { raw, anchor, count, stride };
})();
const allowFail = new Set(allowFailFlagIdxs.map((i) => args[i + 1]));
if (process.env.SCENARIO_ALLOW_FAIL)
{
    for (const s of process.env.SCENARIO_ALLOW_FAIL.split(/[,\s]+/).filter(Boolean)) { allowFail.add(s); }
}
const scenarioArg = args.find((a, i) => !a.startsWith("--") && !flagValueIdxs.has(i));
if (!scenarioArg)
{
    console.error("usage: node run-scenarios.mjs <scenario.json> [--bless] [--pkg <dir>] [--verbose] [--dump-frame <anchor>] [--dump-sequence <anchor>[:count[:stride]]] [--allow-fail <scenario>]");
    process.exit(2);
}
// CLI paths resolve against cwd first, then against the script dir (so a bare
// scenario name works from any directory).
let scenarioPath = path.resolve(scenarioArg);
if (!fs.existsSync(scenarioPath))
{
    scenarioPath = path.resolve(SCRIPT_DIR, scenarioArg);
}

const origError = console.error.bind(console);
function fail(msg)
{
    origError(`[scenario-fail] ${msg}`);
    process.exit(1);
}

// --dump-sequence validation (see the parse site above): anchor non-empty,
// count/stride positive integers.
if (seqSpec !== null)
{
    if (!seqSpec.anchor || !Number.isInteger(seqSpec.count) || seqSpec.count < 1
        || !Number.isInteger(seqSpec.stride) || seqSpec.stride < 1)
    {
        fail(`--dump-sequence spec ${JSON.stringify(seqSpec.raw)} invalid: expected <anchor>[:count[:stride]] with count/stride >= 1`);
    }
}

// Run-phase failure (engine exception, failed assert, console gate, golden
// mismatch): under --allow-fail for this scenario, record and exit 0 instead.
// Never reaches a golden write: both callers sit on paths that skip bless.
function runFail(msg)
{
    if (allowFail.has(manifest.name))
    {
        origError(`[expected-fail] ${manifest.name}: ${msg}`);
        console.log(`[runner] EXPECTED-FAIL: ${manifest.name} (allowed via --allow-fail) - failure recorded, NO golden written`);
        process.exit(0);
    }
    fail(msg);
}

function gitHead()
{
    try
    {
        return execFileSync("git", ["rev-parse", "HEAD"], { cwd: REPO_ROOT, encoding: "utf8" }).trim();
    }
    catch
    {
        return "unknown";
    }
}

// Canonical hash form: "0x" + 16 lowercase hex digits (wasm u64 -> BigInt).
function toHex(v)
{
    if (typeof v !== "bigint")
    {
        fail(`hash export returned ${typeof v}, expected BigInt (u64 wasm export)`);
    }
    return "0x" + BigInt.asUintN(64, v).toString(16).padStart(16, "0");
}

// ------------------------------------------------------- console gate (No. 3)

let consoleErrors = 0;
let consoleWarns = 0;
let consoleInfos = 0;
const errorLines = [];
const warnLines = [];
const origErrFn = console.error.bind(console);
const origWarnFn = console.warn.bind(console);
const origInfoFn = console.info.bind(console);
console.error = (...a) =>
{
    consoleErrors += 1;
    errorLines.push(a.map(String).join(" "));
    origErrFn("[captured-error]", ...a);
};
console.warn = (...a) =>
{
    consoleWarns += 1;
    warnLines.push(a.map(String).join(" "));
    origWarnFn(...a);
};
console.info = (...a) => { consoleInfos += 1; origInfoFn(...a); };

// ------------------------------------------------------------- manifest load

if (!fs.existsSync(scenarioPath))
{
    fail(`scenario manifest not found: ${scenarioArg}`);
}
const manifest = JSON.parse(fs.readFileSync(scenarioPath, "utf8"));

// Strict manifest validation: the schema is the cross-target contract, so
// unknown shapes fail loudly instead of being silently ignored.
function validateManifest(m)
{
    const where = `manifest ${path.basename(scenarioPath)}`;
    for (const key of ["name", "iwad", "sf2", "pwads", "boot", "steps"])
    {
        if (!(key in m)) { fail(`${where}: missing "${key}"`); }
    }
    for (const key of Object.keys(m))
    {
        if (!["name", "iwad", "sf2", "pwads", "boot", "steps"].includes(key))
        {
            fail(`${where}: unknown key "${key}"`);
        }
    }
    if (typeof m.name !== "string" || m.name === "") { fail(`${where}: "name" must be a non-empty string`); }
    // Stem gate: ledger and golden keys derive from "name", so a misnamed
    // manifest must fail instead of blessing its golden under the wrong key
    // (the host twin enforces the same identity against its file stem).
    const stem = path.basename(scenarioPath, path.extname(scenarioPath));
    if (m.name !== stem) { fail(`manifest name "${m.name}" does not match file stem "${stem}"`); }
    if (typeof m.iwad !== "string" || m.iwad === "") { fail(`${where}: "iwad" must be a non-empty string`); }
    if (m.sf2 !== null && (typeof m.sf2 !== "string" || m.sf2 === "")) { fail(`${where}: "sf2" must be null or a non-empty string`); }
    if (!Array.isArray(m.pwads) || m.pwads.some((p) => typeof p !== "string")) { fail(`${where}: "pwads" must be an array of strings`); }
    const boot = m.boot;
    if (typeof boot !== "object" || boot === null) { fail(`${where}: "boot" must be an object`); }
    if (boot.mode !== "standard") { fail(`${where}: "boot.mode" must be "standard" (scenarios drive the standard entry)`); }
    if (!Array.isArray(boot.args)) { fail(`${where}: "boot.args" must be an array`); }
    for (const a of boot.args)
    {
        if (typeof a !== "string") { fail(`${where}: "boot.args" entries must be strings`); }
        if (/\s/.test(a)) { fail(`${where}: "boot.args" entry ${JSON.stringify(a)} contains whitespace - engineArgs must be pre-split into single argv tokens`); }
    }
    if (typeof boot.clock_armed !== "boolean") { fail(`${where}: "boot.clock_armed" must be a boolean`); }
    if (typeof boot.boot_ms !== "number" || !Number.isFinite(boot.boot_ms) || boot.boot_ms < 0) { fail(`${where}: "boot.boot_ms" must be a non-negative number`); }
    if (!Array.isArray(m.steps) || m.steps.length === 0) { fail(`${where}: "steps" must be a non-empty array`); }
    for (const [i, step] of m.steps.entries())
    {
        const stepTypes = ["wait_gametic", "anchor", "key", "assert_gametic_gt", "assert_stats", "console_contains", "expect_state_at_load"].filter((t) => t in step);
        if (stepTypes.length !== 1)
        {
            fail(`${where}: step ${i} must have exactly one of "wait_gametic", "anchor", "key", "assert_gametic_gt", "assert_stats", "console_contains", "expect_state_at_load"`);
        }
        const stepType = stepTypes[0];
        if (stepType === "wait_gametic")
        {
            if (!Number.isInteger(step.wait_gametic) || step.wait_gametic < 0)
            {
                fail(`${where}: step ${i} "wait_gametic" must be a non-negative integer`);
            }
        }
        else if (stepType === "anchor")
        {
            if (typeof step.anchor !== "string" || step.anchor === "") { fail(`${where}: step ${i} "anchor" must be a non-empty string`); }
            for (const key of Object.keys(step))
            {
                if (!["anchor", "state_hash", "frame_hash", "state_hash_load"].includes(key)) { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            if ("state_hash" in step && typeof step.state_hash !== "boolean") { fail(`${where}: step ${i} "state_hash" must be a boolean`); }
            if ("frame_hash" in step && typeof step.frame_hash !== "boolean") { fail(`${where}: step ${i} "frame_hash" must be a boolean`); }
            if ("state_hash_load" in step && typeof step.state_hash_load !== "boolean") { fail(`${where}: step ${i} "state_hash_load" must be a boolean`); }
        }
        else if (stepType === "key")
        {
            // {"key": {"code": <doomkey 0-255>, "pressed": <bool>}} - the two
            // fields map 1:1 onto woom24_push_key's parameters.
            for (const key of Object.keys(step))
            {
                if (key !== "key") { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            const k = step.key;
            if (typeof k !== "object" || k === null || Array.isArray(k)) { fail(`${where}: step ${i} "key" must be an object`); }
            if (!Number.isInteger(k.code) || k.code < 0 || k.code > 255) { fail(`${where}: step ${i} "key.code" must be an integer 0-255 (doomkey value)`); }
            if (typeof k.pressed !== "boolean") { fail(`${where}: step ${i} "key.pressed" must be a boolean`); }
            for (const key of Object.keys(k))
            {
                if (!["code", "pressed"].includes(key)) { fail(`${where}: step ${i} "key" has unknown field "${key}"`); }
            }
        }
        else if (stepType === "assert_stats")
        {
            // {"assert_stats": {"music_active": <bool>, "underruns": <int>,
            // "voices": <int>, "scheduled_seconds": <num>}} - subset match
            // against the harness_audio_stats() JSON object; only the fields
            // present in the step are asserted (exact equality).
            for (const key of Object.keys(step))
            {
                if (key !== "assert_stats") { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            const a = step.assert_stats;
            if (typeof a !== "object" || a === null || Array.isArray(a)) { fail(`${where}: step ${i} "assert_stats" must be an object`); }
            const known = ["music_active", "underruns", "voices", "scheduled_seconds"];
            for (const key of Object.keys(a))
            {
                if (!known.includes(key)) { fail(`${where}: step ${i} "assert_stats" has unknown field "${key}" (known: ${known.join(", ")})`); }
                if (key === "music_active" && typeof a[key] !== "boolean") { fail(`${where}: step ${i} "assert_stats.music_active" must be a boolean`); }
                if ((key === "underruns" || key === "voices") && (!Number.isInteger(a[key]) || a[key] < 0)) { fail(`${where}: step ${i} "assert_stats.${key}" must be a non-negative integer`); }
                if (key === "scheduled_seconds" && (typeof a[key] !== "number" || !Number.isFinite(a[key]))) { fail(`${where}: step ${i} "assert_stats.scheduled_seconds" must be a finite number`); }
            }
            if (Object.keys(a).length === 0) { fail(`${where}: step ${i} "assert_stats" must assert at least one field`); }
        }
        else if (stepType === "console_contains")
        {
            // {"console_contains": {"channel": "warn"|"error", "text": ...}}
            // - fails the run when no captured console line of that channel
            // (up to and including this step) contains `text` as a substring.
            for (const key of Object.keys(step))
            {
                if (key !== "console_contains") { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            const c = step.console_contains;
            if (typeof c !== "object" || c === null || Array.isArray(c)) { fail(`${where}: step ${i} "console_contains" must be an object`); }
            for (const key of Object.keys(c))
            {
                if (!["channel", "text"].includes(key)) { fail(`${where}: step ${i} "console_contains" has unknown field "${key}"`); }
            }
            if (c.channel !== "warn" && c.channel !== "error") { fail(`${where}: step ${i} "console_contains.channel" must be "warn" or "error"`); }
            if (typeof c.text !== "string" || c.text === "") { fail(`${where}: step ${i} "console_contains.text" must be a non-empty string`); }
        }
        else if (stepType === "expect_state_at_load")
        {
            // {"expect_state_at_load": "<anchor>"} - compares the CURRENT
            // harness_state_hash_load() against the named anchor's recorded
            // state_hash_load (the anchor step must carry
            // "state_hash_load": true). Resolved from the in-memory ledger.
            for (const key of Object.keys(step))
            {
                if (key !== "expect_state_at_load") { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            if (typeof step.expect_state_at_load !== "string" || step.expect_state_at_load === "")
            {
                fail(`${where}: step ${i} "expect_state_at_load" must be a non-empty anchor name`);
            }
        }
        else
        {
            // {"assert_gametic_gt": N} - fails the run when the engine has
            // not reached tic N yet (menu-scripting progression pin).
            for (const key of Object.keys(step))
            {
                if (key !== "assert_gametic_gt") { fail(`${where}: step ${i} has unknown key "${key}"`); }
            }
            if (!Number.isInteger(step.assert_gametic_gt) || step.assert_gametic_gt < 0)
            {
                fail(`${where}: step ${i} "assert_gametic_gt" must be a non-negative integer`);
            }
        }
    }
}
validateManifest(manifest);

// --------------------------------------------------------------- wasm boot

// Clock fix (div-probe doctrine, see node-tick-smoke.mjs): web_sys::window()
// resolves the `window` global, which Node lacks. Point `window` at
// globalThis BEFORE the glue import (ESM imports hoist, so the glue comes in
// via a dynamic import below).
globalThis.window = globalThis;

const gluePath = path.join(pkgDir, "room_shell_web.js");
const wasmPath = path.join(pkgDir, "room_shell_web_bg.wasm");
for (const p of [gluePath, wasmPath])
{
    if (!fs.existsSync(p))
    {
        fail(`pkg artifact missing: ${p} - build the harness pkg first (see header comment)`);
    }
}

const glue = await import(pathToFileURL(gluePath).href);

const REQUIRED_EXPORTS = [
    "woom24_version",
    "woom24_register_file",
    "woom24_standard_start",
    "woom24_tick",
    "woom24_frame",
    "woom24_push_key",
    "woom24_probe_gametic",
    "woom24_probe_set_clock_armed",
    "woom24_probe_set_singletics",
    "harness_frame_hash",
    "harness_state_hash",
    "harness_state_hash_load",
    "harness_set_interp_enabled",
    "harness_audio_stats",
];
for (const name of REQUIRED_EXPORTS)
{
    if (typeof glue[name] !== "function")
    {
        fail(`pkg glue missing export "${name}" - was the pkg built with --features harness? (${gluePath})`);
    }
}
const {
    woom24_version,
    woom24_register_file,
    woom24_standard_start,
    woom24_tick,
    woom24_frame,
    woom24_push_key,
    woom24_probe_gametic,
    woom24_probe_set_clock_armed,
    woom24_probe_set_singletics,
    harness_frame_hash,
    harness_state_hash,
    harness_state_hash_load,
    harness_set_interp_enabled,
    harness_audio_stats,
} = glue;

await glue.default(fs.readFileSync(wasmPath));
console.log(`[runner] wasm online, version = ${woom24_version()}, pkg = ${pkgDir}`);

function registerFromRoot(name)
{
    const p = path.join(REPO_ROOT, name);
    if (!fs.existsSync(p))
    {
        fail(`file not found for VFS registration: ${p}`);
    }
    const bytes = fs.readFileSync(p);
    woom24_register_file(name, bytes);
    console.log(`[runner] registered ${name}, ${bytes.length} bytes`);
}

registerFromRoot(manifest.iwad);
for (const pwad of manifest.pwads) { registerFromRoot(pwad); }
// SF2: register only when the file exists on disk. A named-but-absent SF2 is
// the flow-4 negative contract (defect B): the name still flows into the
// boot profile, but nothing is registered, faithfully modeling "an SF2 the
// engine cannot see" -- i_sound's exists() gate never consults the VFS.
if (manifest.sf2 !== null)
{
    if (fs.existsSync(path.join(REPO_ROOT, manifest.sf2)))
    {
        registerFromRoot(manifest.sf2);
    }
    else
    {
        console.log(`[runner] sf2 "${manifest.sf2}" not found on disk - registering nothing (negative-contract scenario)`);
    }
}

// Install the engine console logger via attach_canvas side effect (a stub
// canvas fails presenter selection AFTER logger install - expected: caught;
// hashing targets the engine's own DG_ScreenBuffer, not the canvas).
try { glue.woom24_attach_canvas({ getContext: () => null, style: {} }); }
catch (e) { console.log(`[runner] attach_canvas expected failure: ${String(e).slice(0, 80)}`); }

// Pacing hooks arm BEFORE standard_start (pre-boot doctrine, see header).
if (manifest.boot.clock_armed)
{
    woom24_probe_set_clock_armed(1, manifest.boot.boot_ms);
    console.log(`[runner] fake clock armed, seeded ${manifest.boot.boot_ms} ms (pre-boot)`);
}
// Singletics: exactly one tic per tick call - the determinism backbone of the
// wait loops and anchor frames (see header "Determinism strategy").
woom24_probe_set_singletics(1);
console.log("[runner] singletics armed: one tic per woom24_tick (pre-boot)");

// Standard entry: complete boot profile -> straight into the game. Field
// casing follows shells/web/src/profile.rs (maxRenderRes / engineArgs); the
// manifest does not carry a render cap, so maxRenderRes stays null.
const profileJson = JSON.stringify({
    iwad: manifest.iwad,
    pwads: manifest.pwads,
    sf2: manifest.sf2,
    maxRenderRes: null,
    engineArgs: manifest.boot.args,
});
try
{
    woom24_standard_start(profileJson);
}
catch (e)
{
    runFail(`standard_start threw: ${String(e)}`);
}
console.log(`[runner] standard start issued for "${manifest.name}", gametic = ${woom24_probe_gametic()}`);

// ------------------------------------------------------------ step executor

const ledger = [];

function waitGametic(target)
{
    let calls = 0;
    while (woom24_probe_gametic() < target)
    {
        woom24_tick();
        calls += 1;
        if (calls > MAX_TICKS_PER_WAIT)
        {
            fail(`wait_gametic ${target}: exceeded ${MAX_TICKS_PER_WAIT} tick calls (gametic = ${woom24_probe_gametic()}) - engine stalled`);
        }
    }
    if (verbose) { console.log(`[runner] wait_gametic ${target} reached at gametic ${woom24_probe_gametic()} after ${calls} tick calls`); }
}

function anchor(step)
{
    // State first: digest the state at the waited gametic. The frame call
    // below advances the sim by its 4-tic pump cap before presenting.
    const line = {
        scenario: manifest.name,
        anchor: step.anchor,
        gametic: woom24_probe_gametic(),
        state_hash: null,
        frame_hash: null,
    };
    if (step.state_hash === true)
    {
        line.state_hash = toHex(harness_state_hash());
    }
    if (step.state_hash_load === true)
    {
        // Load-comparable digest (savegame-unsaved words zeroed): the field
        // the expect_state_at_load step compares against after the load.
        line.state_hash_load = toHex(harness_state_hash_load());
    }
    if (step.frame_hash === true)
    {
        harness_set_interp_enabled(0);
        woom24_frame(ANCHOR_FRAME_NOW_MS); // pumps exactly 4 tics (cap) under singletics, then presents
        line.frame_hash = toHex(harness_frame_hash());
        harness_set_interp_enabled(1);
    }
    ledger.push(line);
    if (verbose)
    {
        console.log(`[runner] anchor ${line.anchor}: gametic = ${line.gametic} state = ${line.state_hash} load = ${line.state_hash_load} frame = ${line.frame_hash}`);
    }
    dumpFramePpm(step.anchor);
    dumpSequencePpm(step.anchor);
}

// --dump-frame <anchor>: after that anchor's ledger line is taken, write the
// live present buffer as a P6 PPM (BGRA -> RGB swizzle) for human inspection.
// Pure debugging aid: it feeds no gate, no hash, no golden.
let dumpedAnchor = null;
function dumpFramePpm(anchorName)
{
    if (dumpAnchor !== anchorName || dumpedAnchor === anchorName) { return; }
    dumpedAnchor = anchorName;
    const bgra = glue.harness_screen_bytes();
    const res = glue.harness_screen_res();
    const w = res >>> 16;
    const h = res & 0xffff;
    if (bgra.length === 0 || w === 0 || h === 0)
    {
        origError(`[runner] --dump-frame ${anchorName}: present buffer empty (w=${w} h=${h}) - no PPM written`);
        return;
    }
    const rgb = Buffer.alloc(w * h * 3);
    for (let p = 0; p < w * h; p += 1)
    {
        rgb[p * 3] = bgra[p * 4 + 2]; // R
        rgb[p * 3 + 1] = bgra[p * 4 + 1]; // G
        rgb[p * 3 + 2] = bgra[p * 4]; // B
    }
    const ppmPath = path.join(OUT_DIR, `${manifest.name}-${anchorName}.ppm`);
    fs.mkdirSync(OUT_DIR, { recursive: true });
    fs.writeFileSync(ppmPath, Buffer.concat([Buffer.from(`P6\n${w} ${h}\n255\n`), rgb]));
    console.log(`[runner] dumped frame ${anchorName} (${w}x${h} PPM) -> ${path.relative(REPO_ROOT, ppmPath)}`);
}

// --dump-sequence (spec F9 §4): starting at the named anchor, render `count`
// frames under the anchor's interp-off discipline (one woom24_frame render
// per stride tics - the frame call itself pumps its standard catch-up cap,
// exactly like the anchor frame), writing each present buffer as a P6 PPM
// into scenarios/out/seq-<anchor>/. Filenames carry the frame index and the
// live gametic at render time so the strip is self-describing in a review.
// Pure debugging aid: feeds no gate, no hash, no golden.
let seqArmed = false;
function dumpSequencePpm(anchorName)
{
    if (seqSpec === null || seqSpec.anchor !== anchorName) { return; }
    seqArmed = true;
    const dir = path.join(OUT_DIR, `seq-${anchorName}`);
    fs.mkdirSync(dir, { recursive: true });
    harness_set_interp_enabled(0);
    for (let i = 0; i < seqSpec.count; i += 1)
    {
        woom24_frame(ANCHOR_FRAME_NOW_MS);
        const g = woom24_probe_gametic();
        const bgra = glue.harness_screen_bytes();
        const res = glue.harness_screen_res();
        const w = res >>> 16;
        const h = res & 0xffff;
        if (bgra.length === 0 || w === 0 || h === 0)
        {
            origError(`[runner] --dump-sequence ${anchorName}: present buffer empty at frame ${i} (w=${w} h=${h}) - frame skipped`);
            continue;
        }
        const rgb = Buffer.alloc(w * h * 3);
        for (let p = 0; p < w * h; p += 1)
        {
            rgb[p * 3] = bgra[p * 4 + 2]; // R
            rgb[p * 3 + 1] = bgra[p * 4 + 1]; // G
            rgb[p * 3 + 2] = bgra[p * 4]; // B
        }
        const name = `f${String(i).padStart(2, "0")}-g${String(g).padStart(6, "0")}.ppm`;
        fs.writeFileSync(path.join(dir, name), Buffer.concat([Buffer.from(`P6\n${w} ${h}\n255\n`), rgb]));
    }
    harness_set_interp_enabled(1);
    console.log(`[runner] dumped sequence ${anchorName} (${seqSpec.count} frames, stride ${seqSpec.stride} tics) -> ${path.relative(REPO_ROOT, dir)}/`);
}

function keyStep(k)
{
    woom24_push_key(k.pressed, k.code);
    if (verbose) { console.log(`[runner] key ${k.pressed ? "down" : "up"} code=${k.code} at gametic ${woom24_probe_gametic()}`); }
}

function assertGameticGt(n)
{
    const t = woom24_probe_gametic();
    if (t <= n)
    {
        runFail(`assert_gametic_gt ${n} failed: gametic = ${t}`);
    }
    if (verbose) { console.log(`[runner] assert_gametic_gt ${n} ok (gametic = ${t})`); }
}

// {"assert_stats": {...}}: parse the harness_audio_stats() JSON and compare
// every field named in the step (exact equality; subset match).
function assertStats(expected)
{
    let stats;
    try
    {
        stats = JSON.parse(harness_audio_stats());
    }
    catch (e)
    {
        runFail(`assert_stats: harness_audio_stats() is not valid JSON: ${String(e).slice(0, 120)}`);
    }
    const problems = [];
    for (const [field, want] of Object.entries(expected))
    {
        const got = stats[field];
        if (got !== want)
        {
            problems.push(`${field}: got ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`);
        }
    }
    if (problems.length > 0)
    {
        runFail(`assert_stats failed (stats = ${harness_audio_stats()}): ${problems.join(" | ")}`);
    }
    if (verbose) { console.log(`[runner] assert_stats ok: ${harness_audio_stats()}`); }
}

// {"console_contains": {...}}: search the captured lines of one channel
// (accumulated up to and including this step - boot-time warnings land
// before the first step) for a substring.
function consoleContains(c)
{
    const lines = c.channel === "error" ? errorLines : warnLines;
    if (!lines.some((l) => l.includes(c.text)))
    {
        runFail(`console_contains failed: no ${c.channel} line contains ${JSON.stringify(c.text)} (captured ${lines.length} ${c.channel} line(s))`);
    }
    if (verbose) { console.log(`[runner] console_contains ok (${c.channel}): ${JSON.stringify(c.text)}`); }
}

// {"expect_state_at_load": "<anchor>"}: the save/load roundtrip oracle.
// Compares the CURRENT harness_state_hash_load() (the digest with the
// savegame-unsaved words zeroed) against the named anchor's recorded
// state_hash_load from this run's in-memory ledger.
function expectStateAtLoad(anchorName)
{
    const ref = ledger.find((l) => l.anchor === anchorName);
    if (!ref)
    {
        runFail(`expect_state_at_load: anchor "${anchorName}" not in this run's ledger`);
    }
    if (typeof ref.state_hash_load !== "string")
    {
        runFail(`expect_state_at_load: anchor "${anchorName}" has no state_hash_load - add "state_hash_load": true to that anchor step`);
    }
    const now = toHex(harness_state_hash_load());
    if (now !== ref.state_hash_load)
    {
        runFail(`expect_state_at_load "${anchorName}" failed: load digest ${now} vs anchor ${ref.state_hash_load}`);
    }
    if (verbose)
    {
        console.log(`[runner] expect_state_at_load ok: current load digest ${now} == anchor "${anchorName}" (${ref.state_hash_load})`);
    }
}

// A panic inside an export surfaces as a JS exception (or, under panic=abort,
// as a trap that rejects the same way): the run-fail path records it.
try
{
    for (const [i, step] of manifest.steps.entries())
    {
        if ("wait_gametic" in step) { waitGametic(step.wait_gametic); }
        else if ("key" in step) { keyStep(step.key); }
        else if ("assert_gametic_gt" in step) { assertGameticGt(step.assert_gametic_gt); }
        else if ("assert_stats" in step) { assertStats(step.assert_stats); }
        else if ("console_contains" in step) { consoleContains(step.console_contains); }
        else if ("expect_state_at_load" in step) { expectStateAtLoad(step.expect_state_at_load); }
        else { anchor(step); }
    }
}
catch (e)
{
    runFail(`engine exception during steps: ${String(e).slice(0, 400)}`);
}
if (dumpAnchor !== null && dumpedAnchor !== dumpAnchor)
{
    origError(`[runner] warning: --dump-frame ${dumpAnchor} matched no anchor in this manifest`);
}
if (seqSpec !== null && !seqArmed)
{
    origError(`[runner] warning: --dump-sequence ${seqSpec.raw} matched no anchor in this manifest`);
}

// ------------------------------------------------------------------- ledger

fs.mkdirSync(OUT_DIR, { recursive: true });
const ledgerPath = path.join(OUT_DIR, `wasm-${manifest.name}.jsonl`);
// The ledger is a run log: lines accumulate across runs (brief contract);
// golden comparison always uses this run's in-memory ledger.
fs.appendFileSync(ledgerPath, ledger.map((l) => JSON.stringify(l)).join("\n") + "\n");
console.log(`[runner] ledger appended (${ledger.length} anchors) -> ${path.relative(REPO_ROOT, ledgerPath)}`);

// ------------------------------------------------------- goldens / compare

// Gate 3 is unconditional (brief): ANY console error-level output during the
// run fails the scenario, on the --bless path too - a golden must only ever
// persist a clean run's ledger, never a poisoned one.
if (consoleErrors > 0)
{
    runFail(`console error-level output captured ${consoleErrors} time(s) (gate 3): ${errorLines.slice(0, 5).join(" | ")}`);
}

const goldenPath = path.join(GOLDEN_DIR, `${manifest.name}.json`);
if (bless)
{
    // Per-scenario blessing rationale: the golden is the provenance record,
    // so the text must describe THIS scenario's anchors, not a stale default.
    const rationales = {
        title_attract:
            "Initial blessing: title screen (t140) and shareware demo1 mid-play (t490, t980); " +
            "hashes are harness 64-bit digests normalized to 0x + 16 hex digits.",
        e1m1_combat:
            "Initial blessing: E1M1 combat via boot args -warp 1 1 (skill = default sk_medium); " +
            "anchors fight_a (t140), fight_b (t420), fight_c (t700) under scripted " +
            "fire/forward/turn/strafe input; hashes are harness 64-bit digests normalized to 0x + 16 hex digits.",
        restart_flow:
            "Initial blessing: title (t140), then menu-scripted new game x2 (ESC/ENTER x3, skill = Hurt Me Plenty); " +
            "game1 (t350) after the first start, game2 (t700) after the second restart crossed the melt wipe on a " +
            "loaded zone (zone held under harness pacing); hashes are harness 64-bit digests normalized to 0x + 16 hex digits.",
        sf2_music:
            "Negative-contract blessing (defect B, F9 spec §Problem): SF2 name 'test.sf2' is in the boot profile " +
            "but registered nowhere, so the engine's exists() gate cannot see it -- the flow pins the warning " +
            "'No soundfont found' on the captured warn channel plus music_active=false / zero counters via " +
            "harness_audio_stats. EXPECTED FLIP: when the fork bridges the SF2 gate to the backend, this warning " +
            "disappears and the flow must be upgraded to the positive contract (music_active=true with a real " +
            "committed SF2 fixture); until then the warning assert is the defect-B catch, blessed as-is.",
        save_load_roundtrip:
            "Initial blessing of the menu-scripted save/load roundtrip (ESC -> Save Game -> slot 0 -> typed name " +
            "-> ENTER; play on (turn only, RNG stays frozen); ESC -> Load Game -> slot 0). Anchor saved_a carries " +
            "state_hash_load (gametic + both RNG indices zeroed -- G_InitNew's M_ClearRandom and the loop tic " +
            "counter are the words a savegame does not carry, so the full state_hash can never agree across a " +
            "load); the final expect_state_at_load step re-digests the loaded state after the same idle-tic count " +
            "and must equal saved_a's load digest. Full state_hash at loaded_a intentionally differs (gametic).",
    };
    const doc = {
        scenario: manifest.name,
        provenance: {
            commit: gitHead(),
            rationale: rationales[manifest.name] ??
                "Initial blessing; hashes are harness 64-bit digests normalized to 0x + 16 hex digits.",
        },
        anchors: ledger,
    };
    fs.mkdirSync(GOLDEN_DIR, { recursive: true });
    fs.writeFileSync(goldenPath, JSON.stringify(doc, null, 2) + "\n");
    console.log(`[runner] BLESSED ${ledger.length} anchors -> ${path.relative(REPO_ROOT, goldenPath)} (commit ${doc.provenance.commit})`);
}
else
{
    if (!fs.existsSync(goldenPath))
    {
        fail(`golden missing: ${goldenPath} - run with --bless first`);
    }
    const golden = JSON.parse(fs.readFileSync(goldenPath, "utf8"));
    if (golden.scenario !== manifest.name)
    {
        fail(`golden scenario "${golden.scenario}" does not match manifest "${manifest.name}"`);
    }
    if (!Array.isArray(golden.anchors))
    {
        fail(`golden is malformed: "anchors" must be an array`);
    }

    const problems = [];
    if (golden.anchors.length !== ledger.length)
    {
        problems.push(`anchor count: golden has ${golden.anchors.length}, run produced ${ledger.length}`);
    }
    for (const [i, actual] of ledger.entries())
    {
        const expected = golden.anchors[i];
        if (!expected) { break; }
        if (expected.anchor !== actual.anchor)
        {
            problems.push(`anchor[${i}]: golden "${expected.anchor}" vs run "${actual.anchor}"`);
            continue;
        }
        // Compare every ledger field present on either side (undefined on
        // both sides compares equal): the optional state_hash_load field
        // only exists for flows whose anchors request it, so the pre-M2
        // goldens stay valid unchanged.
        for (const field of ["gametic", "state_hash", "frame_hash", "state_hash_load"])
        {
            if (expected[field] !== undefined || actual[field] !== undefined)
            {
                if (expected[field] !== actual[field])
                {
                    problems.push(`anchor ${actual.anchor}: ${field} mismatch - golden ${JSON.stringify(expected[field])} vs run ${JSON.stringify(actual[field])}`);
                }
            }
        }
    }

    if (problems.length > 0)
    {
        runFail(`golden mismatch (${problems.length} problem(s)): ${problems.slice(0, 5).join(" | ")}`);
    }
    console.log(`[runner] PASS: ${manifest.name} matches golden (${ledger.length} anchors), console channels: error=${consoleErrors} warn=${consoleWarns} info=${consoleInfos}`);
}
