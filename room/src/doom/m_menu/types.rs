//! The menu type tier: `menuitem_t`/`menu_t` with their model-aware
//! size guards, the `patch_stub` measurement prefix, and the `const fn`
//! table constructors that replace C compound literals.

use std::ffi::{c_char, c_int};

/// Build a `menuitem_t` at compile time.
///
/// `status` encodes the item type: 1 = activatable, 2 = slider, -1 = gap.
/// `name` is the WAD lump name (up to 10 bytes, NUL-padded).
/// `routine` is the callback invoked on selection or slider movement.
/// `alpha` is the keyboard shortcut character used for quick navigation.
pub(super) const fn mi(
    status: i16,
    name: &[u8],
    routine: Option<extern "C" fn(c_int)>,
    alpha: u8,
) -> menuitem_t {
    let mut n = [0i8; 10];
    let mut i = 0;
    while i < name.len() && i < 10 {
        n[i] = name[i] as c_char;
        i += 1;
    }
    menuitem_t {
        status,
        name: n,
        routine,
        alphaKey: alpha as c_char,
    }
}

/// Convert a string literal into a fixed-length `[c_char; 26]` buffer at compile time.
///
/// Used to initialise the `gammamsg` table without heap allocation.
pub(super) const fn make_gamma_msg(s: &str) -> [c_char; 26] {
    let bytes = s.as_bytes();
    let mut arr = [0i8; 26];
    let mut i = 0;
    while i < bytes.len() {
        arr[i] = bytes[i] as c_char;
        i += 1;
    }
    arr
}

/// A single entry in a Doom menu (C typedef `menuitem_t` from `m_menu.h`).
///
/// Layout is ABI-identical to the C struct; `#[repr(C)]` ensures this.
/// `status` encodes item kind: `1` = normal action, `2` = slider, `-1` = spacing row.
/// `name` is a NUL-terminated WAD lump name (10 bytes, 0-padded).
/// `routine` is called with the item index on activation.
/// `alphaKey` is the lowercase ASCII shortcut character.
#[repr(C)]
pub struct menuitem_t {
    /// Item kind: 1 = action, 2 = slider, -1 = gap row.
    pub status: i16,
    /// WAD lump name for the item's graphic patch (NUL-terminated, 10 bytes).
    pub name: [c_char; 10],
    /// Callback invoked when the item is activated or a slider is adjusted.
    pub routine: Option<extern "C" fn(c_int)>,
    /// Keyboard shortcut character for fast navigation within the menu.
    pub alphaKey: c_char,
}

/// A Doom menu page descriptor (C typedef `menu_t` from `m_menu.h`).
///
/// Each menu screen is described by one of these structs.
/// `menuitems` points to the item array for this page; it cannot be initialised
/// as a static const because Rust forbids raw-pointer cross-references between
/// statics, so the pointer is wired up at runtime in `lifecycle::init`.
#[repr(C)]
pub struct menu_t {
    /// Number of items in `menuitems`.
    pub numitems: i16,
    /// Pointer to the parent menu page, or null for the root menu.
    pub prevMenu: *mut menu_t,
    /// Pointer to the first element of this page's `menuitem_t` array.
    pub menuitems: *mut menuitem_t,
    /// Draw callback invoked each frame while this page is active.
    pub routine: Option<extern "C" fn()>,
    /// Screen x coordinate of the first menu item.
    pub x: i16,
    /// Screen y coordinate of the first menu item.
    pub y: i16,
    /// Index of the item that was last highlighted; restored on re-entry.
    pub lastOn: i16,
}

// Layout guards for `menuitem_t` / `menu_t` (C `m_menu.h` originals).
// menuitem_t arithmetic, both models:
//   status(i16) 2 + name([c_char; 10]) 10 + routine(function pointer)
//   + alphaKey(c_char) 1.
//   LP64: 2 + 10 + pad 4 (pointer alignment) + fn-pointer 8 + 1 = 25,
//   rounded up to the 8-byte struct alignment = 32.
//   ILP32 (wasm32): 2 + 10 + fn-pointer 4 + 1 = 17, rounded up to the
//   4-byte struct alignment = 20.
// menu_t arithmetic, both models:
//   numitems(i16) 2 + prevMenu(pointer) + menuitems(pointer)
//   + routine(function pointer) + x(i16) 2 + y(i16) 2 + lastOn(i16) 2.
//   LP64: 2 + pad 6 + 3 pointers x 8 + 3x2 = 38, rounded up to the
//   8-byte struct alignment = 40.
//   ILP32 (wasm32): 2 + pad 2 + 3 pointers x 4 + 3x2 = 22, rounded up to
//   the 4-byte struct alignment = 24.
// Numbers verified against the real structs with a wasm32 const-assert
// scratch check (c_tests/LP64 task 2).
#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    std::mem::size_of::<menuitem_t>() == 32,
    "menuitem_t size mismatch"
);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(
    std::mem::size_of::<menuitem_t>() == 20,
    "menuitem_t size mismatch"
);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<menu_t>() == 40, "menu_t size mismatch");
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<menu_t>() == 24, "menu_t size mismatch");

/// Minimal prefix of `patch_t` needed to read width/height without pulling in the full type.
///
/// Used when measuring HUD font glyphs for text centering.
#[repr(C)]
pub(super) struct patch_stub {
    /// Pixel width of the patch.
    pub width: i16,
    /// Pixel height of the patch.
    pub height: i16,
}
