//! Doom engine key code constants.
//!
//! These match the `#define KEY_*` constants in `vendor/doomgeneric/doomkeys.h`
//! and are pinned by the header-parity tests in `codes`. The `to_doom_key`
//! mapping lives in the binary's `platform/keys.rs` because it depends on
//! winit.
//!
//! ## Submodule Responsibility
//!
//! - `codes.rs` -- the 37 `KEY_*` constants and the header-parity tests
//!
//! The module root is documentation + wiring only: the `mod` declaration and
//! the path-stability re-export below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! No functions: the module is data wholesale (the `info.rs` / `tables.rs`
//! precedent -- no logic to split, `Surface: data`). Every constant keeps its
//! upstream name byte-for-byte (`m_fixed` precedent: constants are upstream
//! symbol set, not renamed), so there are no shims -- only the path-stability
//! re-export above. The constants encode `doomkeys.h`'s `#define`s verbatim
//! and each per-name byte array address is part of the FFI surface.
//!
//! `doomkeys.h` defines several additional constants (`KEY_CAPSLOCK`,
//! `KEY_NUMLOCK`, `KEY_SCRLCK`, `KEY_PRTSCR`, and the full numpad `KEYP_*`
//! aliases) that are not yet needed by this port and are intentionally
//! omitted -- the omission stays documented here as before graduation.
//! `m_config` duplicates a *separate* `c_int` `SCANTOKEY` constant set (the
//! DOS scancode translation tables); that set stays in `m_config` -- different
//! type and role, not merged here.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module holds only key-code constants. They are
//! immutable data consumed by input marshalling (`i_input`), menu/chat text
//! entry (`m_menu`, `hu_stuff`, `hu_lib`), control bindings (`m_controls`),
//! and the native shell's winit mapping; none of them is computed, so nothing
//! here can drift between hosts or targets.

mod codes;

//* path-stability re-export: the key constants keep their module-root
//* paths for the freeze-zone callers (`hu_stuff`, `hu_lib`, `i_input`,
//* `m_controls`, ...).
pub use codes::{
    KEY_BACKSPACE, KEY_DEL, KEY_DOWNARROW, KEY_END, KEY_ENTER, KEY_EQUALS, KEY_ESCAPE, KEY_F1,
    KEY_F10, KEY_F11, KEY_F12, KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9,
    KEY_FIRE, KEY_HOME, KEY_INS, KEY_LALT, KEY_LEFTARROW, KEY_MINUS, KEY_PAUSE, KEY_PGDN,
    KEY_PGUP, KEY_RALT, KEY_RCTRL, KEY_RIGHTARROW, KEY_RSHIFT, KEY_STRAFE_L, KEY_STRAFE_R,
    KEY_TAB, KEY_UPARROW, KEY_USE,
};
