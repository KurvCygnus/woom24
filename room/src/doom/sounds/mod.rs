//! Rust port of `vendor/doomgeneric/sounds.c` plus the `sounds.h` enums.
//!
//! Holds the static SFX (`S_sfx`) and music (`S_music`) tables consumed by
//! `s_sound.rs` and the low-level audio drivers, plus the matching id enums
//! (`Sfx`, `Mus`). Layout of `SfxInfo`/`MusicInfo` exactly mirrors
//! `sfxinfo_t`/`musicinfo_t` so the tables can be handed to C-linkage code
//! unchanged.
//!
//! Sound lump names live in the WAD as `DS<short_name>` (SFX) and
//! `D_<short_name>` (music); only the short part is stored here. Each
//! per-name byte array is exposed as a `const` so it has a stable address
//! the table can take a pointer to.
//!
//! Rust-vs-C differences:
//! - Names are built with the `const fn name::<N>` helper at compile time,
//!   producing fixed-size `[c_char; N]` arrays instead of C string literals.
//! - The C source initialises every `sfxinfo_t` field by hand. Here, the
//!   `SfxInfo::new` / `with_pitch` / `with_volume` builders fill defaults
//!   matching `S_sfx[]` in `sounds.c` (pitch = -1, volume = -1, etc.).
//!
//! ## Submodule Responsibility
//!
//! - `enums.rs` -- the `Sfx` / `Mus` id enums (with their `sfxenum_t` /
//!   `musicenum_t` doc aliases) and the `NUMSFX` / `NUMMUSIC` count
//!   constants with their C-size pin asserts
//! - `sfx_table.rs` -- the `SfxInfo` record, its builders, the shared
//!   `name::<N>` const helper (housed beside its heaviest users; `pub(super)`
//!   for `music_table`), the `N_*` short-name buffers, and `S_sfx`
//! - `music_table.rs` -- the `MusicInfo` record, its builders, the `MUS_*`
//!   short-name buffers, and `S_music`
//! - `links.rs` -- `init_sfx_links`, the runtime wiring of the
//!   self-referential `S_sfx` link entries
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! The module is data wholesale (the `info.rs` / `tables.rs` precedent: no
//! logic to split, `Surface: data`). Every type, constant, and table keeps
//! its upstream name byte-for-byte, so there are no shims for them -- only
//! path-stability re-exports. One function exists:
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `S_InitSfxLinks` | `links::init_sfx_links` | glue | one-line boot-time wiring of `sfx_chgun -> sfx_pistol`, runs inside `S_Init` before any `S_StartSound`; C symbol pinned via `#[export_name]` (callers `s_sound.rs`, `tables.rs` import the upstream name through the root shim); C does this in the `SOUND_LINK` initializer (`sounds.c:205`), Rust needs runtime init because `link` is self-referential; upstream `vendor/doomgeneric/sounds.c` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the tables feed the audio driver, and sound state never
//! enters demo bytes. The table data itself is upstream content, not a
//! workaround; given identical WAD inputs the tables are immutable after
//! `S_Init`'s `usefulness` reset and `init_sfx_links` wiring (both boot-time,
//! pre-first-tic), so nothing here participates in a tic's observables.
//! Layout is pinned to the C structs by the per-file tests (64-byte
//! `SfxInfo`, 32-byte `MusicInfo`, 109/68 table lengths) -- that IS the
//! baseline; it reruns unchanged after graduation.

pub mod enums;
pub mod links;
pub mod music_table;
pub mod sfx_table;

//* path-stability re-export: the shared tables keep their module-root
//* paths for the freeze-zone callers (`info.rs`, `tables.rs`,
//* `hu_stuff`, `m_menu`, `wi_stuff`, `f_finale`, and the graduated
//* `i_sound` / `s_sound` / `p_*` sound callers).
pub use enums::{Mus, Sfx, NUMMUSIC, NUMSFX};
pub use music_table::{MusicInfo, S_music};
pub use sfx_table::{S_sfx, SfxInfo};

//* upstream-name shim: freeze-zone callers keep the upstream name. The C
//* symbol it forwards to is re-pinned at the definition with
//* `#[export_name = "S_InitSfxLinks"]`.
pub use links::init_sfx_links as S_InitSfxLinks;
