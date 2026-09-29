//! The melt wipe (`wipeno` 1): screen columns slide downward at
//! seeded speeds, revealing the new frame beneath the old one. The
//! seeding is the module's dtmc surface -- see `super::dtmc`.

use std::ffi::c_void;
use std::mem::size_of;
use std::os::raw::c_int;

use super::dtmc;
use super::{WIPE_SCR, WIPE_SCR_END, WIPE_SCR_START, Y};
use crate::doom::m_random::M_Random;
use crate::doom::z_zone::PU_STATIC;

/// Transpose a `width x height` array of `i16` from row-major to column-major
/// in-place.
///
/// Allocates a temporary buffer via `Z_Malloc`, performs the transposition,
/// copies the result back, then frees the buffer.  The function is named after
/// the original C identifier `wipe_shittyColMajorXform`.
///
/// Preconditions: `array` must point to at least `width * height` valid `i16`
/// elements; `Z_Malloc` must succeed.
///
/// C origin: `wipe_shittyColMajorXform` in f_wipe.c.
///
/// # Safety
///
/// `array` must be a valid, non-null pointer to `width * height` `i16` values.
/// The caller is responsible for ensuring no aliasing with the temporary
/// `Z_Malloc` buffer.
#[doc(alias = "wipe_shittyColMajorXform")]
unsafe fn col_major_xform(array: *mut i16, width: c_int, height: c_int) {
    let total = (width * height) as usize;
    let dest = super::Z_Malloc(total as c_int * 2, PU_STATIC, std::ptr::null_mut()) as *mut i16;

    for y in 0..height {
        for x in 0..width {
            *dest.add((x * height + y) as usize) = *array.add((y * width + x) as usize);
        }
    }

    std::ptr::copy(dest, array, total);
    super::Z_Free(dest as *mut c_void);
}

/// Initialise the melt wipe.
///
/// Copies the start screen into the working buffer, transposes both pixel
/// buffers into column-major order for efficient column access, allocates the
/// per-column Y-position array, and seeds each column's starting offset with a
/// random negative value so columns begin falling at slightly different times.
///
/// Returns 0.  C origin: `wipe_initMelt` in f_wipe.c.
///
/// # Safety
///
/// `WIPE_SCR_START`, `WIPE_SCR_END`, and `WIPE_SCR` must be valid pointers to
/// at least `width * height` bytes.  `M_Random` must be safe to call.
///
/// The `M_Random` call sites are the RNG-ledger surface: exactly one draw
/// per column, first-to-last, after the `Z_Malloc`/transpose preamble --
/// `dtmc::melt_column_start`/`dtmc::melt_column_seed` wrap the drawn value
/// without touching the draw order.
#[doc(alias = "wipe_initMelt")]
pub(super) unsafe extern "C" fn init_melt(width: c_int, height: c_int, _ticks: c_int) -> c_int {
    let len = (width * height) as usize;
    std::ptr::copy(WIPE_SCR_START, WIPE_SCR, len);

    col_major_xform(WIPE_SCR_START as *mut i16, width / 2, height);
    col_major_xform(WIPE_SCR_END as *mut i16, width / 2, height);

    Y = super::Z_Malloc(
        width * size_of::<c_int>() as c_int,
        PU_STATIC,
        std::ptr::null_mut(),
    ) as *mut c_int;

    *Y.add(0) = dtmc::melt_column_start(M_Random());
    for i in 1..width as usize {
        *Y.add(i) = dtmc::melt_column_seed(*Y.add(i - 1), M_Random());
    }

    0
}

/// Advance the melt wipe by `ticks` game-ticks.
///
/// For each column that has not yet fully melted, the column slides down
/// according to an accelerating schedule (speed increases up to a maximum of 8
/// pixels per tick once the column has moved 16 or more pixels).  The
/// end-screen pixels are revealed from the top; the start-screen pixels are
/// pushed down and off the bottom.  Returns 1 when all columns are done, 0
/// otherwise.
///
/// The algorithm works on transposed (column-major) copies of the pixel
/// buffers, hence the `width / 2` column count (each 16-bit word covers two
/// 8-bit pixels).
///
/// C origin: `wipe_doMelt` in f_wipe.c.
///
/// # Safety
///
/// `Y`, `WIPE_SCR_START`, `WIPE_SCR_END`, and `WIPE_SCR` must be valid
/// pointers initialised by `init_melt`.
#[doc(alias = "wipe_doMelt")]
pub(super) unsafe extern "C" fn do_melt(width: c_int, height: c_int, ticks: c_int) -> c_int {
    let width = width / 2;
    let mut done = true;
    let mut ticks = ticks;

    while ticks > 0 {
        ticks -= 1;
        for i in 0..width as usize {
            if *Y.add(i) < 0 {
                *Y.add(i) += 1;
                done = false;
            } else if *Y.add(i) < height {
                let mut dy: c_int;
                if *Y.add(i) < 16 {
                    dy = *Y.add(i) + 1;
                } else {
                    dy = 8;
                }
                if *Y.add(i) + dy >= height {
                    dy = height - *Y.add(i);
                }

                let yi = *Y.add(i) as usize;
                let src = (WIPE_SCR_END as *mut i16).add(i * height as usize + yi);
                let dst = (WIPE_SCR as *mut i16).add(yi * width as usize + i);
                for j in 0..dy as usize {
                    *dst.add(j * width as usize) = *src.add(j);
                }

                *Y.add(i) += dy;

                let src = (WIPE_SCR_START as *mut i16).add(i * height as usize);
                let dst = (WIPE_SCR as *mut i16).add((*Y.add(i) as usize) * width as usize + i);
                for j in 0..(height as usize - *Y.add(i) as usize) {
                    *dst.add(j * width as usize) = *src.add(j);
                }

                done = false;
            }
        }
    }

    if done {
        1
    } else {
        0
    }
}

/// Clean up after the melt wipe by freeing the Y-position and pixel snapshot
/// buffers.
///
/// Always returns 0.  C origin: `wipe_exitMelt` in f_wipe.c.
///
/// # Safety
///
/// `Y`, `WIPE_SCR_START`, and `WIPE_SCR_END` must have been allocated by
/// `init_melt` / `super::screenwipe::start_screen` / `super::screenwipe::end_screen`.
#[doc(alias = "wipe_exitMelt")]
pub(super) unsafe extern "C" fn exit_melt(_width: c_int, _height: c_int, _ticks: c_int) -> c_int {
    super::Z_Free(Y as *mut c_void);
    super::Z_Free(WIPE_SCR_START as *mut c_void);
    super::Z_Free(WIPE_SCR_END as *mut c_void);
    Y = std::ptr::null_mut();
    WIPE_SCR_START = std::ptr::null_mut();
    WIPE_SCR_END = std::ptr::null_mut();
    0
}
