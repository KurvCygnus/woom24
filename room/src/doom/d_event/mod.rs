//! Input event handling: the `event_t` struct, a fixed-capacity
//! ring-buffer queue, and the two public functions that post and
//! consume events -- bit-exact with `vendor/doomgeneric/d_event.c`.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the `repr(C)` `event_t` struct with its
//!   data-datum table and the ABI-pinning layout test
//! - `queue.rs` -- the queue capacity and ring state
//!   (`MAXEVENTS`, `EVENTS`, `EVENT_HEAD`, `EVENT_TAIL`),
//!   `D_PostEvent`, `D_PopEvent`, and the queue tests
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic
//! Aspects. The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::d_event::*` -- the `event_t` struct
//! importers `am_map.rs`, `f_finale.rs`, `g_game.rs`, `hu_stuff.rs`,
//! `i_input.rs`, `m_menu.rs`, `st_stuff.rs`, `wi_stuff.rs`, and the
//! `D_PopEvent` import at `d_main.rs:425`); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `D_PostEvent` | `queue::D_PostEvent` | glue | live-input transport: demo playback injects ticcmds from the demo stream through the net loop (`d_net/protocol_types.rs:131` binds `D_ProcessEvents` into the loop interface), never through the event queue -- queue contents are async host timing with no exact-sequence guarantee; `#[no_mangle]` MANDATORY: extern-declared at `i_input.rs:38`, and dropping it would break that link on some targets with no Rust-visible error; upstream `d_event.c:35-39` |
//! | `D_PopEvent` | `queue::D_PopEvent` | glue | same transport argument; the demo-relevant event-to-ticcmd conversion lives in `G_BuildTiccmd`/`G_Responder` (`g_game.rs:1364-1449`, freeze zone), not here; `#[no_mangle]` retained so the wasm export surface stays byte-identical; upstream `d_event.c:43-61` |
//! | `event_t` | `types::event_t` | data | `repr(C)` struct, ABI-pinned at 20 bytes by the moved layout test: it crosses the extern-block boundary at `i_input.rs:38`; the `ev_*` type constants are deliberately not exported (freeze-zone consumers use private copies / raw ints -- a post-graduation cleanup, not this wave) |
//! | `MAXEVENTS` / `EVENTS` / `EVENT_HEAD` / `EVENT_TAIL` | `queue` private statics | data | capacity and ring state, private exactly like the C file-statics (`d_event.c:25-29`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! both exported functions keep their C symbol and `#[no_mangle]`,
//! which keeps the wasm export surface byte-identical. No compiled C
//! translation unit references these symbols
//! (`doomgeneric-sys/build.rs` excludes d_event.c), so the retention
//! is pure conservatism plus the live `i_input` extern declarer.
//!
//! ## Deterministic Aspects
//!
//! Module adjudicated with no `dtmc` surface: the event queue is
//! asynchronous host-input plumbing. Its contents arrive at
//! wall-clock timing (`i_input`'s DG_GetKey pump posts between tics),
//! the ring is deliberately lossy at 64 entries (oldest silently
//! overwritten, matching the C original), and no exact-sequence
//! guarantee exists or is needed -- demo playback never traverses the
//! queue, because the demo stream feeds ticcmds through the net loop
//! (`d_net/protocol_types.rs:131`). The demo-relevant conversion of an event into
//! simulation input happens in the freeze-zone responders
//! (`g_game.rs:1364-1449`), outside this module. The F9 golden demo
//! tests therefore do not exercise this queue's contents.

pub mod queue;
pub mod types;

//* path-stability re-export: the repr(C) event struct keeps its
//* module-root path (am_map, f_finale, g_game, hu_stuff, i_input,
//* m_menu, st_stuff, wi_stuff).
pub use types::event_t;

//* path-stability re-export: the queue entry points keep their
//* module-root paths (d_main.rs:425 drains via D_PopEvent; i_input
//* posts through the extern-block symbol, not this path).
pub use queue::{D_PopEvent, D_PostEvent};
