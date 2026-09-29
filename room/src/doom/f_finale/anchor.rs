//! The dead-but-exported linker anchor for the finale surface.

use std::ptr;

/// Ensures all exported symbols are retained by the linker.
///
/// Calls every public `extern "C"` function in this module with null/zero
/// arguments.  Never intended to be called at runtime.
/// C origin: not present in f_finale.c; added for the Rust link model.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
///
/// # Safety
///
/// This function must never be called at runtime; it exists solely to prevent
/// the linker from discarding exported symbols during dead-code elimination.
///
/// The pre-move export symbol is kept with `#[export_name]` below.
#[doc(alias = "F_Finale_Link_Anchor")]
#[export_name = "F_Finale_Link_Anchor"]
pub unsafe extern "C" fn finale_link_anchor() {
    super::lifecycle::start_finale();
    super::lifecycle::responder(ptr::null_mut());
    super::lifecycle::ticker();
    super::textstage::text_write();
    super::cast::start_cast();
    super::cast::cast_ticker();
    super::cast::cast_responder(ptr::null_mut());
    super::cast::cast_print(ptr::null_mut());
    super::cast::cast_drawer();
    super::artscreen::draw_patch_col(0, ptr::null_mut(), 0);
    super::artscreen::bunny_scroll();
    super::artscreen::art_screen_drawer();
    super::lifecycle::drawer();
}
