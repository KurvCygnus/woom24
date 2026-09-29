//! The extracted demo-synchronization surface of `r_data`: the texture
//! hash-chain lookup core that resolves wall-texture names at map load.
//! Mirrors the `p_setup` dtmc shape -- each item is the pure computation
//! its caller embeds. The bucket selection
//! (`W_LumpNameHash(name) % numtextures`) stays at the caller
//! (`lookup::check_texture_num_for_name`) because it reads module
//! statics; only the decision core is extracted.

use std::ffi::{c_char, c_int};

use crate::doom::crt::strncasecmp;

use super::types::texture_t;

/// Resolve a texture name against one hash-bucket chain.
///
/// A name starting with `'-'` is the "no texture" marker and returns `0`
/// immediately, without consulting the chain.  Otherwise the chain is
/// walked and the first node whose 8-byte name matches case-insensitively
/// (`strncasecmp`) wins by its recorded index; an exhausted chain returns
/// `-1`.  Duplicate names keep vanilla's first-insert-wins tie-break
/// because `generate_texture_hash_table` appends in index order, so the
/// lower index sits at the chain head.
///
/// The result is simulation content: the map loader stores it into side-def
/// texture slots, and every wall draw later resolves pixels through it.  The
/// wrappers that feed this core live in `lookup`.
pub(super) unsafe fn texture_index_for_name(name: *const c_char, head: *mut texture_t) -> c_int {
    if *name == b'-' as c_char { return 0; }

    let mut texture = head;

    while !texture.is_null() {
        if strncasecmp((*texture).name.as_ptr(), name, 8) == 0 {
            return (*texture).index;
        }
        texture = (*texture).next;
    }

    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    use super::super::column_cache::textures_hashtable;
    use super::super::globals::numtextures;
    use super::super::types::texpatch_t;
    use crate::doom::r_data::R_CheckTextureNumForName;
    use crate::doom::w_wad::W_LumpNameHash;

    // -----------------------------------------------------------------------
    // F10 wave D2-A dtmc baseline vectors, captured against the inline
    // `R_CheckTextureNumForName` body BEFORE the graduation split
    // (commit `1fcfd54`, F10 §2.3) and re-pointed here onto the extracted
    // function by the graduation commit; the transcription helper is gone.
    // -----------------------------------------------------------------------

    /// Build an 8-byte NUL-padded texture name from a byte string.
    fn tex_name(name: &[u8]) -> [c_char; 8] {
        let mut out = [0; 8];
        for (i, &b) in name.iter().take(8).enumerate() {
            out[i] = b as c_char;
        }
        out
    }

    /// Allocate a fake `texture_t` with the given name and index, chained
    /// through `next`.
    unsafe fn fake_texture(name: &[u8], index: c_int, next: *mut texture_t) -> *mut texture_t {
        let tex = Box::leak(Box::new(texture_t {
            name: tex_name(name),
            width: 64,
            height: 64,
            index,
            next,
            patchcount: 1,
            patches: texpatch_t { originx: 0, originy: 0, patch: 0 },
        }));
        tex
    }

    /// Known vectors over the three decision paths: the `'-'` no-texture
    /// marker returns 0 WITHOUT consulting the chain (head is null here --
    /// a hash-driven walk would have nowhere to go and anything but the
    /// early return could not produce 0); a miss walks to the end and
    /// returns -1; a hit returns the matched node's index; a duplicate
    /// name returns the LOWER index (vanilla first-insert-wins: the hash
    /// table builder appends in index order, so the lower index sits at
    /// the chain head).
    #[test]
    fn texture_index_for_name_baseline_vectors() {
        unsafe {
            // Duplicate pair: two textures named "DUP1", indices 4 and 7,
            // chained head -> 4 -> 7 (insertion order).
            let dup7 = fake_texture(b"DUP1\0\0\0\0", 7, ptr::null_mut());
            let dup4 = fake_texture(b"DUP1\0\0\0\0", 4, dup7);
            // A differently named node in front of the chain exercises the
            // non-matching walk step.
            let wall = fake_texture(b"WALL1\0\0\0", 11, dup4);
            let miss = fake_texture(b"MISS\0\0\0\0", 2, ptr::null_mut());

            assert_eq!(texture_index_for_name(c"-ANY".as_ptr() as *const c_char, ptr::null_mut()), 0);
            assert_eq!(texture_index_for_name(c"NOPE1".as_ptr() as *const c_char, miss), -1);
            assert_eq!(texture_index_for_name(c"WALL1".as_ptr() as *const c_char, wall), 11);
            // Case-insensitive compare (strncasecmp, 8 bytes).
            assert_eq!(texture_index_for_name(c"wall1".as_ptr() as *const c_char, wall), 11);
            // Duplicate name: the walk stops at the head, lower index wins.
            assert_eq!(texture_index_for_name(c"DUP1".as_ptr() as *const c_char, dup4), 4);
            assert_eq!(texture_index_for_name(c"dup1".as_ptr() as *const c_char, dup4), 4);
        }
    }

    /// Drive the LIVE wrapper through `R_CheckTextureNumForName`
    /// itself: install a two-bucket hash table, hang the fake chain at the
    /// bucket `W_LumpNameHash(name) % numtextures` actually selects, and
    /// assert the wrapper resolves the same vectors (hit / duplicate
    /// tie-break / `'-'` early return / miss). State is restored on exit.
    #[test]
    fn check_texture_num_for_name_live_wrapper_matches_vectors() {
        unsafe {
            let saved_numtextures = numtextures;
            let saved_hashtable = textures_hashtable;

            let wall = fake_texture(b"WALL1\0\0\0", 11, ptr::null_mut());
            let dup7 = fake_texture(b"DUP1\0\0\0\0", 7, wall);
            let dup4 = fake_texture(b"DUP1\0\0\0\0", 4, dup7);

            numtextures = 2;
            let table = vec![ptr::null_mut::<texture_t>(); 2].into_boxed_slice();
            let table_ptr = Box::leak(table).as_mut_ptr();
            let key = (W_LumpNameHash(c"DUP1".as_ptr() as *const c_char) % 2) as usize;
            *table_ptr.add(key) = dup4;
            textures_hashtable = table_ptr;

            assert_eq!(R_CheckTextureNumForName(c"DUP1".as_ptr() as *mut c_char), 4);
            assert_eq!(R_CheckTextureNumForName(c"dup1".as_ptr() as *mut c_char), 4);
            assert_eq!(R_CheckTextureNumForName(c"-ANY".as_ptr() as *mut c_char), 0);
            assert_eq!(R_CheckTextureNumForName(c"NOPE".as_ptr() as *mut c_char), -1);

            numtextures = saved_numtextures;
            textures_hashtable = saved_hashtable;
        }
    }
}
