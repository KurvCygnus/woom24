//! The music table: the `MusicInfo` record (a locked `repr(C)` mirror of
//! `musicinfo_t`), its compile-time builders, the short lump-name buffers,
//! and the master `S_music` table indexed by `Mus` ids.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};

use super::enums::NUMMUSIC;
use super::sfx_table::name;

/// Rust mirror of `musicinfo_t` (`sounds.h`).
///
/// Layout-compatible with the C struct so `S_music` can be exposed via
/// `#[no_mangle]`. `name` is the short lump name (the engine prepends `d_`),
/// `data` holds the cached lump, `handle` is the driver-side song handle.
#[repr(C)]
#[derive(Default)]
pub struct MusicInfo {
    /// Short lump name (no `d_` prefix), NUL-terminated. Null for the sentinel.
    pub name: *mut c_char,
    /// Cached lump number; 0 means "not yet looked up".
    pub lumpnum: c_int,
    /// Pointer to the cached MIDI/MUS lump in zone memory.
    pub data: *mut c_void,
    /// Driver-side song handle returned by `I_RegisterSong`.
    pub handle: *mut c_void,
}

/// Manual `Sync` for `MusicInfo`: same justification as `SfxInfo` -
/// `S_music` is treated as effectively immutable after `S_Init`.
unsafe impl Sync for MusicInfo {}

/// Constructors used to build the static `S_music` table at compile time.
impl MusicInfo {
    /// `MusicInfo` for the slot-zero sentinel ("no music"); all fields null.
    const fn none() -> Self {
        MusicInfo {
            name: std::ptr::null_mut(),
            lumpnum: 0,
            data: std::ptr::null_mut(),
            handle: std::ptr::null_mut(),
        }
    }

    /// `MusicInfo` pointing at the given const name buffer. The buffer must
    /// have static lifetime so the recorded pointer remains valid.
    const fn new<const N: usize>(name: &'static [c_char; N]) -> Self {
        MusicInfo {
            name: name.as_ptr() as *mut c_char,
            lumpnum: 0,
            data: std::ptr::null_mut(),
            handle: std::ptr::null_mut(),
        }
    }
}

