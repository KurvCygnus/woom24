//! Heads-up text and input code: text lines, scrolling text, and input widgets.
//!
//! Rust port of `vendor/doomgeneric/hu_lib.c`. Three widget types are provided,
//! mirroring the C originals:
//! * [`types::hu_textline_t`] - a single line of patch-font text drawn at a fixed
//!   screen position; parent type for the other two.
//! * [`types::hu_stext_t`] - a scrolling message widget backed by a ring of up to
//!   `HU_MAXLINES` (4) text lines.
//! * [`types::hu_itext_t`] - a text-input widget with a protected left-margin prefix
//!   (used for chat entry and cheat codes).
//!
//! Rust differences from C: `boolean` is `c_int` (0/1); padding fields have
//! been added to maintain identical ABI layout on 64-bit targets (verified by
//! compile-time `assert!` in `types::layout_checks`).
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the three `repr(C)` widget types (root-re-exported), the
//!   `HU_MAXLINES`/`HU_MAXLINELENGTH` capacity constants, and the
//!   `layout_checks` compile-time ABI guards
//! - `textline.rs` -- the seven text-line primitives (init/clear/add/del/
//!   draw/erase), including the dead-but-exported erase entry
//! - `stext.rs` -- the five scrolling-text primitives
//! - `itext.rs` -- the eight text-input primitives
//!
//! The module root holds the FFI surface carried VERBATIM from the
//! pre-split file: `V_DrawPatchDirect`/`R_VideoErase` (r_draw pins) and
//! the five view statics (`automapactive` from am_map, the r_draw
//! view-window quartet) -- all linked BY SYMBOL, so the graduated
//! modules' `#[no_mangle]`/`#[export_name]` retention is the only thing
//! keeping them resolving. The in-tree `R_VideoErase` callers are exactly
//! the dead erase chain in `textline.rs`.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the upstream-name shim re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module (`doomgeneric.rs:35/:263`
//! takes `HUlib_init`'s address as the module's link anchor through the
//! root shim; `hu_stuff` imports fourteen of the twenty). No address
//! compares exist inside the module.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `HUlib_init` | `textline::init_library` | glue | no-op, matching C; the doomgeneric link anchor resolves through the root shim; shim + pin |
//! | `HUlib_clearTextLine` | `textline::clear_text_line` | glue | shim + pin |
//! | `HUlib_initTextLine` | `textline::init_text_line` | glue | shim + pin |
//! | `HUlib_addCharToTextLine` | `textline::add_char` | glue | capacity-guarded append; shim + pin |
//! | `HUlib_delCharFromTextLine` | `textline::del_char` | glue | shim + pin |
//! | `HUlib_drawTextLine` | `textline::draw_text_line` | glue | patch-font draw with cursor glyph; shim + pin |
//! | `HUlib_eraseTextLine` | `textline::erase_text_line` | glue | dead-but-exported (zero callers in the tree; vendor d_main.c:208 calls `HU_Erase`, which our display never ported): kept for symbol-set byte-identity, retires with the freeze zone; the `viewwindowx != 0` gate is a crispy-era conditional kept verbatim; shim + pin |
//! | `HUlib_initSText` | `stext::init_stext` | glue | shim + pin |
//! | `HUlib_addLineToSText` | `stext::add_line` | glue | ring-buffer advance; shim + pin |
//! | `HUlib_addMessageToSText` | `stext::add_message` | glue | shim + pin |
//! | `HUlib_drawSText` | `stext::draw_stext` | glue | shim + pin |
//! | `HUlib_eraseSText` | `stext::erase_stext` | glue | dead-but-exported (zero callers in the tree): kept for symbol-set byte-identity, retires with the freeze zone; shim + pin |
//! | `HUlib_initIText` | `itext::init_itext` | glue | shim + pin |
//! | `HUlib_delCharFromIText` | `itext::del_char_respecting_margin` | glue | shim + pin |
//! | `HUlib_eraseLineFromIText` | `itext::erase_line_to_margin` | glue | shim + pin |
//! | `HUlib_resetIText` | `itext::reset_itext` | glue | shim + pin |
//! | `HUlib_addPrefixToIText` | `itext::add_prefix` | glue | locks the left margin; shim + pin |
//! | `HUlib_keyInIText` | `itext::key_in_itext` | glue | the chat-input key surface; shim + pin |
//! | `HUlib_drawIText` | `itext::draw_itext` | glue | shim + pin |
//! | `HUlib_eraseIText` | `itext::erase_itext` | glue | dead-but-exported (zero callers in the tree): kept for symbol-set byte-identity, retires with the freeze zone; shim + pin |
//! | `hu_textline_t`/`hu_stext_t`/`hu_itext_t` | `types` | data | `repr(C)` ABI mirrors; root-re-exported for `hu_stuff` |
//! | `HU_MAXLINES`/`HU_MAXLINELENGTH` | `types` | data | capacity vocabulary beside the types they size |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface, by adjudication (report §3.4): all twenty functions
//! are glue -- no RNG, no simulation reads. The erase chain's only
//! determinism-adjacent behavior (R_VideoErase into the hashed frame)
//! is currently unreachable (dead-but-exported, see the mapping table);
//! the draw path is frame-golden-relevant (HUD text sits inside the
//! `e1m1_combat`/`title_attract` `frame_hash` anchors) with ordering
//! owned by the graduated `d_main/display.rs` conductor. There is
//! deliberately no `dtmc` submodule and no extraction candidates.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;

