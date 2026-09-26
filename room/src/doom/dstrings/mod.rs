//! Quit-message tables: the eight quit messages for Doom 1 and the
//! eight for Doom 2, held as statically stored C strings and indexed
//! by the quit dialog -- bit-exact with
//! `vendor/doomgeneric/dstrings.c`.
//!
//! ## Submodule Responsibility
//!
//! - `messages.rs` -- the `Ptr` C-string newtype (the Rust stand-in
//!   for the C `char *[]` element type), the two `#[no_mangle]`
//!   quit-message tables, and their data-pinning tests
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic
//! Aspects. The module qualifies for the zero-logic allowance
//! (`mod.rs` + one data file). The module root is documentation +
//! wiring only: the `mod` declaration and the re-exports below keep
//! every existing consumer path valid (`crate::doom::dstrings::*` --
//! the sole consumer `m_menu.rs` imports both tables); no content
//! lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `doom1_endmsg` | `messages::doom1_endmsg` | data | quit-dialog text, presentation-only (`M_QuitDOOM` via `M_SelectEndMessage`, `m_menu.rs:1273-1284`); `#[no_mangle]` retained so the wasm export surface stays byte-identical -- no compiled C translation unit references it (`doomgeneric-sys/build.rs` excludes dstrings.c), so the retention is pure conservatism; upstream `dstrings.c:23-33` |
//! | `doom2_endmsg` | `messages::doom2_endmsg` | data | same handling, Doom-II-themed table (used when `gamemission` is `doom2` / `pack_tnt` / `pack_plut`); upstream `dstrings.c:35-46` |
//! | `Ptr` | `messages::Ptr` | data | Rust-only structural wrapper: C's element type is mutable `char *`, Rust string literals are `'static` read-only, so a `#[repr(transparent)]` `*const c_char` newtype with `unsafe impl Sync` enables `static` storage; field `.0` stays public (`m_menu.rs:1277,1279` reads it directly); upstream `dstrings.c:23` element type |
//!
//! C's `#if 0` block of unused messages (`dstrings.c:48-69`) is
//! correctly absent from the port.
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin.
//! The pre-graduation doc's claim that the tables are read "via
//! `M_Random`" is corrected in this rewrite: selection is
//! **gametic-keyed**, not RNG-keyed (see Deterministic Aspects).
//!
//! ## Deterministic Aspects
//!
//! Module adjudicated with no `dtmc` surface: both tables are
//! presentation-only quit-dialog text consumed by `M_QuitDOOM`
//! (`m_menu.rs:1284` onward) and never touch simulation state. The
//! slot selection is `(gametic as usize) & 7` in
//! `M_SelectEndMessage` (`m_menu.rs:1273-1281`) -- keyed by the tic
//! counter at quit time, not by an `M_Random` draw, as the stale
//! pre-graduation doc claimed; that `gametic` read lives in the
//! freeze zone (`m_menu.rs`), not here. The table length 8 is
//! load-bearing for that `& 7` index math and is pinned by the moved
//! length tests.

pub mod messages;

//* path-stability re-export: the C-string newtype and both
//* quit-message tables keep their module-root paths (m_menu.rs, the
//* sole consumer).
pub use messages::{Ptr, doom1_endmsg, doom2_endmsg};
