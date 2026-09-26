//! Lump access for the WAD directory: the count and size accessors,
//! disk reads bracketed by the platform disk-icon callbacks, and the
//! zone-managed lump cache acquire/release surface.

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::ptr;

use super::lookup::get_num_for_name;
use super::{lumpinfo, numlumps};
use crate::doom::i_video::{I_BeginRead, I_EndRead};
use crate::doom::w_file::W_Read;
use crate::doom::z_zone::{Z_ChangeTag2, Z_Malloc, PU_CACHE};
use crate::i_error;

/// Return the total number of registered lumps as a signed integer.
/// Mirrors `W_NumLumps` from `w_wad.c`.
#[doc(alias = "W_NumLumps")]
pub extern "C" fn num_lumps() -> c_int
{
    unsafe { numlumps as c_int }
}

/// Return the byte size of `lump`. Errors out via `I_Error` if
/// `lump` is out of range. Mirrors `W_LumpLength` from `w_wad.c`.
#[doc(alias = "W_LumpLength")]
pub extern "C" fn lump_length(lump: c_uint) -> c_int
{
    unsafe
    {
        if lump >= numlumps
        {
            i_error!("W_LumpLength: {} >= numlumps", lump as c_int);
        }
        (*lumpinfo.add(lump as usize)).size
    }
}

/// Read `lump` into the caller-supplied buffer `dest`.
///
/// The buffer must be at least `W_LumpLength(lump)` bytes. Wraps
/// the read in `I_BeginRead` / `I_EndRead` so the platform can
/// display a disk icon. If the underlying `W_Read` returns fewer
/// bytes than requested, calls `I_Error`.
///
/// Differs from the C source by also emitting a stderr diagnostic
/// when the actual read exceeds `lump.size` (a "shouldn't happen"
/// defensive log, not present in `w_wad.c`).
#[doc(alias = "W_ReadLump")]
pub extern "C" fn read_lump(lump: c_uint, dest: *mut c_void)
{
    unsafe
    {
        if lump >= numlumps
        {
            i_error!("W_ReadLump: {} >= numlumps", lump as c_int);
        }
        let l = lumpinfo.add(lump as usize);
        I_BeginRead();
        let c = W_Read(
            (*l).wad_file,
            (*l).position as c_uint,
            dest,
            (*l).size as usize,
        );
        if c > (*l).size as usize
        {
            eprintln!(
                "[W_ReadLump] OVERFLOW: lump={}, requested={}, actually_read={}, lump.size={}",
                lump,
                (*l).size,
                c,
                (*l).size
            );
        }
        if c < (*l).size as usize
        {
            i_error!(
                "W_ReadLump: only read {} of {} on lump {}",
                c as c_int,
                (*l).size,
                lump as c_int
            );
        }
        I_EndRead();
    }
}

/// Return a borrowed pointer to the bytes of `lumpnum`, loading
/// them into the zone cache on first miss.
///
/// Three branches:
///  - Memory-mapped wad: returns a pointer inside the mapping (no
///    copy, no zone allocation).
///  - Already cached: returns the cached pointer and switches its
///    zone tag to `tag` via `Z_ChangeTag2`.
///  - Cold miss: `Z_Malloc(size, tag, &cache)` and `W_ReadLump`.
///
/// `tag` is typically `PU_STATIC` (long-lived) or `PU_CACHE`
/// (purgeable). Out-of-range `lumpnum` triggers `I_Error`.
#[doc(alias = "W_CacheLumpNum")]
pub extern "C" fn cache_lump_num(lumpnum: c_int, tag: c_int) -> *mut c_void
{
    unsafe
    {
        if lumpnum < 0 || (lumpnum as c_uint) >= numlumps
        {
            i_error!("W_CacheLumpNum: {} >= numlumps", lumpnum);
        }
        let lump = lumpinfo.add(lumpnum as usize);

        if !(*(*lump).wad_file).mapped.is_null()
        {
            // Memory-mapped file
            (*(*lump).wad_file).mapped.add((*lump).position as usize) as *mut c_void
        }
        else if !(*lump).cache.is_null()
        {
            // Already cached
            let result = (*lump).cache;
            Z_ChangeTag2(result, tag, ptr::null(), 0);
            result
        }
        else
        {
            // Not yet loaded
            (*lump).cache = Z_Malloc(
                lump_length(lumpnum as c_uint),
                tag,
                &mut (*lump).cache as *mut *mut c_void as *mut c_void,
            );
            read_lump(lumpnum as c_uint, (*lump).cache);
            (*lump).cache
        }
    }
}

