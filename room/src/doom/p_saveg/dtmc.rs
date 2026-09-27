//! Extracted demo-synchronization surface of the savegame byte codec:
//! the stream-alignment formula, the 3-byte big-endian `leveltime`
//! codec, and the NUL-padded header version buffer. Each is the pure
//! computation its caller embeds; the surrounding stream I/O, global
//! mutation, and header marshalling stay at the call sites.

use std::ffi::c_int;

use crate::doom::c_ffi::VERSIONSIZE;

/// The 4-byte stream-alignment formula of the save stream, extracted
/// verbatim from `read_padding`/`write_padding` (upstream C
/// `saveg_read_pad`/`saveg_write_pad`, `p_saveg.rs:198-216`). Every
/// padded record (players, mobjs, specials) sits at a 4-byte boundary in
/// the save file, so byte-exactness of the whole savegame depends on
/// this value.
///
/// ## Technical Details
///
/// `(4 - (pos & 3)) & 3` is 0 for an already-aligned position and 1..=3
/// otherwise; the outer `& 3` folds the `pos % 4 == 0` case. Rust `&`
/// on signed integers is two's-complement, so a negative `pos` (an
/// `ftell` error) yields a deterministic value. Widening `c_long` to
/// `i64` at the call sites is value-preserving.
///
/// ## On Calling
///
/// `pos` is the current stream position (`libc::ftell(save_stream)`).
/// Pure function -- no state, no threading assumptions. Never inline a
/// second copy of the formula: drift here desyncs every padded record.
pub(super) fn pad_len(pos: i64) -> i64
{
    (4 - (pos & 3)) & 3
}

/// The 3-byte big-endian `leveltime` write codec, extracted verbatim
/// from the three shifted `write_byte` calls in
/// `write_save_game_header` (`p_saveg.rs:1335-1339`; upstream C
/// `P_WriteSaveGameHeader`). `leveltime` is a demo-sync input -- the
/// Nightmare respawn gate tests it per-tic -- so the bytes restored on
/// load decide post-load behavior.
///
/// ## Technical Details
///
/// Only the low 24 bits are serialized: a `leveltime` at or above
/// `0x1000000` folds (documented upstream shape, mirrored by the
/// `& 0xff` masks). Byte order is big-endian; reversing it would
/// silently restore a different tic count.
///
/// ## On Calling
///
/// Pure function of the 32-bit tic count; [`leveltime_unpack3`] must
/// stay its exact inverse over the serialized 24-bit domain.
pub(super) fn leveltime_pack3(lt: u32) -> [u8; 3]
{
    [((lt >> 16) & 0xff) as u8, ((lt >> 8) & 0xff) as u8, (lt & 0xff) as u8]
}

/// The 3-byte big-endian `leveltime` read codec, extracted verbatim
/// from the three `read_byte` shifts in `read_save_game_header`
/// (`p_saveg.rs:1394-1398`; upstream C `P_ReadSaveGameHeader`).
/// Counterpart to [`leveltime_pack3`]; see there for the determinism
/// notes.
pub(super) fn leveltime_unpack3(bytes: [u8; 3]) -> u32
{
    ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32)
}

/// The NUL-padded `"version N\0"` header version buffer, extracted
/// verbatim from the write side of `write_save_game_header`
/// (`p_saveg.rs:1314-1322`) and the read side's `expected_padded`
/// (`p_saveg.rs:1370-1382`; upstream C `P_WriteSaveGameHeader` /
/// `P_ReadSaveGameHeader`). Shared by both directions so a write and
/// its read-back always agree; the version COMPARE itself stays at the
/// call site.
///
/// ## Technical Details
///
/// `VERSIONSIZE` is 16. A version string that does not fit truncates
/// silently at 16 bytes (the upstream `.take(VERSIONSIZE)` bound);
/// write and read truncate identically, so the compare stays
/// consistent. Padding bytes are NUL.
///
/// ## On Calling
///
/// `version_code` is the `G_VanillaVersionCode()` value. Pure
/// function -- no state, no threading assumptions.
pub(super) fn version_bytes(version_code: c_int) -> [u8; VERSIONSIZE]
{
    let expected = format!("version {}\0", version_code);
    let expected_bytes = expected.as_bytes();
    let mut expected_padded = [0u8; VERSIONSIZE];
    for (i, &b) in expected_bytes.iter().enumerate().take(VERSIONSIZE)
    {
        expected_padded[i] = b;
    }
    expected_padded
}

