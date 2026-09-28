//! The REJECT table loader: caches the precomputed line-of-sight reject
//! bit array, and -- when the WAD's lump is undersized -- pads its tail
//! with a modeled vanilla zone block header so that reads past the lump
//! end reproduce what vanilla read. The padding is cataloged as
//! `docs/vanilla-workarounds.md` row 4 ("REJECT undersized-lump read");
//! the header formula itself lives in `dtmc::reject_pad_words`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};

use crate::doom::m_argv::M_CheckParm;
use crate::doom::violations::{self, VanillaViolation};
use crate::doom::w_wad::{W_CacheLumpNum, W_LumpLength, W_ReadLump};
use crate::doom::z_zone::{PU_LEVEL, Z_Malloc};

use super::globals::{numsectors, rejectmatrix};
use super::grouplines::totallines;

/// Pad the tail of an undersized REJECT lump to simulate vanilla Doom's
/// behavior of reading past the end of the lump into the zone-memory block
/// header.
///
/// Vanilla Doom allocated the REJECT array with `Z_Malloc`, and when the lump
/// was shorter than the required `(numsectors * numsectors + 7) / 8` bytes,
/// reads of the missing bytes would fall into the zone block header that
/// immediately precedes the allocation in memory. This function reproduces
/// those header bytes so that WADs relying on the overflow behavior work
/// correctly.
///
/// `rejectpad` encodes the first 16 bytes of a zone block header (the
/// initializer is the extracted [`super::dtmc::reject_pad_words`]):
/// - `[(totallines * 4 + 3) & !3) + 24]` - block size field
/// - `0` - user pointer (low word of z_zone header)
/// - `50` - `PU_LEVEL` tag
/// - `0x1d4a11` - `DOOM_CONST_ZONEID`
///
/// If `len > sizeof(rejectpad)` (i.e. the lump is extremely short), a warning
/// is printed and the remainder is filled with 0x00 or 0xff depending on
/// whether `-reject_pad_with_ff` was passed on the command line.
///
/// # Safety
///
/// `array` must point to at least `len` writable bytes. The caller
/// (`load_reject`) ensures this by passing `rejectmatrix + lumplen` with
/// `len = minlength - lumplen`.
#[doc(alias = "PadRejectArray")]
unsafe fn pad_reject_array(array: *mut u8, len: usize)
{
    let rejectpad: [u32; 4] = super::dtmc::reject_pad_words(totallines);

    let mut dest = array;
    for i in 0..len.min(std::mem::size_of_val(&rejectpad))
    {
        let byte_num = i % 4;
        *dest = ((rejectpad[i / 4] >> (byte_num * 8)) & 0xff) as u8;
        dest = dest.add(1);
    }

    if len > std::mem::size_of_val(&rejectpad)
    {
        eprintln!(
            "PadRejectArray: REJECT lump too short to pad! ({} > {})",
            len,
            std::mem::size_of_val(&rejectpad)
        );

        let padvalue = if M_CheckParm(c"-reject_pad_with_ff".as_ptr() as *mut c_char) != 0
        {
            0xff
        }
        else { 0xf00 };

        let pad_byte = padvalue as u8;
        let pad_start = array.add(std::mem::size_of_val(&rejectpad));
        let pad_len = len - std::mem::size_of_val(&rejectpad);
        for i in 0..pad_len { *pad_start.add(i) = pad_byte; }
    }
}

/// Load or synthesize the REJECT lump for the current map.
///
/// The REJECT table is a packed bit array of size
/// `ceil(numsectors^2 / 8)` bytes. Bit `(s1 * numsectors + s2)` is set if
/// sectors `s1` and `s2` cannot see each other, allowing the enemy AI to skip
/// expensive LOS checks.
///
/// If the WAD's REJECT lump is large enough (`lumplen >= minlength`), it is
/// cached directly at `PU_LEVEL`. Otherwise a `minlength`-byte buffer is
/// allocated, the lump is read into it, and the remaining bytes are filled by
/// `PadRejectArray` to simulate vanilla Doom's zone-header overflow.
///
/// # Safety
///
/// Writes to the global `rejectmatrix`. Must be called after `P_GroupLines`
/// so that `totallines` is valid (needed by `PadRejectArray`).
#[doc(alias = "P_LoadReject")]
pub(super) unsafe fn load_reject(lumpnum: c_int)
{
    let minlength = (numsectors * numsectors + 7) / 8;
    let lumplen = W_LumpLength(lumpnum as c_uint);

    if lumplen >= minlength { rejectmatrix = W_CacheLumpNum(lumpnum, PU_LEVEL) as *mut u8; }
    else
    {
        violations::record(VanillaViolation::RejectPadOverrun);
        rejectmatrix =
            Z_Malloc(minlength, PU_LEVEL, &raw mut rejectmatrix as *mut c_void) as *mut u8;
        W_ReadLump(lumpnum as c_uint, rejectmatrix as *mut c_void);
        pad_reject_array(
            rejectmatrix.add(lumplen as usize),
            (minlength - lumplen) as usize,
        );
    }
}
