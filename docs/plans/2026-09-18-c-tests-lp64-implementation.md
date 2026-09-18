# c_tests / LP64 Strategy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land spec ③ — the c_tests gating policy as a written decision, model-aware struct-size guards replacing the interim pointer-width gates, and a committed `c_long` audit proving wasm determinism.

**Architecture:** No runtime behavior changes anywhere. Three artifacts: an audit document (grep-driven classification), three `#[cfg]`-selected const asserts in `room/src/doom/` (comment-only + cfg-structure edits), and policy/status doc updates.

**Tech Stack:** Rust stable, grep/awk for the audit, the existing workspace.

**Spec:** `docs/specs/2026-09-17-c-tests-lp64-design.md` (approved; decisions marked [AUTO] executed as specified).

## Global Constraints

- **Zero runtime behavior changes.** Simulation code is not touched; only `const` assert structure and comments in `doom/` files (the sanctioned "mechanical cfg" + comment classes per the wasm plan's amended constraint).
- Host suite stays green: 375 room + 4 native + 47 web unit + 2 demo; pinned wasm check 0 errors/0 warnings.
- Comments: ENGLISH, ASCII punctuation; `//*`/`//!`/`//?` markers only in `//` comments, never in doc comments (policies of 2026-09-18).
- Escalation rule (spec D3): if the audit finds a simulation-facing `c_long`, STOP and report — that is the user's decision, not ours.
- Bounded timeouts; no concurrent cargo; conventional commits; never commit WADs/SF2s.
- Note: `init_pipeline.rs:71` (shells/web) carries a ledgered stale comment referencing the deleted `ARG_STORAGE` — fixing it is in scope for this plan (Task 3), as parked.

---

### Task 1: `c_long` audit artifact

**Files:**
- Create: `docs/specs/2026-09-18-c-tests-lp64-audit.md`

**Interfaces:**
- Produces: the committed classification table spec D3 requires; a boolean conclusion ("no simulation-facing `c_long`") that Task 3's policy docs cite.

- [ ] **Step 1: Enumerate every `c_long`/`c_ulong` occurrence**

```bash
grep -rn "c_long\|c_ulong" room/src/doom/ room/src/audio/ room/src/types/ shells/native/src/ shells/web/src/
```

- [ ] **Step 2: Classify each occurrence**

For every hit, record: file:line, declaration or call site, and classification:
- **CRT-adjacent** — the type crosses the platform I/O boundary (fseek/ftell offsets, file lengths, libc shims). Values originate from or terminate at the VFS/CRT layer; wasm's 4-byte `long` is what the wasm CRT shim itself defines, so behavior is self-consistent.
- **Simulation-facing** — the value reaches game state, RNG, physics, or demo playback. **Any hit here = STOP, report BLOCKED to the controller.**

Expected conclusion (verify, don't assume): all uses are CRT-adjacent; simulation state is `c_int`/`u32`/fixed-point; demo determinism is therefore data-model independent.

- [ ] **Step 3: Write the audit document**

`docs/specs/2026-09-18-c-tests-lp64-audit.md` (English): method (the grep), the full table, the conclusion, and the c_tests policy statement (differential suite stays `#[cfg(all(test, unix))]` — its oracle is the LP64 C build; wasm determinism is anchored by golden demo tests on the host, same simulation code).

- [ ] **Step 4: Commit**

```bash
git add docs/specs/2026-09-18-c-tests-lp64-audit.md
git commit -m "docs(spec3): c_long audit - all uses CRT-adjacent, determinism model-independent"
```

(If the conclusion differs from expected, commit nothing — report BLOCKED with the table.)

---

### Task 2: Model-aware struct-size guards

**Files:**
- Modify: `room/src/doom/info.rs` (~line 910, the `size_of::<State>() == 40` guard)
- Modify: `room/src/doom/m_menu.rs` (~lines 175, 179, the `menuitem_t == 32` / `menu_t == 40` guards)

**Interfaces:**
- Consumes: the current interim gates (`#[cfg(target_pointer_width = "64")]` wrapping the whole assert, from W-T2).
- Produces: per-model const asserts active on BOTH 64-bit and 32-bit targets, each ILP32 expectation carrying a field-by-field arithmetic comment; pinned wasm check still 0 errors (the guards now PASS on wasm instead of being absent).

- [ ] **Step 1: Compute the ILP32 sizes by hand**

For each struct, enumerate every field from its `#[repr(C)]` definition and compute `size_of` under `long = 4, pointer = 4, int = 4` (wasm32 rules), showing the addition in the comment. Example shape:

```rust
// ILP32 (wasm32): 2×c_long(4) + 2×c_int(4) + … = 24  (LP64: 40 — two longs widen to 8)
const _: () = assert!(std::mem::size_of::<State>() == { if cfg!(…) … });
```

The exact Rust mechanism: two cfg-selected consts (not `cfg!(…)` — that is not const-evaluable in an assert position):

```rust
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<State>() == 40); // LP64: 2×u64 + …
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<State>() == <COMPUTED>); // ILP32: <arithmetic>
```

**Step 1 sub-step — verify the hand math before writing it:** write a tiny scratch `cargo script`-style check (a `#[test]` in the same file under `#[cfg(test)]`, deleted after, or a scratch crate in `target/`) that prints `std::mem::size_of` under `--target wasm32-unknown-unknown`; record the numbers in the report. The comment must match the verified number exactly.

- [ ] **Step 2: Apply to all three guards** (`State`, `menuitem_t`, `menu_t`), replacing the interim whole-assert gates.

- [ ] **Step 3: Verify both models**

```bash
cargo test
cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown
```

Expected: host suite green; wasm check 0 errors AND the guards now active on wasm (proven by the scratch number matching).

- [ ] **Step 4: Commit**

```bash
git add room/src/doom/info.rs room/src/doom/m_menu.rs
git commit -m "fix(doom): model-aware struct-size guards (LP64 + ILP32 expectations)"
```

---

### Task 3: Policy docs + status sync + parked nit

**Files:**
- Modify: `room/src/doom.rs` (the `mod c_tests` gating comment)
- Modify: `AGENTS.md` (status line)
- Modify: `docs/specs/2026-09-17-c-tests-lp64-design.md` (Status → Implemented)
- Modify: `shells/web/src/init_pipeline.rs:71` (the parked stale `ARG_STORAGE` comment → current truth)

**Interfaces:**
- Consumes: Task 1's audit path; Task 2's guard state.

- [ ] **Step 1: Update the `c_tests` gating comment** in `room/src/doom.rs` to cite `docs/specs/2026-09-18-c-tests-lp64-audit.md` and the spec (English).

- [ ] **Step 2: AGENTS.md status line** — extend to note spec ③ landed (c_tests policy + model-aware guards + audit), keeping the rest of the paragraph intact.

- [ ] **Step 3: Spec ③ status** → `**Status:** Implemented (2026-09-18; [AUTO] decisions executed as specified — see the audit artifact for the D3 conclusion).`

- [ ] **Step 4: Fix the parked nit** — `shells/web/src/init_pipeline.rs:71` comment: replace the `ARG_STORAGE` reference with the current argv-anchoring description (`Vec::leak`/`Box::leak`, per the final fix wave).

- [ ] **Step 5: Full battery + commit + push**

```bash
cargo test
cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown
git add -A
git commit -m "docs(spec3): c_tests policy landed; AGENTS/spec status; stale comment nit"
git push origin feat/spec2-3
```

Expected: all green; push succeeds.
