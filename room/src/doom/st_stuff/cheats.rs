//! The cheat-code surface: the ten `#[no_mangle]` cheat-sequence
//! statics, the compile-time `cheatseq_t` constructors, and
//! `responder` -- the event-driven cheat feeder plus the automap-state
//! event receiver.

use std::ffi::c_char;
use std::ffi::c_int;

use super::consts::{
    AM_MSGENTERED, AM_MSGEXITED, AM_MSGHEADER, STSTR_BEHOLD, STSTR_BEHOLDX, STSTR_CHOPPERS,
    STSTR_CLEV, STSTR_DQDOFF, STSTR_DQDON, STSTR_FAADDED, STSTR_KFAADDED, STSTR_MUS, STSTR_NCOFF,
    STSTR_NCON, STSTR_NOMUS,
};
use super::{DEH_String, logical_gamemission, plyr, st_firsttime, st_gamestate};
use crate::c_write;
use crate::doom::d_event::event_t;
use crate::doom::d_mode;
use crate::doom::d_player::{NUMAMMO, NUMCARDS, NUMWEAPONS};
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::g_game::G_DeferedInitNew;
use crate::doom::g_game::{consoleplayer, gameskill, netgame, players};
use crate::doom::m_cheat::{cheatseq_t, cht_CheckCheat, cht_GetParam};
use crate::doom::p_inter::P_GivePower;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_ChangeMusic;

/// Copy a byte slice into a 25-element `c_char` array, padding the remainder with zeros.
///
/// Used at compile time to initialise cheat-sequence buffers inside `cheatseq_t`.
const fn make_cheat_seq(seq: &[u8]) -> [c_char; 25] {
    let mut arr = [0i8; 25];
    let mut i = 0;
    while i < seq.len() {
        arr[i] = seq[i] as c_char;
        i += 1;
    }
    arr
}

/// Construct a `cheatseq_t` from a key sequence and a parameter character count.
///
/// All runtime state fields (`chars_read`, `param_chars_read`, `parameter_buf`) are
/// zeroed; they will be updated by `cht_CheckCheat` during gameplay.
const fn cheat(seq: &[u8], params: c_int) -> cheatseq_t {
    cheatseq_t {
        sequence: make_cheat_seq(seq),
        sequence_len: seq.len(),
        parameter_chars: params,
        chars_read: 0,
        param_chars_read: 0,
        parameter_buf: [0; 5],
    }
}

/// Cheat sequence for changing the playing music track (`idmus##`).
///
/// Exported for C linkage. Two parameter characters encode the track number.
#[no_mangle]
pub static mut cheat_mus: cheatseq_t = cheat(b"idmus", 2);

/// Cheat sequence for toggling god mode (`iddqd`).
///
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_god: cheatseq_t = cheat(b"iddqd", 0);

/// Cheat sequence for full ammo and armor, including all keys (`idkfa`).
///
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_ammo: cheatseq_t = cheat(b"idkfa", 0);

/// Cheat sequence for full ammo and armor, without keys (`idfa`).
///
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_ammonokey: cheatseq_t = cheat(b"idfa", 0);

/// No-clip cheat sequence for Doom episode maps (`idspispopd`).
///
/// Exported for C linkage. Only active when `logical_gamemission() == doom`.
#[no_mangle]
pub static mut cheat_noclip: cheatseq_t = cheat(b"idspispopd", 0);

/// No-clip cheat sequence for Doom II maps (`idclip`).
///
/// Exported for C linkage. Active when `logical_gamemission() != doom`.
#[no_mangle]
pub static mut cheat_commercial_noclip: cheatseq_t = cheat(b"idclip", 0);

/// Power-up cheat sequences (`idbeholdv/s/i/r/a/l` and the bare `idbehold` prefix).
///
/// Indices 0-5 correspond to the six togglable power-ups
/// (invulnerability, berserk, invisibility, radiation suit, automap, light amp).
/// Index 6 is the bare `idbehold` prefix which displays the prompt.
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_powerup: [cheatseq_t; 7] = [
    cheat(b"idbeholdv", 0),
    cheat(b"idbeholds", 0),
    cheat(b"idbeholdi", 0),
    cheat(b"idbeholdr", 0),
    cheat(b"idbeholda", 0),
    cheat(b"idbeholdl", 0),
    cheat(b"idbehold", 0),
];

