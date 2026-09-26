//! Demo-synchronization surface extracted from `m_random`: the random
//! sequence primitive whose exact byte stream -- the pre-increment
//! cursor walk over `RNDTABLE` -- feeds every demo-pinned simulation
//! decision.

use super::RNDTABLE;
use std::ffi::c_int;

/// The DOOM random sequence primitive: advance a cursor by the
/// upstream pre-increment rule and return the next demo-visible byte,
/// bit-exact with the bodies of `P_Random` and `M_Random`
/// (`vendor/doomgeneric/m_random.c`), whose exact stream demos and
/// the simulation state hash pin.
///
/// ## Technical Details
///
/// The pre-increment order is load-bearing: the cursor advances
/// FIRST -- `cursor.wrapping_add(1) & 0xff`, the exact C expression
/// `(cursor + 1) & 0xff` including two's-complement wrap -- and the
/// table byte is read at the NEW cursor, so index 0 is never produced
/// until the 256-wrap lands on it and a zeroed generator's first draw
/// is `RNDTABLE[1]`, never `RNDTABLE[0]`. The wrap at 256 gives the
/// sequence period 256, and the table bytes ARE the demo-visible
/// sequence: any off-by-one here shifts every later draw and moves
/// every demo golden. `wrapping_add` is deliberate -- it reproduces
/// the C two's-complement behavior for every input and never
/// debug-panics.
///
/// ## On Calling
///
/// Pass the CURRENT cursor value, store the returned new cursor back
/// into the caller's static, and never skip or reorder the tuple --
/// it is `(new_cursor, table_byte)`, and reading the byte at the old
/// cursor or dropping the store inverts the whole generator sequence.
/// The function is pure with no threading assumptions of its own;
/// upstream advances the cursors only on the single-threaded game
/// tick, and the root wrappers here preserve exactly that.
#[doc(alias = "P_Random")]
#[doc(alias = "M_Random")]
pub fn random_advance(cursor: c_int) -> (c_int, c_int)
{
    let new_cursor = cursor.wrapping_add(1) & 0xff;
    (new_cursor, RNDTABLE[new_cursor as usize] as c_int)
}

#[cfg(test)]
mod tests
{
    use crate::doom::m_random::dtmc::random_advance;
    use crate::doom::m_random::{M_ClearRandom, M_Random, P_Random, RNDTABLE, prndindex, rndindex};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;
    use std::ffi::c_int;

    /// Baseline contract (F10 graduate #3): these vectors were
    /// written and run against the original `P_Random` / `M_Random`
    /// wrapper bodies BEFORE the sequence primitive moved here, then
    /// retargeted to `random_advance` -- same vectors, same results.
    ///
    /// Adjudication (F10 dtmc criterion: "does this function's
    /// observable behavior belong to the demo synchronization
    /// surface?"): `random_advance` -> dtmc, wholly qualifying -- it
    /// IS the demo generator's integer sequence, consumed by 69
    /// simulation call sites (p_enemy, p_pspr, p_mobj, p_map,
    /// p_inter, p_spec, p_plats, p_lights, g_game); the `static mut`
    /// cursor marshalling stays behind at the root wrappers.
    ///
    /// Transcribed formula: `cursor.wrapping_add(1) & 0xff`, then
    /// `RNDTABLE[new_cursor]`; the tuple order is
    /// `(new_cursor, table_byte)`.
    #[test]
    fn baseline_random_advance()
    {
        // Cursor 0 -> (1, 8): the pre-increment makes index 0 unused
        // until the wrap.
        assert_eq!(random_advance(0), (1, 8));
        assert_eq!(RNDTABLE[1], 8);
        // Cursor 5 -> (6, 149). (The investigation report's draft
        // vector said 107, which is RNDTABLE[7]; the pre-extraction
        // oracle run pinned the actual byte, 149.)
        assert_eq!(random_advance(5), (6, 149));
        assert_eq!(RNDTABLE[6], 149);
        // Wrap case: cursor 255 -> (0, RNDTABLE[0] = 0).
        assert_eq!(random_advance(255), (0, 0));
        assert_eq!(RNDTABLE[0], 0);
        // Tuple-order pin: the byte is read at the NEW cursor, so
        // cursor 0 produces RNDTABLE[1], never RNDTABLE[0].
        let (new_cursor, byte) = random_advance(0);
        assert_eq!(byte, RNDTABLE[new_cursor as usize] as c_int);
        assert_eq!(byte, RNDTABLE[1] as c_int);
    }

    /// Baseline contract (F10 graduate #3): the sequence period --
    /// consecutive draws walk `RNDTABLE[1]`, `RNDTABLE[2]`,
    /// `RNDTABLE[3]`, and exactly 256 advances from a zeroed cursor
    /// land back on cursor 0, so the 257th draw repeats the first
    /// byte.
    #[test]
    fn baseline_random_advance_period()
    {
        let (mut cursor, first) = random_advance(0);
        assert_eq!(first, RNDTABLE[1] as c_int);
        let (c2, b2) = random_advance(cursor);
        cursor = c2;
        assert_eq!(b2, RNDTABLE[2] as c_int);
        let (c3, b3) = random_advance(cursor);
        cursor = c3;
        assert_eq!(b3, RNDTABLE[3] as c_int);
        // 1 + 2 + 253 = 256 advances total: back to the start.
        for _ in 0..253
        {
            let (next, _) = random_advance(cursor);
            cursor = next;
        }
        assert_eq!(cursor, 0);
        let (_, wrapped) = random_advance(cursor);
        assert_eq!(wrapped, first);
    }

    /// Baseline contract (F10 graduate #3): the root wrappers keep
    /// the exact advance-then-store-then-read order through
    /// `dtmc::random_advance`. These vectors were captured green
    /// against the pre-extraction wrapper bodies and re-run here
    /// after the re-route: same vectors, same results.
    #[test]
    fn baseline_wrappers_preserve_sequence()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            prndindex = 0;
            let p = P_Random();
            assert_eq!(p, RNDTABLE[1] as c_int);
            assert_eq!(p, 8);
            assert_eq!(prndindex, 1);
            prndindex = 255;
            let w = P_Random();
            assert_eq!(w, RNDTABLE[0] as c_int);
            assert_eq!(w, 0);
            assert_eq!(prndindex, 0);
            rndindex = 5;
            let m = M_Random();
            assert_eq!(m, RNDTABLE[6] as c_int);
            assert_eq!(m, 149);
            assert_eq!(rndindex, 6);
        }
    }

    /// Baseline contract (F10 graduate #3): the two cursors advance
    /// independently and `M_ClearRandom` zeroes both atomically -- the
    /// reset semantics the state-digest exclusion and demo
    /// reproduction assume. Adjudication note: `M_ClearRandom` stays
    /// whole at module root (its dtmc behavior IS the two stores; no
    /// pure part exists to extract).
    #[test]
    fn baseline_wrappers_independence_and_atomic_reset()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            M_ClearRandom();
            P_Random();
            P_Random();
            M_Random();
            assert_eq!(prndindex, 2);
            assert_eq!(rndindex, 1);
            M_ClearRandom();
            assert_eq!(prndindex, 0);
            assert_eq!(rndindex, 0);
        }
    }
}
