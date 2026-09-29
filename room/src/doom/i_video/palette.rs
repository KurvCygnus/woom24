//! The active palette: the gamma-corrected 256-entry `COLORS` table, its
//! entry type, and the palette query entry points.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;
use std::ptr;

use super::buffer::usegamma;
use crate::doom::tables::gammatable;

/// 32-bit pixel as the framebuffer expects it: BGRA with alpha last.
///
/// Mirrors `struct color` in `i_video.c`. `#[repr(C)]` because
/// pointers into this layout cross the FFI boundary (the
/// `DG_ScreenBuffer` is written byte by byte but interpreted as
/// 32-bit pixels by the host platform's display code).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Color {
    pub(super) b: u8,
    pub(super) g: u8,
    pub(super) r: u8,
    pub(super) a: u8,
}

/// Active 256-entry gamma-corrected palette. Filled from the WAD
/// PLAYPAL by `set_palette` and consumed by `cmap_to_framebuffer` /
/// `cmap_to_rgb565`. Mirrors the C `colors[256]` static array.
pub(super) static mut COLORS: [Color; 256] = [Color {
    b: 0,
    g: 0,
    r: 0,
    a: 0,
}; 256];

/// Apply a new 768-byte (256 x R/G/B) PLAYPAL palette by
/// gamma-correcting each component through `gammatable[usegamma]`
/// and storing into `COLORS`. Mirrors `I_SetPalette` from `i_video.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// `palette` must point to at least `256 * 3` readable bytes. The
/// function trusts `usegamma` to index `gammatable` in range.
#[doc(alias = "I_SetPalette")]
#[export_name = "I_SetPalette"]
pub unsafe extern "C" fn set_palette(palette: *mut u8) {
    let mut p = palette;
    for i in 0..256 {
        COLORS[i].a = 0;
        COLORS[i].r = gammatable[usegamma as usize][*p as usize];
        p = p.add(1);
        COLORS[i].g = gammatable[usegamma as usize][*p as usize];
        p = p.add(1);
        COLORS[i].b = gammatable[usegamma as usize][*p as usize];
        p = p.add(1);
    }
}

/// Return the palette index closest to the given RGB triple.
///
/// Mirrors `I_GetPaletteIndex` in `i_video.c`: linear scan over the
/// 256-entry active palette, returning the index with the smallest
/// squared RGB distance. Exits early on the first exact match. The C
/// source searches the gamma-corrected `rgb565_palette`; this port
/// searches the gamma-corrected `COLORS` array directly (no 5/6/5
/// quantization because the Rust framebuffer is 32 bpp).
///
/// The diff is accumulated in `i64` so out-of-range `r`/`g`/`b` values
/// (the C ABI accepts any `int`) cannot overflow even though all real
/// callers pass values in `0..=255`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `v_video` imports the upstream name through the root shim.
///
/// # Safety
///
/// Reads `static mut COLORS`. The caller must have run `set_palette`
/// at least once and must not race a concurrent writer.
#[doc(alias = "I_GetPaletteIndex")]
#[export_name = "I_GetPaletteIndex"]
pub unsafe extern "C" fn get_palette_index(r: c_int, g: c_int, b: c_int) -> c_int {
    let r = r as i64;
    let g = g as i64;
    let b = b as i64;
    // `best = 0` is a sound default only because the palette is
    // always 256 entries and the loop visits all of them.
    let mut best: c_int = 0;
    let mut best_diff: i64 = i64::MAX;
    let colors = ptr::addr_of!(COLORS) as *const Color;
    for i in 0..256 {
        let color = colors.add(i).read();
        let dr = r - color.r as i64;
        let dg = g - color.g as i64;
        let db = b - color.b as i64;
        let diff = dr * dr + dg * dg + db * db;
        if diff < best_diff {
            best = i as c_int;
            best_diff = diff;
        }
        if diff == 0 {
            break;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// `COLORS` is a `static mut` shared across the whole crate; the
    /// tests below mutate it, so they must run serially.
    static PALETTE_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that owns the palette lock for the duration of a
    /// test and restores the `COLORS` snapshot captured at acquisition
    /// time on drop, even on panic. Keeps the global palette
    /// invisible to subsequent tests in the same binary.
    struct PaletteGuard {
        _lock: MutexGuard<'static, ()>,
        saved: [Color; 256],
    }

    impl PaletteGuard {
        fn acquire() -> Self {
            let lock = PALETTE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let saved = unsafe { ptr::addr_of!(COLORS).read() };
            Self { _lock: lock, saved }
        }
    }

    impl Drop for PaletteGuard {
        fn drop(&mut self) {
            unsafe { ptr::addr_of_mut!(COLORS).write(self.saved) };
        }
    }

    fn set_test_palette(entries: &[(u8, u8, u8)]) {
        let colors = ptr::addr_of_mut!(COLORS) as *mut Color;
        unsafe {
            for i in 0..256 {
                let (r, g, b) = entries.get(i).copied().unwrap_or((0, 0, 0));
                colors.add(i).write(Color { b, g, r, a: 0 });
            }
        }
    }

    #[test]
    fn get_palette_index_nearest_rgb() {
        let _guard = PaletteGuard::acquire();
        let palette: [(u8, u8, u8); 4] = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255)];
        set_test_palette(&palette);

        unsafe {
            assert_eq!(get_palette_index(0, 0, 0), 0, "exact black");
            assert_eq!(get_palette_index(255, 0, 0), 1, "exact red");
            assert_eq!(get_palette_index(0, 255, 0), 2, "exact green");
            assert_eq!(get_palette_index(0, 0, 255), 3, "exact blue");

            assert_eq!(get_palette_index(250, 5, 5), 1, "near red");
            assert_eq!(get_palette_index(5, 250, 5), 2, "near green");
            assert_eq!(get_palette_index(10, 10, 10), 0, "near black");

            assert_eq!(
                get_palette_index(128, 0, 0),
                1,
                "128 is one step closer to 255 (diff 127^2) than to 0 (diff 128^2)"
            );
            assert_eq!(
                get_palette_index(127, 0, 0),
                0,
                "127 is one step closer to 0 than to 255"
            );

            // Out-of-range inputs must not overflow the diff accumulator.
            assert_eq!(get_palette_index(c_int::MAX, 0, 0), 1, "saturate to red");
            assert_eq!(get_palette_index(c_int::MIN, 0, 0), 0, "saturate to black");
        }
    }

    #[test]
    fn get_palette_index_returns_first_exact_match() {
        let _guard = PaletteGuard::acquire();

        // Index 42 is the target; index 200 is a duplicate. Strict
        // `diff < best_diff` plus the outer-scope `if diff == 0
        // { break }` together guarantee the first exact match wins
        // and the loop terminates there; this test pins the
        // first-match contract so regressions in either branch show
        // up as a wrong index.
        let mut palette: Vec<(u8, u8, u8)> = (0..256).map(|_| (200, 0, 0)).collect();
        palette[42] = (42, 0, 0);
        palette[200] = (42, 0, 0);
        set_test_palette(&palette);

        unsafe {
            assert_eq!(get_palette_index(42, 0, 0), 42);
        }
    }
}
