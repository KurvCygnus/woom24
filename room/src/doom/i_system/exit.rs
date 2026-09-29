//! The exit-hook registry: the `atexit` list types and `exit_funcs`
//! static, `at_exit` registration, `quit_engine` teardown, and the
//! `I_System_Link_Anchor` link anchor that keeps the module's C symbols
//! alive against dead-code elimination.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::ffi::c_int;
use std::ptr;

use crate::types::Boolean;

use super::banner::{console_stdout, print_banner, print_divider, print_startup_banner, tactile};
use super::dosmem::get_memory_value;
use super::error::fatal_error_var;
use super::zone::zone_base;

/// C signature for an exit callback registered via `I_AtExit`.
/// Matches `typedef void (*atexit_func_t)(void)` from `i_system.h`.
pub(super) type atexit_func_t = extern "C" fn();

/// Single entry in the linked list of registered exit callbacks.
///
/// `#[repr(C)]` because C code in `i_system.c` (and the test harness)
/// may walk this list. Fields mirror `struct atexit_listentry_s`.
#[repr(C)]
pub(super) struct atexit_listentry_t
{
    /// Callback to invoke when the engine exits or errors out.
    pub(super) func: atexit_func_t,
    /// If truthy, the callback is invoked from `I_Error` too, not
    /// only from `I_Quit`.
    pub(super) run_on_error: Boolean,
    /// Next entry in the singly-linked list (most-recently registered
    /// first). Null terminates the list.
    pub(super) next: *mut atexit_listentry_t,
}

/// Head of the singly-linked list of registered exit callbacks.
/// Mirrors the file-scope `exit_funcs` variable in `i_system.c`.
/// Mutated only by `at_exit`, read by `quit_engine` and `fatal_error`.
pub(super) static mut exit_funcs: *mut atexit_listentry_t = ptr::null_mut();

/// Register `func` as an exit callback. If `run_on_error` is truthy,
/// the callback also fires from `I_Error`, not just `I_Quit`.
///
/// Entries form a stack (LIFO): the most recently registered callback
/// runs first. Mirrors `I_AtExit` from `i_system.c`. Allocates the
/// list node with `libc::malloc` to match the C lifetime convention.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs` extern-declares it (freeze-zone linkage surface)
/// and `s_sound/music.rs` imports the upstream name through the root
/// shim.
///
/// # Safety
///
/// `func` must remain valid until invoked at exit or error time.
#[doc(alias = "I_AtExit")]
#[export_name = "I_AtExit"]
pub extern "C" fn at_exit(func: atexit_func_t, run_on_error: Boolean)
{
    unsafe
    {
        let entry =
            libc::malloc(std::mem::size_of::<atexit_listentry_t>()) as *mut atexit_listentry_t;
        (*entry).func = func;
        (*entry).run_on_error = run_on_error;
        (*entry).next = exit_funcs;
        exit_funcs = entry;
    }
}

/// Walk the `exit_funcs` list and invoke every registered callback.
///
/// Unlike the C version this does **not** call `SDL_Quit` or
/// `std::process::exit`: the C source only does so when `ORIGCODE` is
/// defined. The caller is expected to terminate after `I_Quit`
/// returns. Mirrors `I_Quit` from `i_system.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/demo.rs` extern-declares it `-> !` (freeze-zone linkage
/// surface) and the menus import the upstream name through the root
/// shim.
///
/// # Safety
///
/// Trivially safe (walks only registered callbacks).
#[doc(alias = "I_Quit")]
#[export_name = "I_Quit"]
pub extern "C" fn quit_engine()
{
    unsafe
    {
        let mut entry = exit_funcs;
        while !entry.is_null()
        {
            ((*entry).func)();
            entry = (*entry).next;
        }
    }
}

/// Empty exit callback referenced only by `I_System_Link_Anchor` to
/// give `I_AtExit` something to take the address of.
extern "C" fn dummy_atexit() {}

/// Link anchor that references every public C entry point of this
/// module so the linker cannot drop them as dead code.
///
/// Not part of the original Doom API; exists purely so the
/// statically-linked C side can find every `#[no_mangle]` symbol when
/// the Rust crate is built as a library. Never call this; it would
/// allocate and then exit through `I_Error`.
///
/// # Safety
///
/// All arguments are null/zero, so calling this would dereference
/// null pointers in every wrapped entry. Treat as link-only.
#[no_mangle]
pub unsafe extern "C" fn I_System_Link_Anchor()
{
    at_exit(dummy_atexit, Boolean::FALSE);
    tactile(0, 0, 0);
    let mut size: c_int = 0;
    zone_base(&mut size);
    console_stdout();
    quit_engine();
    get_memory_value(0, ptr::null_mut(), 0);
    print_banner(ptr::null_mut());
    print_divider();
    print_startup_banner(ptr::null_mut());
    fatal_error_var(ptr::null());
}
