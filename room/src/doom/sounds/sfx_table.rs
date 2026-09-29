//! The SFX table: the `SfxInfo` record (a locked `repr(C)` mirror of
//! `sfxinfo_t`), its compile-time builders, the short lump-name buffers, and
//! the master `S_sfx` table indexed by `Sfx` ids.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};

use super::enums::NUMSFX;

/// Rust mirror of `sfxinfo_t` (`sounds.h`).
///
/// The layout is locked to the C struct so the table can be passed to driver
/// code via `#[no_mangle] S_sfx`. `name` is a fixed 9-byte slot (8 chars +
/// NUL) holding the short lump name; the engine prepends `DS` when looking
/// up the actual WAD lump. `link` lets two effects share underlying audio
/// (e.g. `sfx_chgun -> sfx_pistol`).
#[repr(C)]
pub struct SfxInfo {
    /// Tag name pointer (DEH support); unused at runtime, always null.
    pub tagname: *mut c_char,
    /// Short lump name (no `DS` prefix), NUL-terminated.
    pub name: [c_char; 9],
    /// Channel priority; higher values evict lower ones in `S_GetChannel`.
    pub priority: c_int,
    /// Optional alias to another `SfxInfo` so several effects share audio.
    pub link: *mut SfxInfo,
    /// Pitch override (-1 = use sound default). Only meaningful when `link`.
    pub pitch: c_int,
    /// Volume modifier in 0-127 range, added when the effect aliases via `link`.
    pub volume: c_int,
    /// Soft reference count for the cached lump; decremented on `S_StopChannel`.
    pub usefulness: c_int,
    /// Cached lump number, or -1 before the lump is first loaded.
    pub lumpnum: c_int,
    /// Maximum simultaneous channels (-1 = unlimited).
    pub numchannels: c_int,
    /// Pointer to driver-side decoded audio data.
    pub driver_data: *mut c_void,
}

/// Manual `Sync` for `SfxInfo`: the static table is mutated only at startup
/// (or by single-threaded driver callbacks), so sharing the static across
/// threads is sound under the engine's threading model.
unsafe impl Sync for SfxInfo {}

/// Constructors used to build the static `S_sfx` table at compile time.
impl SfxInfo {
    /// Build a default `SfxInfo` for the given short lump name and priority.
    /// Pitch and volume default to -1 (driver default), `numchannels` to -1
    /// (unlimited); other fields are null/zero. Matches the C
    /// `S_sfx[]` initialiser pattern in `sounds.c`.
    const fn new(name: [c_char; 9], priority: c_int) -> Self {
        SfxInfo {
            tagname: std::ptr::null_mut(),
            name,
            priority,
            link: std::ptr::null_mut(),
            pitch: -1,
            volume: -1,
            usefulness: 0,
            lumpnum: 0,
            numchannels: -1,
            driver_data: std::ptr::null_mut(),
        }
    }

    /// Builder helper: set the `pitch` override and return `self`.
    const fn with_pitch(mut self, pitch: c_int) -> Self {
        self.pitch = pitch;
        self
    }

    /// Builder helper: set the `volume` modifier and return `self`.
    const fn with_volume(mut self, volume: c_int) -> Self {
        self.volume = volume;
        self
    }
}

/// Compile-time helper: turn a Rust `&str` into a fixed `[c_char; N]` buffer,
/// NUL-padded. Panics at const-eval if `s` would not fit (must leave room for
/// the trailing NUL).
pub(super) const fn name<const N: usize>(s: &str) -> [c_char; N] {
    let b = s.as_bytes();
    assert!(b.len() < N, "String is too long for the array");
    let mut a = [0i8; N];
    let mut i = 0;
    while i < b.len() {
        a[i] = b[i] as c_char;
        i += 1;
    }
    a
}

