//! File ingestion for the WAD lump directory: opening a WAD or
//! single-lump file, validating its header, and appending its lumps
//! to the global `lumpinfo` directory owned by the module root.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::ptr;

use super::{calloc, filelump_t, free, lumpinfo, lumpinfo_t, lumphash, numlumps, strlen, strncmp, strncpy, wadinfo_t};
use crate::doom::crt::{c_printf1, strcasecmp};
use crate::doom::m_misc::M_ExtractFileBase;
use crate::doom::w_file::{wad_file_t, W_OpenFile, W_Read};
use crate::doom::z_zone::{Z_ChangeUser, Z_Free, Z_Malloc, PU_STATIC};
use crate::i_error;

/// Grow the global `lumpinfo` array to `newnumlumps` entries, copy
/// the existing entries across, fix up any zone-allocator user
/// pointers (`Z_ChangeUser`), and re-link any in-flight `next`
/// chains so they point into the new array.
///
/// On allocation failure, calls `I_Error`. After return,
/// `lumpinfo` points at the new array, `numlumps == newnumlumps`,
/// and the old array has been `free`'d.
///
/// # Safety
///
/// Mutates the `lumpinfo` and `numlumps` globals. Assumes
/// `newnumlumps >= numlumps`. Mirrors the file-static helper of
/// the same name in `w_wad.c`.
unsafe fn ExtendLumpInfo(newnumlumps: c_uint)
{
    let newlumpinfo =
        calloc(newnumlumps as usize, std::mem::size_of::<lumpinfo_t>()) as *mut lumpinfo_t;
    if newlumpinfo.is_null()
    {
        i_error!("Couldn't realloc lumpinfo");
    }

    for i in 0..numlumps.min(newnumlumps)
    {
        std::ptr::copy_nonoverlapping(lumpinfo.add(i as usize), newlumpinfo.add(i as usize), 1);

        if !(*newlumpinfo.add(i as usize)).cache.is_null()
        {
            Z_ChangeUser(
                (*newlumpinfo.add(i as usize)).cache,
                &mut (*newlumpinfo.add(i as usize)).cache as *mut *mut c_void,
            );
        }

        if !(*lumpinfo.add(i as usize)).next.is_null()
        {
            let nextlumpnum = ((*lumpinfo.add(i as usize)).next as usize - lumpinfo as usize)
                / std::mem::size_of::<lumpinfo_t>();
            (*newlumpinfo.add(i as usize)).next = newlumpinfo.add(nextlumpnum);
        }
    }

    free(lumpinfo as *mut c_void);
    lumpinfo = newlumpinfo;
    numlumps = newnumlumps;
}

/// Open `filename` and append its lumps to the global directory.
///
/// Files whose extension is not `wad` (case-insensitive) are loaded
/// as single-lump files: a synthetic `filelump_t` is built whose
/// name is the file's basename (via `M_ExtractFileBase`). True WAD
/// files read the 12-byte `wadinfo_t` header, validate the `IWAD`
/// or `PWAD` magic, and then load the full directory at
/// `infotableofs`. Little-endian fields are byte-swapped via
/// `i32::from_le`.
///
/// On success returns the borrowed `wad_file_t*`, owned by
/// `w_file`. On open failure prints `couldn't open <path>` and
/// returns null. Any existing `lumphash` is freed so the next
/// lookup falls back to the linear scan until
/// `W_GenerateHashTable` is called again.
#[doc(alias = "W_AddFile")]
pub extern "C" fn add_file(filename: *mut c_char) -> *mut wad_file_t
{
    unsafe
    {
        let wad_file = W_OpenFile(filename);
        if wad_file.is_null()
        {
            c_printf1(c" couldn't open %s\n".as_ptr(), filename);
            return ptr::null_mut();
        }

        let mut newnumlumps = numlumps;
        let startlump = numlumps;

        let fileinfo: *mut filelump_t;

        let fname_len = strlen(filename);
        if fname_len < 3 || strcasecmp(filename.add(fname_len - 3), c"wad".as_ptr()) != 0
        {
            // Single lump file
            fileinfo = Z_Malloc(
                std::mem::size_of::<filelump_t>() as c_int,
                PU_STATIC,
                ptr::null_mut(),
            ) as *mut filelump_t;
            (*fileinfo).filepos = 0;
            (*fileinfo).size = (*wad_file).length as c_int;
            M_ExtractFileBase(filename, (*fileinfo).name.as_mut_ptr());
            newnumlumps += 1;
        }
        else
        {
            // WAD file
            let mut header: wadinfo_t = std::mem::zeroed();
            W_Read(
                wad_file,
                0,
                &mut header as *mut _ as *mut c_void,
                std::mem::size_of::<wadinfo_t>(),
            );

            if strncmp(header.identification.as_ptr(), c"IWAD".as_ptr(), 4) != 0
                && strncmp(header.identification.as_ptr(), c"PWAD".as_ptr(), 4) != 0
            {
                i_error!(
                    "Wad file {} doesn't have IWAD or PWAD id",
                    CStr::from_ptr(filename).to_string_lossy()
                );
            }

            let header_numlumps = i32::from_le(header.numlumps);
            let header_infotableofs = i32::from_le(header.infotableofs);

            let length = (header_numlumps as usize) * std::mem::size_of::<filelump_t>();
            fileinfo = Z_Malloc(length as c_int, PU_STATIC, ptr::null_mut()) as *mut filelump_t;

            W_Read(
                wad_file,
                header_infotableofs as c_uint,
                fileinfo as *mut c_void,
                length,
            );
            newnumlumps += header_numlumps as c_uint;
        }

        ExtendLumpInfo(newnumlumps);

        let mut lump_p = lumpinfo.add(startlump as usize);
        let mut filerover = fileinfo;

        for _i in startlump..numlumps
        {
            (*lump_p).wad_file = wad_file;
            (*lump_p).position = i32::from_le((*filerover).filepos);
            (*lump_p).size = i32::from_le((*filerover).size);
            (*lump_p).cache = ptr::null_mut();
            strncpy((*lump_p).name.as_mut_ptr(), (*filerover).name.as_ptr(), 8);

            lump_p = lump_p.add(1);
            filerover = filerover.add(1);
        }

        Z_Free(fileinfo as *mut c_void);

        if !lumphash.is_null()
        {
            Z_Free(lumphash as *mut c_void);
            lumphash = ptr::null_mut();
        }

        wad_file
    }
}
