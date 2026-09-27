//! The module's C-linkage keep-alive anchor: `spec_link_anchor` takes the
//! address of every exported function so the linker cannot dead-strip
//! symbols no Rust code references (the mirror of the C-side link anchor
//! pattern in `vendor/doomgeneric/p_spec.c`).

use super::anims::init_pic_anims;
use super::crossline::{cross_special_line, shoot_special_line};
use super::donut::do_donut;
use super::geometry::{
    get_next_sector, get_sector, get_side, highest_ceiling_surrounding, highest_floor_surrounding,
    lowest_ceiling_surrounding, lowest_floor_surrounding, min_surrounding_light,
    next_highest_floor, sector_from_line_tag, two_sided,
};
use super::player_sector::player_in_special_sector;
use super::spawn::spawn_specials;
use super::ticker::update_specials;

/// Forces all public symbols in this module to be included in the final binary.
///
/// The linker may discard `pub unsafe extern "C"` functions that are not
/// referenced from Rust code.  This anchor function takes the address of every
/// exported symbol to prevent dead-code elimination.  It is itself exported
/// with C linkage and called from the C-side link anchor in p_spec.c (or the
/// equivalent build glue). Upstream name `P_Spec_Link_Anchor` is kept via the
/// export pin; note the anchor has zero callers in this port (the
/// `doomgeneric.rs` anchor list does not include it) -- it is kept
/// dead-but-exported for symbol-set stability until freeze-zone retirement.
///
/// # Safety
///
/// Takes function addresses only; calling it has no effect.
#[doc(alias = "P_Spec_Link_Anchor")]
#[export_name = "P_Spec_Link_Anchor"]
pub unsafe extern "C" fn spec_link_anchor()
{
    let _ = init_pic_anims as *const () as usize;
    let _ = spawn_specials as *const () as usize;
    let _ = update_specials as *const () as usize;
    let _ = cross_special_line as *const () as usize;
    let _ = shoot_special_line as *const () as usize;
    let _ = player_in_special_sector as *const () as usize;
    let _ = do_donut as *const () as usize;
    let _ = get_side as *const () as usize;
    let _ = get_sector as *const () as usize;
    let _ = two_sided as *const () as usize;
    let _ = get_next_sector as *const () as usize;
    let _ = lowest_floor_surrounding as *const () as usize;
    let _ = highest_floor_surrounding as *const () as usize;
    let _ = next_highest_floor as *const () as usize;
    let _ = lowest_ceiling_surrounding as *const () as usize;
    let _ = highest_ceiling_surrounding as *const () as usize;
    let _ = sector_from_line_tag as *const () as usize;
    let _ = min_surrounding_light as *const () as usize;
}