/// Short lump-name buffer for SFX `DSNONE` (`sfx_none`).
const N_none: [c_char; 9] = name("none");
/// Short lump-name buffer for SFX `DSPISTOL` (`sfx_pistol`).
const N_pistol: [c_char; 9] = name("pistol");
/// Short lump-name buffer for SFX `DSSHOTGN` (`sfx_shotgn`).
const N_shotgn: [c_char; 9] = name("shotgn");
/// Short lump-name buffer for SFX `DSSGCOCK` (`sfx_sgcock`).
const N_sgcock: [c_char; 9] = name("sgcock");
/// Short lump-name buffer for SFX `DSDSHTGN` (`sfx_dshtgn`).
const N_dshtgn: [c_char; 9] = name("dshtgn");
/// Short lump-name buffer for SFX `DSDBOPN` (`sfx_dbopn`).
const N_dbopn: [c_char; 9] = name("dbopn");
/// Short lump-name buffer for SFX `DSDBCLS` (`sfx_dbcls`).
const N_dbcls: [c_char; 9] = name("dbcls");
/// Short lump-name buffer for SFX `DSDBLOAD` (`sfx_dbload`).
const N_dbload: [c_char; 9] = name("dbload");
/// Short lump-name buffer for SFX `DSPLASMA` (`sfx_plasma`).
const N_plasma: [c_char; 9] = name("plasma");
/// Short lump-name buffer for SFX `DSBFG` (`sfx_bfg`).
const N_bfg: [c_char; 9] = name("bfg");
/// Short lump-name buffer for SFX `DSSAWUP` (`sfx_sawup`).
const N_sawup: [c_char; 9] = name("sawup");
/// Short lump-name buffer for SFX `DSSAWIDL` (`sfx_sawidl`).
const N_sawidl: [c_char; 9] = name("sawidl");
/// Short lump-name buffer for SFX `DSSAWFUL` (`sfx_sawful`).
const N_sawful: [c_char; 9] = name("sawful");
/// Short lump-name buffer for SFX `DSSAWHIT` (`sfx_sawhit`).
const N_sawhit: [c_char; 9] = name("sawhit");
/// Short lump-name buffer for SFX `DSRLAUNC` (`sfx_rlaunc`).
const N_rlaunc: [c_char; 9] = name("rlaunc");
/// Short lump-name buffer for SFX `DSRXPLOD` (`sfx_rxplod`).
const N_rxplod: [c_char; 9] = name("rxplod");
/// Short lump-name buffer for SFX `DSFIRSHT` (`sfx_firsht`).
const N_firsht: [c_char; 9] = name("firsht");
/// Short lump-name buffer for SFX `DSFIRXPL` (`sfx_firxpl`).
const N_firxpl: [c_char; 9] = name("firxpl");
/// Short lump-name buffer for SFX `DSPSTART` (`sfx_pstart`).
const N_pstart: [c_char; 9] = name("pstart");
/// Short lump-name buffer for SFX `DSPSTOP` (`sfx_pstop`).
const N_pstop: [c_char; 9] = name("pstop");
/// Short lump-name buffer for SFX `DSDOROPN` (`sfx_doropn`).
const N_doropn: [c_char; 9] = name("doropn");
/// Short lump-name buffer for SFX `DSDORCLS` (`sfx_dorcls`).
const N_dorcls: [c_char; 9] = name("dorcls");
/// Short lump-name buffer for SFX `DSSTNMOV` (`sfx_stnmov`).
const N_stnmov: [c_char; 9] = name("stnmov");
/// Short lump-name buffer for SFX `DSSWTCHN` (`sfx_swtchn`).
const N_swtchn: [c_char; 9] = name("swtchn");
/// Short lump-name buffer for SFX `DSSWTCHX` (`sfx_swtchx`).
const N_swtchx: [c_char; 9] = name("swtchx");
/// Short lump-name buffer for SFX `DSPLPAIN` (`sfx_plpain`).
const N_plpain: [c_char; 9] = name("plpain");
/// Short lump-name buffer for SFX `DSDMPAIN` (`sfx_dmpain`).
const N_dmpain: [c_char; 9] = name("dmpain");
/// Short lump-name buffer for SFX `DSPOPAIN` (`sfx_popain`).
const N_popain: [c_char; 9] = name("popain");
/// Short lump-name buffer for SFX `DSVIPAIN` (`sfx_vipain`).
const N_vipain: [c_char; 9] = name("vipain");
/// Short lump-name buffer for SFX `DSMNPAIN` (`sfx_mnpain`).
const N_mnpain: [c_char; 9] = name("mnpain");
/// Short lump-name buffer for SFX `DSPEPAIN` (`sfx_pepain`).
const N_pepain: [c_char; 9] = name("pepain");
/// Short lump-name buffer for SFX `DSSLOP` (`sfx_slop`).
const N_slop: [c_char; 9] = name("slop");
/// Short lump-name buffer for SFX `DSITEMUP` (`sfx_itemup`).
const N_itemup: [c_char; 9] = name("itemup");
/// Short lump-name buffer for SFX `DSWPNUP` (`sfx_wpnup`).
const N_wpnup: [c_char; 9] = name("wpnup");
/// Short lump-name buffer for SFX `DSOOF` (`sfx_oof`).
const N_oof: [c_char; 9] = name("oof");
/// Short lump-name buffer for SFX `DSTELEPT` (`sfx_telept`).
const N_telept: [c_char; 9] = name("telept");
/// Short lump-name buffer for SFX `DSPOSIT1` (`sfx_posit1`).
const N_posit1: [c_char; 9] = name("posit1");
/// Short lump-name buffer for SFX `DSPOSIT2` (`sfx_posit2`).
const N_posit2: [c_char; 9] = name("posit2");
/// Short lump-name buffer for SFX `DSPOSIT3` (`sfx_posit3`).
const N_posit3: [c_char; 9] = name("posit3");
/// Short lump-name buffer for SFX `DSBGSIT1` (`sfx_bgsit1`).
const N_bgsit1: [c_char; 9] = name("bgsit1");
/// Short lump-name buffer for SFX `DSBGSIT2` (`sfx_bgsit2`).
const N_bgsit2: [c_char; 9] = name("bgsit2");
/// Short lump-name buffer for SFX `DSSGTSIT` (`sfx_sgtsit`).
const N_sgtsit: [c_char; 9] = name("sgtsit");
/// Short lump-name buffer for SFX `DSCACSIT` (`sfx_cacsit`).
const N_cacsit: [c_char; 9] = name("cacsit");
/// Short lump-name buffer for SFX `DSBRSSIT` (`sfx_brssit`).
const N_brssit: [c_char; 9] = name("brssit");
/// Short lump-name buffer for SFX `DSCYBSIT` (`sfx_cybsit`).
const N_cybsit: [c_char; 9] = name("cybsit");
/// Short lump-name buffer for SFX `DSSPISIT` (`sfx_spisit`).
const N_spisit: [c_char; 9] = name("spisit");
/// Short lump-name buffer for SFX `DSBSPSIT` (`sfx_bspit`).
const N_bspit: [c_char; 9] = name("bspsit");
/// Short lump-name buffer for SFX `DSKNTSIT` (`sfx_kntsit`).
const N_kntsit: [c_char; 9] = name("kntsit");
/// Short lump-name buffer for SFX `DSVILSIT` (`sfx_vilsit`).
const N_vilsit: [c_char; 9] = name("vilsit");
/// Short lump-name buffer for SFX `DSMANSIT` (`sfx_mansit`).
const N_mansit: [c_char; 9] = name("mansit");
/// Short lump-name buffer for SFX `DSPESIT` (`sfx_pesit`).
const N_pesit: [c_char; 9] = name("pesit");
/// Short lump-name buffer for SFX `DSSKLATK` (`sfx_sklatk`).
const N_sklatk: [c_char; 9] = name("sklatk");
/// Short lump-name buffer for SFX `DSSGTATK` (`sfx_sgtatk`).
const N_sgtatk: [c_char; 9] = name("sgtatk");
/// Short lump-name buffer for SFX `DSSKEPCH` (`sfx_skepch`).
const N_skepch: [c_char; 9] = name("skepch");
/// Short lump-name buffer for SFX `DSVILATK` (`sfx_vilatk`).
const N_vilatk: [c_char; 9] = name("vilatk");
/// Short lump-name buffer for SFX `DSCLAW` (`sfx_claw`).
const N_claw: [c_char; 9] = name("claw");
/// Short lump-name buffer for SFX `DSSKESWG` (`sfx_skeswg`).
const N_skeswg: [c_char; 9] = name("skeswg");
/// Short lump-name buffer for SFX `DSPLDETH` (`sfx_pldeth`).
const N_pldeth: [c_char; 9] = name("pldeth");
/// Short lump-name buffer for SFX `DSPDIEHI` (`sfx_pdiehi`).
const N_pdiehi: [c_char; 9] = name("pdiehi");
/// Short lump-name buffer for SFX `DSPODTH1` (`sfx_podth1`).
const N_podth1: [c_char; 9] = name("podth1");
/// Short lump-name buffer for SFX `DSPODTH2` (`sfx_podth2`).
const N_podth2: [c_char; 9] = name("podth2");
/// Short lump-name buffer for SFX `DSPODTH3` (`sfx_podth3`).
const N_podth3: [c_char; 9] = name("podth3");
/// Short lump-name buffer for SFX `DSBGDTH1` (`sfx_bgdth1`).
const N_bgdth1: [c_char; 9] = name("bgdth1");
/// Short lump-name buffer for SFX `DSBGDTH2` (`sfx_bgdth2`).
const N_bgdth2: [c_char; 9] = name("bgdth2");
/// Short lump-name buffer for SFX `DSSGTDTH` (`sfx_sgtdth`).
const N_sgtdth: [c_char; 9] = name("sgtdth");
/// Short lump-name buffer for SFX `DSCACDTH` (`sfx_cacdth`).
const N_cacdth: [c_char; 9] = name("cacdth");
/// Short lump-name buffer for SFX `DSSKLDTH` (`sfx_skldth`).
const N_skldth: [c_char; 9] = name("skldth");
/// Short lump-name buffer for SFX `DSBRSDTH` (`sfx_brsdth`).
const N_brsdth: [c_char; 9] = name("brsdth");
/// Short lump-name buffer for SFX `DSCYBDTH` (`sfx_cybdth`).
const N_cybdth: [c_char; 9] = name("cybdth");
/// Short lump-name buffer for SFX `DSSPIDTH` (`sfx_spidth`).
const N_spidth: [c_char; 9] = name("spidth");
/// Short lump-name buffer for SFX `DSBSPDTH` (`sfx_bspdth`).
const N_bspdth: [c_char; 9] = name("bspdth");
/// Short lump-name buffer for SFX `DSVILDTH` (`sfx_vildth`).
const N_vildth: [c_char; 9] = name("vildth");
/// Short lump-name buffer for SFX `DSKNTDTH` (`sfx_kntdth`).
const N_kntdth: [c_char; 9] = name("kntdth");
/// Short lump-name buffer for SFX `DSPEDTH` (`sfx_pedth`).
const N_pedth: [c_char; 9] = name("pedth");
/// Short lump-name buffer for SFX `DSSKEDTH` (`sfx_skedth`).
const N_skedth: [c_char; 9] = name("skedth");
/// Short lump-name buffer for SFX `DSPOSACT` (`sfx_posact`).
const N_posact: [c_char; 9] = name("posact");
/// Short lump-name buffer for SFX `DSBGACT` (`sfx_bgact`).
const N_bgact: [c_char; 9] = name("bgact");
/// Short lump-name buffer for SFX `DSDMACT` (`sfx_dmact`).
const N_dmact: [c_char; 9] = name("dmact");
/// Short lump-name buffer for SFX `DSBSPACT` (`sfx_bspact`).
const N_bspact: [c_char; 9] = name("bspact");
/// Short lump-name buffer for SFX `DSBSPWLK` (`sfx_bspwlk`).
const N_bspwlk: [c_char; 9] = name("bspwlk");
/// Short lump-name buffer for SFX `DSVILACT` (`sfx_vilact`).
const N_vilact: [c_char; 9] = name("vilact");
/// Short lump-name buffer for SFX `DSNOWAY` (`sfx_noway`).
const N_noway: [c_char; 9] = name("noway");
/// Short lump-name buffer for SFX `DSBAREXP` (`sfx_barexp`).
const N_barexp: [c_char; 9] = name("barexp");
/// Short lump-name buffer for SFX `DSPUNCH` (`sfx_punch`).
const N_punch: [c_char; 9] = name("punch");
/// Short lump-name buffer for SFX `DSHOOF` (`sfx_hoof`).
const N_hoof: [c_char; 9] = name("hoof");
/// Short lump-name buffer for SFX `DSMETAL` (`sfx_metal`).
const N_metal: [c_char; 9] = name("metal");
/// Short lump-name buffer for SFX `DSCHGUN` (`sfx_chgun`).
const N_chgun: [c_char; 9] = name("chgun");
/// Short lump-name buffer for SFX `DSTINK` (`sfx_tink`).
const N_tink: [c_char; 9] = name("tink");
/// Short lump-name buffer for SFX `DSBDOPN` (`sfx_bdopn`).
const N_bdopn: [c_char; 9] = name("bdopn");
/// Short lump-name buffer for SFX `DSBDCLS` (`sfx_bdcls`).
const N_bdcls: [c_char; 9] = name("bdcls");
/// Short lump-name buffer for SFX `DSITMBK` (`sfx_itmbk`).
const N_itmbk: [c_char; 9] = name("itmbk");
/// Short lump-name buffer for SFX `DSFLAME` (`sfx_flame`).
const N_flame: [c_char; 9] = name("flame");
/// Short lump-name buffer for SFX `DSFLAMST` (`sfx_flamst`).
const N_flamst: [c_char; 9] = name("flamst");
/// Short lump-name buffer for SFX `DSGETPOW` (`sfx_getpow`).
const N_getpow: [c_char; 9] = name("getpow");
/// Short lump-name buffer for SFX `DSBOSPIT` (`sfx_bospit`).
const N_bospit: [c_char; 9] = name("bospit");
/// Short lump-name buffer for SFX `DSBOSCUB` (`sfx_boscub`).
const N_boscub: [c_char; 9] = name("boscub");
/// Short lump-name buffer for SFX `DSBOSSIT` (`sfx_bossit`).
const N_bossit: [c_char; 9] = name("bossit");
/// Short lump-name buffer for SFX `DSBOSPN` (`sfx_bospn`).
const N_bospn: [c_char; 9] = name("bospn");
/// Short lump-name buffer for SFX `DSBOSDTH` (`sfx_bosdth`).
const N_bosdth: [c_char; 9] = name("bosdth");
/// Short lump-name buffer for SFX `DSMANATK` (`sfx_manatk`).
const N_manatk: [c_char; 9] = name("manatk");
/// Short lump-name buffer for SFX `DSMANDTH` (`sfx_mandth`).
const N_mandth: [c_char; 9] = name("mandth");
/// Short lump-name buffer for SFX `DSSSSIT` (`sfx_sssit`).
const N_sssit: [c_char; 9] = name("sssit");
/// Short lump-name buffer for SFX `DSSSDTH` (`sfx_ssdth`).
const N_ssdth: [c_char; 9] = name("ssdth");
/// Short lump-name buffer for SFX `DSKEENPN` (`sfx_keenpn`).
const N_keenpn: [c_char; 9] = name("keenpn");
/// Short lump-name buffer for SFX `DSKEENDT` (`sfx_keendt`).
const N_keendt: [c_char; 9] = name("keendt");
/// Short lump-name buffer for SFX `DSSKEACT` (`sfx_skeact`).
const N_skeact: [c_char; 9] = name("skeact");
/// Short lump-name buffer for SFX `DSSKESIT` (`sfx_skesit`).
const N_skesit: [c_char; 9] = name("skesit");
/// Short lump-name buffer for SFX `DSSKEATK` (`sfx_skeatk`).
const N_skeatk: [c_char; 9] = name("skeatk");
/// Short lump-name buffer for SFX `DSRADIO` (`sfx_radio`).
const N_radio: [c_char; 9] = name("radio");

