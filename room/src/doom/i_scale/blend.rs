//! The palette blend machinery: nearest-colour search, the stretch/squash
//! lookup tables, and their lazy generation / reset entry points.

#![allow(non_upper_case_globals)]

use crate::doom::crt::c_printf;
use crate::doom::z_zone::{Z_Free, Z_Malloc, PU_STATIC};
use std::ffi::{c_int, c_void};
use std::ptr;

/// `stdout` for the progress-print flushes. UCRT exposes no `stdout`
/// data symbol, so Windows flushes the NULL stream (i.e. all streams)
/// instead of the POSIX `stdout` object.
unsafe fn stdout_stream() -> *mut libc::FILE
{
    #[cfg(unix)]
    {
        extern "C" { static mut stdout: *mut libc::FILE; }
        std::ptr::addr_of_mut!(stdout)
    }
    //* wasm32 is neither unix nor windows: without this branch the function
    //* body would be empty on ILP32 (E0308). The wasm fflush shim ignores the
    //* stream argument, so the null semantics match the windows branch.
    #[cfg(not(unix))] { ptr::null_mut() }
}

/// 20/80 and 40/60 palette blend tables. `stretch_tables[0]` is the
/// 20/80 mix, `stretch_tables[1]` is the 40/60 mix; the 60/40 and 80/20
/// mixes are reached by swapping the two source pixels when indexing.
/// Each table is `256 * 256` bytes mapped to the nearest palette index.
/// Lazily populated by [`init_stretch_tables`].
pub(super) static mut stretch_tables: [*mut u8; 2] = [ptr::null_mut(); 2];

/// 50/50 palette blend table used only by the `mode_squash_3x` mode
/// (800x600). Lazily populated by [`init_squash_table`].
pub(super) static mut half_stretch_table: *mut u8 = ptr::null_mut();

/// Returns the palette index whose RGB triple has the smallest squared
/// Euclidean distance to `(r, g, b)`. Exact matches short-circuit.
///
/// `palette` must point to 256 contiguous `(R, G, B)` byte triples.
///
/// # Safety
///
/// `palette` must point to at least `256 * 3` readable bytes.
unsafe fn find_nearest_color(palette: *mut u8, r: c_int, g: c_int, b: c_int) -> c_int {
    let mut best: c_int = 0;
    let mut best_diff = c_int::MAX;
    for i in 0..256 {
        let col = palette.add(i * 3);
        let dr = r - *col as c_int;
        let dg = g - *col.add(1) as c_int;
        let db = b - *col.add(2) as c_int;
        let diff = dr * dr + dg * dg + db * db;
        if diff == 0 {
            return i as c_int;
        }
        if diff < best_diff {
            best = i as c_int;
            best_diff = diff;
        }
    }
    best
}

/// Builds a 256x256 palette-mix lookup table whose `(x, y)` entry is
/// the palette index closest to `pct` of palette colour `x` plus
/// `100 - pct` of palette colour `y`.
///
/// The result is allocated from the zone heap (`Z_Malloc`, `PU_STATIC`)
/// and returned as a raw owning pointer; callers are responsible for
/// passing it to `Z_Free` when discarding it.
///
/// This is the same construction used in other Doom source ports for
/// translucency tables.
///
/// # Safety
///
/// `palette` must satisfy [`find_nearest_color`]'s contract.
unsafe fn generate_stretch_table(palette: *mut u8, pct: c_int) -> *mut u8 {
    let result = Z_Malloc(256 * 256, PU_STATIC, ptr::null_mut()) as *mut u8;
    for x in 0..256 {
        for y in 0..256 {
            let col1 = palette.add(x * 3);
            let col2 = palette.add(y * 3);
            let r = ((*col1 as c_int) * pct + (*col2 as c_int) * (100 - pct)) / 100;
            let g = ((*col1.add(1) as c_int) * pct + (*col2.add(1) as c_int) * (100 - pct)) / 100;
            let b = ((*col1.add(2) as c_int) * pct + (*col2.add(2) as c_int) * (100 - pct)) / 100;
            *result.add(x * 256 + y) = find_nearest_color(palette, r, g, b) as u8;
        }
    }
    result
}

/// `init_mode` callback for every `mode_stretch_*` and the 1x/2x/4x/5x
/// `mode_squash_*` modes. Lazily generates the 20/80 and 40/60 palette
/// blend tables, printing a one-line progress message. No-op once the
/// tables exist; `I_ResetScaleTables` must be called first to force
/// regeneration after a palette switch.
///
/// # Safety
///
/// See [`generate_stretch_table`].
pub(super) unsafe extern "C" fn init_stretch_tables(palette: *mut u8) {
    if !stretch_tables[0].is_null() {
        return;
    }
    c_printf(c"I_InitStretchTables: Generating lookup tables..".as_ptr());
    libc::fflush(stdout_stream());
    stretch_tables[0] = generate_stretch_table(palette, 20);
    c_printf(c"..".as_ptr());
    libc::fflush(stdout_stream());
    stretch_tables[1] = generate_stretch_table(palette, 40);
    libc::puts(c"".as_ptr());
}

/// `init_mode` callback for [`super::drivers::mode_squash_3x`] (800x600).
/// Lazily generates only the 50/50 blend table used by that mode's column
/// expansion. No-op once the table exists.
///
/// # Safety
///
/// See [`generate_stretch_table`].
pub(super) unsafe extern "C" fn init_squash_table(palette: *mut u8) {
    if !half_stretch_table.is_null() {
        return;
    }
    c_printf(c"I_InitSquashTable: Generating lookup table..".as_ptr());
    libc::fflush(stdout_stream());
    half_stretch_table = generate_stretch_table(palette, 50);
    libc::puts(c"".as_ptr());
}

/// Frees whichever blend tables are currently populated and regenerates
/// them from `palette`. Called by `i_video` after a palette switch (for
/// example, exiting the palette-shifting menus) so that subsequent
/// stretch/squash output stays close to the in-game colour set.
///
/// Tables that were never allocated stay null - this is not a forced
/// generation, only a refresh.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export
/// symbol is kept with `#[export_name]` below.
///
/// # Safety
///
/// - `palette` must satisfy `find_nearest_color`'s contract.
/// - Any pointers previously handed out from the `stretch_tables` /
///   `half_stretch_table` statics are invalidated by this call.
#[doc(alias = "I_ResetScaleTables")]
#[export_name = "I_ResetScaleTables"]
pub unsafe extern "C" fn reset_scale_tables(palette: *mut u8) {
    if !stretch_tables[0].is_null() {
        Z_Free(stretch_tables[0] as *mut c_void);
        Z_Free(stretch_tables[1] as *mut c_void);
        c_printf(c"I_ResetScaleTables: Regenerating lookup tables..\n".as_ptr());
        stretch_tables[0] = generate_stretch_table(palette, 20);
        stretch_tables[1] = generate_stretch_table(palette, 40);
    }
    if !half_stretch_table.is_null() {
        Z_Free(half_stretch_table as *mut c_void);
        c_printf(c"I_ResetScaleTables: Regenerating lookup table..\n".as_ptr());
        half_stretch_table = generate_stretch_table(palette, 50);
    }
}
