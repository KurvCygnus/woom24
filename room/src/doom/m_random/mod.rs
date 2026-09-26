//! The DOOM random number generator: the canonical 256-byte random
//! sequence and the two cursors that walk it -- `prndindex` for the
//! play simulation (the demo-synchronized generator) and `rndindex`
//! for presentation-layer randomness -- plus the reset that zeroes
//! both.
//!
//! ## Submodule Responsibility
//!
//! - `table.rs` -- the canonical 256-byte random sequence
//!   (`RNDTABLE`), byte-for-byte identical to upstream `rndtable`
//!   and pinned by the vendor cross-check in `tables.rs`
//! - `state.rs` -- the two demo-visible cursor statics (`rndindex`,
//!   `prndindex`), the shared generator state every wrapper advances
//!   and the test crates read through their `extern "C"` blocks
//! - `random.rs` -- the three C-linkage wrappers (`P_Random`,
//!   `M_Random`, `M_ClearRandom`): cursor marshalling around
//!   `dtmc::random_advance`, plus their cursor/reset test module
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing
//! consumer path valid (`crate::doom::m_random::*` across the freeze
//! zone, `tables.rs`' vendor cross-check, and the
//! `room::doom::m_random::*` test-crate imports); no content lives
//! here. `dtmc` reaches the table through the root re-export
//! (`super::RNDTABLE`), and the `random.rs` wrappers marshal the
//! `state.rs` statics through `dtmc::random_advance`.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_Random` | `random::P_Random` | dtmc | THE demo generator: 69 simulation call sites (AI, weapons, movement, damage, specials, plus the demo-visible map-setup deathmatch spot burn); the body re-routes through `dtmc::random_advance` while name, `#[no_mangle]` and `extern "C"` kind stay unchanged -- no rename, so no shim and no wasm export churn; the `state.rs` cursor store stays in the wrapper; upstream `vendor/doomgeneric/m_random.c:50-54` |
//! | `M_Random` | `random::M_Random` | glue | explicitly non-simulation by upstream design (`m_random.h:32` reserves `P_Random` for the play simulation only): screen wipe, status-bar face, and intermission pacing consume `rndindex` precisely so they never perturb the demo sequence; same re-route through `dtmc::random_advance`; upstream `vendor/doomgeneric/m_random.c:56-60` |
//! | `M_ClearRandom` | `random::M_ClearRandom` | dtmc | the sequence-reproduction reset at new-game/demo start (`G_InitNew` analog); its whole observable behavior is the atomic zeroing of both demo-visible cursors, so the verdict is dtmc, but no pure computation exists to separate -- the function stays whole in `random.rs` as the `state.rs` cursors' marshalling glue; upstream `vendor/doomgeneric/m_random.c:62-65` |
//! | `rndtable` | `RNDTABLE` (`table::RNDTABLE`) | data | the canonical 256-byte sequence, byte-for-byte identical to upstream (enforced by the vendor cross-check in `tables.rs`); name, path and `pub(crate)` visibility unchanged; upstream `vendor/doomgeneric/m_random.c:24-44` |
//! | `rndindex` | `state::rndindex` (static) | data | game-thinker cursor, demo-visible shared state: the net-consistency cookie (`g_game.c:957` analog), state-hash word 2, and the golden snapshots; `#[no_mangle]` retained; upstream `vendor/doomgeneric/m_random.c:46`, `doomstat.h:276` |
//! | `prndindex` | `state::prndindex` (static) | data | play-simulation cursor, demo-visible shared state: the demo-playthrough and frame-split golden snapshots read it through two test-crate `extern "C"` blocks, plus state-hash word 3; `#[no_mangle]` retained so those exact symbols keep linking; upstream `vendor/doomgeneric/m_random.c:47` |
//!
//! No symbol was renamed, so there are no boundary shims and no link
//! anchor, and nothing qualified for a `#[no_mangle]` drop or an
//! `#[export_name]` pin -- the module root's re-exports carry the
//! unchanged names for path stability only. The pre-graduation module
//! doc's C-linkage rationale is corrected here: no compiled C
//! translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes the engine sources), so the
//! `doomstat.h:276` / `g_game.c:957` citations above are upstream
//! provenance only -- retention keeps the wasm export surface
//! byte-identical and satisfies the two test-crate `extern "C"`
//! declarers of `prndindex` (`demo_playthrough.rs`,
//! `frame_split_common/mod.rs`).
//!
//! ## Deterministic Aspects
//!
//! The two-cursor design IS the determinism mechanism: `prndindex`
//! (advanced by `P_Random`) feeds the demo synchronization surface --
//! every draw perturbs the exact simulation sequence that demos and
//! the state hash pin -- while `rndindex` (advanced by `M_Random`)
//! is deliberately isolated so presentation-layer randomness never
//! reaches the demo sequence. `M_ClearRandom` zeroes both cursors
//! atomically at every sequence-reproduction point (new game, demo
//! start); the save/load digest layout assumes that atomicity
//! (savegames carry neither cursor). The extracted
//! `dtmc::random_advance` must stay LSB-exact: the pre-increment
//! order (index 0 unused until the wrap), the 256-wrap mask, and the
//! table bytes themselves are the demo-visible sequence -- pinned by
//! the baseline vectors in `dtmc`'s test module, written and run
//! against the original wrapper bodies before extraction (F10
//! graduate #3).

pub mod dtmc;
pub mod random;
pub mod state;
pub mod table;

//* path-stability re-export: the C-linkage wrappers keep their
//* module-root paths for the freeze-zone callers (`g_game.rs`,
//* `f_wipe.rs`, `p_*.rs`, `c_tests/harness.rs`).
pub use random::{M_ClearRandom, M_Random, P_Random};

//* path-stability re-export: the cursor statics keep their
//* module-root paths (`g_game.rs`, `harness_hash.rs`, `tables.rs`
//* consumers, and the `room::doom::m_random::*` test-crate imports).
pub use state::{prndindex, rndindex};

//* path-stability re-export: `pub(crate)` visibility preserved for
//* `tables.rs`' vendor cross-check and the in-module test imports.
pub(crate) use table::RNDTABLE;