/// Convenience wrapper: resolve `name` to a lump number via
/// `W_GetNumForName` (fatal if missing) and call `W_CacheLumpNum`.
//* Freeze-zone legacy `extern "C"` blocks link this function by its
//* upstream C symbol (`hu_stuff.rs`, `r_draw.rs`), so the symbol is
//* pinned with `#[export_name]` instead of being dropped with the
//* rename.
#[doc(alias = "W_CacheLumpName")]
#[export_name = "W_CacheLumpName"]
pub extern "C" fn cache_lump_name(name: *const c_char, tag: c_int) -> *mut c_void
{
    cache_lump_num(get_num_for_name(name), tag)
}

/// Mark `lumpnum` as releasable by demoting its cached block to
/// `PU_CACHE`, so the zone allocator may purge it under memory
/// pressure. No-op for memory-mapped wads.
///
/// Mirrors `W_ReleaseLumpNum` from `w_wad.c`.
#[doc(alias = "W_ReleaseLumpNum")]
pub extern "C" fn release_lump_num(lumpnum: c_int)
{
    unsafe
    {
        if lumpnum < 0 || (lumpnum as c_uint) >= numlumps
        {
            i_error!("W_ReleaseLumpNum: {} >= numlumps", lumpnum);
        }
        let lump = lumpinfo.add(lumpnum as usize);
        if !(*(*lump).wad_file).mapped.is_null()
        {
            // Memory-mapped: nothing to do
        }
        else
        {
            Z_ChangeTag2((*lump).cache, PU_CACHE, ptr::null(), 0);
        }
    }
}

/// Convenience wrapper: resolve `name` and call `W_ReleaseLumpNum`.
/// Mirrors `W_ReleaseLumpName` from `w_wad.c`.
#[doc(alias = "W_ReleaseLumpName")]
pub extern "C" fn release_lump_name(name: *const c_char)
{
    release_lump_num(get_num_for_name(name))
}

#[cfg(test)]
mod tests
{
    use super::release_lump_name;
    use crate::doom::w_file::wad_file_t;
    use crate::doom::w_wad::{lumpinfo, lumphash, lumpinfo_t, numlumps};
    use std::ffi::{c_char, c_uint};
    use std::ptr;
    use std::sync::Mutex;

    /// Mutex serialising tests that swap out the global
    /// `lumpinfo` / `numlumps` / `lumphash` state. Cargo runs tests
    /// in parallel by default; without this guard two tests could
    /// see each other's fake globals.
    static WAD_LOCK: Mutex<()> = Mutex::new(());

    /// RAII scope that installs fake values for the WAD globals on
    /// construction and restores the originals on drop. Declared
    /// **before** the local backing storage in each test so it
    /// drops first (LIFO), restoring the globals before the storage
    /// itself goes out of scope.
    struct WadTestScope
    {
        /// Original `lumpinfo` pointer to restore on drop.
        saved_lumpinfo: *mut lumpinfo_t,
        /// Original `numlumps` count to restore on drop.
        saved_numlumps: c_uint,
        /// Original `lumphash` pointer to restore on drop.
        saved_lumphash: *mut *mut lumpinfo_t,
    }

