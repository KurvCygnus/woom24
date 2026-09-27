//! Weapon animation state table: the `weaponinfo` array and its
//! `weaponinfo_t` record -- pure data ported from
//! `vendor/doomgeneric/d_items.c`. Each entry describes the ammo type and
//! the state-machine indices for raise, lower, idle, attack, and
//! muzzle-flash animations of one weapon.
//!
//! ## Submodule Responsibility
//!
//! - `table.rs` -- the `weaponinfo_t` struct, the `am_*` ammo-type consts,
//!   `NUMWEAPONS`, the `#[no_mangle]` `weaponinfo` table itself, the
//!   off-by-one history note, and the pinning tests. (One-table module:
//!   everything is data, so there is nothing to split and no `dtmc` --
//!   see Deterministic Aspects.)
//!
//! ## Original Fn Name Mapping
//!
//! No functions exist in this module (upstream `d_items.c` is pure data),
//! so there is nothing to rename under the m_fixed naming pattern -- the
//! whole module is the data tier and keeps its upstream names verbatim.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `weaponinfo` (C array) | `table::weaponinfo` | data | upstream name + `#[no_mangle]` retained; held at the module root by `pub use` (path-stability re-export, data tier) |
//! | `weaponinfo_t` (C struct) | `table::weaponinfo_t` | data | upstream name; `repr(C)` 6 x `c_int` = 24 bytes, pinned by test |
//! | `am_clip` / `am_shell` / `am_cell` / `am_misl` / `am_noammo` | `table` | data | module-private consts, upstream names |
//!
//! ## Deterministic Aspects
//!
//! Adjudicated **no dtmc surface** (wholesale `Surface: data`, the F10
//! `info.rs`/`tables.rs`/`statenum.rs` class): there is no logic, no RNG,
//! and no sequence in this module. The table's VALUES do gate
//! demo-observable behavior -- `p_pspr` selects weapon animation states
//! through `weaponinfo` every raise/lower/fire -- so the only requirement
//! is bit-stability of the 9 x 6 table, which is pinned by the existing
//! tests (length, first/last entry values, and the 24-byte layout pin).

pub mod table;

//* path-stability re-export: the five freeze-zone consumers (st_stuff,
//* p_inter/give, p_pspr/{actions,engine,weapons}) import `weaponinfo`
//* through this module root. Data tier -- upstream name + #[no_mangle]
//* retained at the definition.
pub use table::{weaponinfo, weaponinfo_t};
