//! The four `repr(C)` widget types and the `STlib_init*` constructor
//! family.

use std::os::raw::c_int;

use crate::doom::st_lib::sttminus;
use crate::doom::v_video::patch_t;
use crate::doom::w_wad::W_CacheLumpName;
use crate::doom::z_zone::PU_STATIC;

/// A right-justified integer display widget.
///
/// Corresponds to `st_number_t` in `st_lib.h`. Digits are rendered
/// right-to-left using a patch font; a minus sign patch is prepended for
/// negative values. The magic value `1994` means "do not draw" (used when
/// a slot is inactive).
///
/// # Layout invariants
/// * `p[0..9]` must point to valid digit patches when drawing.
/// * `on` and `num` must be valid non-null pointers for the widget's lifetime.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct st_number_t {
    /// Right edge X coordinate of the number (digits extend leftward from here).
    pub x: c_int,
    /// Top Y coordinate of the number.
    pub y: c_int,
    /// Maximum number of digits to display (controls field width).
    pub width: c_int,
    /// Cached value from the previous frame; used to detect changes.
    pub oldnum: c_int,
    /// Pointer to the current integer value to display.
    pub num: *mut c_int,
    /// Visibility flag pointer; widget is only drawn when `*on != 0`.
    /// Maps to `boolean*` in C.
    pub on: *mut c_int, // boolean* (c_int in C)
    /// Array of digit patches; `p[d]` is the patch for digit `d` (0-9).
    pub p: *mut *mut patch_t,
    /// User-defined auxiliary data (unused by the widget library itself).
    pub data: c_int,
}

/// A percentage display widget: a number followed by a `%` sign patch.
///
/// Corresponds to `st_percent_t` in `st_lib.h`. The embedded [`st_number_t`]
/// renders the numeric portion; the additional `p` patch is drawn immediately
/// to the right of the number on each refresh.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct st_percent_t {
    /// The underlying number widget (renders the digits).
    pub n: st_number_t,
    /// The `%` percent-sign patch drawn after the number.
    pub p: *mut patch_t,
}

/// A widget that displays one patch selected from an indexed array.
///
/// Corresponds to `st_multicon_t` in `st_lib.h`. Used for key-card slots,
/// player face sprites, and any other status bar element that cycles through
/// a discrete set of images. When the selected index changes, the old patch
/// area is erased by blitting from the backing screen before the new patch is
/// drawn.
///
/// # Layout invariants
/// * `p[0..N]` must contain valid patch pointers for all possible values of
///   `*inum`.
/// * `inum == -1` means "no image"; the widget skips drawing entirely.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct st_multicon_t {
    /// Center-justified screen X position (patch offset subtracted at draw time).
    pub x: c_int,
    /// Center-justified screen Y position (patch offset subtracted at draw time).
    pub y: c_int,
    /// Index of the patch displayed in the previous frame; `-1` if no patch was
    /// shown.
    pub oldinum: c_int,
    /// Pointer to the current icon index; `-1` suppresses drawing.
    pub inum: *mut c_int,
    /// Visibility flag pointer; widget is only drawn when `*on != 0`.
    pub on: *mut c_int,
    /// Array of icon patches indexed by `*inum`.
    pub p: *mut *mut patch_t,
    /// User-defined auxiliary data (unused by the widget library itself).
    pub data: c_int,
}

/// A widget that shows a single patch when a boolean flag is set.
///
/// Corresponds to `st_binicon_t` in `st_lib.h`. When `*val` transitions from
/// zero to non-zero the patch is drawn; when it transitions back to zero the
/// patch area is erased from the backing screen. Used for key-card presence
/// indicators and similar binary status elements.
///
/// # Layout invariants
/// * `p` must point to a valid patch for the lifetime of the widget.
/// * `val` and `on` must be valid non-null pointers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct st_binicon_t {
    /// Center-justified screen X position.
    pub x: c_int,
    /// Center-justified screen Y position.
    pub y: c_int,
    /// Cached value of `*val` from the previous frame.
    pub oldval: c_int,
    /// Pointer to the current boolean value (non-zero = draw, zero = hide).
    pub val: *mut c_int,
    /// Visibility flag pointer; widget is only drawn when `*on != 0`.
    pub on: *mut c_int,
    /// The patch to display when `*val != 0`.
    pub p: *mut patch_t,
    /// User-defined auxiliary data (unused by the widget library itself).
    pub data: c_int,
}

/// Initialize the status bar widget library.
///
/// Loads the `STTMINUS` WAD lump as `PU_STATIC` and stores it in
/// [`sttminus`] (the module-root static). All other widget state is
/// initialized via the individual `STlib_init*` functions. Called once at
/// startup from `ST_Init` in `st_stuff.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_init")]
#[export_name = "STlib_init"]
pub extern "C" fn init_widget_library() {
    unsafe {
        // DEH_String("STTMINUS") is identity — just pass the string.
        sttminus = W_CacheLumpName(c"STTMINUS".as_ptr(), PU_STATIC) as *mut patch_t;
    }
}

