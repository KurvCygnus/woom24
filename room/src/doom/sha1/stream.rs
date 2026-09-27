//! The SHA-1 streaming API: init, update, finalize, and the
//! typed-feed helpers, over the private `update_stream` core.

use std::ffi::{c_char, c_uint};

use super::compress::transform;
use super::context::SHA1Context;

/// Initialises a `SHA1Context` with the standard SHA-1 IV.
///
/// Must be called before the first [`update`]. The `buf` field is left
/// untouched (its contents are only ever read up to `count` bytes, and
/// `count` is reset to 0 here). Exported with C linkage for legacy call
/// sites.
///
/// # Safety
///
/// * `hd` must be a valid, properly aligned pointer to a `SHA1Context`,
///   valid for write for the duration of the call.
#[doc(alias = "SHA1_Init")]
pub extern "C" fn init(hd: *mut SHA1Context)
{
    unsafe
    {
        let hd = &mut *hd;
        hd.h0 = 0x67452301;
        hd.h1 = 0xefcdab89;
        hd.h2 = 0x98badcfe;
        hd.h3 = 0x10325476;
        hd.h4 = 0xc3d2e1f0;
        hd.nblocks = 0;
        hd.count = 0;
    }
}

/// Safe-Rust core of the SHA-1 streaming update.
///
/// Accepts the input as `Option<&[u8]>`: `None` is the flush request the C
/// source spells `SHA1_Update(hd, NULL, 0)`. Behaviour mirrors the C source:
///
/// 1. If the staging buffer is full, compress it.
/// 2. If a flush was requested, return.
/// 3. Top up the staging buffer from `inbuf` if it already contains a
///    partial block, then recursively flush.
/// 4. Compress whole 64-byte blocks straight out of `inbuf`.
/// 5. Stash any leftover tail in the staging buffer for the next call.
///
/// `buf_copy` snapshots in steps 1 and 3 work around the aliasing rule that
/// forbids passing `&mut hd` while also borrowing `hd.buf`.
fn update_stream(hd: &mut SHA1Context, inbuf: Option<&[u8]>)
{
    if hd.count == 64
    {
        let buf_copy = hd.buf;
        transform(hd, &buf_copy);
        hd.count = 0;
        hd.nblocks += 1;
    }

    let Some(inbuf) = inbuf else { return };

    let mut inlen = inbuf.len();
    let mut pos = 0;

    if hd.count != 0
    {
        while inlen > 0 && hd.count < 64
        {
            hd.buf[hd.count as usize] = inbuf[pos];
            hd.count += 1;
            pos += 1;
            inlen -= 1;
        }
        let _buf_copy = hd.buf;
        update_stream(hd, None);
        if inlen == 0
        {
            return;
        }
    }

    while inlen >= 64
    {
        let mut block = [0u8; 64];
        block.copy_from_slice(&inbuf[pos..pos + 64]);
        transform(hd, &block);
        hd.count = 0;
        hd.nblocks += 1;
        inlen -= 64;
        pos += 64;
    }

    while inlen > 0 && hd.count < 64
    {
        hd.buf[hd.count as usize] = inbuf[pos];
        hd.count += 1;
        pos += 1;
        inlen -= 1;
    }
}

/// C-ABI wrapper: absorb `inlen` bytes from `inbuf` into `hd`.
///
/// A null `inbuf` is treated as a flush request, matching the C source's
/// `SHA1_Update(hd, NULL, 0)` convention. After flushing, all complete
/// 64-byte blocks of input are compressed in place; any remainder is staged
/// for the next call.
///
/// # Safety
///
/// * `hd` must be a valid, properly aligned pointer to a `SHA1Context`
///   previously initialised with [`init`], valid for read and write for
///   the duration of the call.
/// * If `inbuf` is non-null, it must point to at least `inlen` readable
///   bytes that do not alias `hd`.
/// * If `inbuf` is null, `inlen` is ignored.
#[doc(alias = "SHA1_Update")]
pub unsafe extern "C" fn update(hd: *mut SHA1Context, inbuf: *mut u8, inlen: usize)
{
    let hd = &mut *hd;
    if inbuf.is_null()
    {
        update_stream(hd, None);
    }
    else
    {
        let slice = std::slice::from_raw_parts(inbuf, inlen);
        update_stream(hd, Some(slice));
    }
}

