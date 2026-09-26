//! 16.16 fixed-point arithmetic for the DOOM engine: the shared
//! vocabulary (`fixed_t`, `FRACBITS`, `FRACUNIT`) plus the graduated
//! demo-synchronization surface in `dtmc`, bit-exact with
//! `vendor/doomgeneric/m_fixed.c`.
//!
//! ## Submodule Responsibility
//!
//! - `defs.rs` -- shared fixed-point vocabulary: the `fixed_t` /
//!   `angle_t` type aliases and the `FRACBITS` / `FRACUNIT`
//!   constants, re-exported at the module root so freeze-zone import
//!   paths stay valid
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the `defs` re-exports, and the two upstream-name
//! shims below; no content lives here. `dtmc` holds the module's
//! whole qualifying surface (F10 pilot) and is covered by
//! Deterministic Aspects.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location        | Surface | Notes                                                                                                                  |
//! |--------------|---------------------|---------|------------------------------------------------------------------------------------------------------------------------|
//! | `FixedMul`   | `dtmc::fixed_mul`   | dtmc    | pure 16.16 multiply feeding movement/angle/damage every tic; `#[no_mangle]` dropped with the rename; zero C/wasm export consumers; upstream `vendor/doomgeneric/m_fixed.c:33-39` |
//! | `FixedDiv`   | `dtmc::fixed_div`   | dtmc    | `>> 14` saturation guard + 64-bit division, demo-observable; `#[no_mangle]` dropped with the rename; zero C/wasm export consumers; upstream `vendor/doomgeneric/m_fixed.c:47-61` |
//! | `FixedDiv2`  | -- (never ported)    | --       | absent from this port: the vendored C is the Chocolate-style version with only `FixedMul`/`FixedDiv`, so vanilla's asm `FixedDiv2` (the `I_Error` abort path) has no counterpart and no Rust callers -- nothing to extract |
//! | `fixed_t`    | `defs::fixed_t`     | data    | type alias stayed `fixed_t` (spec's `FixedT` rename rejected -- would have rippled into freeze-zone callers); housed in `defs.rs` with the constants, re-exported at the root |
//! | `angle_t`    | `defs::angle_t`     | data    | BAM angle alias (upstream `tables.h`), housed beside `fixed_t` as shared vocabulary |
//!
//! ## Deterministic Aspects
//!
//! `dtmc::fixed_mul` and `dtmc::fixed_div` are pure integer functions on
//! the demo synchronization surface: movement (`p_map`, `p_user`,
//! `p_mobj`), aiming and attacks (`p_enemy`, `p_pspr`), damage and
//! pickup scaling (`p_inter`), and sight/render math (`p_sight`, `r_*`)
//! all consume their results inside the 35 tics/s simulation, so every
//! wrap, shift, and saturation must stay bit-exact across hosts and
//! targets. LSB exactness is load-bearing: `fixed_mul` computes the
//! product in a full 64-bit intermediate before the arithmetic (floor)
//! shift, and `fixed_div` routes overflow through the `>> 14` guard --
//! pinned by the baseline vectors in `dtmc`'s test module, written and
//! run against the original bodies before extraction (F10 pilot).

pub mod defs;
pub mod dtmc;

//* path-stability re-export: the shared vocabulary keeps its
//* module-root paths for the freeze-zone callers (`p_*.rs`,
//* `am_map.rs`, `c_ffi.rs`, `m_bbox.rs`, ...).
pub use defs::{angle_t, fixed_t, FRACBITS, FRACUNIT};

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use dtmc::fixed_div as FixedDiv;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use dtmc::fixed_mul as FixedMul;