/// Master SFX table indexed by `Sfx` ids. Mirrors `S_sfx[]` in `sounds.c`.
///
/// Built at compile time from the `N_*` short-name buffers and per-entry
/// priorities. Entry 0 is a "none" sentinel. `usefulness` is reset to -1 for
/// every non-sentinel entry by `S_Init`. Linked entries (currently only
/// `sfx_chgun -> sfx_pistol`) are wired up at runtime by `S_InitSfxLinks`.
///
/// `#[no_mangle]` so it is reachable from any remaining C-linkage callers
/// (the test harness verifies layout against the C `sizeof`).
#[no_mangle]
pub static mut S_sfx: [SfxInfo; NUMSFX] = [
    // [0] sfx_none
    SfxInfo::new(N_none, 0),
    // [1] sfx_pistol
    SfxInfo::new(N_pistol, 64),
    // [2] sfx_shotgn
    SfxInfo::new(N_shotgn, 64),
    // [3] sfx_sgcock
    SfxInfo::new(N_sgcock, 64),
    // [4] sfx_dshtgn
    SfxInfo::new(N_dshtgn, 64),
    // [5] sfx_dbopn
    SfxInfo::new(N_dbopn, 64),
    // [6] sfx_dbcls
    SfxInfo::new(N_dbcls, 64),
    // [7] sfx_dbload
    SfxInfo::new(N_dbload, 64),
    // [8] sfx_plasma
    SfxInfo::new(N_plasma, 64),
    // [9] sfx_bfg
    SfxInfo::new(N_bfg, 64),
    // [10] sfx_sawup
    SfxInfo::new(N_sawup, 64),
    // [11] sfx_sawidl
    SfxInfo::new(N_sawidl, 118),
    // [12] sfx_sawful
    SfxInfo::new(N_sawful, 64),
    // [13] sfx_sawhit
    SfxInfo::new(N_sawhit, 64),
    // [14] sfx_rlaunc
    SfxInfo::new(N_rlaunc, 64),
    // [15] sfx_rxplod
    SfxInfo::new(N_rxplod, 70),
    // [16] sfx_firsht
    SfxInfo::new(N_firsht, 70),
    // [17] sfx_firxpl
    SfxInfo::new(N_firxpl, 70),
    // [18] sfx_pstart
    SfxInfo::new(N_pstart, 100),
    // [19] sfx_pstop
    SfxInfo::new(N_pstop, 100),
    // [20] sfx_doropn
    SfxInfo::new(N_doropn, 100),
    // [21] sfx_dorcls
    SfxInfo::new(N_dorcls, 100),
    // [22] sfx_stnmov
    SfxInfo::new(N_stnmov, 119),
    // [23] sfx_swtchn
    SfxInfo::new(N_swtchn, 78),
    // [24] sfx_swtchx
    SfxInfo::new(N_swtchx, 78),
    // [25] sfx_plpain
    SfxInfo::new(N_plpain, 96),
    // [26] sfx_dmpain
    SfxInfo::new(N_dmpain, 96),
    // [27] sfx_popain
    SfxInfo::new(N_popain, 96),
    // [28] sfx_vipain
    SfxInfo::new(N_vipain, 96),
    // [29] sfx_mnpain
    SfxInfo::new(N_mnpain, 96),
    // [30] sfx_pepain
    SfxInfo::new(N_pepain, 96),
    // [31] sfx_slop
    SfxInfo::new(N_slop, 78),
    // [32] sfx_itemup
    SfxInfo::new(N_itemup, 78),
    // [33] sfx_wpnup
    SfxInfo::new(N_wpnup, 78),
    // [34] sfx_oof
    SfxInfo::new(N_oof, 96),
    // [35] sfx_telept
    SfxInfo::new(N_telept, 32),
    // [36] sfx_posit1
    SfxInfo::new(N_posit1, 98),
    // [37] sfx_posit2
    SfxInfo::new(N_posit2, 98),
    // [38] sfx_posit3
    SfxInfo::new(N_posit3, 98),
    // [39] sfx_bgsit1
    SfxInfo::new(N_bgsit1, 98),
    // [40] sfx_bgsit2
    SfxInfo::new(N_bgsit2, 98),
    // [41] sfx_sgtsit
    SfxInfo::new(N_sgtsit, 98),
    // [42] sfx_cacsit
    SfxInfo::new(N_cacsit, 98),
    // [43] sfx_brssit
    SfxInfo::new(N_brssit, 94),
    // [44] sfx_cybsit
    SfxInfo::new(N_cybsit, 92),
    // [45] sfx_spisit
    SfxInfo::new(N_spisit, 90),
    // [46] sfx_bspit
    SfxInfo::new(N_bspit, 90),
    // [47] sfx_kntsit
    SfxInfo::new(N_kntsit, 90),
    // [48] sfx_vilsit
    SfxInfo::new(N_vilsit, 90),
    // [49] sfx_mansit
    SfxInfo::new(N_mansit, 90),
    // [50] sfx_pesit
    SfxInfo::new(N_pesit, 90),
    // [51] sfx_sklatk
    SfxInfo::new(N_sklatk, 70),
    // [52] sfx_sgtatk
    SfxInfo::new(N_sgtatk, 70),
    // [53] sfx_skepch
    SfxInfo::new(N_skepch, 70),
    // [54] sfx_vilatk
    SfxInfo::new(N_vilatk, 70),
    // [55] sfx_claw
    SfxInfo::new(N_claw, 70),
    // [56] sfx_skeswg
    SfxInfo::new(N_skeswg, 70),
    // [57] sfx_pldeth
    SfxInfo::new(N_pldeth, 32),
    // [58] sfx_pdiehi
    SfxInfo::new(N_pdiehi, 32),
    // [59] sfx_podth1
    SfxInfo::new(N_podth1, 70),
    // [60] sfx_podth2
    SfxInfo::new(N_podth2, 70),
    // [61] sfx_podth3
    SfxInfo::new(N_podth3, 70),
    // [62] sfx_bgdth1
    SfxInfo::new(N_bgdth1, 70),
    // [63] sfx_bgdth2
    SfxInfo::new(N_bgdth2, 70),
    // [64] sfx_sgtdth
    SfxInfo::new(N_sgtdth, 70),
    // [65] sfx_cacdth
    SfxInfo::new(N_cacdth, 70),
    // [66] sfx_skldth
    SfxInfo::new(N_skldth, 70),
    // [67] sfx_brsdth
    SfxInfo::new(N_brsdth, 32),
    // [68] sfx_cybdth
    SfxInfo::new(N_cybdth, 32),
    // [69] sfx_spidth
    SfxInfo::new(N_spidth, 32),
    // [70] sfx_bspdth
    SfxInfo::new(N_bspdth, 32),
    // [71] sfx_vildth
    SfxInfo::new(N_vildth, 32),
    // [72] sfx_kntdth
    SfxInfo::new(N_kntdth, 32),
    // [73] sfx_pedth
    SfxInfo::new(N_pedth, 32),
    // [74] sfx_skedth
    SfxInfo::new(N_skedth, 32),
    // [75] sfx_posact
    SfxInfo::new(N_posact, 120),
    // [76] sfx_bgact
    SfxInfo::new(N_bgact, 120),
    // [77] sfx_dmact
    SfxInfo::new(N_dmact, 120),
    // [78] sfx_bspact
    SfxInfo::new(N_bspact, 100),
    // [79] sfx_bspwlk
    SfxInfo::new(N_bspwlk, 100),
    // [80] sfx_vilact
    SfxInfo::new(N_vilact, 100),
    // [81] sfx_noway
    SfxInfo::new(N_noway, 78),
    // [82] sfx_barexp
    SfxInfo::new(N_barexp, 60),
    // [83] sfx_punch
    SfxInfo::new(N_punch, 64),
    // [84] sfx_hoof
    SfxInfo::new(N_hoof, 70),
    // [85] sfx_metal
    SfxInfo::new(N_metal, 70),
    // [86] sfx_chgun
    SfxInfo::new(N_chgun, 64).with_volume(0).with_pitch(150),
    // [87] sfx_tink
    SfxInfo::new(N_tink, 60),
    // [88] sfx_bdopn
    SfxInfo::new(N_bdopn, 100),
    // [89] sfx_bdcls
    SfxInfo::new(N_bdcls, 100),
    // [90] sfx_itmbk
    SfxInfo::new(N_itmbk, 100),
    // [91] sfx_flame
    SfxInfo::new(N_flame, 32),
    // [92] sfx_flamst
    SfxInfo::new(N_flamst, 32),
    // [93] sfx_getpow
    SfxInfo::new(N_getpow, 60),
    // [94] sfx_bospit
    SfxInfo::new(N_bospit, 70),
    // [95] sfx_boscub
    SfxInfo::new(N_boscub, 70),
    // [96] sfx_bossit
    SfxInfo::new(N_bossit, 70),
    // [97] sfx_bospn
    SfxInfo::new(N_bospn, 70),
    // [98] sfx_bosdth
    SfxInfo::new(N_bosdth, 70),
    // [99] sfx_manatk
    SfxInfo::new(N_manatk, 70),
    // [100] sfx_mandth
    SfxInfo::new(N_mandth, 70),
    // [101] sfx_sssit
    SfxInfo::new(N_sssit, 70),
    // [102] sfx_ssdth
    SfxInfo::new(N_ssdth, 70),
    // [103] sfx_keenpn
    SfxInfo::new(N_keenpn, 70),
    // [104] sfx_keendt
    SfxInfo::new(N_keendt, 70),
    // [105] sfx_skeact
    SfxInfo::new(N_skeact, 70),
    // [106] sfx_skesit
    SfxInfo::new(N_skesit, 70),
    // [107] sfx_skeatk
    SfxInfo::new(N_skeatk, 70),
    // [108] sfx_radio
    SfxInfo::new(N_radio, 60),
];