/// Finalises a SHA-1 stream and writes the 20-byte digest to `digest`.
///
/// Appends the standard SHA-1 padding (`0x80` byte, zero fill, big-endian
/// 64-bit bit count) and compresses the resulting block(s), then serialises
/// `h0..h4` big-endian into the staging buffer and copies the first 20
/// bytes out. After this call `hd` is left in a state where any further
/// [`update`] will produce garbage; re-initialise with [`init`] to
/// reuse the context.
///
/// # Safety
///
/// * `hd` must be a valid, properly aligned pointer to a `SHA1Context`
///   previously initialised with [`init`], valid for read and write for
///   the duration of the call.
/// * `digest` must point to at least 20 writable bytes that do not alias
///   `hd`.
#[doc(alias = "SHA1_Final")]
pub unsafe extern "C" fn finalize(digest: *mut u8, hd: *mut SHA1Context)
{
    let hd = &mut *hd;

    update_stream(hd, None);

    let t = hd.nblocks;
    let mut lsb = t << 6;
    let mut msb = t >> 26;
    let t_orig = lsb;
    lsb = lsb.wrapping_add(hd.count as u32);
    if lsb < t_orig
    {
        msb += 1;
    }
    let t_after = lsb;
    lsb <<= 3;
    msb <<= 3;
    msb |= t_after >> 29;

    if hd.count < 56
    {
        hd.buf[hd.count as usize] = 0x80;
        hd.count += 1;
        while hd.count < 56
        {
            hd.buf[hd.count as usize] = 0;
            hd.count += 1;
        }
    }
    else
    {
        hd.buf[hd.count as usize] = 0x80;
        hd.count += 1;
        while hd.count < 64
        {
            hd.buf[hd.count as usize] = 0;
            hd.count += 1;
        }
        let _buf_copy = hd.buf;
        update_stream(hd, None);
        for i in 0..56
        {
            hd.buf[i] = 0;
        }
    }

    hd.buf[56] = (msb >> 24) as u8;
    hd.buf[57] = (msb >> 16) as u8;
    hd.buf[58] = (msb >> 8) as u8;
    hd.buf[59] = msb as u8;
    hd.buf[60] = (lsb >> 24) as u8;
    hd.buf[61] = (lsb >> 16) as u8;
    hd.buf[62] = (lsb >> 8) as u8;
    hd.buf[63] = lsb as u8;

    let buf_copy = hd.buf;
    transform(hd, &buf_copy);

    hd.buf[0] = (hd.h0 >> 24) as u8;
    hd.buf[1] = (hd.h0 >> 16) as u8;
    hd.buf[2] = (hd.h0 >> 8) as u8;
    hd.buf[3] = hd.h0 as u8;
    hd.buf[4] = (hd.h1 >> 24) as u8;
    hd.buf[5] = (hd.h1 >> 16) as u8;
    hd.buf[6] = (hd.h1 >> 8) as u8;
    hd.buf[7] = hd.h1 as u8;
    hd.buf[8] = (hd.h2 >> 24) as u8;
    hd.buf[9] = (hd.h2 >> 16) as u8;
    hd.buf[10] = (hd.h2 >> 8) as u8;
    hd.buf[11] = hd.h2 as u8;
    hd.buf[12] = (hd.h3 >> 24) as u8;
    hd.buf[13] = (hd.h3 >> 16) as u8;
    hd.buf[14] = (hd.h3 >> 8) as u8;
    hd.buf[15] = hd.h3 as u8;
    hd.buf[16] = (hd.h4 >> 24) as u8;
    hd.buf[17] = (hd.h4 >> 16) as u8;
    hd.buf[18] = (hd.h4 >> 8) as u8;
    hd.buf[19] = hd.h4 as u8;

    std::ptr::copy_nonoverlapping(hd.buf.as_ptr(), digest, 20);
}

