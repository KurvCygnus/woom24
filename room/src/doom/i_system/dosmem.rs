//! The DOS memory-dump store: the four dump statics (three built-in
//! snapshots plus the `-setmem`-populated custom dump), the first-call
//! `-setmem` selection, and the exported `get_memory_value` entry that
//! links the selected dump to the pure `dtmc::read_mem_dump` core.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void};

use crate::doom::crt::strcasecmp;
use crate::doom::m_argv::{myargc, myargv, M_CheckParmWithArgs};
use crate::doom::m_misc::M_StrToInt;

use super::dtmc::{read_mem_dump, DOS_MEM_DUMP_SIZE};

/// Snapshot of the first 10 bytes at DOS 6.22 segment 0:0, used to
/// emulate the read access violation hack from PrBoom+ for demo
/// playback compatibility. Mirrors `mem_dump_dos622` in `i_system.c`.
pub(super) static MEM_DUMP_DOS622: [u8; DOS_MEM_DUMP_SIZE] =
    [0x57, 0x92, 0x19, 0x00, 0xF4, 0x06, 0x70, 0x00, 0x16, 0x00];

/// Snapshot of the first 10 bytes at DOS 7.1 / Windows 98 segment 0:0
/// for the same NULL-pointer-dereference emulation. Mirrors
/// `mem_dump_win98` in `i_system.c`.
pub(super) static MEM_DUMP_WIN98: [u8; DOS_MEM_DUMP_SIZE] =
    [0x9E, 0x0F, 0xC9, 0x00, 0x65, 0x04, 0x70, 0x00, 0x16, 0x00];

/// Snapshot of the first 10 bytes seen when running DOSBox under
/// Windows XP. Mirrors `mem_dump_dosbox` in `i_system.c`.
pub(super) static MEM_DUMP_DOSBOX: [u8; DOS_MEM_DUMP_SIZE] =
    [0x00, 0x00, 0x00, 0xF1, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00];

/// User-supplied custom memory dump, populated from `-setmem <bytes>`
/// on the command line. Initially zeroed. Mirrors `mem_dump_custom`.
static mut MEM_DUMP_CUSTOM: [u8; DOS_MEM_DUMP_SIZE] = [0; DOS_MEM_DUMP_SIZE];

/// Which of the four built-in memory dumps `get_memory_value` returns.
/// Selected from the `-setmem dos622|dos71|dosbox|<bytes...>` argument
/// on first invocation. In the C source this is a `const unsigned
/// char *` rebinding rather than an enum.
#[derive(Clone, Copy)]
enum DosMemDump
{
    Dos622,
    Win98,
    Dosbox,
    Custom,
}

/// Currently selected DOS memory dump. Mirrors the file-scope
/// `dos_mem_dump` pointer in `i_system.c`, which is initialized to
/// `mem_dump_dos622`.
static mut dos_mem_dump: DosMemDump = DosMemDump::Dos622;

/// First-call `-setmem` selection: parse the command line once and
/// point `dos_mem_dump` at the requested dump (or fill
/// `MEM_DUMP_CUSTOM` with the explicit byte sequence). Boot glue --
/// never demo-sync: the selection happens before the engine reads any
/// demo bytes and is configuration, not simulation state.
///
/// Mirrors the first-call block of `I_GetMemoryValue` in `i_system.c`.
/// The Rust port uses an `else if` chain for the dos622/dos71/dosbox
/// cases, which avoids a latent bug in the C source (missing `else`
/// after the `dos622` branch lets it fall through to `dos71`). The
/// custom-byte loop also uses a single index, fixing a
/// double-increment bug in C.
fn select_mem_dump()
{
    unsafe
    {
        let p = M_CheckParmWithArgs(c"-setmem".as_ptr().cast_mut(), 1);
        if p > 0
        {
            let arg = *myargv.offset((p + 1) as isize);
            if strcasecmp(arg, c"dos622".as_ptr()) == 0
            {
                dos_mem_dump = DosMemDump::Dos622;
            }
            else if strcasecmp(arg, c"dos71".as_ptr()) == 0
            {
                dos_mem_dump = DosMemDump::Win98;
            }
            else if strcasecmp(arg, c"dosbox".as_ptr()) == 0
            {
                dos_mem_dump = DosMemDump::Dosbox;
            }
            else
            {
                let mut idx: usize = 0;
                let mut pp = (p + 1) as isize;
                while idx < DOS_MEM_DUMP_SIZE
                {
                    pp += 1;
                    if pp >= myargc as isize || **myargv.offset(pp) == b'-' as c_char
                    {
                        break;
                    }
                    let mut val: c_int = 0;
                    M_StrToInt(*myargv.offset(pp), &mut val);
                    MEM_DUMP_CUSTOM[idx] = val as u8;
                    idx += 1;
                }
                dos_mem_dump = DosMemDump::Custom;
            }
        }
    }
}

