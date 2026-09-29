//! Data home for the video layer: the WAD patch vocabulary types plus the
//! module statics (three `#[no_mangle]` exported, two private).

use std::ffi::c_int;
use std::ptr;

/// A WAD patch image header (`patch_t` in `doomdata.h`).
///
/// The variable-length `columnofs` array follows the four header fields in
/// the WAD data; readers walk it via pointer arithmetic (see
/// [`crate::doom::v_video::patch::draw_patch`]) instead of declaring it as
/// an array member.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct patch_t {
    pub width: i16,
    pub height: i16,
    pub leftoffset: i16,
    pub topoffset: i16,
    // columnofs follows in the WAD data but is variable-length;
    // we read it via pointer arithmetic to avoid Rust array bounds checks.
}

/// One vertical run of pixels inside a patch column (`post_t` in
/// `doomdata.h`); a `topdelta` of `0xff` terminates the column.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct post_t {
    pub topdelta: u8,
    pub length: u8,
}

/// A patch column is just a chain of posts.
pub type column_t = post_t;

/// Automap patch-clip callback signature installed by
/// [`crate::doom::v_video::patch::set_patch_clip_callback`].
pub type vpatchclipfunc_t = Option<extern "C" fn(*mut patch_t, c_int, c_int) -> c_int>;

/// Column-lookup table for translucent (`TL`) patch drawing; loaded from the
/// `TINTTAB` lump by [`crate::doom::v_video::screenshot::load_tint_table`].
/// Only the dead-but-exported `TL` patch family reads it.
#[no_mangle]
pub static mut tinttable: *mut u8 = ptr::null_mut();

/// Column-lookup table for `XLA` patch drawing; loaded from the `XLATAB` lump
/// by [`crate::doom::v_video::screenshot::load_xla_table`]. Only the
/// dead-but-exported `XLA` patch family reads it.
#[no_mangle]
pub static mut xlatab: *mut u8 = ptr::null_mut();

/// Dirty-rectangle tracker (`dirtybox`): the four edges of the region touched
/// since the last present. Written here through the `M_AddToBox` protocol;
/// consumed by the `i_video` present side to shrink the blit region.
#[no_mangle]
pub static mut dirtybox: [c_int; 4] = [0; 4];

/// Buffer the `V_*` drawing primitives currently target. `use_buffer`
/// selects an off-screen buffer, `restore_buffer` returns to
/// `I_VideoBuffer`; `retarget_after_framebuffer_swap` follows a
/// `video_cfg` framebuffer swap.
pub(super) static mut dest_screen: *mut u8 = ptr::null_mut();

/// Automap clip callback consulted by every patch draw; `None` until
/// `set_patch_clip_callback` installs one.
pub(super) static mut patchclip_callback: vpatchclipfunc_t = None;
