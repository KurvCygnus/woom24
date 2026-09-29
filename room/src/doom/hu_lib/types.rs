//! The three `repr(C)` HUD widget types, their capacity constants, and
//! the compile-time ABI layout guards.

use std::ffi::{c_char, c_int};

use crate::doom::v_video::patch_t;

/// Maximum number of text lines in a scrolling text widget (`hu_stext_t`).
/// Mirrors the C constant `HU_MAXLINES` from `hu_lib.h`.
pub(super) const HU_MAXLINES: usize = 4;

/// Maximum number of characters per text line, excluding the NUL terminator.
/// Mirrors the C constant `HU_MAXLINELENGTH` from `hu_lib.h`.
pub(super) const HU_MAXLINELENGTH: usize = 80;

/// A single line of text rendered with a patch font.
///
/// Corresponds to `hu_textline_t` in `hu_lib.h`. All other HUD text widgets
/// embed or inherit this struct. The layout is identical to the C struct on
/// 64-bit targets (verified by `layout_checks`).
///
/// # Layout invariants
/// * `l[len] == 0` at all times (NUL-terminated prefix).
/// * `len <= HU_MAXLINELENGTH` (the array has `HU_MAXLINELENGTH + 1` elements).
/// * `f` points into the `hu_font` array and must not be null when drawing.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_textline_t {
    /// Left-justified screen X position of the text line.
    pub x: c_int,
    /// Screen Y position of the text line.
    pub y: c_int,
    /// Pointer to the start of the font patch array; `f.add(c - sc)` yields a
    /// pointer to the patch for character `c`. Mirrors `patch_t **f` in
    /// `hu_textline_t`. Access requires pointer arithmetic - direct indexing is
    /// unsafe.
    pub f: *mut *mut patch_t,
    /// ASCII code of the first character in the font array (`l->sc` in C).
    /// Characters below this value are rendered as spaces.
    pub sc: c_int,
    /// NUL-terminated text buffer; `l[0..len]` holds the visible characters.
    pub l: [c_char; HU_MAXLINELENGTH + 1],
    /// Number of valid characters currently in `l` (excludes the NUL terminator).
    pub len: c_int,
    /// Dirty-flag countdown: non-zero means the line must be redrawn / erased.
    /// Set to 4 on modification, decremented by `erase_text_line` each
    /// frame until it reaches 0.
    pub needsupdate: c_int,
}

/// A scrolling message widget backed by a ring of text lines.
///
/// Corresponds to `hu_stext_t` in `hu_lib.h`. Lines are stored in a circular
/// buffer; `cl` is the index of the most-recently-added line, and older lines
/// are at `(cl - i + h) % h` for `i in 1..h`.
///
/// # Layout invariants
/// * `h <= HU_MAXLINES`.
/// * `cl` is always in `0..h`.
/// * `on` is a non-null pointer to a `c_int` flag; 0 = hidden, non-zero = visible.
/// * The four-byte `_pad` field exists solely to match the C ABI on 64-bit
///   platforms where `boolean` following a pointer leaves a gap.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_stext_t {
    /// Ring of text lines; only `l[0..h]` are used.
    pub l: [hu_textline_t; HU_MAXLINES],
    /// Height of the widget in lines (`s->h` in C).
    pub h: c_int,
    /// Index of the current (most recently written) line in `l` (`s->cl` in C).
    pub cl: c_int,
    /// Pointer to the visibility flag; widget is drawn only when `*on != 0`.
    pub on: *mut c_int,
    /// Cached value of `*on` from the previous frame, used to detect
    /// transitions from visible to hidden so dirty flags can be set.
    pub laston: c_int,
    /// ABI-alignment filler (C `boolean` after a pointer leaves a gap).
    pub _pad: [u8; 4],
}

/// A text-input widget with a protected prefix region.
///
/// Corresponds to `hu_itext_t` in `hu_lib.h`. Used for chat entry and
/// (conceptually) cheat-code input. The first `lm` characters of `l` form
/// an immutable prefix set by [`crate::doom::hu_lib::itext::add_prefix`]; delete operations
/// via [`crate::doom::hu_lib::itext::del_char_respecting_margin`] and
/// [`crate::doom::hu_lib::itext::erase_line_to_margin`] refuse to go past this left margin.
///
/// # Layout invariants
/// * `lm <= l.len` at all times.
/// * `on` must be a non-null pointer.
/// * `_pad0` and `_pad1` are ABI-alignment fillers only.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_itext_t {
    /// The underlying text line that receives key input.
    pub l: hu_textline_t,
    /// Left-margin character count: characters at indices `0..lm` are
    /// protected from deletion (`it->lm` in C).
    pub lm: c_int,
    /// ABI-alignment filler.
    pub _pad0: [u8; 4],
    /// Pointer to the visibility flag; widget is drawn only when `*on != 0`.
    pub on: *mut c_int,
    /// Cached value of `*on` from the previous frame (see [`hu_stext_t::laston`]).
    pub laston: c_int,
    /// ABI-alignment filler.
    pub _pad1: [u8; 4],
}

#[cfg(target_pointer_width = "64")]
mod layout_checks {
    use super::*;
    const _: () = assert!(std::mem::size_of::<hu_textline_t>() == 112);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, x) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, y) == 4);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, f) == 8);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, sc) == 16);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, l) == 20);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, len) == 104);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, needsupdate) == 108);

    const _: () = assert!(std::mem::size_of::<hu_stext_t>() == 472);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, l) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, h) == 448);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, cl) == 452);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, on) == 456);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, laston) == 464);

    const _: () = assert!(std::mem::size_of::<hu_itext_t>() == 136);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, l) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, lm) == 112);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, on) == 120);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, laston) == 128);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const HU_TEXTLINE_T_SIZEOF: usize = 112;
    const HU_STEXT_T_SIZEOF: usize = 472;
    const HU_ITEXT_T_SIZEOF: usize = 136;

    #[test]
    fn hu_textline_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_textline_t>(), HU_TEXTLINE_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_textline_t, x), 0);
        assert_eq!(std::mem::offset_of!(hu_textline_t, y), 4);
        assert_eq!(std::mem::offset_of!(hu_textline_t, f), 8);
        assert_eq!(std::mem::offset_of!(hu_textline_t, sc), 16);
        assert_eq!(std::mem::offset_of!(hu_textline_t, l), 20);
        assert_eq!(std::mem::offset_of!(hu_textline_t, len), 104);
        assert_eq!(std::mem::offset_of!(hu_textline_t, needsupdate), 108);
    }

    #[test]
    fn hu_stext_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_stext_t>(), HU_STEXT_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_stext_t, l), 0);
        assert_eq!(std::mem::offset_of!(hu_stext_t, h), 448);
        assert_eq!(std::mem::offset_of!(hu_stext_t, cl), 452);
        assert_eq!(std::mem::offset_of!(hu_stext_t, on), 456);
        assert_eq!(std::mem::offset_of!(hu_stext_t, laston), 464);
    }

    #[test]
    fn hu_itext_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_itext_t>(), HU_ITEXT_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_itext_t, l), 0);
        assert_eq!(std::mem::offset_of!(hu_itext_t, lm), 112);
        assert_eq!(std::mem::offset_of!(hu_itext_t, on), 120);
        assert_eq!(std::mem::offset_of!(hu_itext_t, laston), 128);
    }
}