#[cfg(test)]
mod tests
{
    // --- F10 wave B4b baseline vectors, retargeted post-move (F10 §2.3) ---

    /// Baseline vectors for the alignment formula: aligned (0, 4),
    /// every unaligned residue (1, 2, 3, 5). Retargeted onto
    /// `dtmc::pad_len` -- the pre-move `padding_calculation` vectors
    /// carried unchanged (commit `b7f07b7` ran them against the inline
    /// formula).
    #[test]
    fn padding_calculation()
    {
        assert_eq!(super::pad_len(0), 0); // aligned
        assert_eq!(super::pad_len(1), 3);
        assert_eq!(super::pad_len(2), 2);
        assert_eq!(super::pad_len(3), 1);
        assert_eq!(super::pad_len(4), 0); // aligned
        assert_eq!(super::pad_len(5), 3);
    }

    /// Baseline vectors: zero, a mid-range value, the full 24-bit
    /// maximum, and bit>=24 truncation (only the low 24 bits are
    /// serialized, so a wider value folds -- documented upstream shape,
    /// mirrored by the `& 0xff` masks). Round-trip identity holds over
    /// the serialized 24-bit domain and folds identically outside it.
    /// Retargeted onto `dtmc::leveltime_pack3`/`leveltime_unpack3`
    /// (pre-move commit `b7f07b7` ran the same vectors against the
    /// in-file transcription of the header-codec shifts).
    #[test]
    fn baseline_leveltime_pack3_unpack3()
    {
        assert_eq!(super::leveltime_pack3(0), [0, 0, 0]);
        assert_eq!(super::leveltime_pack3(0x123456), [0x12, 0x34, 0x56]);
        assert_eq!(super::leveltime_pack3(0xFFFFFF), [0xFF, 0xFF, 0xFF]);
        // bit >= 24 truncation: 0x12345678 serializes as 0x345678.
        assert_eq!(super::leveltime_pack3(0x12345678), [0x34, 0x56, 0x78]);
        assert_eq!(super::leveltime_unpack3([0, 0, 0]), 0);
        assert_eq!(super::leveltime_unpack3([0x12, 0x34, 0x56]), 0x123456);
        assert_eq!(super::leveltime_unpack3([0xFF, 0xFF, 0xFF]), 0xFFFFFF);
        assert_eq!(super::leveltime_unpack3([0x34, 0x56, 0x78]), 0x345678);
        for lt in [0u32, 1, 35, 0x123456, 0xFFFFFF, 0x12345678, 0xDEAD_BEEF]
        {
            assert_eq!(
                super::leveltime_unpack3(super::leveltime_pack3(lt)),
                lt & 0xFFFFFF
            );
        }
    }

    /// Baseline vectors for the header version buffer: the vanilla code
    /// 109 pads to exactly `VERSIONSIZE` bytes with NUL, code 0
    /// likewise, and a code whose string does not fit truncates
    /// silently (no panic) and deterministically -- the `len < 16`
    /// guard shape. Retargeted onto `dtmc::version_bytes` (pre-move
    /// commit `b7f07b7` ran the same vectors against the in-file
    /// transcription of the padded-buffer build).
    #[test]
    fn baseline_version_bytes()
    {
        assert_eq!(super::version_bytes(109), *b"version 109\0\0\0\0\0");
        assert_eq!(super::version_bytes(0), *b"version 0\0\0\0\0\0\0\0");
        // "version 2147483647\0" is 19 bytes; the first VERSIONSIZE
        // bytes are kept, the rest dropped, no panic.
        assert_eq!(super::version_bytes(0x7FFF_FFFF), *b"version 21474836");
    }
}