/// Short lump-name buffer for music `D_E1M1` (`mus_e1m1`).
const MUS_e1m1: [c_char; 5] = name("e1m1");
/// Short lump-name buffer for music `D_E1M2` (`mus_e1m2`).
const MUS_e1m2: [c_char; 5] = name("e1m2");
/// Short lump-name buffer for music `D_E1M3` (`mus_e1m3`).
const MUS_e1m3: [c_char; 5] = name("e1m3");
/// Short lump-name buffer for music `D_E1M4` (`mus_e1m4`).
const MUS_e1m4: [c_char; 5] = name("e1m4");
/// Short lump-name buffer for music `D_E1M5` (`mus_e1m5`).
const MUS_e1m5: [c_char; 5] = name("e1m5");
/// Short lump-name buffer for music `D_E1M6` (`mus_e1m6`).
const MUS_e1m6: [c_char; 5] = name("e1m6");
/// Short lump-name buffer for music `D_E1M7` (`mus_e1m7`).
const MUS_e1m7: [c_char; 5] = name("e1m7");
/// Short lump-name buffer for music `D_E1M8` (`mus_e1m8`).
const MUS_e1m8: [c_char; 5] = name("e1m8");
/// Short lump-name buffer for music `D_E1M9` (`mus_e1m9`).
const MUS_e1m9: [c_char; 5] = name("e1m9");
/// Short lump-name buffer for music `D_E2M1` (`mus_e2m1`).
const MUS_e2m1: [c_char; 5] = name("e2m1");
/// Short lump-name buffer for music `D_E2M2` (`mus_e2m2`).
const MUS_e2m2: [c_char; 5] = name("e2m2");
/// Short lump-name buffer for music `D_E2M3` (`mus_e2m3`).
const MUS_e2m3: [c_char; 5] = name("e2m3");
/// Short lump-name buffer for music `D_E2M4` (`mus_e2m4`).
const MUS_e2m4: [c_char; 5] = name("e2m4");
/// Short lump-name buffer for music `D_E2M5` (`mus_e2m5`).
const MUS_e2m5: [c_char; 5] = name("e2m5");
/// Short lump-name buffer for music `D_E2M6` (`mus_e2m6`).
const MUS_e2m6: [c_char; 5] = name("e2m6");
/// Short lump-name buffer for music `D_E2M7` (`mus_e2m7`).
const MUS_e2m7: [c_char; 5] = name("e2m7");
/// Short lump-name buffer for music `D_E2M8` (`mus_e2m8`).
const MUS_e2m8: [c_char; 5] = name("e2m8");
/// Short lump-name buffer for music `D_E2M9` (`mus_e2m9`).
const MUS_e2m9: [c_char; 5] = name("e2m9");
/// Short lump-name buffer for music `D_E3M1` (`mus_e3m1`).
const MUS_e3m1: [c_char; 5] = name("e3m1");
/// Short lump-name buffer for music `D_E3M2` (`mus_e3m2`).
const MUS_e3m2: [c_char; 5] = name("e3m2");
/// Short lump-name buffer for music `D_E3M3` (`mus_e3m3`).
const MUS_e3m3: [c_char; 5] = name("e3m3");
/// Short lump-name buffer for music `D_E3M4` (`mus_e3m4`).
const MUS_e3m4: [c_char; 5] = name("e3m4");
/// Short lump-name buffer for music `D_E3M5` (`mus_e3m5`).
const MUS_e3m5: [c_char; 5] = name("e3m5");
/// Short lump-name buffer for music `D_E3M6` (`mus_e3m6`).
const MUS_e3m6: [c_char; 5] = name("e3m6");
/// Short lump-name buffer for music `D_E3M7` (`mus_e3m7`).
const MUS_e3m7: [c_char; 5] = name("e3m7");
/// Short lump-name buffer for music `D_E3M8` (`mus_e3m8`).
const MUS_e3m8: [c_char; 5] = name("e3m8");
/// Short lump-name buffer for music `D_E3M9` (`mus_e3m9`).
const MUS_e3m9: [c_char; 5] = name("e3m9");
/// Short lump-name buffer for music `D_INTER` (`mus_inter`).
const MUS_inter: [c_char; 6] = name("inter");
/// Short lump-name buffer for music `D_INTRO` (`mus_intro`).
const MUS_intro: [c_char; 6] = name("intro");
/// Short lump-name buffer for music `D_BUNNY` (`mus_bunny`).
const MUS_bunny: [c_char; 6] = name("bunny");
/// Short lump-name buffer for music `D_VICTOR` (`mus_victor`).
const MUS_victor: [c_char; 7] = name("victor");
/// Short lump-name buffer for music `D_INTROA` (`mus_introa`).
const MUS_introa: [c_char; 7] = name("introa");
/// Short lump-name buffer for music `D_RUNNIN` (`mus_runnin`).
const MUS_runnin: [c_char; 7] = name("runnin");
/// Short lump-name buffer for music `D_STALKS` (`mus_stalks`).
const MUS_stalks: [c_char; 7] = name("stalks");
/// Short lump-name buffer for music `D_COUNTD` (`mus_countd`).
const MUS_countd: [c_char; 7] = name("countd");
/// Short lump-name buffer for music `D_BETWEE` (`mus_betwee`).
const MUS_betwee: [c_char; 7] = name("betwee");
/// Short lump-name buffer for music `D_DOOM` (`mus_doom`).
const MUS_doom: [c_char; 5] = name("doom");
/// Short lump-name buffer for music `D_THE_DA` (`mus_the_da`).
const MUS_the_da: [c_char; 7] = name("the_da");
/// Short lump-name buffer for music `D_SHAWN` (`mus_shawn`).
const MUS_shawn: [c_char; 6] = name("shawn");
/// Short lump-name buffer for music `D_DDTBLU` (`mus_ddtblu`).
const MUS_ddtblu: [c_char; 7] = name("ddtblu");
/// Short lump-name buffer for music `D_IN_CIT` (`mus_in_cit`).
const MUS_in_cit: [c_char; 7] = name("in_cit");
/// Short lump-name buffer for music `D_DEAD` (`mus_dead`).
const MUS_dead: [c_char; 5] = name("dead");
/// Short lump-name buffer for music `D_STLKS2` (`mus_stlks2`).
const MUS_stlks2: [c_char; 7] = name("stlks2");
/// Short lump-name buffer for music `D_THEDA2` (`mus_theda2`).
const MUS_theda2: [c_char; 7] = name("theda2");
/// Short lump-name buffer for music `D_DOOM2` (`mus_doom2`).
const MUS_doom2: [c_char; 6] = name("doom2");
/// Short lump-name buffer for music `D_DDTBL2` (`mus_ddtbl2`).
const MUS_ddtbl2: [c_char; 7] = name("ddtbl2");
/// Short lump-name buffer for music `D_RUNNI2` (`mus_runni2`).
const MUS_runni2: [c_char; 7] = name("runni2");
/// Short lump-name buffer for music `D_DEAD2` (`mus_dead2`).
const MUS_dead2: [c_char; 6] = name("dead2");
/// Short lump-name buffer for music `D_STLKS3` (`mus_stlks3`).
const MUS_stlks3: [c_char; 7] = name("stlks3");
/// Short lump-name buffer for music `D_ROMERO` (`mus_romero`).
const MUS_romero: [c_char; 7] = name("romero");
/// Short lump-name buffer for music `D_SHAWN2` (`mus_shawn2`).
const MUS_shawn2: [c_char; 7] = name("shawn2");
/// Short lump-name buffer for music `D_MESSAG` (`mus_messag`).
const MUS_messag: [c_char; 7] = name("messag");
/// Short lump-name buffer for music `D_COUNT2` (`mus_count2`).
const MUS_count2: [c_char; 7] = name("count2");
/// Short lump-name buffer for music `D_DDTBL3` (`mus_ddtbl3`).
const MUS_ddtbl3: [c_char; 7] = name("ddtbl3");
/// Short lump-name buffer for music `D_AMPIE` (`mus_ampie`).
const MUS_ampie: [c_char; 6] = name("ampie");
/// Short lump-name buffer for music `D_THEDA3` (`mus_theda3`).
const MUS_theda3: [c_char; 7] = name("theda3");
/// Short lump-name buffer for music `D_ADRIAN` (`mus_adrian`).
const MUS_adrian: [c_char; 7] = name("adrian");
/// Short lump-name buffer for music `D_MESSG2` (`mus_messg2`).
const MUS_messg2: [c_char; 7] = name("messg2");
/// Short lump-name buffer for music `D_ROMER2` (`mus_romer2`).
const MUS_romer2: [c_char; 7] = name("romer2");
/// Short lump-name buffer for music `D_TENSE` (`mus_tense`).
const MUS_tense: [c_char; 6] = name("tense");
/// Short lump-name buffer for music `D_SHAWN3` (`mus_shawn3`).
const MUS_shawn3: [c_char; 7] = name("shawn3");
/// Short lump-name buffer for music `D_OPENIN` (`mus_openin`).
const MUS_openin: [c_char; 7] = name("openin");
/// Short lump-name buffer for music `D_EVIL` (`mus_evil`).
const MUS_evil: [c_char; 5] = name("evil");
/// Short lump-name buffer for music `D_ULTIMA` (`mus_ultima`).
const MUS_ultima: [c_char; 7] = name("ultima");
/// Short lump-name buffer for music `D_READ_M` (`mus_read_m`).
const MUS_read_m: [c_char; 7] = name("read_m");
/// Short lump-name buffer for music `D_DM2TTL` (`mus_dm2ttl`).
const MUS_dm2ttl: [c_char; 7] = name("dm2ttl");
/// Short lump-name buffer for music `D_DM2INT` (`mus_dm2int`).
const MUS_dm2int: [c_char; 7] = name("dm2int");