/// PrBoom+ read-access-violation emulator: returns the byte/word/dword
/// at `offset` in the currently selected DOS memory dump. Writes the
/// value through `value` and returns 1 on success, 0 on out-of-range
/// access or unsupported `size`.
///
/// On the first invocation the function parses `-setmem` from the
/// command line to choose between `dos622`, `dos71` (treated as
/// Win98), `dosbox`, or an explicit byte sequence; the read itself is
/// the pure `dtmc::read_mem_dump` core (the demo-synchronization
/// surface -- see that module).
///
/// Mirrors `I_GetMemoryValue` from `i_system.c`. The deliberate
/// C-bug fixes inherited from the flat file (missing-`else`
/// fall-through, double-increment) live in `select_mem_dump` and are
/// documented there.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_setup/null_sector.rs` imports the upstream name through the
/// root shim.
///
/// # Safety
///
/// `value` must be a valid writable slot of at least `size` bytes
/// whenever `size` is 1, 2, or 4.
#[doc(alias = "I_GetMemoryValue")]
#[export_name = "I_GetMemoryValue"]
pub extern "C" fn get_memory_value(offset: c_uint, value: *mut c_void, size: c_int) -> c_int
{
    unsafe
    {
        static mut firsttime: bool = true;

        if firsttime
        {
            firsttime = false;
            select_mem_dump();
        }

        let dump: &[u8; DOS_MEM_DUMP_SIZE] = match dos_mem_dump
        {
            DosMemDump::Dos622 => &MEM_DUMP_DOS622,
            DosMemDump::Win98 => &MEM_DUMP_WIN98,
            DosMemDump::Dosbox => &MEM_DUMP_DOSBOX,
            DosMemDump::Custom => &*std::ptr::addr_of!(MEM_DUMP_CUSTOM),
        };

        read_mem_dump(dump, offset, value, size)
    }
}

