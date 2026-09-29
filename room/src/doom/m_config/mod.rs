//! Rust port of vendor/doomgeneric/m_config.c.
//!
//! Configuration-file interface: binds a static table of named variables to
//! locations in memory, parses values from text, and (in the original C)
//! loads / saves them to disk. The variable tables are organised into two
//! collections - `doom_defaults` (the vanilla `.cfg` settings) and
//! `extra_defaults` (chocolate-doom extensions).
//!
//! Notable port-level differences vs. the C source:
//! - The disk I/O code is guarded by `ORIGCODE` upstream and is also a
//!   no-op here; `save_defaults` and `load_defaults` still set up
//!   filenames but do not actually parse or write `.cfg` files.
//! - The Windows-only config-dir resolution paths are skipped; the
//!   fallback always returns `"."`.
//! - Variable definitions are encoded as the `config_var_entry` const fn
//!   in place of the C `CONFIG_VARIABLE_*` macros.
//!
//! ## Submodule Responsibility
//!
//! - `tables.rs` -- the entry/collection types and the two static tables
//!   (`DOOM_DEFAULTS_LIST` 76 entries, `EXTRA_DEFAULTS_LIST` 119 entries)
//! - `scankeys.rs` -- the `SCANTOKEY` DOS-scancode translation set and its
//!   local `KEY_*` constants (separate from the `doomkeys` module set by
//!   type and role)
//! - `vars.rs` -- lookup / parse / set / bind, the `M_Bind`/`M_Set`/`M_Get`
//!   family, and the `.cfg` filename staging
//! - `paths.rs` -- the `configdir` global, `set_config_dir`, and the
//!   savegame-directory computation
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `SearchCollection` | `vars::search_collection` | glue | private linear probe, name kept (already snake); upstream `m_config.c:1563` |
//! | `GetDefaultForName` | `vars::get_default_for_name` | glue | private, doom table then extra table, `I_Error` on miss; upstream `m_config.c:1937` |
//! | `ParseIntParameter` | `vars::parse_int_parameter` | glue | private, name kept; documented deviations (`0X` accepted, octal dropped) pinned by tests; upstream `m_config.c:1716` |
//! | `SetVariable` | `vars::set_variable` | glue | private, name kept; the `strdup` old-buffer leak matches C; upstream `m_config.c:1728` |
//! | `M_SetConfigFilenames` | `vars::set_config_filenames` | glue | C symbol pinned via `#[export_name]`; upstream `m_config.c:1836` |
//! | `M_SaveDefaults` | `vars::save_defaults` | glue | no-op (`ORIGCODE`); C symbol pinned; upstream `m_config.c:1846` |
//! | `M_SaveDefaultsAlternate` | `vars::save_defaults_alternate` | glue | filename-pointer swap only; C symbol pinned (dead-but-exported); upstream `m_config.c:1856` |
//! | `M_LoadDefaults` | `vars::load_defaults` | glue | `-config` / `-extraconfig` staging, parse half no-op; C symbol pinned; upstream `m_config.c:1881` |
//! | `M_BindVariable` | `vars::bind_variable` | glue | C symbol pinned via `#[export_name]` (`i_sound/config.rs` / `d_main/bind.rs` import through the root shim; the `i_joystick` / `m_controls` extern declarations converted to this path call in the graduation commit); upstream `m_config.c:1964` |
//! | `M_SetVariable` | `vars::set_config_variable` | glue | C symbol pinned (dead-but-exported); upstream `m_config.c:1977` |
//! | `M_GetIntVariable` | `vars::get_int_variable` | glue | C symbol pinned (dead-but-exported); upstream `m_config.c:1995` |
//! | `M_GetStrVariable` | `vars::get_str_variable` | glue | C symbol pinned (dead-but-exported) |
//! | `M_GetFloatVariable` | `vars::get_float_variable` | glue | C symbol pinned (dead-but-exported) |
//! | `GetDefaultConfigDir` | `paths::default_config_dir_fallback` | glue | private malloc'd `"."` fallback (Windows discovery skipped); upstream `m_config.c:2043` |
//! | `M_SetConfigDir` | `paths::set_config_dir` | glue | C symbol pinned; upstream `m_config.c:2059` |
//! | `M_GetSaveGameDir` | `paths::get_save_game_dir` | glue | `"<configdir>/.savegame/"` on this port; C symbol pinned; upstream `m_config.c:2087` |
//! | `configdir` (static) | `paths::configdir` | data | static keeps name + `#[no_mangle]`; legacy C call sites read it directly |
//! | `default_t` / `default_type_t` / `default_collection_t` (C types) | `tables::{ConfigVarEntry, ConfigValueType, ConfigVarCollection}` | data | private types renamed to plain English at graduation |
//! | `SCANTOKEY` + 34 `KEY_*` consts | `scankeys` | data | m_config-local DOS-scan translation set keeps names; separate from `doomkeys` by type/role |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: `.cfg` values are boot inputs to the
//! demo synchronization surface, not part of it -- the tables are parsed
//! and bound once during startup (same adjudication as `d_main/bind.rs`),
//! and nothing here runs per tic. The four baseline tests (table counts
//! 76/119, `SCANTOKEY` length, `repr(C)` entry layout, parse vectors) were
//! written before the move and pin the `.cfg` round-trip shape.

pub mod paths;
pub mod scankeys;
pub mod tables;
pub mod vars;

//* path-stability re-export: the config-dir global keeps its module-root
//* path (read directly by boot / savegame code, exported with
//* `#[no_mangle]` unchanged).
pub use paths::configdir;

//* path-stability re-export: `i_joystick` and `m_controls` call the
//* binding entry by its plain new name (their extern declarations
//* converted to this path call in the graduation commit).
pub use vars::bind_variable;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use paths::{get_save_game_dir as M_GetSaveGameDir, set_config_dir as M_SetConfigDir};
pub use vars::{
    bind_variable as M_BindVariable, get_float_variable as M_GetFloatVariable,
    get_int_variable as M_GetIntVariable, get_str_variable as M_GetStrVariable,
    load_defaults as M_LoadDefaults, save_defaults as M_SaveDefaults,
    save_defaults_alternate as M_SaveDefaultsAlternate,
    set_config_filenames as M_SetConfigFilenames, set_config_variable as M_SetVariable,
};