#[cfg(test)]
mod tests {
    use crate::doom::sounds::{S_sfx, Sfx, SfxInfo, NUMSFX};
    use std::mem::size_of;

    /// `S_sfx` length must equal `NUMSFX` (catches off-by-one in the table).
    #[test]
    fn sfx_table_has_correct_length() {
        // The static array is sized by NUMSFX; this confirms no off-by-one.
        unsafe {
            assert_eq!(S_sfx.len(), NUMSFX);
        }
    }

    /// Entry 0 is the "none" sentinel - priority and numchannels must be 0 / -1.
    #[test]
    fn sfx_entry_zero_is_none_sentinel() {
        unsafe {
            assert_eq!(S_sfx[0].priority, 0);
            assert_eq!(S_sfx[0].numchannels, -1);
            assert_eq!(S_sfx[0].lumpnum, 0);
            // tagname and link are null initially
            assert!(S_sfx[0].tagname.is_null());
            assert!(S_sfx[0].link.is_null());
        }
    }

    /// sfx_pistol is entry 1 with priority 64.
    #[test]
    fn sfx_pistol_is_entry_1_with_priority_64() {
        unsafe {
            let entry = &S_sfx[Sfx::Pistol as usize];
            assert_eq!(entry.priority, 64);
            // name must start with "pistol"
            let name_bytes: Vec<u8> = entry.name.iter().map(|&c| c as u8).collect();
            assert!(
                name_bytes.starts_with(b"pistol"),
                "name should start with 'pistol'"
            );
        }
    }

    /// `sizeof(sfxinfo_t)` must be 64 on 64-bit, matching the C struct.
    #[test]
    fn sfx_info_size_matches_c() {
        // sizeof(sfxinfo_t) in C on 64-bit: tagname(8) + name[9](9) + pad(3)
        // + priority(4) + link(8) + pitch(4) + volume(4) + usefulness(4)
        // + lumpnum(4) + numchannels(4) + pad(4) + driver_data(8) = 64 bytes
        assert_eq!(size_of::<SfxInfo>(), 64);
    }
}
