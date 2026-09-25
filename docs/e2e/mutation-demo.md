# F9 M3 Mutation Demo - every gate demonstrated on its failure class

**Date:** 2026-09-26. **Acceptance basis:** F9 spec SS6 (M3 close; spec is local-only): each
2026-09-25 browser defect class must have a demonstrated catch - at M3 close, run a mutation
demo and show the corresponding gate go red. This document is that evidence.

**Environment of the live runs below:** branch `feat/spec2-3`, HEAD `382edf9` (F9 merged at
`0f6cd28`, spec-4 defect fixes in), host = Windows / Git Bash, node v24.18.0, harness pkg
`target/pkg-harness` (`--features harness`, wasm32 release), shareware `doom1.wad` via LFS.
Ledger and PPM artifacts live under `shells/web/scripts/scenarios/out/` (gitignored run
space); CI regenerates both sides fresh on every push.

## Gate inventory

| Gate | Mechanism | Surface | Red condition |
|---|---|---|---|
| 1 pixel golden | wasm runner compares the run's hash ledger against the committed golden | `run-scenarios.mjs` + `scenarios/goldens/<flow>.json` | any `gametic` / `state_hash` / `state_hash_load` / `frame_hash` / anchor-name mismatch, anchor-count drift, malformed golden |
| 2 cross-target | host twin (`room/tests/scenario_harness`) and the wasm runner consume the same manifests; `diff-ledgers.mjs` compares the ledgers | `diff-ledgers.mjs` (CI `cross-target` job) | any field mismatch or missing anchor; frame_hash mismatches on `KNOWN_FRAME_DIVERGENCE` flows are WARN-only, everything else fails |
| 3 zero I_Error / clean console | the engine's log facade routes through the shell `ConsoleLogger`; the runner captures all console output | `run-scenarios.mjs` (compare AND bless path) | ANY error-level console line (the engine has exactly one `log::error!` site: fatal `I_Error`), or an engine exception/trap from an export call |

Baselines before mutation (both green): clean `title_attract` run `PASS ... (3 anchors),
error=0 warn=1019 info=315`, exit 0; clean `diff-ledgers` under the CI env exits 0.

## Drill 1 - gate 1 (pixel golden): one flipped frame hash in a golden

**Fault injected:** `anchors[1].frame_hash` in the committed
`shells/web/scripts/scenarios/goldens/title_attract.json` set to `0xdeadbeefdeadbeef`
(one 64-bit digest corrupted; nothing else touched).

```text
$ node shells/web/scripts/run-scenarios.mjs shells/web/scripts/scenarios/title_attract.json
[scenario-fail] golden mismatch (1 problem(s)): anchor t490: frame_hash mismatch -
  golden "0xdeadbeefdeadbeef" vs run "0xc9a969af2f4647e5"
NODE_EXIT=1
```

**Observed red:** exit 1 with the anchor and both digests named. **Restore:** `git checkout`
of the golden; `grep deadbeef` count back to 0. (First recorded instance of this drill:
F9 Task 3 corruption probe, commit `30377c8` era - same anchor, same red.)

This is the defect-C tripwire class: any change in what the engine actually rasterizes into
`DG_ScreenBuffer` at a blessed anchor flips a frame hash and lands here.

## Drill 2 - gate 3 (flow / I_Error): garbage IWAD at `W_AddFile`

**Fault injected:** the boot profile's IWAD is not a WAD
(`printf 'GARBAGE-NOT-A-WAD-2026'` registered as `scenarios/out/fake-ierror.wad`, probe
manifest in the same gitignored directory) - the same red path as the brief's "rename
`doom1.wad` in the manifest" form, and the same `I_Error` class as defect A (any engine
death between boot and anchors).

```text
$ node shells/web/scripts/run-scenarios.mjs .../out/ierror_probe.json
[captured-error] woom24: Wad file shells/web/scripts/scenarios/out/fake-ierror.wad
  doesn't have IWAD or PWAD id
[scenario-fail] standard_start threw: RuntimeError: unreachable
NODE_EXIT=1
```

Under `--allow-fail ierror_probe` the same run is recorded, not swallowed:

```text
[expected-fail] ierror_probe: standard_start threw: RuntimeError: unreachable
[runner] EXPECTED-FAIL: ierror_probe (allowed via --allow-fail) - failure recorded,
  NO golden written
NODE_EXIT=0
```

**Observed red:** exit 1; the `I_Error` text appears on the captured error channel AND the
`panic = abort` trap surfaces as the export-call exception - either alone fails the run.
`--allow-fail` downgrades a *run* failure to a recorded expected-fail (exit 0, never a
golden write); setup errors still exit 1. Recorded origin of this probe: F9 Task 5
(commit `6e6efea`), where it proved the error-overlay hook. Defect A linkage: `restart_flow`
drives the melt wipe on a loaded zone - the exact crash path of 2026-09-25 defect A - and
passes since the spec-4 zone fix (`15ab796`); a regression re-introducing any `I_Error`
re-fires the red demonstrated here.

## Drill 3 - gate 2 (cross-target): one-byte ledger tamper + divergence triage

Host ledgers are untracked run artifacts, so the tampered side is a copy under `out/`
(removed after the drill). Env mirrors the CI job:
`KNOWN_FRAME_DIVERGENCE=title_attract,e1m1_combat,restart_flow,sf2_music`.

**(a) Warn-not-fail on a listed flow.** Clean comparison of the real wasm vs host
`title_attract` ledgers - the frame hashes genuinely differ cross-target (the documented
native `R_DrawSprite` drawseg-silhouette clipping, first caught by this very gate on
2026-09-25 - a real engine divergence, not a synthetic one):

