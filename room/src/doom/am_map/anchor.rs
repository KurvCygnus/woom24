//! The dead-but-exported linker anchor for the automap surface.

use std::ptr;

/// Ensure all exported automap symbols are retained by the linker.
///
/// Calls every public `extern "C"` function in this module with null/zero
/// arguments.  Never intended to be called at runtime.
/// C origin: not present in am_map.c; added for the Rust link model.
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
#[doc(alias = "AM_Map_Link_Anchor")]
#[export_name = "AM_Map_Link_Anchor"]
pub unsafe extern "C" fn amap_link_anchor() {
    super::responder::responder(ptr::null_mut());
    super::lifecycle::ticker();
    super::raster::drawer();
    super::lifecycle::stop();
}
