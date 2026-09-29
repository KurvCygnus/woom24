//! Rust port of vendor/doomgeneric/i_system.c.
//!
//! System interface: zone memory allocation, error handling, exit hooks,
//! startup banners, and DOS memory-dump emulation for compatibility.
//!
//! All `#[no_mangle] pub extern "C"` entry points keep the original C
//! symbol names so the rest of the engine (still partly C-shaped) can
//! link against them unchanged. The Rust implementations use libc for
//! the original printf/malloc behaviour and `std::process` for the
//! Linux-only Zenity error dialog (the C source has Win32 / macOS /
//! DJGPP variants which this port intentionally drops).
//!
//! ## Submodule Responsibility
//!
//! - `banner.rs` -- the startup-banner printers (`print_banner`,
//!   `print_divider`, `print_startup_banner`), the `console_stdout`
//!   probe, and the `tactile` no-op
//! - `zone.rs` -- the zone-heap budget (`DEFAULT_RAM`, `MIN_RAM`),
//!   `auto_alloc_memory`, and `zone_base`
//! - `exit.rs` -- the `atexit` list types and `exit_funcs` static,
//!   `at_exit`, `quit_engine`, and the `I_System_Link_Anchor` link
//!   anchor
//! - `error.rs` -- the fatal-error path: `LAST_I_ERROR` diagnostics,
//!   `fatal_error` (log-facade emission + recursion guard + Zenity
//!   popup), the varargs-era `fatal_error_var` wrapper, and the Zenity
//!   process pair
//! - `dosmem.rs` -- the four DOS memory-dump statics, the `-setmem`
//!   first-call selection (`select_mem_dump`), and the exported
//!   `get_memory_value` entry that links the dumps to the dtmc core
//! - `dtmc.rs` -- the extracted pure dump-read core (`read_mem_dump`)
//!   with its baseline vectors
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `AutoAllocMemory` | `zone::auto_alloc_memory` | glue | private in C too (`i_system.c:95`); halve-until-malloc loop, `i_error!` on the floor |
//! | `I_AtExit` | `exit::at_exit` | glue | LIFO registration, malloc'd nodes (`libc::malloc` lifetime convention); C symbol pinned (`d_loop/mod.rs` extern-declares it); upstream `i_system.c:73` |
//! | `I_Tactile` | `banner::tactile` | glue | Logitech Cyberman no-op stub; C symbol pinned (dead-but-exported); upstream `i_system.c:87` |
//! | `I_ZoneBase` | `zone::zone_base` | glue | `-mb` parsing + zone base allocation; `DEFAULT_RAM` stays 32 MiB (see `docs/vanilla-workarounds.md`, zone sizing policy); C symbol pinned; upstream `i_system.c:133` |
//! | `I_PrintBanner` | `banner::print_banner` | glue | 70-column centring via `libc::putchar`/`puts`; C symbol pinned; upstream `i_system.c:166` |
//! | `I_PrintDivider` | `banner::print_divider` | glue | 75-`=` divider; C symbol pinned; upstream `i_system.c:177` |
//! | `I_PrintStartupBanner` | `banner::print_startup_banner` | glue | GPL notice substituted for `PACKAGE_NAME`; C symbol pinned; upstream `i_system.c:189` |
//! | `I_ConsoleStdout` | `banner::console_stdout` | glue | always 0 (the non-`ORIGCODE` C branch); gates the `fatal_error` GUI popup; C symbol pinned; upstream `i_system.c:210` |
//! | `I_Quit` | `exit::quit_engine` | glue | walks `exit_funcs`, no `SDL_Quit`/`exit` (non-`ORIGCODE`); C symbol pinned (`g_game/demo.rs` extern-declares it `-> !`); upstream `i_system.c:246` |
//! | `ZenityAvailable` | `error::zenity_available` | glue | private in C too (`i_system.c:272`); Linux-only; `std::process` probe |
//! | `ZenityErrorBox` | `error::zenity_error_box` | glue | private in C too (`i_system.c:323`); argv-passing instead of the C shell-escape + `system()` |
//! | `I_Error` | `error::fatal_error` | glue | single pre-formatted string (the `i_error!` macro formats on the caller side); `already_quitting` recursion guard, log-facade emission (wasm has no stderr) and the last-error channel are behavior, carried verbatim; C symbol pinned; upstream `i_system.c:359` |
//! | `I_ErrorV` | `error::fatal_error_var` | glue | C preprocessor-macro wrapper, identical behaviour; C symbol pinned (dead-but-exported besides the anchor) |
//! | `I_GetMemoryValue` | `dosmem::get_memory_value` | dtmc + glue | first-call `-setmem` parse stays boot glue (`select_mem_dump`); the pure dump-read core is EXTRACTED to `dtmc::read_mem_dump` -- the returned bytes define the synthetic null-sector's floor/ceiling heights (`p_setup/null_sector.rs`), demo-observable on glass-hack maps; the missing-`else` fall-through and double-increment C bugs stay deliberately fixed; C symbol pinned; upstream `i_system.c:503` |
//! | (extraction) | `dtmc::read_mem_dump` | dtmc | no separate C name -- the read tail of `I_GetMemoryValue`; baseline vectors captured pre-move (commit `dbf097e`) |
//! | `I_System_Link_Anchor` (Rust-only) | `exit::I_System_Link_Anchor` | glue | link anchor kept verbatim (name and body); retires with the freeze zone |
//! | statics | `exit` / `error` / `zone` / `dosmem` | data | `exit_funcs`, `LAST_I_ERROR`, `DEFAULT_RAM`, `MIN_RAM`, `MEM_DUMP_DOS622`, `MEM_DUMP_WIN98`, `MEM_DUMP_DOSBOX`, `MEM_DUMP_CUSTOM`, `dos_mem_dump` keep names; private, no C symbols |
//!
//! ## Deterministic Aspects
//!
//! One dtmc surface by adjudication (f1 report §9.4): the pure read core
//! `dtmc::read_mem_dump`. Its returned bytes are the synthetic
//! null-sector's `floorheight`/`ceilingheight`
//! (`p_setup/null_sector.rs`, the catalogued "impassible glass"
//! emulation in `docs/vanilla-workarounds.md` row 5), which is
//! demo-observable on vex6d-family glass-hack maps -- a desync there is
//! a demo desync. The `-setmem` selection around it is boot
//! configuration (first call, command-line driven, never per-tic), so
//! `dosmem.rs` stays glue; `DosMemDump`/`MEM_DUMP_CUSTOM` are
//! process-lifetime boot state. Everything else is glue: exit hooks,
//! banners, zone sizing (size is not demo-observable, see the zone
//! sizing catalog row) and the fatal-error path are diagnostics/teardown
//! -- never read back by the simulation (`LAST_I_ERROR` feeds host
//! overlays only).

pub mod banner;
pub mod dosmem;
pub mod dtmc;
pub mod error;
pub mod exit;
pub mod zone;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use banner::{
    console_stdout as I_ConsoleStdout, print_banner as I_PrintBanner,
    print_divider as I_PrintDivider, print_startup_banner as I_PrintStartupBanner,
    tactile as I_Tactile,
};
pub use dosmem::get_memory_value as I_GetMemoryValue;
pub use error::{fatal_error as I_Error, fatal_error_var as I_ErrorV, last_i_error};
pub use exit::{at_exit as I_AtExit, quit_engine as I_Quit};
pub use zone::zone_base as I_ZoneBase;

//* path-stability re-export: the link anchor keeps its module-root path
//* (the name was never renamed, so this is wiring, not a shim).
pub use exit::I_System_Link_Anchor;
