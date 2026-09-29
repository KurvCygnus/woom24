//! The sound/music id enums (`Sfx`, `Mus`) and their count constants.
//!
//! `NUMSFX` / `NUMMUSIC` are derived from the enum discriminants and pinned
//! to the C enum sizes (109 / 68) by the tests below.

/// Number of `Sfx` ids, matching `NUMSFX` (109) in `sounds.h`.
// sfxenum_t in sounds.h ends with NUMSFX = 109
pub const NUMSFX: usize = Sfx::Radio as usize + 1;
/// Number of `Mus` ids, matching `NUMMUSIC` (68) in `sounds.h`.
// musicenum_t in sounds.h ends with NUMMUSIC = 68
pub const NUMMUSIC: usize = Mus::Dm2int as usize + 1;

const _: () = assert!(
    std::mem::size_of::<Sfx>() == std::mem::size_of::<std::ffi::c_int>(),
    "Sfx must be the same size as c_int"
);
const _: () = assert!(
    std::mem::size_of::<Mus>() == std::mem::size_of::<std::ffi::c_int>(),
    "Mus must be the same size as c_int"
);

/// Sound effect IDs, matching the `sfxenum_t` C enum in `sounds.h`.
#[doc(alias = "sfxenum_t")]
#[repr(C)]
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Sfx {
    #[default]
    /// Sentinel "no sound" id.
    #[doc(alias = "sfx_None")]
    None = 0,
    /// Sound effect `sfx_pistol`.
    #[doc(alias = "sfx_pistol")]
    Pistol = 1,
    /// Sound effect `sfx_shotgn`.
    #[doc(alias = "sfx_shotgn")]
    Shotgn = 2,
    /// Sound effect `sfx_sgcock`.
    #[doc(alias = "sfx_sgcock")]
    Sgcock = 3,
    /// Sound effect `sfx_dshtgn`.
    #[doc(alias = "sfx_dshtgn")]
    Dshtgn = 4,
    /// Sound effect `sfx_dbopn`.
    #[doc(alias = "sfx_dbopn")]
    Dbopn = 5,
    /// Sound effect `sfx_dbcls`.
    #[doc(alias = "sfx_dbcls")]
    Dbcls = 6,
    /// Sound effect `sfx_dbload`.
    #[doc(alias = "sfx_dbload")]
    Dbload = 7,
    /// Sound effect `sfx_plasma`.
    #[doc(alias = "sfx_plasma")]
    Plasma = 8,
    /// Sound effect `sfx_bfg`.
    #[doc(alias = "sfx_bfg")]
    Bfg = 9,
    /// Sound effect `sfx_sawup`.
    #[doc(alias = "sfx_sawup")]
    Sawup = 10,
    /// Sound effect `sfx_sawidl`.
    #[doc(alias = "sfx_sawidl")]
    Sawidl = 11,
    /// Sound effect `sfx_sawful`.
    #[doc(alias = "sfx_sawful")]
    Sawful = 12,
    /// Sound effect `sfx_sawhit`.
    #[doc(alias = "sfx_sawhit")]
    Sawhit = 13,
    /// Sound effect `sfx_rlaunc`.
    #[doc(alias = "sfx_rlaunc")]
    Rlaunc = 14,
    /// Sound effect `sfx_rxplod`.
    #[doc(alias = "sfx_rxplod")]
    Rxplod = 15,
    /// Sound effect `sfx_firsht`.
    #[doc(alias = "sfx_firsht")]
    Firsht = 16,
    /// Sound effect `sfx_firxpl`.
    #[doc(alias = "sfx_firxpl")]
    Firxpl = 17,
    /// Sound effect `sfx_pstart`.
    #[doc(alias = "sfx_pstart")]
    Pstart = 18,
    /// Sound effect `sfx_pstop`.
    #[doc(alias = "sfx_pstop")]
    Pstop = 19,
    /// Sound effect `sfx_doropn`.
    #[doc(alias = "sfx_doropn")]
    Doropn = 20,
    /// Sound effect `sfx_dorcls`.
    #[doc(alias = "sfx_dorcls")]
    Dorcls = 21,
    /// Sound effect `sfx_stnmov`.
    #[doc(alias = "sfx_stnmov")]
    Stnmov = 22,
    /// Sound effect `sfx_swtchn`.
    #[doc(alias = "sfx_swtchn")]
    Swtchn = 23,
    /// Sound effect `sfx_swtchx`.
    #[doc(alias = "sfx_swtchx")]
    Swtchx = 24,
    /// Sound effect `sfx_plpain`.
    #[doc(alias = "sfx_plpain")]
    Plpain = 25,
    /// Sound effect `sfx_dmpain`.
    #[doc(alias = "sfx_dmpain")]
    Dmpain = 26,
    /// Sound effect `sfx_popain`.
    #[doc(alias = "sfx_popain")]
    Popain = 27,
    /// Sound effect `sfx_vipain`.
    #[doc(alias = "sfx_vipain")]
    Vipain = 28,
    /// Sound effect `sfx_mnpain`.
    #[doc(alias = "sfx_mnpain")]
    Mnpain = 29,
    /// Sound effect `sfx_pepain`.
    #[doc(alias = "sfx_pepain")]
    Pepain = 30,
    /// Sound effect `sfx_slop`.
    #[doc(alias = "sfx_slop")]
    Slop = 31,
    /// Sound effect `sfx_itemup`.
    #[doc(alias = "sfx_itemup")]
    Itemup = 32,
    /// Sound effect `sfx_wpnup`.
    #[doc(alias = "sfx_wpnup")]
    Wpnup = 33,
    /// Sound effect `sfx_oof`.
    #[doc(alias = "sfx_oof")]
    Oof = 34,
    /// Sound effect `sfx_telept`.
    #[doc(alias = "sfx_telept")]
    Telept = 35,
    /// Sound effect `sfx_posit1`.
    #[doc(alias = "sfx_posit1")]
    Posit1 = 36,
    /// Sound effect `sfx_posit2`.
    #[doc(alias = "sfx_posit2")]
    Posit2 = 37,
    /// Sound effect `sfx_posit3`.
    #[doc(alias = "sfx_posit3")]
    Posit3 = 38,
    /// Sound effect `sfx_bgsit1`.
    #[doc(alias = "sfx_bgsit1")]
    Bgsit1 = 39,
    /// Sound effect `sfx_bgsit2`.
    #[doc(alias = "sfx_bgsit2")]
    Bgsit2 = 40,
    /// Sound effect `sfx_sgtsit`.
    #[doc(alias = "sfx_sgtsit")]
    Sgtsit = 41,
    /// Sound effect `sfx_cacsit`.
    #[doc(alias = "sfx_cacsit")]
    Cacsit = 42,
    /// Sound effect `sfx_brssit`.
    #[doc(alias = "sfx_brssit")]
    Brssit = 43,
    /// Sound effect `sfx_cybsit`.
    #[doc(alias = "sfx_cybsit")]
    Cybsit = 44,
    /// Sound effect `sfx_spisit`.
    #[doc(alias = "sfx_spisit")]
    Spisit = 45,
    /// Sound effect `sfx_bspsit`.
    #[doc(alias = "sfx_bspsit")]
    Bspsit = 46,
    /// Sound effect `sfx_kntsit`.
    #[doc(alias = "sfx_kntsit")]
    Kntsit = 47,
    /// Sound effect `sfx_vilsit`.
    #[doc(alias = "sfx_vilsit")]
    Vilsit = 48,
    /// Sound effect `sfx_mansit`.
    #[doc(alias = "sfx_mansit")]
    Mansit = 49,
    /// Sound effect `sfx_pesit`.
    #[doc(alias = "sfx_pesit")]
    Pesit = 50,
    /// Sound effect `sfx_sklatk`.
    #[doc(alias = "sfx_sklatk")]
    Sklatk = 51,
    /// Sound effect `sfx_sgtatk`.
    #[doc(alias = "sfx_sgtatk")]
    Sgtatk = 52,
    /// Sound effect `sfx_skepch`.
    #[doc(alias = "sfx_skepch")]
    Skepch = 53,
    /// Sound effect `sfx_vilatk`.
    #[doc(alias = "sfx_vilatk")]
    Vilatk = 54,
    /// Sound effect `sfx_claw`.
    #[doc(alias = "sfx_claw")]
    Claw = 55,
    /// Sound effect `sfx_skeswg`.
    #[doc(alias = "sfx_skeswg")]
    Skeswg = 56,
    /// Sound effect `sfx_pldeth`.
    #[doc(alias = "sfx_pldeth")]
    Pldeth = 57,
    /// Sound effect `sfx_pdiehi`.
    #[doc(alias = "sfx_pdiehi")]
    Pdiehi = 58,
    /// Sound effect `sfx_podth1`.
    #[doc(alias = "sfx_podth1")]
    Podth1 = 59,
    /// Sound effect `sfx_podth2`.
    #[doc(alias = "sfx_podth2")]
    Podth2 = 60,
    /// Sound effect `sfx_podth3`.
    #[doc(alias = "sfx_podth3")]
    Podth3 = 61,
    /// Sound effect `sfx_bgdth1`.
    #[doc(alias = "sfx_bgdth1")]
    Bgdth1 = 62,
    /// Sound effect `sfx_bgdth2`.
    #[doc(alias = "sfx_bgdth2")]
    Bgdth2 = 63,
    /// Sound effect `sfx_sgtdth`.
    #[doc(alias = "sfx_sgtdth")]
    Sgtdth = 64,
    /// Sound effect `sfx_cacdth`.
    #[doc(alias = "sfx_cacdth")]
    Cacdth = 65,
    /// Sound effect `sfx_skldth`.
    #[doc(alias = "sfx_skldth")]
    Skldth = 66,
    /// Sound effect `sfx_brsdth`.
    #[doc(alias = "sfx_brsdth")]
    Brsdth = 67,
    /// Sound effect `sfx_cybdth`.
    #[doc(alias = "sfx_cybdth")]
    Cybdth = 68,
    /// Sound effect `sfx_spidth`.
    #[doc(alias = "sfx_spidth")]
    Spidth = 69,
    /// Sound effect `sfx_bspdth`.
    #[doc(alias = "sfx_bspdth")]
    Bspdth = 70,
    /// Sound effect `sfx_vildth`.
    #[doc(alias = "sfx_vildth")]
    Vildth = 71,
    /// Sound effect `sfx_kntdth`.
    #[doc(alias = "sfx_kntdth")]
    Kntdth = 72,
    /// Sound effect `sfx_pedth`.
    #[doc(alias = "sfx_pedth")]
    Pedth = 73,
    /// Sound effect `sfx_skedth`.
    #[doc(alias = "sfx_skedth")]
    Skedth = 74,
    /// Sound effect `sfx_posact`.
    #[doc(alias = "sfx_posact")]
    Posact = 75,
    /// Sound effect `sfx_bgact`.
    #[doc(alias = "sfx_bgact")]
    Bgact = 76,
    /// Sound effect `sfx_dmact`.
    #[doc(alias = "sfx_dmact")]
    Dmact = 77,
    /// Sound effect `sfx_bspact`.
    #[doc(alias = "sfx_bspact")]
    Bspact = 78,
    /// Sound effect `sfx_bspwlk`.
    #[doc(alias = "sfx_bspwlk")]
    Bspwlk = 79,
    /// Sound effect `sfx_vilact`.
    #[doc(alias = "sfx_vilact")]
    Vilact = 80,
    /// Sound effect `sfx_noway`.
    #[doc(alias = "sfx_noway")]
    Noway = 81,
    /// Sound effect `sfx_barexp`.
    #[doc(alias = "sfx_barexp")]
    Barexp = 82,
    /// Sound effect `sfx_punch`.
    #[doc(alias = "sfx_punch")]
    Punch = 83,
    /// Sound effect `sfx_hoof`.
    #[doc(alias = "sfx_hoof")]
    Hoof = 84,
    /// Sound effect `sfx_metal`.
    #[doc(alias = "sfx_metal")]
    Metal = 85,
    /// Sound effect `sfx_chgun`.
    #[doc(alias = "sfx_chgun")]
    Chgun = 86,
    /// Sound effect `sfx_tink`.
    #[doc(alias = "sfx_tink")]
    Tink = 87,
    /// Sound effect `sfx_bdopn`.
    #[doc(alias = "sfx_bdopn")]
    Bdopn = 88,
    /// Sound effect `sfx_bdcls`.
    #[doc(alias = "sfx_bdcls")]
    Bdcls = 89,
    /// Sound effect `sfx_itmbk`.
    #[doc(alias = "sfx_itmbk")]
    Itmbk = 90,
    /// Sound effect `sfx_flame`.
    #[doc(alias = "sfx_flame")]
    Flame = 91,
    /// Sound effect `sfx_flamst`.
    #[doc(alias = "sfx_flamst")]
    Flamst = 92,
    /// Sound effect `sfx_getpow`.
    #[doc(alias = "sfx_getpow")]
    Getpow = 93,
    /// Sound effect `sfx_bospit`.
    #[doc(alias = "sfx_bospit")]
    Bospit = 94,
    /// Sound effect `sfx_boscub`.
    #[doc(alias = "sfx_boscub")]
    Boscub = 95,
    /// Sound effect `sfx_bossit`.
    #[doc(alias = "sfx_bossit")]
    Bossit = 96,
    /// Sound effect `sfx_bospn`.
    #[doc(alias = "sfx_bospn")]
    Bospn = 97,
    /// Sound effect `sfx_bosdth`.
    #[doc(alias = "sfx_bosdth")]
    Bosdth = 98,
    /// Sound effect `sfx_manatk`.
    #[doc(alias = "sfx_manatk")]
    Manatk = 99,
    /// Sound effect `sfx_mandth`.
    #[doc(alias = "sfx_mandth")]
    Mandth = 100,
    /// Sound effect `sfx_sssit`.
    #[doc(alias = "sfx_sssit")]
    Sssit = 101,
    /// Sound effect `sfx_ssdth`.
    #[doc(alias = "sfx_ssdth")]
    Ssdth = 102,
    /// Sound effect `sfx_keenpn`.
    #[doc(alias = "sfx_keenpn")]
    Keenpn = 103,
    /// Sound effect `sfx_keendt`.
    #[doc(alias = "sfx_keendt")]
    Keendt = 104,
    /// Sound effect `sfx_skeact`.
    #[doc(alias = "sfx_skeact")]
    Skeact = 105,
    /// Sound effect `sfx_skesit`.
    #[doc(alias = "sfx_skesit")]
    Skesit = 106,
    /// Sound effect `sfx_skeatk`.
    #[doc(alias = "sfx_skeatk")]
    Skeatk = 107,
    /// Sound effect `sfx_radio`.
    #[doc(alias = "sfx_radio")]
    Radio = 108,
}

