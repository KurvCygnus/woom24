//! The module link anchor.

use super::diagnose::{
    check_heap, check_heap_quiet, dump_heap, file_dump_heap, free_memory,
};
use super::zone::{
    change_tag, change_user, zone_alloc, zone_free, zone_free_tags, zone_init, zone_size,
};

//* Dead-but-exported: zero callers exist (`doomgeneric.rs`'s anchor
//* list has no z_zone entry) and the rename drops its `#[no_mangle]`
//* with the rest of the module's C surface -- only the two live
//* symbols (`Z_Malloc`/`Z_Free`) are pinned at their definitions.
//* Kept as a call-shaped anchor for parity with the pre-split module;
//* retires with the freeze zone (p_spec precedent).
/// Forces all public allocator entry points to be included in the
/// final binary by calling them with placeholder arguments.
///
/// Without this anchor the linker may dead-strip functions that are only
/// called from C translation units, since Rust does not see those call
/// sites.
///
/// # Safety
/// - Calls every wrapped function with placeholder null/zero arguments.
///   Must never actually be invoked at runtime (it would corrupt the
///   Zone immediately).
#[doc(alias = "Z_Zone_Link_Anchor")]
pub unsafe extern "C" fn zone_link_anchor()
{
    zone_init();
    zone_free(std::ptr::null_mut());
    zone_alloc(0, 0, std::ptr::null_mut());
    zone_free_tags(0, 0);
    dump_heap(0, 0);
    file_dump_heap(std::ptr::null_mut());
    check_heap();
    change_tag(std::ptr::null_mut(), 0, std::ptr::null(), 0);
    change_user(std::ptr::null_mut(), std::ptr::null_mut());
    free_memory();
    zone_size();
    let _ = check_heap_quiet as unsafe extern "C" fn() -> bool;
}
