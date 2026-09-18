# Design: c_tests gating & LP64 strategy

**Date:** 2026-09-17
**Status:** Implemented (2026-09-18; [AUTO] decisions executed as specified — D3 conclusion: all 28 c_long/c_ulong uses CRT-adjacent, zero simulation-facing; see docs/specs/2026-09-18-c-tests-lp64-audit.md).
**Series:** follows spec ② (`2026-09-17-wasm-shell-design.md`) — its libc/VFS shim removes the
CRT-class wasm errors; this spec removes the layout-class ones and fixes the testing policy.

## Problem

1. The C-vs-Rust differential suite (`room/src/doom/c_tests/`, 437 tests) is gated
   `#[cfg(all(test, unix))]` (since the Windows bring-up commit): its oracle is the **compiled C
   side under LP64** (`long` = 8 bytes), and on wasm there is no C side at all. The gating is
   correct but was an emergency measure — the policy needs to be stated, not improvised.
2. Three compile-time struct-size guards fail on ILP32 targets (wasm32): `info.rs:910`
   (`size_of::<State>() == 40`), `m_menu.rs:175` (`menuitem_t == 32`), `m_menu.rs:179`
   (`menu_t == 40`). These pin `#[repr(C)]` layouts against the C originals **under LP64**.
3. No documented statement exists of where `c_long`/`c_ulong` (the LP64-sensitive types) may
   appear, so nobody can reason about wasm determinism without re-auditing.

## Goals

1. A written testing policy: which oracle covers which target.
2. The three size guards become model-aware, so the wasm32 check passes once spec ②'s shim lands.
3. A committed audit artifact: every `c_long`/`c_ulong` use in `room/src/doom/` classified as
   CRT-adjacent (safe: values come from/go to the platform I/O layer) or simulation-facing
   (would need action).

## Non-goals

- Running the differential suite on wasm (no C oracle exists there).
- Changing any simulation type from `c_long` to a fixed-width type (audit first; only act if the
  audit finds a simulation-facing use — none is expected).
- 437-test cross-target execution (follow-up: `wasm-bindgen-test` demo-hash harness, recorded in
  spec ② follow-ups).

## Decisions (all [AUTO] — sleep-mandate)

| # | Decision | Rationale |
|---|---|---|
| D1 | **c_tests stay `#[cfg(all(test, unix))]` permanently**, with the in-code comment upgraded to point at this spec. The suite's oracle is the C compilation of `vendor/doomgeneric` (LP64 on Linux CI); wasm has no C oracle. Demo-exactness for wasm is anchored instead by golden demo tests on the host (LP64 native), which is the same simulation code. | The alternative — making the C side build for wasm32 to get an ILP32 oracle — is a doomgeneric-sys project with no payoff: demo-exact semantics are defined by the LP64 Linux reference, and the wasm build must reproduce THAT, not a 32-bit C build. |
| D2 | **The three size guards become model-aware**: expected sizes selected by `cfg(target_pointer_width = "64")` (40/32/40 as today) vs `"32"` (values computed by hand-layout from the `#[repr(C)]` field lists with `c_long = 4`, each cited in a comment showing the arithmetic), so the guards keep pinning layouts on every target instead of being deleted or gated off. | Deleting the guards would lose real layout protection; keeping LP64-only values would break the wasm check forever. |
| D3 | **`c_long` audit artifact** committed at `docs/specs/2026-09-18-c-tests-lp64-audit.md`: table of every `c_long`/`c_ulong` occurrence in `room/src/doom/` with classification (CRT-adjacent I/O vs simulation state) and a concluding determinism statement. Expected conclusion: all uses are CRT/I/O-adjacent; simulation state is fixed-width (`c_int`/`u32`/fixed-point), so demo determinism is data-model independent. If the audit finds a simulation-facing `c_long`, that is an escalation to the user, not an autonomous fix. | Turns "nobody knows" into a reviewed artifact; cheap (grep + classification). |

## Architecture / tasks sketch

1. Guard rework in `info.rs` / `m_menu.rs` (D2) — `cfg`-selected expected values with layout
   arithmetic comments; wasm check contribution: these 3 errors disappear.
2. Audit artifact (D3) — grep-driven table + conclusion, committed.
3. Policy docs: `room/src/doom.rs` c_tests comment → cite this spec; AGENTS.md status line
   mentions spec ③ landing.
4. Acceptance: `cargo check --workspace --target wasm32-unknown-unknown --exclude room-shell-native`
   reports **zero** errors remaining in `room` (spec ② delivered the CRT shim); host suite stays
   green (376 unit + 2 demo); `cargo test --features dhat-heap --test demo_playthrough` unaffected.

## Risks

- Hand-computed ILP32 sizes could be wrong → the guards would pin wrong layouts. Mitigation: the
  layout arithmetic comment must enumerate every field; a wrong comment is reviewable, a silent
  number is not.
- The audit could reveal simulation-facing `c_long` → escalation (see D3).
