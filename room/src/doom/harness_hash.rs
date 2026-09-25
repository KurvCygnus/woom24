//! Canonical harness hashes shared by the wasm shell exports and the host
//! scenario twin (F9).
//!
//! Diagnostics only: nothing in the simulation may ever read these (same
//! contract as `violations.rs`) - a state hash that fed back into the
//! simulation would break demo-exact determinism. The field set and order of
//! [`state_hash`] mirror the `Snapshot` struct of the `demo_playthrough`
//! regression (minus tic/virtual_ms/frames), so every F9 ledger anchor pins
//! the same observable state that golden demo test pins.

use crate::doom::d_loop::gametic;
use crate::doom::g_game::{gamestate, players};
use crate::doom::m_random::{prndindex, rndindex};
use crate::doom::p_telept::mobj_t;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

/// 64-bit FNV-1a over `bytes` - the ledger's canonical byte-string digest.
pub fn fnv1a64(bytes: &[u8]) -> u64
{
    let mut h = FNV_OFFSET;
    for b in bytes
    {
        h ^= u64::from(*b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Frame-ledger digest: FNV-1a over one frame's bytes. A thin named wrapper
/// so the ledger semantic has exactly one name across the wasm shell export
/// and the host scenario twin.
pub fn frame_hash(buf: &[u8]) -> u64 { fnv1a64(buf) }

/// Canonical engine-state digest for F9 ledger anchors.
///
/// Hashes exactly this field order, each as a fixed-width little-endian
/// encoding appended in sequence: `gametic:i32, gamestate:i32, rndindex:i32,
/// prndindex:i32, health:i32, armorpoints:i32, killcount:i32, itemcount:i32,
/// secretcount:i32, readyweapon:i32, ammo[4]:i32, mo_x:i32, mo_y:i32,
/// mo_z:i32, mo_angle:u32`. This is the `demo_playthrough` `Snapshot` field
/// set, minus tic/virtual_ms/frames. Player index 0; when `players[0].mo` is
/// null the four mo fields hash as zero (no panic - the fallback is a pure
/// constant), so a pristine or title-screen state hashes fine.
pub fn state_hash() -> u64
{
    fnv1a64(&state_hash_bytes())
}

/// Load-comparable variant of [`state_hash`] (F9 `save_load_roundtrip`).
///
/// Hashes the exact same 72-byte ledger encoding with the three words a
/// savegame does not carry zeroed: `gametic` (word 0, the engine loop's own
/// tic counter), `rndindex` (word 2) and `prndindex` (word 3) -- a load
/// restores the archived game state but keeps the loop tic counting, and
/// `G_InitNew` resets both RNG indices via `M_ClearRandom`, so the full
/// `state_hash` can never agree across a save/load roundtrip even when the
/// restore is byte-exact. Everything else (gamestate, player ledger fields,
/// mobj position/angle) is archived by the savegame and must agree; the
/// runner's `expect_state_at_load` step compares this variant against the
/// pre-load anchor's.
pub fn state_hash_load() -> u64
{
    let mut v = state_hash_bytes();
    v[0..4].copy_from_slice(&0_i32.to_le_bytes());
    v[8..12].copy_from_slice(&0_i32.to_le_bytes());
    v[12..16].copy_from_slice(&0_i32.to_le_bytes());
    fnv1a64(&v)
}

/// The exact byte encoding digested by [`state_hash`]: the ledger fields in
/// the documented order, each fixed-width little-endian (18 fields x 4 bytes
/// = 72). Split out so the layout golden test can pin order, width, and
/// endianness directly; [`state_hash`] is just FNV-1a over this.
fn state_hash_bytes() -> Vec<u8>
{
    let mut v: Vec<u8> = Vec::with_capacity(18 * 4);

    // SAFETY: diagnostics-only snapshot of engine statics for the F9 anchor.
    // Every access is a by-value place read into locals (no reference is
    // ever taken into a `static mut`), and the anchor is invoked on the
    // thread that drives the engine, so no simulation write can interleave
    // with the snapshot.
    let (gametic_v, gamestate_v, rndindex_v, prndindex_v) =
        unsafe { (gametic, gamestate, rndindex, prndindex) };
    let (health, armorpoints, killcount, itemcount, secretcount, readyweapon, ammo, mo) = unsafe
    {
        (
            players[0].health,
            players[0].armorpoints,
            players[0].killcount,
            players[0].itemcount,
            players[0].secretcount,
            players[0].readyweapon,
            players[0].ammo,
            players[0].mo,
        )
    };

    v.extend_from_slice(&gametic_v.to_le_bytes());
    v.extend_from_slice(&gamestate_v.to_le_bytes());
    v.extend_from_slice(&rndindex_v.to_le_bytes());
    v.extend_from_slice(&prndindex_v.to_le_bytes());
    v.extend_from_slice(&health.to_le_bytes());
    v.extend_from_slice(&armorpoints.to_le_bytes());
    v.extend_from_slice(&killcount.to_le_bytes());
    v.extend_from_slice(&itemcount.to_le_bytes());
    v.extend_from_slice(&secretcount.to_le_bytes());
    v.extend_from_slice(&readyweapon.to_le_bytes());
    for a in &ammo
    {
        v.extend_from_slice(&a.to_le_bytes());
    }

    if mo.is_null()
    {
        // Null-mo fallback: the four mo fields hash as zero.
        v.extend_from_slice(&0_i32.to_le_bytes());
        v.extend_from_slice(&0_i32.to_le_bytes());
        v.extend_from_slice(&0_i32.to_le_bytes());
        v.extend_from_slice(&0_u32.to_le_bytes());
    }
    else
    {
        // SAFETY: mo is a live engine mobj read while the engine is quiescent
        // between tics. `d_player::mobj_t` is the opaque FFI handle; cast to
        // the concrete `#[repr(C)]` `Copy` struct (the am_map.rs pattern), so
        // the by-value load cannot panic.
        let m: mobj_t = unsafe { *(mo as *mut mobj_t) };
        v.extend_from_slice(&m.x.to_le_bytes());
        v.extend_from_slice(&m.y.to_le_bytes());
        v.extend_from_slice(&m.z.to_le_bytes());
        v.extend_from_slice(&m.angle.to_le_bytes());
    }

    v
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    #[test]
    fn fnv1a64_known_vectors()
    {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn state_hash_bytes_layout_golden()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Pin the full 72-byte ledger encoding: every slot gets a DISTINCT
        // sentinel so any field permutation (not just add/remove/encode) or
        // width change trips this test. The word order below is derived from
        // the field order documented on state_hash; together with the FNV-1a
        // vectors in fnv1a64_known_vectors it is the cross-target layout
        // contract for Task 2 (wasm shell exports) and Task 6 (host twin).
        let old_gametic = unsafe { gametic };
        let old_gamestate = unsafe { gamestate };
        let old_rndindex = unsafe { rndindex };
        let old_prndindex = unsafe { prndindex };
        let old_health = unsafe { players[0].health };
        let old_armorpoints = unsafe { players[0].armorpoints };
        let old_killcount = unsafe { players[0].killcount };
        let old_itemcount = unsafe { players[0].itemcount };
        let old_secretcount = unsafe { players[0].secretcount };
        let old_readyweapon = unsafe { players[0].readyweapon };
        let old_ammo = unsafe { players[0].ammo };
        let old_mo = unsafe { players[0].mo };

        let mut scratch: mobj_t = unsafe { std::mem::zeroed() };
        let scratch_ptr: *mut mobj_t = std::ptr::addr_of_mut!(scratch);
        unsafe
        {
            gametic = 1;
            gamestate = 2;
            rndindex = 3;
            prndindex = 4;
            players[0].health = 5;
            players[0].armorpoints = 6;
            players[0].killcount = 7;
            players[0].itemcount = 8;
            players[0].secretcount = 9;
            players[0].readyweapon = 10;
            players[0].ammo = [11, 12, 13, 14];
            players[0].mo = scratch_ptr as *mut crate::doom::d_player::mobj_t;
            (*scratch_ptr).x = 15;
            (*scratch_ptr).y = 16;
            (*scratch_ptr).z = 17;
            (*scratch_ptr).angle = 18;
        }

        let b = state_hash_bytes();
        assert_eq!(b.len(), 72, "ledger encoding is 18 fixed 32-bit words");
        let word =
            |i: usize| -> u32 { u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) };
        assert_eq!(word(0), 1, "gametic");
        assert_eq!(word(1), 2, "gamestate");
        assert_eq!(word(2), 3, "rndindex");
        assert_eq!(word(3), 4, "prndindex");
        assert_eq!(word(4), 5, "health");
        assert_eq!(word(5), 6, "armorpoints");
        assert_eq!(word(6), 7, "killcount");
        assert_eq!(word(7), 8, "itemcount");
        assert_eq!(word(8), 9, "secretcount");
        assert_eq!(word(9), 10, "readyweapon");
        assert_eq!(word(10), 11, "ammo[0]");
        assert_eq!(word(11), 12, "ammo[1]");
        assert_eq!(word(12), 13, "ammo[2]");
        assert_eq!(word(13), 14, "ammo[3]");
        assert_eq!(word(14), 15, "mo_x");
        assert_eq!(word(15), 16, "mo_y");
        assert_eq!(word(16), 17, "mo_z");
        assert_eq!(word(17), 18, "mo_angle");
        assert_eq!(fnv1a64(&b), 0xa32c862c1ce55eb6, "sentinel-layout digest");

        unsafe
        {
            gametic = old_gametic;
            gamestate = old_gamestate;
            rndindex = old_rndindex;
            prndindex = old_prndindex;
            players[0].health = old_health;
            players[0].armorpoints = old_armorpoints;
            players[0].killcount = old_killcount;
            players[0].itemcount = old_itemcount;
            players[0].secretcount = old_secretcount;
            players[0].readyweapon = old_readyweapon;
            players[0].ammo = old_ammo;
            players[0].mo = old_mo;
        }
    }

    #[test]
    fn state_hash_load_zeroes_exactly_the_unsaved_words()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Pin the load-digest contract: words 0 (gametic), 2 (rndindex) and
        // 3 (prndindex) are excluded (a savegame restores neither the loop
        // tic counter nor the RNG indices, see state_hash_load's docs), and
        // EVERY other word still participates.
        let old_gametic = unsafe { gametic };
        let old_rndindex = unsafe { rndindex };
        let old_prndindex = unsafe { prndindex };
        unsafe
        {
            gametic = 0x1111_1111;
            rndindex = 0x2222_2222;
            prndindex = 0x3333_3333;
        }

        // Changing only the three unsaved words leaves state_hash_load
        // untouched while moving state_hash.
        let load_base = state_hash_load();
        let full_base = state_hash();
        unsafe
        {
            gametic = gametic.wrapping_add(1);
            rndindex = rndindex.wrapping_add(1);
            prndindex = prndindex.wrapping_add(1);
        }
        assert_ne!(state_hash(), full_base, "sanity: state_hash read the new statics");
        assert_eq!(state_hash_load(), load_base, "the three unsaved words must not participate");
        assert_ne!(load_base, state_hash(), "with non-zero statics the two digests must differ");

        // With all three statics zeroed, state_hash_load == state_hash
        // exactly: same encoding, only those words differ.
        unsafe
        {
            gametic = 0;
            rndindex = 0;
            prndindex = 0;
        }
        assert_eq!(state_hash_load(), state_hash(), "zeroed unsaved words collapse the two digests");

        unsafe
        {
            gametic = old_gametic;
            rndindex = old_rndindex;
            prndindex = old_prndindex;
        }
    }

    #[test]
    fn state_hash_changes_when_each_pinned_field_changes()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Direct writes to the statics (test-only, serialized against the
        // other engine-static readers via ENGINE_STATICS_TEST_LOCK). Pin that
        // EVERY ledger field participates - a future field-order edit must
        // trip this.
        let base = state_hash();

        let old = unsafe { gametic };
        unsafe { gametic = old + 1 };
        assert_ne!(state_hash(), base, "gametic must participate");
        unsafe { gametic = old };

        let old = unsafe { gamestate };
        unsafe { gamestate = old + 1 };
        assert_ne!(state_hash(), base, "gamestate must participate");
        unsafe { gamestate = old };

        let old = unsafe { rndindex };
        unsafe { rndindex = old + 1 };
        assert_ne!(state_hash(), base, "rndindex must participate");
        unsafe { rndindex = old };

        let old = unsafe { prndindex };
        unsafe { prndindex = old + 1 };
        assert_ne!(state_hash(), base, "prndindex must participate");
        unsafe { prndindex = old };

        let old = unsafe { players[0].health };
        unsafe { players[0].health = old + 1 };
        assert_ne!(state_hash(), base, "health must participate");
        unsafe { players[0].health = old };

        let old = unsafe { players[0].armorpoints };
        unsafe { players[0].armorpoints = old + 1 };
        assert_ne!(state_hash(), base, "armorpoints must participate");
        unsafe { players[0].armorpoints = old };

        let old = unsafe { players[0].killcount };
        unsafe { players[0].killcount = old + 1 };
        assert_ne!(state_hash(), base, "killcount must participate");
        unsafe { players[0].killcount = old };

        let old = unsafe { players[0].itemcount };
        unsafe { players[0].itemcount = old + 1 };
        assert_ne!(state_hash(), base, "itemcount must participate");
        unsafe { players[0].itemcount = old };

        let old = unsafe { players[0].secretcount };
        unsafe { players[0].secretcount = old + 1 };
        assert_ne!(state_hash(), base, "secretcount must participate");
        unsafe { players[0].secretcount = old };

        let old = unsafe { players[0].readyweapon };
        unsafe { players[0].readyweapon = old + 1 };
        assert_ne!(state_hash(), base, "readyweapon must participate");
        unsafe { players[0].readyweapon = old };

        let old_ammo = unsafe { players[0].ammo };
        for slot in 0..old_ammo.len()
        {
            let mut changed = old_ammo;
            changed[slot] += 1;
            unsafe { players[0].ammo = changed };
            assert_ne!(state_hash(), base, "ammo[{slot}] must participate");
            unsafe { players[0].ammo = old_ammo };
        }

        // mo fields: the pristine test binary has players[0].mo == null, so
        // aim it at a scratch mobj for the participation checks (the pointer
        // is restored at the end). The scratch is never touched by the
        // engine, only read through players[0].mo by state_hash itself.
        let mut scratch: mobj_t = unsafe { std::mem::zeroed() };
        let scratch_ptr: *mut mobj_t = std::ptr::addr_of_mut!(scratch);
        let old_mo = unsafe { players[0].mo };
        assert!(old_mo.is_null(), "test assumes a pristine players[0].mo");
        unsafe { players[0].mo = scratch_ptr as *mut crate::doom::d_player::mobj_t };

        // A zeroed mobj and a null mo must produce the same bytes - this pins
        // the documented null-mo fallback (mo fields hash as zero).
        let base_with_mo = state_hash();
        assert_eq!(
            base_with_mo, base,
            "null-mo fallback must hash the four mo fields as zero"
        );

        unsafe { (*scratch_ptr).x = 1 };
        assert_ne!(state_hash(), base_with_mo, "mo_x must participate");
        unsafe { (*scratch_ptr).x = 0 };

        unsafe { (*scratch_ptr).y = 1 };
        assert_ne!(state_hash(), base_with_mo, "mo_y must participate");
        unsafe { (*scratch_ptr).y = 0 };

        unsafe { (*scratch_ptr).z = 1 };
        assert_ne!(state_hash(), base_with_mo, "mo_z must participate");
        unsafe { (*scratch_ptr).z = 0 };

        unsafe { (*scratch_ptr).angle = 1 };
        assert_ne!(state_hash(), base_with_mo, "mo_angle must participate");
        unsafe { (*scratch_ptr).angle = 0 };

        unsafe { players[0].mo = old_mo };

        assert_eq!(state_hash(), base, "restored state must round-trip");
    }
}
