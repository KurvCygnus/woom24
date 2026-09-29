//! The zone-heap budget: `DEFAULT_RAM` / `MIN_RAM`, the
//! halve-until-malloc allocator, and the `I_ZoneBase` boot entry that
//! hands the engine its zone base.

use std::ffi::c_int;

use crate::i_error;

use crate::doom::crt::c_printf2;
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};

/// Default size of the zone heap, in MiB, when `-mb` is not supplied.
/// Mirrors the `DEFAULT_RAM` macro in `i_system.c`.
///
//* Deviation from vanilla's 6: the vanilla-DOS budget starves the modern
//* tenant mix (640x400 present-path buffers, accumulated level caches) on
//* the restart path - spec 4 defect A froze at wipe_init_melt's 1,280-byte
//* Z_Malloc. Zone size is not observable by demos or any cataloged
//* emulation (see docs/vanilla-workarounds.md, zone sizing policy).
const DEFAULT_RAM: c_int = 32; // MiB

/// Minimum size of the zone heap, in MiB. `auto_alloc_memory` keeps
/// halving the request until it succeeds or drops below this floor.
/// Mirrors the `MIN_RAM` macro in `i_system.c`.
const MIN_RAM: c_int = 6; // MiB

/// Allocate zone memory, shrinking the requested size by 1 MiB at a
/// time until `malloc` succeeds or the size drops below `min_ram`.
///
/// Writes the chosen size in bytes through `size` and returns a
/// pointer to the freshly allocated block. If even `min_ram` cannot
/// be allocated, the function calls `I_Error` (which never returns).
///
/// # Safety
///
/// `size` must be a valid mutable pointer to a `c_int`. The returned
/// pointer owns a `libc::malloc` allocation: it must be freed with
/// `libc::free`, never with the Rust allocator. Mirrors the static
/// helper of the same name in `i_system.c`.
#[doc(alias = "AutoAllocMemory")]
unsafe fn auto_alloc_memory(size: *mut c_int, mut default_ram: c_int, min_ram: c_int) -> *mut u8
{
    let mut zonemem: *mut u8 = std::ptr::null_mut();

    while zonemem.is_null()
    {
        if default_ram < min_ram
        {
            i_error!("Unable to allocate {} MiB of RAM for zone", default_ram);
        }

        *size = default_ram * 1024 * 1024;
        zonemem = libc::malloc(*size as usize) as *mut u8;

        if zonemem.is_null()
        {
            default_ram -= 1;
        }
    }

    zonemem
}

/// Allocate the engine's zone heap and return a pointer to its base.
///
/// Honours the `-mb <MiB>` command-line argument; otherwise uses
/// `DEFAULT_RAM` MiB and shrinks via `auto_alloc_memory`. The chosen
/// size in bytes is written through `size`.
///
/// Mirrors `I_ZoneBase` from `i_system.c`. Called once at startup from
/// `Z_Init` in `z_zone.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `z_zone/zone.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// `size` must be a valid mutable pointer to a `c_int`.
#[doc(alias = "I_ZoneBase")]
#[export_name = "I_ZoneBase"]
pub extern "C" fn zone_base(size: *mut c_int) -> *mut u8
{
    unsafe
    {
        let mut default_ram = DEFAULT_RAM;
        let mut min_ram = MIN_RAM;

        let p = M_CheckParmWithArgs(c"-mb".as_ptr().cast_mut(), 1);
        if p > 0
        {
            default_ram = libc::atoi(*myargv.offset((p + 1) as isize));
            min_ram = default_ram;
        }

        let zonemem = auto_alloc_memory(size, default_ram, min_ram);

        c_printf2(
            c"zone memory: %p, %x allocated for zone\n".as_ptr(),
            zonemem,
            *size,
        );

        zonemem
    }
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_int;

    /// The restart-melt freeze (spec 4 defect A) exhausted the 6 MiB
    /// vanilla-DOS budget; the modern tenant mix needs headroom. The -mb
    /// override path is untouched code and pins MIN_RAM semantics.
    #[test]
    fn zone_base_default_is_32_mib()
    {
        let mut size: c_int = 0;
        let base = super::zone_base(&mut size);
        assert_eq!(size, 32 * 1024 * 1024, "default zone budget must be 32 MiB");
        assert!(!base.is_null(), "I_ZoneBase must return a live zone base");
        // Leak on purpose: the zone is process-lifetime state (exactly what the
        // engine does at boot); freeing it would need Z_Free machinery not wired
        // to raw malloc'd bases.
    }
}