#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, c_uint};

    use super::super::dtmc::{read_mem_dump, DOS_MEM_DUMP_SIZE};
    use super::{
        dos_mem_dump, get_memory_value, DosMemDump, MEM_DUMP_DOS622, MEM_DUMP_DOSBOX,
        MEM_DUMP_WIN98,
    };

    /// Live half of the F10 wave F2-c baseline (commit `dbf097e`):
    /// the same vectors, driven through the real exported
    /// `get_memory_value` entry with each dump selected, so the glue
    /// around the dtmc core (dump selection, `-setmem` gating) can
    /// never drift from the shipped body. Runs single-threaded on
    /// purpose: it mutates the `dos_mem_dump` static.
    #[test]
    fn get_memory_value_live_vectors()
    {
        /// One vector: (dump, offset, size, expected return, expected
        /// value). `expected` is only checked when the return is 1.
        type V = (DosMemDump, c_uint, c_int, c_int, u32);

        let vectors: [V; 11] = [
            (DosMemDump::Dos622, 0, 1, 1, 0x57),
            (DosMemDump::Dos622, 0, 4, 1, 0x0019_9257),
            (DosMemDump::Dos622, 4, 4, 1, 0x0070_06F4),
            (DosMemDump::Dos622, 8, 2, 1, 0x0016),
            (DosMemDump::Win98, 0, 4, 1, 0x00C9_0F9E),
            (DosMemDump::Dosbox, 3, 4, 1, 0x0000_00F1),
            (DosMemDump::Dosbox, 6, 4, 1, 0x0007_0000),
            (DosMemDump::Dos622, 10, 1, 0, 0),
            (DosMemDump::Dos622, 9, 2, 0, 0),
            (DosMemDump::Dos622, 7, 4, 0, 0),
            (DosMemDump::Dos622, 0, 3, 0, 0),
        ];

        for &(dump, offset, size, ret, expected) in &vectors
        {
            // SAFETY: static-mut dump selection; this is the only test
            // touching `dos_mem_dump`, and it runs single-threaded.
            unsafe { dos_mem_dump = dump; }

            // The entry writes exactly `size` bytes: mask the sentinel so
            // the expectation keeps the untouched high bytes (the write
            // width is part of the pinned contract -- the null-sector
            // consumer hands us dword slots, but byte/word callers exist
            // in the C surface).
            let width_mask: u32 = if ret == 1 { !0u32 >> (32 - 8 * size) } else { 0 };
            let live_expected = (0xABCD_EF01 & !width_mask) | expected;

            let mut live: u32 = 0xABCD_EF01;
            let got = get_memory_value(offset, std::ptr::addr_of_mut!(live).cast(), size);
            assert_eq!(got, ret, "live return drifted at offset={offset}, size={size}");
            if ret == 1
            {
                assert_eq!(
                    live, live_expected,
                    "live value drifted at offset={offset}, size={size}"
                );
            }
            else
            {
                assert_eq!(live, 0xABCD_EF01, "failed read must not write (offset={offset})");
            }
        }

        // Out-of-range u32 offset must fail the guard, not overflow the
        // `offset as usize` cast.
        // SAFETY: same single-threaded static-mut selection as above.
        unsafe { dos_mem_dump = DosMemDump::Dos622; }
        let mut live: u32 = 0xABCD_EF01;
        assert_eq!(
            get_memory_value(c_uint::MAX, std::ptr::addr_of_mut!(live).cast(), 1),
            0,
            "u32::MAX offset must fail the range guard"
        );
        assert_eq!(live, 0xABCD_EF01, "failed read must not write");
    }

    /// The glue/core seam: `get_memory_value` must agree with the
    /// pure `read_mem_dump` core on the same selected dump for the
    /// null-sector consumer's exact call shapes (offsets 0 and 4,
    /// dword) -- this is the pair that sets the synthetic sector's
    /// heights.
    #[test]
    fn glue_agrees_with_dtmc_core()
    {
        /// (dump, expected offset-0 dword = floor height bytes,
        /// expected offset-4 dword = ceiling height bytes).
        type V = (DosMemDump, u32, u32);

        let vectors: [V; 3] = [
            (DosMemDump::Dos622, 0x0019_9257, 0x0070_06F4),
            (DosMemDump::Win98, 0x00C9_0F9E, 0x0070_0465),
            (DosMemDump::Dosbox, 0xF100_0000, 0x0000_0000),
        ];

        for &(dump, expected_floor, expected_ceiling) in &vectors
        {
            // SAFETY: single-threaded static-mut selection as above.
            unsafe { dos_mem_dump = dump; }

            let mut via_entry: u32 = 0;
            let mut via_core: u32 = 0;
            assert_eq!(get_memory_value(0, std::ptr::addr_of_mut!(via_entry).cast(), 4), 1);
            assert_eq!(
                read_mem_dump(
                    selected_slice(dump),
                    0,
                    std::ptr::addr_of_mut!(via_core).cast(),
                    4
                ),
                1
            );
            assert_eq!(via_entry, expected_floor, "offset-0 dword (floor height) drifted");
            assert_eq!(via_core, expected_floor, "core must reproduce the entry value");

            let mut via_entry4: u32 = 0;
            assert_eq!(get_memory_value(4, std::ptr::addr_of_mut!(via_entry4).cast(), 4), 1);
            assert_eq!(via_entry4, expected_ceiling, "offset-4 dword (ceiling height) drifted");
        }
    }

    // The dump selection is a module-private enum; the helper below
    // exists only so the test above can name the selected slice without
    // widening visibility.
    fn selected_slice(dump: DosMemDump) -> &'static [u8; DOS_MEM_DUMP_SIZE]
    {
        match dump
        {
            DosMemDump::Dos622 => &MEM_DUMP_DOS622,
            DosMemDump::Win98 => &MEM_DUMP_WIN98,
            DosMemDump::Dosbox => &MEM_DUMP_DOSBOX,
            DosMemDump::Custom => unreachable!("custom dump is boot-glue, not a vector"),
        }
    }
}