/// Feeds a `u32` value into `context` in big-endian byte order.
///
/// Used by the netcode and demo-checksum paths so the digest is portable
/// across host endianness.
///
/// # Safety
///
/// `context` must be a valid, properly aligned pointer to a `SHA1Context`
/// previously initialised with [`init`], valid for read and write for
/// the duration of the call.
#[doc(alias = "SHA1_UpdateInt32")]
pub unsafe extern "C" fn update_int32(context: *mut SHA1Context, val: c_uint)
{
    let context = &mut *context;
    let buf = [
        ((val >> 24) & 0xff) as u8,
        ((val >> 16) & 0xff) as u8,
        ((val >> 8) & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    update_stream(context, Some(&buf));
}

/// Feeds a NUL-terminated C string into `context`, including the trailing
/// NUL byte.
///
/// Hashing the NUL is intentional in the C source (`SHA1_Update(context,
/// (byte *) str, strlen(str) + 1)`), so back-to-back strings cannot collide
/// the way `"ab"` + `"c"` would collide with `"a"` + `"bc"`.
///
/// # Safety
///
/// * `context` must be a valid, properly aligned pointer to a `SHA1Context`
///   previously initialised with [`init`], valid for read and write for
///   the duration of the call.
/// * `str` must point to a valid NUL-terminated C string. Memory up to and
///   including the NUL must be readable.
#[doc(alias = "SHA1_UpdateString")]
pub unsafe extern "C" fn update_string(context: *mut SHA1Context, str: *mut c_char)
{
    let context = &mut *context;
    let cstr = std::ffi::CStr::from_ptr(str);
    let bytes = cstr.to_bytes_with_nul();
    update_stream(context, Some(bytes));
}

/// The RFC-vector test module (published SHA-1 test vectors from
/// RFC 3174 / FIPS 180-1 plus the doomgeneric-specific helper APIs),
/// re-pointed to the graduated names after the split -- same vectors,
/// same digests (F10 wave B5). These published vectors are the
/// module's baseline and were already in place pre-move.
#[cfg(test)]
mod tests
{
    use super::*;
    use std::ffi::c_char;

    /// Allocates a fresh zero-filled `SHA1Context` and initialises it.
    /// Zeroing first guarantees the staging `buf` is in a known state, which
    /// matters because `init` deliberately leaves it untouched.
    fn make_ctx() -> SHA1Context
    {
        let mut ctx: SHA1Context = unsafe { std::mem::zeroed() };
        init(&mut ctx);
        ctx
    }

    /// Convenience helper: hashes `data` with the public C API and returns
    /// the digest as a 40-character lowercase hex string.
    fn sha1_hex(data: &[u8]) -> String
    {
        let mut ctx = make_ctx();
        unsafe
        {
            update(&mut ctx, data.as_ptr() as *mut u8, data.len());
            let mut digest = [0u8; 20];
            finalize(digest.as_mut_ptr(), &mut ctx);
            digest.iter().map(|b| format!("{:02x}", b)).collect()
        }
    }

    /// RFC 3174 vector 1: SHA1("abc").
    #[test]
    fn rfc_abc()
    {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    /// RFC 3174 vector 2: 448-bit message.
    #[test]
    fn rfc_448bit()
    {
        let msg = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(sha1_hex(msg), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    }

    /// SHA1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
    #[test]
    fn empty_input()
    {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    /// `update_int32` sends the value in big-endian byte order, so its
    /// digest must match SHA1 of the four raw bytes.
    #[test]
    fn update_int32_big_endian()
    {
        let val: u32 = 0xDEADBEEF;
        let bytes = [
            ((val >> 24) & 0xFF) as u8,
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        ];
        let expected = sha1_hex(&bytes);

        let mut ctx = make_ctx();
        unsafe
        {
            update_int32(&mut ctx, val);
            let mut digest = [0u8; 20];
            finalize(digest.as_mut_ptr(), &mut ctx);
            let got: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
            assert_eq!(got, expected);
        }
    }

    /// `update_string` feeds the string with its NUL terminator, so its
    /// digest must match SHA1 of the bytes including the trailing '\0'.
    #[test]
    fn update_string_includes_nul_terminator()
    {
        let expected = sha1_hex(b"doom\0");

        let mut ctx = make_ctx();
        let mut s = *b"doom\0";
        unsafe
        {
            update_string(&mut ctx, s.as_mut_ptr() as *mut c_char);
            let mut digest = [0u8; 20];
            finalize(digest.as_mut_ptr(), &mut ctx);
            let got: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
            assert_eq!(got, expected);
        }
    }

    /// Feeding data in two separate `update` calls must yield the same
    /// digest as a single call with the concatenated data.
    #[test]
    fn incremental_update_matches_single_update()
    {
        let part1 = b"Hello, ";
        let part2 = b"world!";
        let combined = b"Hello, world!";

        let expected = sha1_hex(combined);

        let mut ctx = make_ctx();
        unsafe
        {
            update(&mut ctx, part1.as_ptr() as *mut u8, part1.len());
            update(&mut ctx, part2.as_ptr() as *mut u8, part2.len());
            let mut digest = [0u8; 20];
            finalize(digest.as_mut_ptr(), &mut ctx);
            let got: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
            assert_eq!(got, expected);
        }
    }

    /// Exactly 64 bytes of input fills one transform block exactly.
    #[test]
    fn exactly_one_block()
    {
        let data = [0x61u8; 64]; // 64 × 'a'
                                 // Must not panic and must produce a non-zero digest.
        let hex = sha1_hex(&data);
        assert_eq!(hex.len(), 40);
        assert_ne!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709"); // != empty
    }

    /// Multi-block input (>64 bytes) exercises the internal loop.
    #[test]
    fn multi_block_input()
    {
        // 128 bytes — exactly two transform blocks
        let data = [0x62u8; 128]; // 128 × 'b'
        let hex = sha1_hex(&data);
        assert_eq!(hex.len(), 40);
        assert_ne!(hex, sha1_hex(b""));
    }
}