```text
[diff-ledgers] WARN frame-divergence (known) title_attract: anchor t490: frame_hash
  mismatch - ...wasm-title_attract.jsonl "0xc9a969af2f4647e5" vs
  ...host-title_attract.jsonl "0x6c3e7e26bb0149bd"
[diff-ledgers] WARN frame-divergence (known) title_attract: anchor t980: ...
[diff-ledgers] PASS (known frame divergence): 3 anchor(s) compared, 2 warned frame_hash
  mismatch(es) on "title_attract" (state hashes all equal)
EXIT=0
```

**(b) Same frame flip WITHOUT the env (or on any unlisted flow) is a hard fail.** One byte
flipped (`...7e5` -> `...7e6`) in the copied host ledger, env unset:

```text
[diff-ledgers] FAIL: ... anchor t490: frame_hash mismatch - ... "0xc9a969af2f4647e5" vs
  "...drill-host-ta-frame.jsonl" "0xc9a969af2f4647e6"
EXIT=1
```

**(c) State hash is strict even where frames are warned.** One byte flipped in the t490
`state_hash` of the copied host ledger, env still set:

```text
[diff-ledgers] WARN frame-divergence (known) title_attract: anchor t980: ...
[diff-ledgers] FAIL: 1 problem(s) ...
[diff-ledgers]   - anchor t490: state_hash mismatch - ... "0xee1b43713c3387dd" vs
  "...drill-host-ta-state.jsonl" "0xee1b43713c3387de"
EXIT=1
```

**Observed red:** exit 1 naming anchor + field in (b) and (c); the triage list can silence
only the documented frame divergence, never a state hash - simulation parity is the
AGENTS cross-target requirement, and its violation fails even under the widest triage.
(An earlier drill variant also flipped `state_hash` on every line of a
`save_load_roundtrip` copy: exit 1 naming each anchor - that flow keeps full parity and
stays off the triage list on purpose.)

## Drill 4 - gate 3 on the bless path: a dirty run must never write a golden

**Fault injected:** one synthetic error-level console line during a `--bless` run, via a
`node --import` preload script (lives in the gitignored `out/`; the committed runner
contains no trace - this is the non-invasive form of Task 3 fix round 1's temporary
in-editor probe, commit `30377c8`).

```text
$ node --import .../out/gate3-inject.mjs shells/web/scripts/run-scenarios.mjs \
    shells/web/scripts/scenarios/title_attract.json --bless
[captured-error] [drill-inject] synthetic error for the gate-3 bless-path drill
[scenario-fail] console error-level output captured 1 time(s) (gate 3):
  [drill-inject] synthetic error for the gate-3 bless-path drill
NODE_EXIT=1
```

**Observed red:** exit 1 before the golden write; the golden's sha256
(`e3a3f8ee227e...6044c`) is byte-identical before and after - a poisoned run can never
persist a poisoned baseline. Without the injection the same `--bless` run exits 0 and
re-blesses deterministically.

## Defect class -> catching gate (2026-09-25 browser session)

| Class | Defect (2026-09-25) | Catching gate | Drill / evidence | Status |
|---|---|---|---|---|
| A | `Z_Malloc: failed` in `wipe_init_melt` - engine death on new-game restart (zone exhaustion) | gate 3 (any `I_Error` red) over flow `restart_flow` (the melt-wipe-on-loaded-zone path) | Drill 2 (the `I_Error` red path live); flow green since the spec-4 32 MiB zone fix (`15ab796`) | fixed (spec-4 A); flow guards regression |
| B | "No soundfont found" despite SF2 selected - engine `exists()` gate never bridged to the backend | flow `sf2_music` asserts (console_contains warn + `music_active=false`); flips to a positive contract when a real SF2 fixture lands | F9 Task 5 blessing rationale (recorded); warning text pinned by the committed manifest | bridge landed (spec-4 B, `683a02b`); flow stays legitimately negative until a real SF2 fixture exists - the golden records the expected flip |
| C | world sprites not rendered in-browser, walls fine, headless all green | gate 1 pixel golden (`e1m1_combat`, engine side) + M4 B-layer (presenter side, pending) | Drill 1 (pixel hash red); gate 2 additionally caught the real native-side sibling on 2026-09-25 (Task 6) - now triaged by `KNOWN_FRAME_DIVERGENCE`, warn-not-fail (Drill 3a-c); browser forensics armed via `woom24-diag:` console lines | open (defect-C family); closes with the sprite-clipping fix, triage list retires with it |
| D | SFX latency/glitches (ScriptProcessor-style block pump) | audio-health counters via `harness_audio_stats` (`assert_stats` invariants, e.g. `underruns`), flow `sf2_music` | counters committed (F9 Task 5); perceived latency stays a human-pass item by design | partial by design (machine-checkable health only) |
| E | wasm crash UX: `I_Error` -> unreachable trap + Firefox `file:///` noise, no on-page error | crash visibility: shell DOM overlay (`woom24_last_error` + `woom24.js`); gate 3 asserts the error channel in headless runs | Drill 2 (the `I_Error` text captured on the gated channel); overlay hook proven live in F9 Task 5; M4 will assert overlay absence in real browsers | fixed (spec-4 E, `e8e4ca0`); supersedes F9 Task 5's Rust-side overlay |

## Residue statement

Every mutation was reverted or confined to the gitignored `scenarios/out/` run space and
removed: drill 1's golden restored via `git checkout` (verified zero `deadbeef` lines,
sha256 unchanged), drill 3's ledger copies deleted, drill 4's preload + drill 2's probe
manifest and fake IWAD deleted. `git status` after the drills shows no drill artifacts.
