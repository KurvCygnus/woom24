//! Demo-synchronization surface extracted from `i_system`: the pure
//! read core of `I_GetMemoryValue` whose bytes define the synthetic
//! null-sector's floor/ceiling heights (`p_setup/null_sector.rs`) --
//! demo-observable state on glass-hack maps.

use std::ffi::{c_int, c_uint, c_void};

/// Length, in bytes, of the simulated low-memory dump consumed by
/// [`read_mem_dump`]. Mirrors the `DOS_MEM_DUMP_SIZE` macro
/// (`i_system.c:491`); the dump statics in `dosmem` conform to it.
pub const DOS_MEM_DUMP_SIZE: usize = 10;

/// Pure dump-read core of `I_GetMemoryValue`
/// (`vendor/doomgeneric/i_system.c:503`, read tail): returns the
/// byte/word/dword at `offset` in `dump`, writing it through the
/// caller's `value` pointer.
///
/// ## Technical Details
///
/// The offset guard runs first (`offset >= DOS_MEM_DUMP_SIZE` fails
/// with a 0 return), then the size dispatch assembles little-endian
/// values byte-by-byte -- `(b0) | (b1 << 8) | (b2 << 16) | (b3 << 24)`
/// -- each arm re-guarded so the read never crosses the 10-byte dump
/// (`offset + 1` / `offset + 3` bounds). Exactness is load-bearing
/// because `p_setup/null_sector.rs` feeds the dword reads at offsets 0
/// and 4 straight into the synthetic sector's `floorheight` /
/// `ceilingheight` (the catalogued "impassible glass" emulation,
/// `docs/vanilla-workarounds.md` row 5): a wrong byte changes sight and
/// rendering on vex6d-family glass-hack maps and desyncs their demos.
/// The baseline vectors in this file were captured against the inline
/// body before the extraction (commit `dbf097e`).
///
/// ## On Calling
///
/// `value` is a raw out-pointer; exactly `size` bytes are written
/// (1, 2, or 4 -- anything else is a no-write failure). A failed read
/// (out-of-range offset or size) returns 0 and never touches `value`,
/// so callers may pass pre-initialised slots and check the return
/// before trusting them. `offset` is `c_uint`; the `as usize` cast
/// cannot overflow the guard (wasm32 `usize` is 32-bit, host 64-bit --
/// both compare correctly against 10). The dump is always the full 10
/// bytes; no allocation, no panic path, no threading assumptions.
#[doc(alias = "I_GetMemoryValue")]
pub fn read_mem_dump(dump: &[u8; DOS_MEM_DUMP_SIZE], offset: c_uint, value: *mut c_void, size: c_int) -> c_int
{
    unsafe
    {
        let offset = offset as usize;
        if offset >= DOS_MEM_DUMP_SIZE
        {
            return 0;
        }

        match size
        {
            1 =>
            {
                *(value as *mut u8) = dump[offset];
                1
            }
            2 =>
            {
                if offset + 1 >= DOS_MEM_DUMP_SIZE
                {
                    return 0;
                }
                *(value as *mut u16) = (dump[offset] as u16) | ((dump[offset + 1] as u16) << 8);
                1
            }
            4 =>
            {
                if offset + 3 >= DOS_MEM_DUMP_SIZE
                {
                    return 0;
                }
                *(value as *mut u32) = (dump[offset] as u32)
                    | ((dump[offset + 1] as u32) << 8)
                    | ((dump[offset + 2] as u32) << 16)
                    | ((dump[offset + 3] as u32) << 24);
                1
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, c_uint, c_void};

    use super::super::dosmem::{MEM_DUMP_DOS622, MEM_DUMP_DOSBOX, MEM_DUMP_WIN98};
    use super::{read_mem_dump, DOS_MEM_DUMP_SIZE};

    /// Baseline contract (F10 wave F2-c): these vectors were written
    /// and run against the inline `I_GetMemoryValue` read core BEFORE
    /// the extraction moved it here (commit `dbf097e`), then
    /// retargeted at `read_mem_dump` -- same vectors, same results
    /// (the live-entry half of that test stays in `dosmem`).
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): the read core -> dtmc, wholly qualifying -- its
    /// bytes are the synthetic null-sector's floor/ceiling heights
    /// (`p_setup/null_sector.rs`), demo-observable on glass-hack maps;
    /// the `-setmem` selection around it is boot glue and stays in
    /// `dosmem`.
    #[test]
    fn baseline_read_mem_dump_vectors()
    {
        /// One vector: (dump, offset, size, expected return, expected
        /// value). `expected` is only checked when the return is 1.
        type V = (&'static [u8; DOS_MEM_DUMP_SIZE], c_uint, c_int, c_int, u32);

        let vectors: [V; 11] = [
            // DOS 6.22: the null-sector consumer's exact reads (offset
            // 0 and 4, dword) plus the byte and word shapes.
            (&MEM_DUMP_DOS622, 0, 1, 1, 0x57),
            (&MEM_DUMP_DOS622, 0, 4, 1, 0x0019_9257),
            (&MEM_DUMP_DOS622, 4, 4, 1, 0x0070_06F4),
            (&MEM_DUMP_DOS622, 8, 2, 1, 0x0016),
            // Win98 (the `-setmem dos71` selection).
            (&MEM_DUMP_WIN98, 0, 4, 1, 0x00C9_0F9E),
            // DOSBox: dword starting in the F1 byte, and the dword that
            // carries the 0x07 high-area byte.
            (&MEM_DUMP_DOSBOX, 3, 4, 1, 0x0000_00F1),
            (&MEM_DUMP_DOSBOX, 6, 4, 1, 0x0007_0000),
            // Failure shapes: the value pointer must stay untouched.
            (&MEM_DUMP_DOS622, 10, 1, 0, 0), // offset == dump size
            (&MEM_DUMP_DOS622, 9, 2, 0, 0),  // word straddles the end
            (&MEM_DUMP_DOS622, 7, 4, 0, 0),  // dword crosses the end
            (&MEM_DUMP_DOS622, 0, 3, 0, 0),  // size not in {1, 2, 4}
        ];

        for &(dump, offset, size, ret, expected) in &vectors
        {
            // The entry writes exactly `size` bytes: mask the sentinel so
            // the expectation keeps the untouched high bytes (the write
            // width is part of the pinned contract).
            let width_mask: u32 = if ret == 1 { !0u32 >> (32 - 8 * size) } else { 0 };
            let expected_word = (0xABCD_EF01 & !width_mask) | expected;

            let mut slot: u32 = 0xABCD_EF01;
            let got =
                read_mem_dump(dump, offset, std::ptr::addr_of_mut!(slot).cast::<c_void>(), size);
            assert_eq!(got, ret, "return drifted at offset={offset}, size={size}");
            if ret == 1
            {
                assert_eq!(slot, expected_word, "value drifted at offset={offset}, size={size}");
            }
            else
            {
                assert_eq!(slot, 0xABCD_EF01, "failed read must not write (offset={offset})");
            }
        }

        // Out-of-range u32 offset must fail the guard, not overflow the
        // `offset as usize` cast.
        let mut slot: u32 = 0xABCD_EF01;
        assert_eq!(
            read_mem_dump(
                &MEM_DUMP_DOS622,
                c_uint::MAX,
                std::ptr::addr_of_mut!(slot).cast::<c_void>(),
                1
            ),
            0,
            "u32::MAX offset must fail the range guard"
        );
        assert_eq!(slot, 0xABCD_EF01, "failed read must not write");
    }

    /// Dump shape pin: every dump conforms to `DOS_MEM_DUMP_SIZE`; the
    /// dtmc vectors index it transitively.
    #[test]
    fn dump_size_contract()
    {
        assert_eq!(MEM_DUMP_DOS622.len(), DOS_MEM_DUMP_SIZE);
        assert_eq!(MEM_DUMP_WIN98.len(), DOS_MEM_DUMP_SIZE);
        assert_eq!(MEM_DUMP_DOSBOX.len(), DOS_MEM_DUMP_SIZE);
    }
}
