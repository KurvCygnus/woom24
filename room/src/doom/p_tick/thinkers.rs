//! The thinker list: the `repr(C)` vocabulary every per-tic game
//! object embeds (`actionf_t`, `thinker_t`), the circular list head
//! (`thinkercap`), and the five C-linkage list operations (init,
//! tail-append, lazy-remove, the upstream no-op allocate stub, and
//! the per-tic dispatcher `P_RunThinkers`), plus their test module.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;

use super::dtmc::{is_sentinel, sentinel_ac};
use crate::doom::z_zone::Z_Free;

/// Union of the three C function-pointer variants stored in every thinker.
///
/// `acv` — a no-argument callback (also used to hold the sentinel value `-1`).
/// `acp1` — the common single-argument callback: `fn(*mut void)`.
/// `acp2` — a two-argument callback: `fn(*mut void, *mut void)`.
///
/// Callers cast the concrete thinker pointer (e.g. `*mut ceiling_t`) to
/// `*mut c_void` when invoking `acp1`.  The active variant is always `acp1`
/// during normal thinker execution; `acv` is only written by
/// [`P_RemoveThinker`] to store the sentinel.
#[repr(C)]
#[derive(Clone, Copy)]
pub union actionf_t
{
    /// No-argument action; also used to carry the removal sentinel (`-1`).
    pub acv: Option<unsafe extern "C" fn()>,
    /// Single-argument action — the common thinker callback form.
    pub acp1: Option<unsafe extern "C" fn(*mut c_void)>,
    /// Two-argument action — used by a small number of thinker types.
    pub acp2: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
}

/// Linked-list node embedded at offset 0 of every thinker-based game object.
///
/// All thinkers must be allocated via `Z_Malloc` so that [`P_RunThinkers`] can
/// call `Z_Free` on removed nodes.  The concrete struct (e.g. `ceiling_t`,
/// `plat_t`, `mobj_t`) begins with this header, enabling safe pointer casts
/// between `*mut thinker_t` and the owning type.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct thinker_t
{
    /// Pointer to the previous node in the circular thinker list.
    pub prev: *mut thinker_t,
    /// Pointer to the next node in the circular thinker list.
    pub next: *mut thinker_t,
    /// The per-tic callback, or the removal sentinel when set by
    /// [`P_RemoveThinker`].
    pub function: actionf_t,
}

/// Sentinel head/tail node of the doubly-linked thinker list.
///
/// The list is circular: `thinkercap.next` is the first real thinker and
/// `thinkercap.prev` is the last.  When the list is empty both fields point
/// back to `thinkercap` itself (see [`P_InitThinkers`]).
///
/// Exported as `thinkercap` for C linkage.
#[no_mangle]
pub static mut thinkercap: thinker_t = thinker_t
{
    prev: std::ptr::null_mut::<thinker_t>(),
    next: std::ptr::null_mut::<thinker_t>(),
    function: actionf_t { acv: None },
};

/// Reset the thinker list to empty by making [`thinkercap`] point to itself.
///
/// Must be called at the start of each level before any thinker is added.
/// Corresponds to `P_InitThinkers` in `p_tick.c`.
#[no_mangle]
pub extern "C" fn P_InitThinkers()
{
    unsafe
    {
        thinkercap.prev = &raw mut thinkercap;
        thinkercap.next = &raw mut thinkercap;
    }
}

/// Append `thinker` to the end of the global thinker list.
///
/// The new node is inserted before [`thinkercap`], making it the logical
/// tail of the list.  Corresponds to `P_AddThinker` in `p_tick.c`.
///
/// # Safety
///
/// `thinker` must be a valid, non-null pointer to a `thinker_t` that was
/// allocated via `Z_Malloc` and will not be freed by the caller — ownership
/// transfers to the thinker list, which frees it in [`P_RunThinkers`] once
/// the removal sentinel is detected.
#[no_mangle]
pub extern "C" fn P_AddThinker(thinker: *mut thinker_t)
{
    unsafe
    {
        let cap = &raw mut thinkercap;
        (*(*cap).prev).next = thinker;
        (*thinker).next = cap;
        (*thinker).prev = (*cap).prev;
        (*cap).prev = thinker;
    }
}

