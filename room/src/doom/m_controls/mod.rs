//! Rust port of vendor/doomgeneric/m_controls.c.
//!
//! Default keyboard / mouse / joystick bindings for every game in the
//! chocolate-doom family (Doom, Heretic, Hexen, Strife), plus the
//! `M_Bind*` registration helpers that wire each binding into the
//! `m_config` variable table so they can be read or written through
//! `default.cfg`.
//!
//! All bindings are exposed as `#[no_mangle] pub static mut` so the
//! original C call sites can read and mutate them directly. Each
//! `M_Bind<Game>Controls` function calls into `m_config::bind_variable` (from
//! `m_config`) for the keys relevant to that game's binding subset.
//!
//! ## Submodule Responsibility
//!
//! - `defaults.rs` -- the 109 `#[no_mangle] pub static mut` binding
//!   statics (names and initializers keep their upstream C form)
//! - `binders.rs` -- the nine `M_Bind*` registration helpers plus the
//!   `cstr` name-literal cast
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_BindBaseControls` | `binders::bind_base_controls` | glue | keyboard / mouse / joystick bindings shared by every game; C symbol pinned via `#[export_name]` (`d_main/bind.rs` imports through the root shim); upstream `m_controls.c:204` |
//! | `M_BindHereticControls` | `binders::bind_heretic_controls` | glue | dead-but-exported in this Doom-only build; C symbol pinned (symbol-set byte-identity, retires with the freeze zone); upstream `m_controls.c:241` |
//! | `M_BindHexenControls` | `binders::bind_hexen_controls` | glue | dead-but-exported; C symbol pinned; upstream `m_controls.c:256` |
//! | `M_BindStrifeControls` | `binders::bind_strife_controls` | glue | dead-but-exported; C symbol pinned; upstream `m_controls.c:272` |
//! | `M_BindWeaponControls` | `binders::bind_weapon_controls` | glue | weapon / arti key bindings; C symbol pinned; upstream `m_controls.c:307` |
//! | `M_BindMapControls` | `binders::bind_map_controls` | glue | automap key bindings; C symbol pinned; upstream `m_controls.c:328` |
//! | `M_BindMenuControls` | `binders::bind_menu_controls` | glue | menu navigation bindings; C symbol pinned; upstream `m_controls.c:344` |
//! | `M_BindChatControls` | `binders::bind_chat_controls` | glue | per-player chat bindings via `c_write!`; C symbol pinned; upstream `m_controls.c:375` |
//! | `M_ApplyPlatformDefaults` | `binders::apply_platform_defaults` | glue | no-op platform-patch extension point; C symbol pinned; upstream `m_controls.c:394` |
//! | `cstr` (Rust helper) | `binders::cstr` | glue | name kept; casts a NUL-terminated literal to `*mut c_char` |
//! | 109 binding statics | `defaults` | data | all keep names + `#[no_mangle]` (`.cfg` compat surface; freeze-zone readers use them by root path) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: this module is binding registration --
//! statics hold config defaults and the binders attach them to the
//! `m_config` tables once during startup. The binding-name strings and
//! call order are `.cfg` round-trip compatibility surface
//! (`default.cfg` byte-stability), which is boot-time configuration, not
//! the per-tic demo synchronization surface. The `defaults_parity_snapshot`
//! test pins a handful of upstream initializers across the move.

pub mod binders;
pub mod defaults;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use binders::{
    apply_platform_defaults as M_ApplyPlatformDefaults, bind_base_controls as M_BindBaseControls,
    bind_chat_controls as M_BindChatControls, bind_heretic_controls as M_BindHereticControls,
    bind_hexen_controls as M_BindHexenControls, bind_map_controls as M_BindMapControls,
    bind_menu_controls as M_BindMenuControls, bind_strife_controls as M_BindStrifeControls,
    bind_weapon_controls as M_BindWeaponControls,
};

//* path-stability re-export: the binding statics keep their module-root
//* paths and `#[no_mangle]` C symbols (`d_main/bind.rs`, `g_game`,
//* `am_map`, `hu_stuff`, `m_menu`).

pub use defaults::{dclick_use, joybfire, joybjump, joybmenu, joybnextweapon, joybprevweapon,
    joybspeed, joybstrafe, joybstrafeleft, joybstraferight, joybuse, key_arti_all,
    key_arti_blastradius, key_arti_egg, key_arti_health, key_arti_invulnerability,
    key_arti_poisonbag, key_arti_teleport, key_arti_teleportother, key_demo_quit, key_down,
    key_fire, key_flycenter, key_flydown, key_flyup, key_invdrop, key_invend, key_invhome,
    key_invkey, key_invleft, key_invpop, key_invquery, key_invright, key_invuse, key_jump,
    key_left, key_lookcenter, key_lookdown, key_lookup, key_map_clearmark, key_map_east,
    key_map_follow, key_map_grid, key_map_mark, key_map_maxzoom, key_map_north, key_map_south,
    key_map_toggle, key_map_west, key_map_zoomin, key_map_zoomout, key_menu_abort,
    key_menu_activate, key_menu_back, key_menu_confirm, key_menu_decscreen, key_menu_detail,
    key_menu_down, key_menu_endgame, key_menu_forward, key_menu_gamma, key_menu_help,
    key_menu_incscreen, key_menu_left, key_menu_load, key_menu_messages, key_menu_qload,
    key_menu_qsave, key_menu_quit, key_menu_right, key_menu_save, key_menu_screenshot,
    key_menu_up, key_menu_volume, key_message_refresh, key_mission, key_multi_msg,
    key_multi_msgplayer, key_nextweapon, key_pause, key_prevweapon, key_right, key_speed,
    key_spy, key_strafe, key_strafeleft, key_straferight, key_up, key_use, key_useartifact,
    key_usehealth, key_weapon1, key_weapon2, key_weapon3, key_weapon4, key_weapon5,
    key_weapon6, key_weapon7, key_weapon8, mousebbackward, mousebfire, mousebforward,
    mousebjump, mousebnextweapon, mousebprevweapon, mousebstrafe, mousebstrafeleft,
    mousebstraferight, mousebuse};
