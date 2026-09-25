// diff-ledgers.mjs - F9 gate 2: cross-target ledger comparison (wasm vs host twin).
//
// Usage (repo root):
//   node shells/web/scripts/diff-ledgers.mjs [--latest-run] <a.jsonl> <b.jsonl>
//
// Exits 0 iff both files parse as JSONL, both cover the SAME anchor set, and
// every shared anchor's gametic + state_hash + state_hash_load + frame_hash
// agree. Those four fields are exactly the gate-2 surface: hash strings are
// the wasm runner's canonical "0x" + 16 lowercase hex digits, and gametic is
// the tic-drift detector (the first bisect signal - anchors are tic counts).
// The environmental asymmetries (audio stats, console channel content) never
// enter the ledger, so they can never false-fail a cross-target diff.
//
// A field that is null on one side and absent on the other compares equal:
// both mean "not requested at this anchor". Everything else must match
// byte-exactly.
//
// Anchors missing on either side are FAILURES, not ignored lines - a silent
// half-run must never pass. Later occurrences of the same anchor win: the
// wasm runner APPENDS to its ledger across runs (run-log contract) while the
// host twin overwrites, so the last occurrence is each side's latest word
// for that anchor.
//
// --latest-run: reduce each input to its LAST run before comparing - the
// maximal line suffix in which no anchor repeats. Needed when comparing
// against the wasm runner's accumulated run-log, where earlier runs may
// carry anchors of older manifest revisions that the current manifest (and
// therefore the host twin) no longer produces. Default OFF: full files are
// compared.
//
// KNOWN_FRAME_DIVERGENCE (env, csv of flow names - mirror of the twin's
// KNOWN_FRAME_DIVERGENCE const in room/tests/scenario_harness/mod.rs):
// frame_hash mismatches on a listed flow are reported as
// `WARN frame-divergence (known)` and exit 0, but both hashes are still
// printed. The listed flows carry the documented engine bug "native
// R_DrawSprite drawseg-silhouette clips world sprites; wasm draws them -
// pre-existing render divergence, first caught by this twin 2026-09-25;
// expected to close with the fork's defect-C fix". Everything else is
// ALWAYS a failure: state_hash, state_hash_load, gametic, missing anchors,
// and frame_hash on any flow NOT in the list (save_load_roundtrip
// deliberately stays off the list - it is strict state-parity with no
// frame anchors yet, frame anchors deferred - see F9 M4 - so any
// frame_hash mismatch there must fail).
//
// Exit codes: 0 = parity; 1 = mismatch or missing anchors; 2 = usage or
// malformed input (not a parity verdict).

import fs from "node:fs";

const args = process.argv.slice(2);
const latestRun = args.includes("--latest-run");
const knownFrameDivergence = (process.env.KNOWN_FRAME_DIVERGENCE ?? "").
    split(/[,\s]+/).
    filter(Boolean);
const paths = args.filter((a) => !a.startsWith("--"));
if (paths.length !== 2)
{
    console.error("usage: node diff-ledgers.mjs [--latest-run] <a.jsonl> <b.jsonl>");
    console.error("  KNOWN_FRAME_DIVERGENCE=<flow,flow,...> env: warn (not fail) frame_hash mismatches on those flows");
    process.exit(2);
}

// The last run = the maximal suffix of lines in which no anchor repeats
// (the runner writes one run's lines contiguously, so walking back from the
// end stops exactly at the previous run's boundary).
function reduceToLatestRun(lines)
{
    const seen = new Set();
    let start = lines.length;
    while (start > 0)
    {
        const anchor = lines[start - 1].anchor;
        if (seen.has(anchor))
        {
            break;
        }
        seen.add(anchor);
        start -= 1;
    }
    return lines.slice(start);
}