/// Mark `thinker` for deferred removal by setting its function to the
/// sentinel value `(actionf_v)(-1)`.
///
/// The thinker is not immediately unlinked or freed; [`P_RunThinkers`]
/// performs the actual removal and deallocation on the next iteration.
/// This matches the "lazy deallocation" comment in `p_tick.c`.
///
/// # Safety
///
/// `thinker` must be a valid, non-null pointer to a thinker that is currently
/// linked into the global thinker list.
#[no_mangle]
pub extern "C" fn P_RemoveThinker(thinker: *mut thinker_t) { unsafe { (*thinker).function.acv = sentinel_ac(); } }

/// No-op stub matching the C `P_AllocateThinker` signature.
///
/// The original C function body is empty; this Rust port preserves that
/// to keep the symbol available for any C caller that references it.
#[no_mangle]
pub extern "C" fn P_AllocateThinker(_thinker: *mut thinker_t) {}

/// Iterate the thinker list, execute each thinker's callback, and free any
/// thinkers that have been marked for removal.
///
/// For each node in the list:
/// - If the node's `function.acv` equals the sentinel (`-1`), the node is
///   unlinked and freed via `Z_Free`.
/// - Otherwise, `function.acp1` is called with the node pointer cast to
///   `*mut c_void`.
///
/// The C original advances the cursor *after* `Z_Free`, reading from already-
/// freed memory.  This Rust port saves `next` before freeing to avoid the
/// undefined behaviour.
///
/// Corresponds to `P_RunThinkers` in `p_tick.c`.
#[no_mangle]
pub extern "C" fn P_RunThinkers()
{
    unsafe
    {
        let cap = &raw mut thinkercap;
        let mut current = (*cap).next;

        while current != cap
        {
            if is_sentinel((*current).function)
            {
                let prev = (*current).prev;
                let next = (*current).next;
                (*prev).next = next;
                (*next).prev = prev;
                Z_Free(current as *mut c_void);
                current = next;
            }
            else
            {
                if let Some(fn_ptr) = (*current).function.acp1 { fn_ptr(current as *mut c_void); }
                current = (*current).next;
            }
        }
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_tick::{P_InitThinkers, thinkercap, thinker_t};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    const THINKER_T_SIZEOF: usize = 24;

    /// `thinker_t` must stay a 24-byte (LP64) struct: it prefixes every
    /// thinker-based game object (`mobj_t` == 224 bytes = 24-byte
    /// prefix plus body), and `c_tests/struct_layouts.rs` pins the
    /// C-side offsets against exactly this layout.
    #[test]
    fn thinker_t_size_matches_c()
    {
        assert_eq!(
            std::mem::size_of::<thinker_t>(),
            THINKER_T_SIZEOF,
            "thinker_t size mismatch: Rust={}, expected={}",
            std::mem::size_of::<thinker_t>(),
            THINKER_T_SIZEOF,
        );
    }

    /// `P_InitThinkers` resets the cap to point at itself in both
    /// directions -- the empty-list state every level's thinker
    /// sequence starts from. Serialized on the shared engine-statics
    /// lock (this test writes the process-global `thinkercap`).
    #[test]
    fn init_sets_self_pointers()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            thinkercap.prev = std::ptr::null_mut::<thinker_t>();
            thinkercap.next = std::ptr::null_mut::<thinker_t>();
            P_InitThinkers();
            let cap = std::ptr::addr_of_mut!(thinkercap);
            assert_eq!(std::ptr::addr_of!(thinkercap.prev).read(), cap);
            assert_eq!(std::ptr::addr_of!(thinkercap.next).read(), cap);
        }
    }
}