/// Master music table indexed by `Mus` ids. Mirrors `S_music[]` in `sounds.c`.
///
/// Each non-sentinel entry stores a pointer into one of the `MUS_*` short-name
/// buffers, which `S_ChangeMusic` formats into the WAD lump name (prefixed
/// with `d_`) on first use. Entry 0 is the "no music" sentinel.
/// `#[no_mangle]` so the symbol matches the original C linkage.
#[no_mangle]
pub static mut S_music: [MusicInfo; NUMMUSIC] = [
    // [0] mus_none (sentinel)
    MusicInfo::none(),
    // [1] mus_e1m1
    MusicInfo::new(&MUS_e1m1),
    // [2] mus_e1m2
    MusicInfo::new(&MUS_e1m2),
    // [3] mus_e1m3
    MusicInfo::new(&MUS_e1m3),
    // [4] mus_e1m4
    MusicInfo::new(&MUS_e1m4),
    // [5] mus_e1m5
    MusicInfo::new(&MUS_e1m5),
    // [6] mus_e1m6
    MusicInfo::new(&MUS_e1m6),
    // [7] mus_e1m7
    MusicInfo::new(&MUS_e1m7),
    // [8] mus_e1m8
    MusicInfo::new(&MUS_e1m8),
    // [9] mus_e1m9
    MusicInfo::new(&MUS_e1m9),
    // [10] mus_e2m1
    MusicInfo::new(&MUS_e2m1),
    // [11] mus_e2m2
    MusicInfo::new(&MUS_e2m2),
    // [12] mus_e2m3
    MusicInfo::new(&MUS_e2m3),
    // [13] mus_e2m4
    MusicInfo::new(&MUS_e2m4),
    // [14] mus_e2m5
    MusicInfo::new(&MUS_e2m5),
    // [15] mus_e2m6
    MusicInfo::new(&MUS_e2m6),
    // [16] mus_e2m7
    MusicInfo::new(&MUS_e2m7),
    // [17] mus_e2m8
    MusicInfo::new(&MUS_e2m8),
    // [18] mus_e2m9
    MusicInfo::new(&MUS_e2m9),
    // [19] mus_e3m1
    MusicInfo::new(&MUS_e3m1),
    // [20] mus_e3m2
    MusicInfo::new(&MUS_e3m2),
    // [21] mus_e3m3
    MusicInfo::new(&MUS_e3m3),
    // [22] mus_e3m4
    MusicInfo::new(&MUS_e3m4),
    // [23] mus_e3m5
    MusicInfo::new(&MUS_e3m5),
    // [24] mus_e3m6
    MusicInfo::new(&MUS_e3m6),
    // [25] mus_e3m7
    MusicInfo::new(&MUS_e3m7),
    // [26] mus_e3m8
    MusicInfo::new(&MUS_e3m8),
    // [27] mus_e3m9
    MusicInfo::new(&MUS_e3m9),
    // [28] mus_inter
    MusicInfo::new(&MUS_inter),
    // [29] mus_intro
    MusicInfo::new(&MUS_intro),
    // [30] mus_bunny
    MusicInfo::new(&MUS_bunny),
    // [31] mus_victor
    MusicInfo::new(&MUS_victor),
    // [32] mus_introa
    MusicInfo::new(&MUS_introa),
    // [33] mus_runnin
    MusicInfo::new(&MUS_runnin),
    // [34] mus_stalks
    MusicInfo::new(&MUS_stalks),
    // [35] mus_countd
    MusicInfo::new(&MUS_countd),
    // [36] mus_betwee
    MusicInfo::new(&MUS_betwee),
    // [37] mus_doom
    MusicInfo::new(&MUS_doom),
    // [38] mus_the_da
    MusicInfo::new(&MUS_the_da),
    // [39] mus_shawn
    MusicInfo::new(&MUS_shawn),
    // [40] mus_ddtblu
    MusicInfo::new(&MUS_ddtblu),
    // [41] mus_in_cit
    MusicInfo::new(&MUS_in_cit),
    // [42] mus_dead
    MusicInfo::new(&MUS_dead),
    // [43] mus_stlks2
    MusicInfo::new(&MUS_stlks2),
    // [44] mus_theda2
    MusicInfo::new(&MUS_theda2),
    // [45] mus_doom2
    MusicInfo::new(&MUS_doom2),
    // [46] mus_ddtbl2
    MusicInfo::new(&MUS_ddtbl2),
    // [47] mus_runni2
    MusicInfo::new(&MUS_runni2),
    // [48] mus_dead2
    MusicInfo::new(&MUS_dead2),
    // [49] mus_stlks3
    MusicInfo::new(&MUS_stlks3),
    // [50] mus_romero
    MusicInfo::new(&MUS_romero),
    // [51] mus_shawn2
    MusicInfo::new(&MUS_shawn2),
    // [52] mus_messag
    MusicInfo::new(&MUS_messag),
    // [53] mus_count2
    MusicInfo::new(&MUS_count2),
    // [54] mus_ddtbl3
    MusicInfo::new(&MUS_ddtbl3),
    // [55] mus_ampie
    MusicInfo::new(&MUS_ampie),
    // [56] mus_theda3
    MusicInfo::new(&MUS_theda3),
    // [57] mus_adrian
    MusicInfo::new(&MUS_adrian),
    // [58] mus_messg2
    MusicInfo::new(&MUS_messg2),
    // [59] mus_romer2
    MusicInfo::new(&MUS_romer2),
    // [60] mus_tense
    MusicInfo::new(&MUS_tense),
    // [61] mus_shawn3
    MusicInfo::new(&MUS_shawn3),
    // [62] mus_openin
    MusicInfo::new(&MUS_openin),
    // [63] mus_evil
    MusicInfo::new(&MUS_evil),
    // [64] mus_ultima
    MusicInfo::new(&MUS_ultima),
    // [65] mus_read_m
    MusicInfo::new(&MUS_read_m),
    // [66] mus_dm2ttl
    MusicInfo::new(&MUS_dm2ttl),
    // [67] mus_dm2int
    MusicInfo::new(&MUS_dm2int),
];

#[cfg(test)]
mod tests {
    use crate::doom::sounds::{MusicInfo, NUMMUSIC, S_music};
    use std::mem::size_of;

    /// `S_music` length must equal `NUMMUSIC`.
    #[test]
    fn music_table_has_correct_length() {
        unsafe {
            assert_eq!(S_music.len(), NUMMUSIC);
        }
    }

    /// `sizeof(musicinfo_t)` must be 32 on 64-bit (four pointer-sized fields).
    #[test]
    fn music_info_size_matches_c() {
        // sizeof(musicinfo_t) is 32 on 64-bit (four pointer-sized fields).
        assert_eq!(size_of::<MusicInfo>(), 32);
    }
}