function loadLedger(path)
{
    let text;
    try
    {
        text = fs.readFileSync(path, "utf8");
    }
    catch (e)
    {
        console.error(`[diff-ledgers] unreadable ledger: ${path} (${e.message})`);
        process.exit(2);
    }
    const byAnchor = new Map();
    let lines = text.split(/\r?\n/).filter((l) => l.trim() !== "");
    for (const [i, line] of lines.entries())
    {
        let obj;
        try
        {
            obj = JSON.parse(line);
        }
        catch (e)
        {
            console.error(`[diff-ledgers] ${path}:${i + 1}: invalid JSON line: ${e.message}`);
            process.exit(2);
        }
        if (typeof obj !== "object" || obj === null || typeof obj.anchor !== "string" || obj.anchor === "")
        {
            console.error(`[diff-ledgers] ${path}:${i + 1}: not a ledger object with a non-empty "anchor"`);
            process.exit(2);
        }
        lines[i] = obj;
        byAnchor.set(obj.anchor, obj);
    }
    if (latestRun)
    {
        const keep = new Set(reduceToLatestRun(lines).map((o) => o.anchor));
        for (const key of [...byAnchor.keys()])
        {
            if (!keep.has(key))
            {
                byAnchor.delete(key);
            }
        }
    }
    return byAnchor;
}

// absent (undefined) and null both mean "not requested at this anchor".
function normalize(v)
{
    return v === undefined || v === null ? null : v;
}

const FIELDS = ["gametic", "state_hash", "state_hash_load", "frame_hash"];

const a = loadLedger(paths[0]);
const b = loadLedger(paths[1]);
const aPath = paths[0];
const bPath = paths[1];

// The flow name every line of a ledger carries (both inputs must describe
// the same flow for a comparison to mean anything).
function flowOf(ledger)
{
    const first = ledger.values().next().value;
    return first && typeof first.scenario === "string" ? first.scenario : null;
}
const flowA = flowOf(a);
const flowB = flowOf(b);
if (flowA !== flowB)
{
    console.error(`[diff-ledgers] FAIL: scenario mismatch - ${aPath} carries "${flowA}", ${bPath} carries "${flowB}"`);
    process.exit(1);
}

const problems = [];
const warnings = [];
for (const key of [...a.keys()].filter((k) => !b.has(k)))
{
    problems.push(`anchor ${key}: present only in ${aPath} (missing from ${bPath})`);
}
for (const key of [...b.keys()].filter((k) => !a.has(k)))
{
    problems.push(`anchor ${key}: present only in ${bPath} (missing from ${aPath})`);
}
const known = knownFrameDivergence.includes(flowA);
const shared = [...a.keys()].filter((k) => b.has(k)).sort();
for (const key of shared)
{
    for (const field of FIELDS)
    {
        const va = normalize(a.get(key)[field]);
        const vb = normalize(b.get(key)[field]);
        if (va === vb)
        {
            continue;
        }
        const detail = `anchor ${key}: ${field} mismatch - ${aPath} ${JSON.stringify(va)} vs ${bPath} ${JSON.stringify(vb)}`;
        if (field === "frame_hash" && known)
        {
            // Known engine divergence: recorded + warned, never silent.
            warnings.push(detail);
        }
        else
        {
            problems.push(detail);
        }
    }
}

for (const w of warnings)
{
    console.error(`[diff-ledgers] WARN frame-divergence (known) ${flowA}: ${w}`);
}

if (problems.length > 0)
{
    console.error(`[diff-ledgers] FAIL: ${problems.length} problem(s) between ${aPath} and ${bPath}:`);
    for (const p of problems)
    {
        console.error(`[diff-ledgers]   - ${p}`);
    }
    process.exit(1);
}
if (warnings.length > 0)
{
    console.log(`[diff-ledgers] PASS (known frame divergence): ${shared.length} anchor(s) compared, ${warnings.length} warned frame_hash mismatch(es) on "${flowA}" (state hashes all equal)`);
    process.exit(0);
}
console.log(`[diff-ledgers] PASS: ${shared.length} anchor(s) agree across ${aPath} and ${bPath}`);
process.exit(0);
