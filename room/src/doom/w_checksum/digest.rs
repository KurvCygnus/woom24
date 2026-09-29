//! The WAD-directory checksum fold: the `LumpInfo` layout-parallel
//! mirror, the first-encounter per-file index assignment, the per-lump
//! SHA-1 fold, and the `wad_directory_checksum` entry point, with the
//! captured baseline vectors.

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::mem::size_of;

use crate::doom::m_misc::M_StringCopy;
use crate::doom::sha1::{
    sha1_digest_t, SHA1Context, SHA1_Final, SHA1_Init, SHA1_UpdateInt32, SHA1_UpdateString,
};
use crate::doom::w_wad::{lumpinfo, lumpinfo_t, numlumps};

/// Layout-parity pin: the checksum reads the global `lumpinfo` array
/// through this local mirror, so it must stay the same size as `w_wad`'s
/// canonical `lumpinfo_t`; the baseline test additionally drives real
/// `lumpinfo_t` entries through the fold.
const _: () = assert!(size_of::<LumpInfo>() == size_of::<lumpinfo_t>());

/// Mirror of the C `lumpinfo_t` layout used by
/// [`wad_directory_checksum`] to read individual lump metadata out of
/// the `lumpinfo` array.
///
/// Layout invariant: `#[repr(C)]` with the same field order, types and
/// padding as the canonical `lumpinfo_t` defined in `w_wad.h`. Only the
/// fields touched by checksumming are exposed; `cache` and `next` are
/// present to keep the struct size and field offsets matching the C
/// definition.
#[repr(C)]
pub struct LumpInfo {
    /// Eight-character lump name (not null-terminated in the WAD on-disk
    /// format; copied through a 9-byte buffer before hashing).
    pub name: [c_char; 8],
    /// Pointer to the owning `wad_file_t`. Used as the key for assigning a
    /// stable numeric file index via `get_file_number`.
    pub wad_file: *mut c_void,
    /// Byte offset of the lump payload inside its WAD.
    pub position: c_int,
    /// Lump size in bytes.
    pub size: c_int,
    /// Cached zone-allocated copy of the lump, if any (unused by
    /// checksumming).
    pub cache: *mut c_void,
    /// Linked list pointer used by the WAD subsystem; unused here but
    /// retained for layout parity with the C struct.
    pub next: *mut LumpInfo,
}

/// Map a `wad_file` handle to a stable small integer for inclusion in the
/// checksum.
///
/// Looks up `handle` in `open_wadfiles`; if present, returns its index. If
/// absent, appends it and returns the new index. This makes the digest
/// depend only on the relative order in which distinct WAD files are first
/// encountered when iterating the lump directory, not on pointer values,
/// which keeps the hash stable across runs.
///
/// # Safety
///
/// `open_wadfiles` must be a valid mutable reference. `handle` is only
/// compared for equality and stored back into the vector, never
/// dereferenced.
unsafe fn get_file_number(handle: *mut c_void, open_wadfiles: &mut Vec<*mut c_void>) -> c_int
{
    for (i, &wad) in open_wadfiles.iter().enumerate()
    {
        if wad == handle { return i as c_int; }
    }

    let result = open_wadfiles.len() as c_int;
    open_wadfiles.push(handle);
    result
}

/// Fold one lump's identifying fields into the running SHA-1 state.
///
/// Hashes (in order) the 9-byte null-padded name, the owning WAD's small
/// integer index (via [`get_file_number`]), the lump position and the
/// lump size. The exact ordering and width of each update must match the
/// C version byte-for-byte, otherwise the digest will diverge.
///
/// # Safety
///
/// `sha1_context` must point to an initialised [`SHA1Context`]. `lump`
/// must point to a valid [`LumpInfo`]. `open_wadfiles` must be a valid
/// mutable reference.
unsafe fn checksum_add_lump(
    sha1_context: *mut SHA1Context,
    lump: *mut LumpInfo,
    open_wadfiles: &mut Vec<*mut c_void>,
)
{
    let lump = &*lump;

    let mut buf: [c_char; 9] = [0; 9];
    M_StringCopy(buf.as_mut_ptr(), lump.name.as_ptr(), buf.len());

    SHA1_UpdateString(sha1_context, buf.as_mut_ptr());
    SHA1_UpdateInt32(sha1_context, get_file_number(lump.wad_file, open_wadfiles) as c_uint);
    SHA1_UpdateInt32(sha1_context, lump.position as c_uint);
    SHA1_UpdateInt32(sha1_context, lump.size as c_uint);
}

/// Compute a SHA-1 digest over the entire WAD directory and write it to
/// `digest`.
///
/// Iterates `lumpinfo[0..numlumps]`, hashing each entry through
/// `checksum_add_lump`. The result is used by netgame code to detect
/// mismatched WAD loadouts between peers.
///
/// Differs from the C version in one detail: the `open_wadfiles` registry
/// is allocated as a local `Vec` rather than a process-wide
/// `realloc`-grown array. Behaviour is otherwise identical.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_net/mod.rs` imports the upstream name through its verbatim
/// extern-by-symbol block.
///
/// # Safety
///
/// `digest` must point to a valid `sha1_digest_t` array with room for
/// `SHA1_DIGEST_SIZE` bytes. The global `lumpinfo` and `numlumps` must be
/// initialised (i.e. `W_InitMultipleFiles` has run).
#[doc(alias = "W_Checksum")]
#[export_name = "W_Checksum"]
pub unsafe extern "C" fn wad_directory_checksum(digest: *mut sha1_digest_t)
{
    let mut sha1_context = std::mem::MaybeUninit::<SHA1Context>::uninit();
    SHA1_Init(sha1_context.as_mut_ptr());

    let mut open_wadfiles: Vec<*mut c_void> = Vec::new();

    for i in 0..numlumps
    {
        let lump = lumpinfo.add(i as usize) as *mut LumpInfo;
        checksum_add_lump(sha1_context.as_mut_ptr(), lump, &mut open_wadfiles);
    }

    SHA1_Final((*digest).as_mut_ptr(), sha1_context.as_mut_ptr());
}

