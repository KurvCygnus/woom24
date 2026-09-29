//! Test-only fixture helpers for the WAD globals (`cfg(test)` only):
//! the mutex serialising tests that swap `lumpinfo` / `numlumps` /
//! `lumphash`, the RAII install/restore scope, and the synthetic
//! `lumpinfo_t` / `wad_file_t` builders. Shared by the w_wad tests and
//! the w_checksum digest baseline (AGENTS rule: when a shared test
//! helper exists for a fixture type, using it is mandatory).

use std::ffi::{c_char, c_uint};
use std::ptr;
use std::sync::Mutex;

use crate::doom::w_file::wad_file_t;
use crate::doom::w_wad::{lumpinfo, lumphash, lumpinfo_t, numlumps};

/// Mutex serialising tests that swap out the global
/// `lumpinfo` / `numlumps` / `lumphash` state. Cargo runs tests
/// in parallel by default; without this guard two tests could
/// see each other's fake globals.
pub static WAD_LOCK: Mutex<()> = Mutex::new(());

/// RAII scope that installs fake values for the WAD globals on
/// construction and restores the originals on drop. Declared
/// **before** the local backing storage in each test so it
/// drops first (LIFO), restoring the globals before the storage
/// itself goes out of scope.
pub struct WadTestScope
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
    pub unsafe fn install(info: *mut lumpinfo_t, count: c_uint) -> Self
    {
        let scope = WadTestScope
        {
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
pub fn make_lump_name(s: &[u8]) -> [c_char; 8]
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
pub fn mapped_wad() -> wad_file_t
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
pub fn make_lump(name: &[u8], wad: *mut wad_file_t) -> lumpinfo_t
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
