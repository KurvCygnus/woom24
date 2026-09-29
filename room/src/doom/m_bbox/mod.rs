//! Bounding-box helpers: `M_ClearBox` initialises an inverted box and
//! `M_AddToBox` expands it to contain a point, over flat
//! `[fixed_t; 4]` arrays indexed by the `BBox` slot constants --
//! bit-exact with `vendor/doomgeneric/m_bbox.c`.
//!
//! ## Submodule Responsibility
//!
//! - `dtmc.rs` -- the extracted demo-synchronization surface: the
//!   pure slice-based `clear_box` / `add_to_box` computation and the
//!   baseline-vector tests
//! - `ffi.rs` -- the `BBox` slot-index constants and the
//!   `#[no_mangle]` `extern "C"` pointer wrappers
//!   (`M_ClearBox` / `M_AddToBox`) marshalling to `dtmc`, plus the
//!   wrapper round-trip test
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::m_bbox::*` -- the function consumers
//! `p_setup.rs:33` and `v_video.rs:14`, the `BBox` constant consumers
//! `p_map.rs:27`, `r_main.rs:15`, `r_bsp/clipper.rs:11` +
//! `r_bsp/traverse.rs:11`, the `c_ffi.rs:14`
//! re-export, and the `c_tests/p_maputl_c.rs:14` test import); no
//! content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_ClearBox` | `ffi::M_ClearBox`, computation extracted to `dtmc::clear_box` | dtmc | the qualifying part (the four sentinel stores) is pure slice computation feeding demo-visible extents; the `#[no_mangle]` pointer wrapper stays in `ffi.rs` (`std::slice::from_raw_parts_mut`, exactly 4 elements) and marshals to `dtmc`; the `i32::MIN`/`i32::MAX` sentinel choice is load-bearing exactness (same class as m_fixed's `FixedMul`) and is preserved verbatim; `#[no_mangle]` retained so the wasm export surface stays byte-identical; upstream `m_bbox.c:29-33` |
//! | `M_AddToBox` | `ffi::M_AddToBox`, computation extracted to `dtmc::add_to_box` | dtmc | the `if`/`else if` asymmetry is vanilla-exact behavior: after the first point, RIGHT/TOP stay inverted until a later point exceeds them -- "fixing" it would change every line-crossing/thing-hit decision the box feeds; pinned by the baseline vectors in `dtmc`'s tests; `#[no_mangle]` retained; upstream `m_bbox.c:35-49` |
//! | `BBox` (Rust-only slot indices) | `ffi::BBox` consts | data | `usize` slot indices matching the `BOXTOP` / `BOXBOTTOM` / `BOXLEFT` / `BOXRIGHT` enum of `m_bbox.h`, each with its `#[doc(alias)]`; consumed directly to index `[fixed_t; 4]` without casts |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! both exported functions keep their C symbol and `#[no_mangle]`.
//! There is no link anchor: both functions are path-referenced (live)
//! through the root re-exports, so none is needed and adding one
//! could change linker decisions. No compiled C translation unit
//! references these symbols (`doomgeneric-sys/build.rs` excludes
//! m_bbox.c), so the retention is pure conservatism (zero wasm
//! export churn).
//!
//! ## Deterministic Aspects
//!
//! Both bodies adjudicated dtmc and extracted as pure computations:
//! their outputs feed demo-visible collision and sight extents --
//! the map bounding box (`p_setup.rs:1056`), the blocktree/blockmap
//! span checks (`p_map.rs:304-307`), the render dirtybox
//! (`v_video.rs:79-80`), and the BSP collantern bounds
//! (`r_main.rs:355-365`, `r_bsp/clipper.rs` `check_bbox`) -- so every
//! comparison direction and sentinel is exactness-bearing. Two
//! details are load-bearing and pinned by the baseline vectors in
//! `dtmc`'s test module (run green against the original bodies
//! before extraction, re-run after -- F10 wave A3): the
//! `i32::MIN`/`i32::MAX` inverted-box sentinels, and the `if`/`else
//! if` asymmetry that leaves RIGHT/TOP inverted after the first
//! point. The extraction takes only the pure computation; the raw
//! pointer marshalling stays behind in `ffi.rs`, byte-exact in the
//! pointer arithmetic it preserves.

pub mod dtmc;
pub mod ffi;

//* path-stability re-export: the C-named wrappers and the slot-index
//* constants keep their module-root paths (p_setup, v_video, p_map,
//* r_main, r_bsp, c_ffi, c_tests/p_maputl_c).
pub use ffi::{BBox, M_AddToBox, M_ClearBox};
