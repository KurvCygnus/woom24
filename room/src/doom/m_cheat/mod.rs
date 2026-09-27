//! Cheat sequence checking: keystrokes are fed one at a time to the
//! matcher, which tracks progress through a fixed sequence, optionally
//! collects trailing parameter characters, and reports when a cheat is
//! fully entered -- vanilla-exact, including the quirk where a
//! too-short sequence on a cheat with parameters never fires. Rust
//! port of `vendor/doomgeneric/m_cheat.c`.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the `cheatseq_t` state struct (LP64/ILP32
//!   layout-pinned) and the `MAX_CHEAT_LEN` / `MAX_CHEAT_PARAMS`
//!   constants
//! - `matcher.rs` -- `check_cheat` / `get_param` and the private
//!   `raw_strlen` helper, plus the baseline test module
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias = "OriginalName")]`,
//! upstream-name shims at this root, `#[no_mangle]` dropped with the
//! rename (no extern declarers anywhere). All functions keep their
//! pre-move SAFE `pub extern "C"` kind. No pins.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `cht_CheckCheat` | `matcher::check_cheat` | glue | shim; input-order-dependent state machine fed by PLATFORM KEY EVENTS (`ST_Responder`, automap), never by the ticcmd/demo stream: during playback no key events are generated so the matcher never fires; during recording a typed cheat's EFFECT enters the simulation through normal game code -- the effect is demo-visible, the matcher is not; upstream `vendor/doomgeneric/m_cheat.c:34` |
//! | `cht_GetParam` | `matcher::get_param` | glue | shim; copies `parameter_chars` bytes out of the consumer-owned `cheatseq_t` state; trusts the caller's cursor (never "fix"); upstream `m_cheat.c:81` |
//! | `raw_strlen` (Rust-only helper) | `matcher::raw_strlen` | glue | NO rename -- already plain-English snake_case (m_random precedent: name kept, so no shim) |
//! | `cheatseq_t` | `types::cheatseq_t` | data | name kept (types ruling) to avoid rippling into the consumer-owned cheat statics (`st_stuff.rs:351-416`, `am_map.rs:837`); layout-pinned 72/52 bytes with the LP64-policy const asserts intact |
//! | `MAX_CHEAT_LEN` / `MAX_CHEAT_PARAMS` | `types.rs` | data | names kept; zero external importers (in-tree users hardcode 25 via `make_cheat_seq`, `st_stuff.rs:310`) |
//!
//! The vanilla short-sequence quirk carried inside `check_cheat` (a
//! too-short declared sequence on a parameter-bearing cheat never
//! fires, `m_cheat.c:39-43`) is upstream's documented intended
//! behaviour, not a bug emulation -- no `docs/vanilla-workarounds.md`
//! row; pinned by the pre-move baseline vector
//! (`short_sequence_on_parameter_cheat_never_fires`).
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave B5, whole module): the
//! matcher is event-path PREPROCESSING outside the synchronization
//! surface, analogous to the menu responder. It consumes no PRNG,
//! advances no demo-visible static (`chars_read` /
//! `param_chars_read` live in st_stuff-owned `cheatseq_t` statics,
//! presentation layer), and during demo playback no key events are
//! generated so it can never perturb a replayed sequence. Honest
//! note: cheat EFFECTS are demo-observable, the matcher itself is
//! not. Accordingly `m_cheat` has no `dtmc` submodule: every function
//! adjudicates to `glue` (event-driven matching and parameter
//! marshalling) or `data` (types and constants) in the mapping table
//! above, and no extraction candidates exist.

pub mod matcher;
pub mod types;

//* path-stability re-export: the cheat vocabulary keeps its
//* module-root paths (`st_stuff.rs`, `am_map.rs`).
pub use types::{cheatseq_t, MAX_CHEAT_LEN, MAX_CHEAT_PARAMS};

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use matcher::check_cheat as cht_CheckCheat;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use matcher::get_param as cht_GetParam;
