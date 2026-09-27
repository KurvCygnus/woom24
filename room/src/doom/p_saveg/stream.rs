//! The save-game byte stream primitives: the three `#[no_mangle]` stream
//! globals (one data home; `g_game` assigns `save_stream` and reads
//! `savegamelength`/`savegame_error`, `c_ffi` re-declares the latter two
//! by symbol for `c_tests`) and the byte/word I/O helpers every archive
//! and unarchive path shares. The C `FILE *` is managed by `g_game` --
//! this module only reads and writes through it.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};
use std::os::raw::c_long;
use std::ptr;

use super::dtmc;

/// Active save-game `FILE *` stream; opened and closed by `g_game.c`.
/// Exported with C linkage so `g_game.c` can assign it before calling any
/// archive or unarchive function.
#[no_mangle]
pub static mut save_stream: *mut libc::FILE = ptr::null_mut();

/// Running byte count of data written to the current save stream.
/// Incremented by `saveg_write8`; read by `g_game.c` after saving completes.
/// C origin: `savegamelength` in `p_saveg.c`.
#[no_mangle]
pub static mut savegamelength: c_int = 0;

/// Non-zero when a read or write error has occurred on `save_stream`.
/// Set to `1` on the first I/O failure; callers check this after
/// archiving/unarchiving to decide whether to accept the save.
/// C origin: `savegame_error` in `p_saveg.c`.
#[no_mangle]
pub static mut savegame_error: c_int = 0;

/// Reads one byte from `save_stream`.
///
/// Sets `savegame_error = 1` on the first short read; subsequent calls still
/// return `0` but do not double-set the flag. C origin: `saveg_read8`.
#[doc(alias = "saveg_read8")]
pub(super) unsafe fn read_byte() -> u8
{
    let mut result: u8 = 0;
    let n = libc::fread(&raw mut result as *mut c_void, 1, 1, save_stream);
    if n < 1 && savegame_error == 0
    {
        savegame_error = 1;
    }
    result
}

/// Writes one byte to `save_stream` and increments `savegamelength`.
///
/// Sets `savegame_error = 1` on the first short write.
///
/// Note: the C version does not increment `savegamelength` here; this port
/// does so to avoid a separate accounting pass.
/// C origin: `saveg_write8`.
#[doc(alias = "saveg_write8")]
pub(super) unsafe fn write_byte(value: u8)
{
    let n = libc::fwrite(&value as *const u8 as *const c_void, 1, 1, save_stream);
    if n < 1 && savegame_error == 0
    {
        savegame_error = 1;
    }
    savegamelength += 1;
}

/// Reads a little-endian 16-bit unsigned integer from `save_stream`.
///
/// Returns the reconstructed value; any read error is recorded in
/// `savegame_error`. C origin: `saveg_read16` (which used `short`; here `u16`
/// avoids sign-extension ambiguity).
#[doc(alias = "saveg_read16")]
pub(super) unsafe fn read_le16() -> u16
{
    let a = read_byte() as u16;
    let b = read_byte() as u16;
    a | (b << 8)
}

/// Writes a little-endian 16-bit unsigned integer to `save_stream`.
///
/// C origin: `saveg_write16`.
#[doc(alias = "saveg_write16")]
pub(super) unsafe fn write_le16(value: u16)
{
    write_byte((value & 0xff) as u8);
    write_byte(((value >> 8) & 0xff) as u8);
}

/// Reads a little-endian 32-bit unsigned integer from `save_stream`.
///
/// C origin: `saveg_read32` (which returned `int`; here `u32` to make
/// bit-pattern semantics explicit before callers cast to signed types).
#[doc(alias = "saveg_read32")]
pub(super) unsafe fn read_le32() -> u32
{
    let a = read_byte() as u32;
    let b = read_byte() as u32;
    let c = read_byte() as u32;
    let d = read_byte() as u32;
    a | (b << 8) | (c << 16) | (d << 24)
}

/// Writes a little-endian 32-bit unsigned integer to `save_stream`.
///
/// C origin: `saveg_write32`.
#[doc(alias = "saveg_write32")]
pub(super) unsafe fn write_le32(value: u32)
{
    write_byte((value & 0xff) as u8);
    write_byte(((value >> 8) & 0xff) as u8);
    write_byte(((value >> 16) & 0xff) as u8);
    write_byte(((value >> 24) & 0xff) as u8);
}

/// Reads and discards padding bytes to align the stream to the next 4-byte
/// boundary.
///
/// Padding is `(4 - (pos & 3)) & 3` bytes, where `pos` is the current stream
/// position; the formula is `dtmc::pad_len`. A stream already aligned reads
/// zero bytes. C origin: `saveg_read_pad`.
#[doc(alias = "saveg_read_pad")]
pub(super) unsafe fn read_padding()
{
    let pos = libc::ftell(save_stream) as c_long;
    let padding = dtmc::pad_len(pos as i64);
    for _ in 0..padding
    {
        read_byte();
    }
}

/// Writes NUL padding bytes to align the stream to the next 4-byte boundary.
///
/// See `read_padding` for the alignment formula (`dtmc::pad_len`). C origin:
/// `saveg_write_pad`.
#[doc(alias = "saveg_write_pad")]
pub(super) unsafe fn write_padding()
{
    let pos = libc::ftell(save_stream) as c_long;
    let padding = dtmc::pad_len(pos as i64);
    for _ in 0..padding
    {
        write_byte(0);
    }
}

/// Reads a 32-bit enum value from the stream as a `u32`.
///
/// Enum values are always stored as 32-bit little-endian integers.
/// C origin: the `saveg_read_enum` macro (alias for `saveg_read32`).
#[doc(alias = "saveg_read_enum")]
pub(super) unsafe fn read_enum32() -> u32
{
    read_le32()
}

/// Writes a 32-bit enum value to the stream.
///
/// C origin: the `saveg_write_enum` macro (alias for `saveg_write32`).
#[doc(alias = "saveg_write_enum")]
pub(super) unsafe fn write_enum32(value: u32)
{
    write_le32(value);
}

#[cfg(test)]
mod tests
{
    // Tests moved with the module split (F10 wave B4b); they are pure
    // byte-vector checks and need no shared-state lock.

    #[test]
    fn endian_write_read_16()
    {
        // Test that write_u16 followed by read_u16 round-trips correctly.
        // Simulate little-endian write: 0x1234 -> [0x34, 0x12]
        let val: u16 = 0x1234;
        let buf = [(val & 0xff) as u8, ((val >> 8) & 0xff) as u8];
        assert_eq!(buf[0], 0x34);
        assert_eq!(buf[1], 0x12);
    }

    #[test]
    fn endian_write_read_32()
    {
        let val: u32 = 0x12345678;
        let buf = [
            (val & 0xff) as u8,
            ((val >> 8) & 0xff) as u8,
            ((val >> 16) & 0xff) as u8,
            ((val >> 24) & 0xff) as u8,
        ];
        assert_eq!(buf[0], 0x78);
        assert_eq!(buf[1], 0x56);
        assert_eq!(buf[2], 0x34);
        assert_eq!(buf[3], 0x12);
    }
}