/// Initialize a number widget.
///
/// Stores the position, digit-patch array, value pointer, visibility flag,
/// and field width into `*n`. Sets `oldnum` to 0.
///
/// # Preconditions
/// * `pl` must point to at least 10 valid patch pointers (digits 0-9).
/// * `num` and `on` must be valid non-null pointers for the widget's lifetime.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_initNum")]
#[export_name = "STlib_initNum"]
pub extern "C" fn init_number_widget(
    n: *mut st_number_t,
    x: c_int,
    y: c_int,
    pl: *mut *mut patch_t,
    num: *mut c_int,
    on: *mut c_int,
    width: c_int,
) {
    unsafe {
        (*n).x = x;
        (*n).y = y;
        (*n).oldnum = 0;
        (*n).width = width;
        (*n).num = num;
        (*n).on = on;
        (*n).p = pl;
    }
}

/// Initialize a percent widget.
///
/// Calls [`init_number_widget`] with `width = 3` for the embedded number, then
/// stores the `%` sign patch in `p.p`.
///
/// # Preconditions
/// * `pl` must point to at least 10 valid digit patches.
/// * `num`, `on`, and `percent` must be valid non-null pointers.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_initPercent")]
#[export_name = "STlib_initPercent"]
pub extern "C" fn init_percent_widget(
    p: *mut st_percent_t,
    x: c_int,
    y: c_int,
    pl: *mut *mut patch_t,
    num: *mut c_int,
    on: *mut c_int,
    percent: *mut patch_t,
) {
    unsafe {
        init_number_widget(&mut (*p).n, x, y, pl, num, on, 3);
        (*p).p = percent;
    }
}

/// Initialize a multi-icon widget.
///
/// Sets position, patch array, current-index pointer, and visibility flag.
/// `oldinum` is initialized to `-1` so the first draw is unconditional.
///
/// # Preconditions
/// * `inum` and `on` must be valid non-null pointers.
/// * `il` must point to a valid patch array covering all expected index values.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_initMultIcon")]
#[export_name = "STlib_initMultIcon"]
pub extern "C" fn init_multicon_widget(
    i: *mut st_multicon_t,
    x: c_int,
    y: c_int,
    il: *mut *mut patch_t,
    inum: *mut c_int,
    on: *mut c_int,
) {
    unsafe {
        (*i).x = x;
        (*i).y = y;
        (*i).oldinum = -1;
        (*i).inum = inum;
        (*i).on = on;
        (*i).p = il;
    }
}

/// Initialize a binary icon widget.
///
/// Sets position, icon patch, value pointer, and visibility flag. `oldval` is
/// initialized to 0.
///
/// # Preconditions
/// * `i`, `val`, and `on` must be valid non-null pointers.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `st_stuff` imports the upstream name through the root shim.
#[doc(alias = "STlib_initBinIcon")]
#[export_name = "STlib_initBinIcon"]
pub extern "C" fn init_binicon_widget(
    b: *mut st_binicon_t,
    x: c_int,
    y: c_int,
    i: *mut patch_t,
    val: *mut c_int,
    on: *mut c_int,
) {
    unsafe {
        (*b).x = x;
        (*b).y = y;
        (*b).oldval = 0;
        (*b).val = val;
        (*b).on = on;
        (*b).p = i;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const ST_NUMBER_T_SIZEOF: usize = 48;
    const ST_PERCENT_T_SIZEOF: usize = 56;
    const ST_MULTICON_T_SIZEOF: usize = 48;
    const ST_BINICON_T_SIZEOF: usize = 48;

    #[test]
    fn st_number_t_size_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<st_number_t>(),
            ST_NUMBER_T_SIZEOF,
            "st_number_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<st_number_t>(),
            ST_NUMBER_T_SIZEOF,
        );
    }

    #[test]
    fn st_percent_t_size_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<st_percent_t>(),
            ST_PERCENT_T_SIZEOF,
            "st_percent_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<st_percent_t>(),
            ST_PERCENT_T_SIZEOF,
        );
    }

    #[test]
    fn st_multicon_t_size_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<st_multicon_t>(),
            ST_MULTICON_T_SIZEOF,
            "st_multicon_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<st_multicon_t>(),
            ST_MULTICON_T_SIZEOF,
        );
    }

    #[test]
    fn st_binicon_t_size_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<st_binicon_t>(),
            ST_BINICON_T_SIZEOF,
            "st_binicon_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<st_binicon_t>(),
            ST_BINICON_T_SIZEOF,
        );
    }
}