/// Cheat sequence that gives the chainsaw and invulnerability (`idchoppers`).
///
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_choppers: cheatseq_t = cheat(b"idchoppers", 0);

/// Cheat sequence for warping to a specific episode+map (`idclev##`).
///
/// Two parameter characters encode the destination. Exported for C linkage.
#[no_mangle]
pub static mut cheat_clev: cheatseq_t = cheat(b"idclev", 2);

/// Cheat sequence that prints the player's current map position (`idmypos`).
///
/// Exported for C linkage.
#[no_mangle]
pub static mut cheat_mypos: cheatseq_t = cheat(b"idmypos", 0);

/// Handle status-bar-related input events: automap state changes and cheat codes.
///
/// Returns 1 if the event was consumed, 0 otherwise. The C original
/// (`ST_Responder` in `st_stuff.c`) is called by `G_Responder` in `g_game.c`.
///
/// Cheat codes are suppressed in network games and on the Nightmare skill level.
/// The level-change cheat (`idclev`) is suppressed in network games regardless of
/// skill.
///
/// # Safety
///
/// Caller must ensure `ev` is a valid, non-null, properly aligned pointer to an
/// initialised `event_t`. Reads and mutates global cheat-sequence state and the
/// status-bar game state (`st_gamestate`, `st_firsttime`, etc.); caller must
/// ensure no concurrent access to these globals.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs` and `am_map`'s AM_MSG forwarders import the
/// upstream name through the root shim.
#[doc(alias = "ST_Responder")]
#[export_name = "ST_Responder"]
pub unsafe extern "C" fn responder(ev: *mut event_t) -> c_int {
    let ev = &*ev;

    if ev.type_ == 1 && (ev.data1 as u32 & 0xffff0000) == AM_MSGHEADER as u32 {
        // ev_keyup + automap message
        match ev.data1 {
            AM_MSGENTERED => {
                st_gamestate = 0; // AutomapState
                st_firsttime = 1;
            }
            AM_MSGEXITED => {
                st_gamestate = 1; // FirstPersonState
            }
            _ => {}
        }
    } else if ev.type_ == 0 {
        // ev_keydown
        if netgame == 0 && gameskill != 4 {
            // sk_nightmare = 4
            if cht_CheckCheat(&raw mut cheat_god, ev.data2 as c_char) != 0 {
                (*plyr).cheats ^= 2; // CF_GODMODE
                if (*plyr).cheats & 2 != 0 {
                    if !(*plyr).mo.is_null() {
                        (*((*plyr).mo as *mut mobj_t)).health = 100;
                    }
                    (*plyr).health = 100; // deh_god_mode_health
                    (*plyr).message = DEH_String(STSTR_DQDON);
                } else {
                    (*plyr).message = DEH_String(STSTR_DQDOFF);
                }
            } else if cht_CheckCheat(&raw mut cheat_ammonokey, ev.data2 as c_char) != 0 {
                (*plyr).armorpoints = 200; // deh_idfa_armor
                (*plyr).armortype = 2; // deh_idfa_armor_class
                for i in 0..NUMWEAPONS {
                    (*plyr).weaponowned[i] = 1;
                }
                for i in 0..NUMAMMO {
                    (*plyr).ammo[i] = (*plyr).maxammo[i];
                }
                (*plyr).message = DEH_String(STSTR_FAADDED);
            } else if cht_CheckCheat(&raw mut cheat_ammo, ev.data2 as c_char) != 0 {
                (*plyr).armorpoints = 200; // deh_idkfa_armor
                (*plyr).armortype = 2; // deh_idkfa_armor_class
                for i in 0..NUMWEAPONS {
                    (*plyr).weaponowned[i] = 1;
                }
                for i in 0..NUMAMMO {
                    (*plyr).ammo[i] = (*plyr).maxammo[i];
                }
                for i in 0..NUMCARDS {
                    (*plyr).cards[i] = 1;
                }
                (*plyr).message = DEH_String(STSTR_KFAADDED);
            } else if cht_CheckCheat(&raw mut cheat_mus, ev.data2 as c_char) != 0 {
                let mut buf = [0i8; 3];
                let musnum: c_int;
                (*plyr).message = DEH_String(STSTR_MUS);
                cht_GetParam(&raw mut cheat_mus, buf.as_mut_ptr());

                if gamemode == d_mode::commercial || gameversion < d_mode::exe_ultimate {
                    musnum = 33
                        + (buf[0] as c_int - '0' as c_int) * 10
                        + (buf[1] as c_int - '0' as c_int)
                        - 1;
                    if ((buf[0] as c_int - '0' as c_int) * 10 + (buf[1] as c_int - '0' as c_int))
                        > 35
                    {
                        (*plyr).message = DEH_String(STSTR_NOMUS);
                    } else {
                        S_ChangeMusic(musnum, 1);
                    }
                } else {
                    musnum =
                        1 + (buf[0] as c_int - '1' as c_int) * 9 + (buf[1] as c_int - '1' as c_int);
                    if ((buf[0] as c_int - '1' as c_int) * 9 + (buf[1] as c_int - '1' as c_int))
                        > 31
                    {
                        (*plyr).message = DEH_String(STSTR_NOMUS);
                    } else {
                        S_ChangeMusic(musnum, 1);
                    }
                }
            } else if (logical_gamemission() == d_mode::doom
                && cht_CheckCheat(&raw mut cheat_noclip, ev.data2 as c_char) != 0)
                || (logical_gamemission() != d_mode::doom
                    && cht_CheckCheat(&raw mut cheat_commercial_noclip, ev.data2 as c_char) != 0)
            {
                (*plyr).cheats ^= 1; // CF_NOCLIP
                if (*plyr).cheats & 1 != 0 {
                    (*plyr).message = DEH_String(STSTR_NCON);
                } else {
                    (*plyr).message = DEH_String(STSTR_NCOFF);
                }
            }

            for i in 0..6 {
                if cht_CheckCheat(&mut cheat_powerup[i], ev.data2 as c_char) != 0 {
                    if (*plyr).powers[i] == 0 {
                        P_GivePower(plyr, i as c_int);
                    } else if i != 1 {
                        // pw_strength = 1
                        (*plyr).powers[i] = 1;
                    } else {
                        (*plyr).powers[i] = 0;
                    }
                    (*plyr).message = DEH_String(STSTR_BEHOLDX);
                }
            }

            if cht_CheckCheat(&mut cheat_powerup[6], ev.data2 as c_char) != 0 {
                (*plyr).message = DEH_String(STSTR_BEHOLD);
            } else if cht_CheckCheat(&raw mut cheat_choppers, ev.data2 as c_char) != 0 {
                (*plyr).weaponowned[7] = 1; // wp_chainsaw
                (*plyr).powers[0] = 1; // pw_invulnerability
                (*plyr).message = DEH_String(STSTR_CHOPPERS);
            } else if cht_CheckCheat(&raw mut cheat_mypos, ev.data2 as c_char) != 0 {
                static mut BUF: [c_char; 52] = [0; 52];
                let mo = players[consoleplayer as usize].mo as *mut mobj_t;
                c_write!(
                    BUF,
                    "ang=0x{:x};x,y=(0x{:x},0x{:x})",
                    (*mo).angle,
                    (*mo).x,
                    (*mo).y
                );
                (*plyr).message = std::ptr::addr_of_mut!(BUF[0]);
            }
        }

        if netgame == 0 && cht_CheckCheat(&raw mut cheat_clev, ev.data2 as c_char) != 0 {
            let mut buf = [0i8; 3];
            let mut epsd: c_int;
            let map: c_int;
            cht_GetParam(&raw mut cheat_clev, buf.as_mut_ptr());

            if gamemode == d_mode::commercial {
                epsd = 1;
                map = (buf[0] as c_int - '0' as c_int) * 10 + (buf[1] as c_int - '0' as c_int);
            } else {
                epsd = buf[0] as c_int - '0' as c_int;
                map = buf[1] as c_int - '0' as c_int;
            }

            if gameversion == d_mode::exe_chex {
                epsd = 1;
            }

            if epsd < 1 {
                return 0;
            }
            if map < 1 {
                return 0;
            }
            if gamemode == d_mode::retail && (epsd > 4 || map > 9) {
                return 0;
            }
            if gamemode == d_mode::registered && (epsd > 3 || map > 9) {
                return 0;
            }
            if gamemode == d_mode::shareware && (epsd > 1 || map > 9) {
                return 0;
            }
            if gamemode == d_mode::commercial && (epsd > 1 || map > 40) {
                return 0;
            }

            (*plyr).message = DEH_String(STSTR_CLEV);
            G_DeferedInitNew(gameskill, epsd, map);
        }
    }

    0
}