#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, c_uint};
    use std::ptr;

    use crate::doom::sha1::sha1_digest_t;
    use crate::doom::w_checksum::W_Checksum;
    use crate::doom::w_file::wad_file_t;
    use crate::doom::w_wad::lumpinfo_t;
    use crate::doom::w_wad::test_support::{make_lump_name, WAD_LOCK, WadTestScope};

    // -----------------------------------------------------------------------
    // F10 wave F2-d baseline: known-vector digests for the WAD-directory
    // checksum, captured against the inline bodies BEFORE the graduation
    // split (F10 §2.3, feb6318/dbf097e precedent). The graduation commit
    // moves these vectors unchanged to `w_checksum/digest.rs`; a digest
    // drift there is a demo-surface-adjacent regression (netgame
    // consistency check, `d_net` extern `W_Checksum`).
    // -----------------------------------------------------------------------

    /// Captured pre-move: three lumps across two WAD files
    /// (`E1M1`/`DSPISTOL` in wad one, `MAP01` in wad two).
    const CAPTURED_THREE_LUMP_DIGEST: &str = "48bea02d0bdda03dda3af78bb50275ac7c59b55b";

    /// Captured pre-move: the same three lumps, reordered
    /// (`MAP01` first, then `E1M1`, then `DSPISTOL`).
    const CAPTURED_REORDERED_DIGEST: &str = "c6726c7aa4a45daee118d57295b25f92aca73995";

    /// Distinct non-null `wad_file` stand-ins: `get_file_number`
    /// compares the pointers for equality only, never dereferences
    /// them, so the addresses of two `u8` statics exercise the
    /// per-file index assignment (wad one gets index 0 on its first
    /// lump, wad two index 1).
    static WAD_ONE: u8 = 0;
    static WAD_TWO: u8 = 0;

    fn wad_one() -> wad_file_t
    {
        wad_file_t
        {
            file_class: ptr::null_mut(),
            mapped: ptr::addr_of!(WAD_ONE).cast_mut(),
            length: 0,
        }
    }

    fn wad_two() -> wad_file_t
    {
        wad_file_t
        {
            file_class: ptr::null_mut(),
            mapped: ptr::addr_of!(WAD_TWO).cast_mut(),
            length: 0,
        }
    }

    fn lump(name: &[u8], wad: *mut wad_file_t, position: c_int, size: c_int) -> lumpinfo_t
    {
        lumpinfo_t
        {
            name: make_lump_name(name),
            wad_file: wad,
            position,
            size,
            cache: ptr::null_mut(),
            next: ptr::null_mut(),
        }
    }

    /// Run the checksum entry over a synthetic directory installed into
    /// the global `lumpinfo` / `numlumps` statics. The swap happens under
    /// the shared `WAD_LOCK`; the scope guard restores the originals
    /// before the backing slice drops (LIFO declaration order).
    fn checksum_of(lumps: &mut [lumpinfo_t]) -> sha1_digest_t
    {
        let _lock = WAD_LOCK.lock().unwrap();
        let _scope = unsafe { WadTestScope::install(lumps.as_mut_ptr(), lumps.len() as c_uint) };
        let mut digest: sha1_digest_t = [0; 20];
        unsafe { W_Checksum(&mut digest) };
        digest
    }

    fn hex(digest: &[u8; 20]) -> String
    {
        digest.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Empty directory: the checksum over zero lumps is exactly
    /// `SHA1_Init` + `SHA1_Final` -- the published SHA-1 empty-input
    /// vector, an oracle independent of the pre-move capture.
    #[test]
    fn empty_directory_matches_empty_sha1()
    {
        assert_eq!(hex(&checksum_of(&mut [])), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    /// Three lumps across two WAD files: the captured known vector,
    /// asserted twice so order stability by construction
    /// (first-encounter file numbering) is pinned alongside the
    /// digest bytes.
    #[test]
    fn synthetic_three_lump_digest_matches_capture()
    {
        let mut a = wad_one();
        let mut b = wad_two();
        let pa: *mut wad_file_t = &mut a;
        let pb: *mut wad_file_t = &mut b;
        let mut lumps =
        [
            lump(b"E1M1", pa, 0x0C, 4096),
            lump(b"DSPISTOL", pa, 0x100C, 60428),
            lump(b"MAP01", pb, 0x8, 12345),
        ];
        assert_eq!(hex(&checksum_of(&mut lumps)), CAPTURED_THREE_LUMP_DIGEST);
        assert_eq!(hex(&checksum_of(&mut lumps)), CAPTURED_THREE_LUMP_DIGEST);
    }

    /// The same directory reordered: the fold is order-sensitive, so
    /// the digest must differ from the captured in-order vector, and
    /// the reordered value is captured in its own right.
    #[test]
    fn reordered_directory_changes_the_digest()
    {
        let mut a = wad_one();
        let mut b = wad_two();
        let pa: *mut wad_file_t = &mut a;
        let pb: *mut wad_file_t = &mut b;
        let mut lumps =
        [
            lump(b"MAP01", pb, 0x8, 12345),
            lump(b"E1M1", pa, 0x0C, 4096),
            lump(b"DSPISTOL", pa, 0x100C, 60428),
        ];
        let digest = hex(&checksum_of(&mut lumps));
        assert_eq!(digest, CAPTURED_REORDERED_DIGEST);
        assert_ne!(digest, CAPTURED_THREE_LUMP_DIGEST);
    }
}