pub mod itext;
pub mod stext;
pub mod textline;
pub mod types;

extern "C" {
    /// Draw a patch at (`x`, `y`) directly to the screen buffer (v_video.c).
    fn V_DrawPatchDirect(x: c_int, y: c_int, patch: *mut crate::doom::v_video::patch_t);
    /// Erase `count` bytes of the screen buffer starting at byte offset `ofs`
    /// by copying from the background buffer (r_draw.c).
    fn R_VideoErase(ofs: c_uint, count: c_int);
    /// Non-zero while the automap overlay is active (am_map.c).
    static mut automapactive: c_int;
    /// X offset of the rendered view within the full screen (r_main.c).
    static mut viewwindowx: c_int;
    /// Y offset of the rendered view within the full screen (r_main.c).
    static mut viewwindowy: c_int;
    /// Width of the rendered view in pixels (r_main.c).
    static mut viewwidth: c_int;
    /// Height of the rendered view in pixels (r_main.c).
    static mut viewheight: c_int;
}

use std::ffi::c_uint;

/// Byte-swap for little-endian (SHORT macro from i_swap.h).
#[inline(always)]
pub(super) fn short_swap(v: i16) -> i16 {
    v
}

//* upstream-name shim: all twenty entry points keep their freeze-zone
//* caller paths (`crate::doom::hu_lib::HUlib_*`; sole in-tree consumer
//* `hu_stuff` imports fourteen, `doomgeneric.rs` takes `HUlib_init`'s
//* address as the link anchor) and their C symbols stay re-pinned at
//* the definitions with `#[export_name = "OriginalName"]` for the
//* oracle/cdylib surface. Shims die with the freeze zone.
pub use itext::{
    add_prefix as HUlib_addPrefixToIText, del_char_respecting_margin as HUlib_delCharFromIText,
    draw_itext as HUlib_drawIText, erase_itext as HUlib_eraseIText,
    erase_line_to_margin as HUlib_eraseLineFromIText, init_itext as HUlib_initIText,
    key_in_itext as HUlib_keyInIText, reset_itext as HUlib_resetIText,
};
pub use stext::{
    add_line as HUlib_addLineToSText, add_message as HUlib_addMessageToSText,
    draw_stext as HUlib_drawSText, erase_stext as HUlib_eraseSText, init_stext as HUlib_initSText,
};
pub use textline::{
    add_char as HUlib_addCharToTextLine, clear_text_line as HUlib_clearTextLine,
    del_char as HUlib_delCharFromTextLine, draw_text_line as HUlib_drawTextLine,
    erase_text_line as HUlib_eraseTextLine, init_library as HUlib_init,
    init_text_line as HUlib_initTextLine,
};
//* path-stability re-export: the widget types keep their module-root
//* paths (`hu_stuff`'s import block, the layout tests).
pub use types::{hu_itext_t, hu_stext_t, hu_textline_t};
