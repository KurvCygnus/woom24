//! The SHA-1 compression function: one 512-bit block absorbed into
//! the chaining state. Shape-identical to upstream `sha1.c` -- the
//! RFC vectors pin the digests, never "modernise" the byte-assembled
//! big-endian schedule or the wrapping arithmetic.

use super::context::SHA1Context;

/// Thin alias for `u32::rotate_left`, kept so the SHA-1 step macros read
/// like the C source's `rol(x, n)` macro.
fn rol(x: u32, n: u32) -> u32
{
    x.rotate_left(n)
}

/// SHA-1 compression function: absorbs one 512-bit (64-byte) message block
/// into the chaining state.
///
/// Builds the 16-word big-endian message schedule from `data`, then runs the
/// four rounds of 20 operations each (`f1` through `f4` with their constants
/// `K1..K4`). All arithmetic uses `wrapping_add` / `rotate_left` to mirror
/// C's defined unsigned overflow without relying on Rust's debug-build
/// overflow checks.
///
/// `M!` extends the 16-word schedule on demand and writes back into the
/// circular buffer, matching the C `M(i)` macro. `R!` performs one SHA-1
/// step with a permutation of the working variables; the C source achieves
/// the same effect by rotating which named argument holds which value.
pub(super) fn transform(hd: &mut SHA1Context, data: &[u8; 64])
{
    let mut x = [0u32; 16];
    for i in 0..16
    {
        let off = i * 4;
        x[i] = ((data[off] as u32) << 24)
            | ((data[off + 1] as u32) << 16)
            | ((data[off + 2] as u32) << 8)
            | (data[off + 3] as u32);
    }

    let mut a = hd.h0;
    let mut b = hd.h1;
    let mut c = hd.h2;
    let mut d = hd.h3;
    let mut e = hd.h4;

    /// Round 1 constant (steps 0..=19).
    const K1: u32 = 0x5A827999;
    /// Round 2 constant (steps 20..=39).
    const K2: u32 = 0x6ED9EBA1;
    /// Round 3 constant (steps 40..=59).
    const K3: u32 = 0x8F1BBCDC;
    /// Round 4 constant (steps 60..=79).
    const K4: u32 = 0xCA62C1D6;

    /// Message-schedule extension macro, port of C `M(i)`.
    ///
    /// Computes `W[i] = rol(W[i-3] ^ W[i-8] ^ W[i-14] ^ W[i-16], 1)` in
    /// place using the 16-entry circular buffer `x`. Returns the new word
    /// so it can be plugged directly into an `R!` invocation.
    macro_rules! M
    {
        ($i:expr) => {{
            let tm = x[$i & 0x0f] ^ x[($i - 14) & 0x0f] ^ x[($i - 8) & 0x0f] ^ x[($i - 3) & 0x0f];
            x[$i & 0x0f] = rol(tm, 1);
            x[$i & 0x0f]
        }};
    }

    /// One SHA-1 round step.
    ///
    /// Implements `temp = rol(a, 5) + f(b, c, d) + e + k + m` followed by
    /// the canonical variable shift `(a, b, c, d, e) <- (temp, a, rol(b, 30), c, d)`.
    /// The C source achieves the same shift by rotating macro arguments
    /// across consecutive invocations; the Rust port keeps the variable
    /// names fixed and shuffles their contents.
    macro_rules! R
    {
        ($f:ident, $k:expr, $m:expr) => {{
            let temp = a
                .rotate_left(5)
                .wrapping_add($f(b, c, d))
                .wrapping_add(e)
                .wrapping_add($k)
                .wrapping_add($m);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }};
    }

    /// Round-1 nonlinear function (steps 0..=19): `(x AND y) OR ((NOT x) AND z)`,
    /// rewritten as `z XOR (x AND (y XOR z))` to save one operation. Direct
    /// port of C `F1`.
    fn f1(x: u32, y: u32, z: u32) -> u32
    {
        z ^ (x & (y ^ z))
    }
    /// Round-2 nonlinear function (steps 20..=39): `x XOR y XOR z`. Direct
    /// port of C `F2`.
    fn f2(x: u32, y: u32, z: u32) -> u32
    {
        x ^ y ^ z
    }
    /// Round-3 nonlinear function (steps 40..=59): majority of `x`, `y`, `z`,
    /// written as `(x AND y) OR (z AND (x OR y))`. Direct port of C `F3`.
    fn f3(x: u32, y: u32, z: u32) -> u32
    {
        (x & y) | (z & (x | y))
    }
    /// Round-4 nonlinear function (steps 60..=79): same as `f2`, `x XOR y XOR z`.
    /// Direct port of C `F4`.
    fn f4(x: u32, y: u32, z: u32) -> u32
    {
        x ^ y ^ z
    }

    R!(f1, K1, x[0]);
    R!(f1, K1, x[1]);
    R!(f1, K1, x[2]);
    R!(f1, K1, x[3]);
    R!(f1, K1, x[4]);
    R!(f1, K1, x[5]);
    R!(f1, K1, x[6]);
    R!(f1, K1, x[7]);
    R!(f1, K1, x[8]);
    R!(f1, K1, x[9]);
    R!(f1, K1, x[10]);
    R!(f1, K1, x[11]);
    R!(f1, K1, x[12]);
    R!(f1, K1, x[13]);
    R!(f1, K1, x[14]);
    R!(f1, K1, x[15]);
    R!(f1, K1, M!(16));
    R!(f1, K1, M!(17));
    R!(f1, K1, M!(18));
    R!(f1, K1, M!(19));
    R!(f2, K2, M!(20));
    R!(f2, K2, M!(21));
    R!(f2, K2, M!(22));
    R!(f2, K2, M!(23));
    R!(f2, K2, M!(24));
    R!(f2, K2, M!(25));
    R!(f2, K2, M!(26));
    R!(f2, K2, M!(27));
    R!(f2, K2, M!(28));
    R!(f2, K2, M!(29));
    R!(f2, K2, M!(30));
    R!(f2, K2, M!(31));
    R!(f2, K2, M!(32));
    R!(f2, K2, M!(33));
    R!(f2, K2, M!(34));
    R!(f2, K2, M!(35));
    R!(f2, K2, M!(36));
    R!(f2, K2, M!(37));
    R!(f2, K2, M!(38));
    R!(f2, K2, M!(39));
    R!(f3, K3, M!(40));
    R!(f3, K3, M!(41));
    R!(f3, K3, M!(42));
    R!(f3, K3, M!(43));
    R!(f3, K3, M!(44));
    R!(f3, K3, M!(45));
    R!(f3, K3, M!(46));
    R!(f3, K3, M!(47));
    R!(f3, K3, M!(48));
    R!(f3, K3, M!(49));
    R!(f3, K3, M!(50));
    R!(f3, K3, M!(51));
    R!(f3, K3, M!(52));
    R!(f3, K3, M!(53));
    R!(f3, K3, M!(54));
    R!(f3, K3, M!(55));
    R!(f3, K3, M!(56));
    R!(f3, K3, M!(57));
    R!(f3, K3, M!(58));
    R!(f3, K3, M!(59));
    R!(f4, K4, M!(60));
    R!(f4, K4, M!(61));
    R!(f4, K4, M!(62));
    R!(f4, K4, M!(63));
    R!(f4, K4, M!(64));
    R!(f4, K4, M!(65));
    R!(f4, K4, M!(66));
    R!(f4, K4, M!(67));
    R!(f4, K4, M!(68));
    R!(f4, K4, M!(69));
    R!(f4, K4, M!(70));
    R!(f4, K4, M!(71));
    R!(f4, K4, M!(72));
    R!(f4, K4, M!(73));
    R!(f4, K4, M!(74));
    R!(f4, K4, M!(75));
    R!(f4, K4, M!(76));
    R!(f4, K4, M!(77));
    R!(f4, K4, M!(78));
    R!(f4, K4, M!(79));

    hd.h0 = hd.h0.wrapping_add(a);
    hd.h1 = hd.h1.wrapping_add(b);
    hd.h2 = hd.h2.wrapping_add(c);
    hd.h3 = hd.h3.wrapping_add(d);
    hd.h4 = hd.h4.wrapping_add(e);
}