/// Music track IDs, matching the `musicenum_t` C enum in `sounds.h`.
#[doc(alias = "musicenum_t")]
#[repr(C)]
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Mus {
    #[default]
    /// Sentinel "no music" id.
    #[doc(alias = "mus_None")]
    None = 0,
    /// Music track `mus_e1m1`.
    #[doc(alias = "mus_e1m1")]
    E1m1 = 1,
    /// Music track `mus_e1m2`.
    #[doc(alias = "mus_e1m2")]
    E1m2 = 2,
    /// Music track `mus_e1m3`.
    #[doc(alias = "mus_e1m3")]
    E1m3 = 3,
    /// Music track `mus_e1m4`.
    #[doc(alias = "mus_e1m4")]
    E1m4 = 4,
    /// Music track `mus_e1m5`.
    #[doc(alias = "mus_e1m5")]
    E1m5 = 5,
    /// Music track `mus_e1m6`.
    #[doc(alias = "mus_e1m6")]
    E1m6 = 6,
    /// Music track `mus_e1m7`.
    #[doc(alias = "mus_e1m7")]
    E1m7 = 7,
    /// Music track `mus_e1m8`.
    #[doc(alias = "mus_e1m8")]
    E1m8 = 8,
    /// Music track `mus_e1m9`.
    #[doc(alias = "mus_e1m9")]
    E1m9 = 9,
    /// Music track `mus_e2m1`.
    #[doc(alias = "mus_e2m1")]
    E2m1 = 10,
    /// Music track `mus_e2m2`.
    #[doc(alias = "mus_e2m2")]
    E2m2 = 11,
    /// Music track `mus_e2m3`.
    #[doc(alias = "mus_e2m3")]
    E2m3 = 12,
    /// Music track `mus_e2m4`.
    #[doc(alias = "mus_e2m4")]
    E2m4 = 13,
    /// Music track `mus_e2m5`.
    #[doc(alias = "mus_e2m5")]
    E2m5 = 14,
    /// Music track `mus_e2m6`.
    #[doc(alias = "mus_e2m6")]
    E2m6 = 15,
    /// Music track `mus_e2m7`.
    #[doc(alias = "mus_e2m7")]
    E2m7 = 16,
    /// Music track `mus_e2m8`.
    #[doc(alias = "mus_e2m8")]
    E2m8 = 17,
    /// Music track `mus_e2m9`.
    #[doc(alias = "mus_e2m9")]
    E2m9 = 18,
    /// Music track `mus_e3m1`.
    #[doc(alias = "mus_e3m1")]
    E3m1 = 19,
    /// Music track `mus_e3m2`.
    #[doc(alias = "mus_e3m2")]
    E3m2 = 20,
    /// Music track `mus_e3m3`.
    #[doc(alias = "mus_e3m3")]
    E3m3 = 21,
    /// Music track `mus_e3m4`.
    #[doc(alias = "mus_e3m4")]
    E3m4 = 22,
    /// Music track `mus_e3m5`.
    #[doc(alias = "mus_e3m5")]
    E3m5 = 23,
    /// Music track `mus_e3m6`.
    #[doc(alias = "mus_e3m6")]
    E3m6 = 24,
    /// Music track `mus_e3m7`.
    #[doc(alias = "mus_e3m7")]
    E3m7 = 25,
    /// Music track `mus_e3m8`.
    #[doc(alias = "mus_e3m8")]
    E3m8 = 26,
    /// Music track `mus_e3m9`.
    #[doc(alias = "mus_e3m9")]
    E3m9 = 27,
    /// Music track `mus_inter`.
    #[doc(alias = "mus_inter")]
    Inter = 28,
    /// Music track `mus_intro`.
    #[doc(alias = "mus_intro")]
    Intro = 29,
    /// Music track `mus_bunny`.
    #[doc(alias = "mus_bunny")]
    Bunny = 30,
    /// Music track `mus_victor`.
    #[doc(alias = "mus_victor")]
    Victor = 31,
    /// Music track `mus_introa`.
    #[doc(alias = "mus_introa")]
    Introa = 32,
    /// Music track `mus_runnin`.
    #[doc(alias = "mus_runnin")]
    Runnin = 33,
    /// Music track `mus_stalks`.
    #[doc(alias = "mus_stalks")]
    Stalks = 34,
    /// Music track `mus_countd`.
    #[doc(alias = "mus_countd")]
    Countd = 35,
    /// Music track `mus_betwee`.
    #[doc(alias = "mus_betwee")]
    Betwee = 36,
    /// Music track `mus_doom`.
    #[doc(alias = "mus_doom")]
    Doom = 37,
    /// Music track `mus_the_da`.
    #[doc(alias = "mus_the_da")]
    TheDa = 38,
    /// Music track `mus_shawn`.
    #[doc(alias = "mus_shawn")]
    Shawn = 39,
    /// Music track `mus_ddtblu`.
    #[doc(alias = "mus_ddtblu")]
    Ddtblu = 40,
    /// Music track `mus_in_cit`.
    #[doc(alias = "mus_in_cit")]
    InCit = 41,
    /// Music track `mus_dead`.
    #[doc(alias = "mus_dead")]
    Dead = 42,
    /// Music track `mus_stlks2`.
    #[doc(alias = "mus_stlks2")]
    Stlks2 = 43,
    /// Music track `mus_theda2`.
    #[doc(alias = "mus_theda2")]
    Theda2 = 44,
    /// Music track `mus_doom2`.
    #[doc(alias = "mus_doom2")]
    Doom2 = 45,
    /// Music track `mus_ddtbl2`.
    #[doc(alias = "mus_ddtbl2")]
    Ddtbl2 = 46,
    /// Music track `mus_runni2`.
    #[doc(alias = "mus_runni2")]
    Runni2 = 47,
    /// Music track `mus_dead2`.
    #[doc(alias = "mus_dead2")]
    Dead2 = 48,
    /// Music track `mus_stlks3`.
    #[doc(alias = "mus_stlks3")]
    Stlks3 = 49,
    /// Music track `mus_romero`.
    #[doc(alias = "mus_romero")]
    Romero = 50,
    /// Music track `mus_shawn2`.
    #[doc(alias = "mus_shawn2")]
    Shawn2 = 51,
    /// Music track `mus_messag`.
    #[doc(alias = "mus_messag")]
    Messag = 52,
    /// Music track `mus_count2`.
    #[doc(alias = "mus_count2")]
    Count2 = 53,
    /// Music track `mus_ddtbl3`.
    #[doc(alias = "mus_ddtbl3")]
    Ddtbl3 = 54,
    /// Music track `mus_ampie`.
    #[doc(alias = "mus_ampie")]
    Ampie = 55,
    /// Music track `mus_theda3`.
    #[doc(alias = "mus_theda3")]
    Theda3 = 56,
    /// Music track `mus_adrian`.
    #[doc(alias = "mus_adrian")]
    Adrian = 57,
    /// Music track `mus_messg2`.
    #[doc(alias = "mus_messg2")]
    Messg2 = 58,
    /// Music track `mus_romer2`.
    #[doc(alias = "mus_romer2")]
    Romer2 = 59,
    /// Music track `mus_tense`.
    #[doc(alias = "mus_tense")]
    Tense = 60,
    /// Music track `mus_shawn3`.
    #[doc(alias = "mus_shawn3")]
    Shawn3 = 61,
    /// Music track `mus_openin`.
    #[doc(alias = "mus_openin")]
    Openin = 62,
    /// Music track `mus_evil`.
    #[doc(alias = "mus_evil")]
    Evil = 63,
    /// Music track `mus_ultima`.
    #[doc(alias = "mus_ultima")]
    Ultima = 64,
    /// Music track `mus_read_m`.
    #[doc(alias = "mus_read_m")]
    ReadM = 65,
    /// Music track `mus_dm2ttl`.
    #[doc(alias = "mus_dm2ttl")]
    Dm2ttl = 66,
    /// Music track `mus_dm2int`.
    #[doc(alias = "mus_dm2int")]
    Dm2int = 67,
}
#[cfg(test)]
mod tests {
    use crate::doom::sounds::{NUMMUSIC, NUMSFX};

    /// `NUMSFX` must stay at 109, matching the C `sfxenum_t`.
    #[test]
    fn numsfx_matches_c_source() {
        assert_eq!(NUMSFX, 109);
    }

    /// `NUMMUSIC` must stay at 68, matching the C `musicenum_t`.
    #[test]
    fn nummusic_matches_c_source() {
        assert_eq!(NUMMUSIC, 68);
    }
}