    impl WadTestScope
    {
        /// Save the current globals and install `info` /`count` /
        /// `null` for the duration of the test. Returns the guard.
        ///
        /// # Safety
        ///
        /// `info` must remain live for the lifetime of the returned
        /// guard. The caller must serialise with `WAD_LOCK`.
        unsafe fn install(info: *mut lumpinfo_t, count: c_uint) -> Self
        {
            let scope = WadTestScope {
                saved_lumpinfo: lumpinfo,
                saved_numlumps: numlumps,
                saved_lumphash: lumphash,
            };
            lumpinfo = info;
            numlumps = count;
            lumphash = ptr::null_mut(); // force linear scan, not hash table
            scope
        }
    }

    /// RAII restoration of the saved WAD globals on test teardown.
    impl Drop for WadTestScope
    {
        /// Restore the saved globals.
        fn drop(&mut self)
        {
            unsafe
            {
                lumpinfo = self.saved_lumpinfo;
                numlumps = self.saved_numlumps;
                lumphash = self.saved_lumphash;
            }
        }
    }

    /// Build a `[c_char; 8]` lump name from a byte slice, NUL-padding
    /// or truncating to 8 bytes.
    fn make_lump_name(s: &[u8]) -> [c_char; 8]
    {
        let mut name = [0i8; 8];
        for (i, &b) in s.iter().take(8).enumerate()
        {
            name[i] = b as c_char;
        }
        name
    }

    /// Build a fake `wad_file_t` whose `mapped` pointer is non-null,
    /// so `W_ReleaseLumpNum` takes the memory-mapped no-op branch
    /// and avoids the zone allocator (which is not initialised in
    /// the test harness).
    fn mapped_wad() -> wad_file_t
    {
        static SENTINEL: u8 = 0;
        wad_file_t
        {
            file_class: ptr::null_mut(),
            mapped: std::ptr::addr_of!(SENTINEL).cast_mut(),
            length: 0,
        }
    }

    /// Build a minimal `lumpinfo_t` for tests: just the name, the
    /// owning fake wad, and zero/null everywhere else.
    fn make_lump(name: &[u8], wad: *mut wad_file_t) -> lumpinfo_t
    {
        lumpinfo_t
        {
            name: make_lump_name(name),
            wad_file: wad,
            position: 0,
            size: 0,
            cache: ptr::null_mut(),
            next: ptr::null_mut(),
        }
    }

    /// `W_ReleaseLumpName` must delegate correctly: the name
    /// resolves to lump 0 and `W_ReleaseLumpNum(0)` completes
    /// without error.
    #[test]
    fn release_lump_name_delegates_to_num()
    {
        let _lock = WAD_LOCK.lock().unwrap();
        let mut wad = mapped_wad();
        let mut lumps = [make_lump(b"TESTLUMP", &mut wad)];
        let _scope = unsafe { WadTestScope::install(lumps.as_mut_ptr(), 1) };

        release_lump_name(c"TESTLUMP".as_ptr());
    }

    /// The underlying `strncasecmp` lookup is case-insensitive, so
    /// "testlump" must resolve to the same entry as "TESTLUMP".
    #[test]
    fn release_lump_name_is_case_insensitive()
    {
        let _lock = WAD_LOCK.lock().unwrap();
        let mut wad = mapped_wad();
        let mut lumps = [make_lump(b"TESTLUMP", &mut wad)];
        let _scope = unsafe { WadTestScope::install(lumps.as_mut_ptr(), 1) };

        release_lump_name(c"testlump".as_ptr());
    }

    /// With multiple lumps loaded, each name must resolve to its
    /// own entry. The linear scan runs backwards so the
    /// last-registered match wins for duplicates - here every name
    /// is unique so order is immaterial.
    #[test]
    fn release_lump_name_picks_correct_lump_among_multiple()
    {
        let _lock = WAD_LOCK.lock().unwrap();
        let mut wad = mapped_wad();
        let mut lumps = [make_lump(b"ALPHA", &mut wad), make_lump(b"BETA", &mut wad)];
        let _scope = unsafe { WadTestScope::install(lumps.as_mut_ptr(), 2) };

        release_lump_name(c"ALPHA".as_ptr());
        release_lump_name(c"BETA".as_ptr());
    }
}
