//! The finale text and cast tables: the per-mission text-screen strings
//! (`E1TEXT`..`P6TEXT`), the cast-roll display names (`CC_*`), the
//! `TEXTSCREENS` matching table, the `CASTORDER` roll order, and the
//! text pacing constants. Data tier -- tables keep their upstream
//! shapes (`textscreens[]`/`castorder[]` in f_finale.c).

use std::ffi::{c_char, c_int};

use super::d_mode;
use crate::doom::info::{
    MT_BABY, MT_BRUISER, MT_CHAINGUY, MT_CYBORG, MT_FATSO, MT_HEAD, MT_KNIGHT, MT_PAIN,
    MT_PLAYER, MT_POSSESSED, MT_SERGEANT, MT_SHOTGUY, MT_SKULL, MT_SPIDER, MT_TROOP, MT_UNDEAD,
    MT_VILE,
};

/// Number of game ticks a single text character takes to appear on screen.
///
/// C origin: `TEXTSPEED` in f_finale.c.
pub(super) const TEXTSPEED: c_int = 3;

/// Number of extra ticks to wait after all text has been displayed before
/// advancing to the art-screen stage.
///
/// C origin: `TEXTWAIT` in f_finale.c.
pub(super) const TEXTWAIT: c_int = 250;

/// Episode 1 finale text shown after defeating the boss on E1M8.
const E1TEXT: *mut c_char = c"Once you beat the big badasses and\nclean out the moon base you're supposed\nto win, aren't you? Aren't you? Where's\nyour fat reward and ticket home? What\nthe hell is this? It's not supposed to\nend this way!\n\nIt stinks like rotten meat, but looks\nlike the lost Deimos base.  Looks like\nyou're stuck on The Shores of Hell.\nThe only way out is through.\n\nTo continue the DOOM experience, play\nThe Shores of Hell and its amazing\nsequel, Inferno!".as_ptr().cast_mut();

/// Episode 2 finale text shown after defeating the boss on E2M8.
const E2TEXT: *mut c_char = c"You've done it! The hideous cyber-\ndemon lord that ruled the lost Deimos\nmoon base has been slain and you\ntriumph over the hordes of hell.\nThe mission is not complete, however.\nThe loathsome vomit of hell still\noozes from the nether regions of\nDeimos.\n\nThe demon spawner, the source of the\nhellish invasion, remains active.\nYou must find it and shut it down.\n\nTo continue the DOOM experience,\nplay Inferno!".as_ptr().cast_mut();

/// Episode 3 finale text shown after defeating the boss on E3M8.
const E3TEXT: *mut c_char = c"The loathsome spiderdemon that\nmaster-minded the invasion of the moon\nbase and caused so much death has had\nits ass kicked for all time.\n\nA hidden doorway opens and you begin\nthe long trek back to the surface.\nThe sensual scent of flowers tickles\nyour nose and you smile.\n\nBut wait! The gateway is open, and\nthe demons of hell are pouring\nthrough! You wonder how you'll ever\nget home.\n\nA demon consumes your flesh.\n\nThe End.\n\n(Well, not really.  To continue the\nDOOM experience, play Thy Flesh\nConsumed!)".as_ptr().cast_mut();

/// Episode 4 finale text shown after defeating the boss on E4M8.
const E4TEXT: *mut c_char = c"The spider mastermind must have sent forth\nits legions of hellspawn before your\nfinal confrontation with that terrible\nbeast from netherworld.  But you stepped\nforward and brought forth eternal damnation\nand suffering upon the horde as a true\nhero would in the face of something so\nevil.\n\nBesides, someone was gonna pay for what\nhappened to daisy, your pet rabbit.\n\nBut now, you see spread before you more\npotential pain and gibbitude as a nation\nof demons run amok among our cities.\n\nNext stop, hell on earth!".as_ptr().cast_mut();

/// Doom II level 6 inter-level text (first story block).
const C1TEXT: *mut c_char = c"YOU HAVE ENTERED DEEPLY INTO THE INFESTED\nSTARPORT. BUT SOMETHING IS WRONG. THE\nMONSTERS HAVE BROUGHT THEIR OWN REALITY\nWITH THEM, AND THE STARPORT'S TECHNOLOGY\nIS BEING SUBVERTED BY THEIR PRESENCE.\n\nAHEAD, YOU SEE AN OUTPOST OF HELL, A\nFORTIFIED ZONE. IF YOU CAN GET PAST IT,\nYOU CAN PENETRATE INTO THE HAUNTED HEART\nOF THE STARBASE AND FIND THE CONTROLLING\nSWITCH WHICH HOLDS EARTH'S POPULATION\nHOSTAGE.".as_ptr().cast_mut();

/// Doom II level 11 inter-level text.
const C2TEXT: *mut c_char = c"YOU HAVE WON! YOUR VICTORY HAS ENABLED\nHUMANKIND TO EVACUATE EARTH AND ESCAPE\nTHE NIGHTMARE.  NOW YOU ARE THE ONLY\nHUMAN LEFT ON THE FACE OF THE PLANET.\nCAN YOU FIND YOUR WAY BACK TO HAPPY\nREALITY?\n\nOR ARE YOU DOOMED TO ROAM ETERNAL\nAMONG THE DEMONS?".as_ptr().cast_mut();

/// Doom II level 20 inter-level text.
const C3TEXT: *mut c_char = c"YOU ARE AT THE CORRUPT HEART OF THE CITY,\nSURROUNDED BY THE CORPSES OF YOUR ENEMIES.\nYOU SEE NO WAY TO ESCAPE FROM THIS FUTURE\nHELL, BUT YOU MAY DELAY THE DAMNATION OF\nHUMANITY BY THROWING YOURSELF INTO THE\nPORTAL, AND HEADING OFF THE DEMONIC\nINVASION AT ITS SOURCE.".as_ptr().cast_mut();

/// Doom II level 30 inter-level text.
const C4TEXT: *mut c_char = c"SENSIBLE, NO?\n\nTHERE WAS NO WAY YOU COULD SURVIVE THIS\nHELL, BUT YOU HAVE SUCCEEDED IN SPOILING\nTHE DEMONS' PLANS.  THE HAZARDOUS-WASTE\nFACILITY HAS BEEN DESTROYED AND HELL'S\nPORTAL HAS BEEN SEALED.\n\nYOU ARE THE ONLY SURVIVOR, BUT THE BATTLE\nCONTINUES ELSEWHERE.  EARTH REMAINS UNDER\nSIEGE, AND THE HELLSPAWN PROWL THE\nSTREETS IN SEARCH OF MORE PREY.\n\nTHE INVASION IS FAR FROM OVER.".as_ptr().cast_mut();

/// Doom II level 15 (secret exit) inter-level text.
const C5TEXT: *mut c_char = c"BUT WAIT!  THERE'S MORE!\n\nIT'S BACK TO THE PITS OF HELL FOR YOU,\nTO FACE MORE DEMONS, MORE HELLSPAWN, AND\nMORE HIDEOUS ACTS OF EVIL.\n\nIT'S A DIRTY JOB, BUT SOMEONE'S GOT TO\nDO IT.  AND THAT SOMEONE IS YOU.".as_ptr().cast_mut();

/// Doom II level 31 inter-level text.
const C6TEXT: *mut c_char = c"CONGRATULATIONS!\n\nYOU HAVE FOUND THE SECRET LEVEL!\n\nHOPEFULLY YOU FOUND THE PLASMA GUN.\n\nTHE DEMON HORDE IS ABOUT TO GET A WAKE-UP\nCALL.".as_ptr().cast_mut();

/// TNT: Evilution level 6 inter-level text.
const T1TEXT: *mut c_char = c"You've fought your way out of the infested\nexperimental labs.   It seems that UAC has\nonce again gulped it down.  Ahead lies\ntheir central complex, now firmly in the\ngrasp of the demon hordes.  Perhaps by\nsabotaging their primary teleporter you\ncan halt the invasion.".as_ptr().cast_mut();

/// TNT: Evilution level 11 inter-level text.
const T2TEXT: *mut c_char = c"The demon spawner you've found appears to\nhave been activated.  The Demons are\npouring through in endless waves.  You\nneed to find a way to deactivate it,\nfast!".as_ptr().cast_mut();

/// TNT: Evilution level 20 inter-level text.
const T3TEXT: *mut c_char = c"The river of blood spills over into the\nnext area.  It seems your arrival hasn't\ngone unnoticed.  Ahead lies the most\ninfested region of the complex.  You must\nfind a way to stem the tide of demons, or\ndie trying.".as_ptr().cast_mut();

/// TNT: Evilution level 30 inter-level text.
const T4TEXT: *mut c_char = c"The stench of rotten flesh and sulfur\nfills the air.  You have reached the\nheart of the infested complex.  Somewhere\nbeyond the next portal lies the Demon\nSpawner itself.  If you can survive long\nenough to find it, you may be able to\nturn the tide of this war.".as_ptr().cast_mut();

/// TNT: Evilution level 15 inter-level text.
const T5TEXT: *mut c_char = c"You've done it!  The hideous Spiderdemon\nthat masterminded the invasion is dead.\nBut the demon spawner still remains,\nand the forces of hell are still pouring\nthrough.  You need to find the primary\nteleporter and destroy it.".as_ptr().cast_mut();

/// TNT: Evilution level 31 inter-level text.
const T6TEXT: *mut c_char = c"The primary teleporter is destroyed, but\nthe forces of hell are still pouring in.\nYou need to find the secondary teleporter\nand shut it down.  The fate of Earth\ndepends on it.".as_ptr().cast_mut();

/// Plutonia Experiment level 6 inter-level text.
const P1TEXT: *mut c_char = c"You gloat over the steaming carcass of the\nGuardian.  With its death, you've wrested\nthe Accelerator from the stinking claws\nof Hell.  You relax and glance around\nthe room.  Damn!  There was supposed to\nbe a bridge around here somewhere!  Did\nthe Invaders sense your victory and\nwithdraw the bridge to prevent your\nescape?\n\nYou hear the sound of claws on stone.\nYou frantically grab your pistol and\ndive for the door, but it's too late.\nThe Demons have arrived.".as_ptr().cast_mut();

/// Plutonia Experiment level 11 inter-level text.
const P2TEXT: *mut c_char = c"You did it!  The hideous Spiderdemon\nthat masterminded the invasion is dead.\nBut the demon spawner still remains,\nand the forces of hell are still pouring\nthrough.  You need to find the primary\nteleporter and destroy it.".as_ptr().cast_mut();

/// Plutonia Experiment level 20 inter-level text.
const P3TEXT: *mut c_char = c"The Vile presence fades.  You feel a\nsense of relief, but it is short lived.\nYou still must find the demon spawner\nand shut it down.  Time is running out.".as_ptr().cast_mut();

/// Plutonia Experiment level 30 inter-level text.
const P4TEXT: *mut c_char = c"The demon spawner lies in ruins before\nyou.  The forces of hell are in full\nretreat, and the invasion is stopped.\nYou step onto the teleporter, eager to\nreturn home and bask in the glory of\nyour victory.".as_ptr().cast_mut();

/// Plutonia Experiment level 15 inter-level text.
const P5TEXT: *mut c_char = c"You have survived the horrors of the\ninfested complex and emerged victorious.\nThe demon spawner lies in ruins, and the\nforces of hell have been driven back.\nYou step onto the teleporter, ready to\nreturn Earth and face whatever\nchallenges lie ahead.".as_ptr().cast_mut();

/// Plutonia Experiment level 31 inter-level text.
const P6TEXT: *mut c_char = c"The primary teleporter is destroyed, but\nthe forces of hell are still pouring in.\nYou need to find the secondary teleporter\nand shut it down.  The fate of Earth\ndepends on it.".as_ptr().cast_mut();

// Cast names (from d_englsh.h)

/// Cast-roll name for the Zombieman.
const CC_ZOMBIE: *mut c_char = c"ZOMBIEMAN".as_ptr().cast_mut();
/// Cast-roll name for the Shotgun Guy.
const CC_SHOTGUN: *mut c_char = c"SHOTGUN GUY".as_ptr().cast_mut();
/// Cast-roll name for the Heavy Weapon Dude (chaingunner).
const CC_HEAVY: *mut c_char = c"HEAVY WEAPON DUDE".as_ptr().cast_mut();
/// Cast-roll name for the Imp.
const CC_IMP: *mut c_char = c"IMP".as_ptr().cast_mut();
/// Cast-roll name for the Demon.
const CC_DEMON: *mut c_char = c"DEMON".as_ptr().cast_mut();
/// Cast-roll name for the Lost Soul.
const CC_LOST: *mut c_char = c"LOST SOUL".as_ptr().cast_mut();
/// Cast-roll name for the Cacodemon.
const CC_CACO: *mut c_char = c"CACODEMON".as_ptr().cast_mut();
/// Cast-roll name for the Hell Knight.
const CC_HELL: *mut c_char = c"HELL KNIGHT".as_ptr().cast_mut();
/// Cast-roll name for the Baron of Hell.
const CC_BARON: *mut c_char = c"BARON OF HELL".as_ptr().cast_mut();
/// Cast-roll name for the Arachnotron.
const CC_ARACH: *mut c_char = c"ARACHNOTRON".as_ptr().cast_mut();
/// Cast-roll name for the Pain Elemental.
const CC_PAIN: *mut c_char = c"PAIN ELEMENTAL".as_ptr().cast_mut();
/// Cast-roll name for the Revenant.
const CC_REVEN: *mut c_char = c"REVENANT".as_ptr().cast_mut();
/// Cast-roll name for the Mancubus.
const CC_MANCU: *mut c_char = c"MANCUBUS".as_ptr().cast_mut();
/// Cast-roll name for the Arch-Vile.
const CC_ARCH: *mut c_char = c"ARCH-VILE".as_ptr().cast_mut();
/// Cast-roll name for the Spider Mastermind.
const CC_SPIDER: *mut c_char = c"THE SPIDER MASTERMIND".as_ptr().cast_mut();
/// Cast-roll name for the Cyberdemon.
const CC_CYBER: *mut c_char = c"THE CYBERDEMON".as_ptr().cast_mut();
/// Cast-roll name for the player character.
const CC_HERO: *mut c_char = c"OUR HERO".as_ptr().cast_mut();

/// Maps an (mission, episode, level) tuple to the background flat name and
/// text string displayed after completing that level.
///
/// C origin: `textscreens[]` in f_finale.c.
#[repr(C)]
pub(super) struct TextScreen {
    /// Which game mission this entry applies to (e.g. `d_mode::doom`).
    pub mission: c_int,
    /// Which episode this entry applies to (Doom only; Doom II uses episode 1 for all).
    pub episode: c_int,
    /// The map number after which this text appears.
    pub level: c_int,
    /// Name of the WAD flat lump used as the tiling background.
    pub background: *mut c_char,
    /// The NUL-terminated text string to display.
    pub text: *mut c_char,
}

/// Pairs a cast-roll display name with the `mobjtype_t` index of the enemy.
///
/// The last entry in [`CASTORDER`] has a null `name` pointer as a sentinel.
/// C origin: `castinfo_t` / `castorder[]` in f_finale.c.
#[repr(C)]
pub(super) struct CastInfo {
    /// NUL-terminated display name shown at the bottom of the cast screen.
    pub name: *mut c_char,
    /// `mobjtype_t` index into `mobjinfo[]`; selects sprite and state machine.
    pub type_: c_int,
}

/// Episode/level to text-screen mapping for all supported IWADs.
///
/// Searched linearly in [`super::lifecycle::start_finale`] to find the
/// matching entry for the current game mission, episode, and map.  The Chex
/// Quest hack adjusts matching level from 8 to 5 inline.  C origin:
/// `textscreens[]` in f_finale.c.
pub(super) const TEXTSCREENS: [TextScreen; 22] = [
    TextScreen {
        mission: d_mode::doom,
        episode: 1,
        level: 8,
        background: c"FLOOR4_8".as_ptr().cast_mut(),
        text: E1TEXT,
    },
    TextScreen {
        mission: d_mode::doom,
        episode: 2,
        level: 8,
        background: c"SFLR6_1".as_ptr().cast_mut(),
        text: E2TEXT,
    },
    TextScreen {
        mission: d_mode::doom,
        episode: 3,
        level: 8,
        background: c"MFLR8_4".as_ptr().cast_mut(),
        text: E3TEXT,
    },
    TextScreen {
        mission: d_mode::doom,
        episode: 4,
        level: 8,
        background: c"MFLR8_3".as_ptr().cast_mut(),
        text: E4TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 6,
        background: c"SLIME16".as_ptr().cast_mut(),
        text: C1TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 11,
        background: c"RROCK14".as_ptr().cast_mut(),
        text: C2TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 20,
        background: c"RROCK07".as_ptr().cast_mut(),
        text: C3TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 30,
        background: c"RROCK17".as_ptr().cast_mut(),
        text: C4TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 15,
        background: c"RROCK13".as_ptr().cast_mut(),
        text: C5TEXT,
    },
    TextScreen {
        mission: d_mode::doom2,
        episode: 1,
        level: 31,
        background: c"RROCK19".as_ptr().cast_mut(),
        text: C6TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 6,
        background: c"SLIME16".as_ptr().cast_mut(),
        text: T1TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 11,
        background: c"RROCK14".as_ptr().cast_mut(),
        text: T2TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 20,
        background: c"RROCK07".as_ptr().cast_mut(),
        text: T3TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 30,
        background: c"RROCK17".as_ptr().cast_mut(),
        text: T4TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 15,
        background: c"RROCK13".as_ptr().cast_mut(),
        text: T5TEXT,
    },
    TextScreen {
        mission: d_mode::pack_tnt,
        episode: 1,
        level: 31,
        background: c"RROCK19".as_ptr().cast_mut(),
        text: T6TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 6,
        background: c"SLIME16".as_ptr().cast_mut(),
        text: P1TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 11,
        background: c"RROCK14".as_ptr().cast_mut(),
        text: P2TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 20,
        background: c"RROCK07".as_ptr().cast_mut(),
        text: P3TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 30,
        background: c"RROCK17".as_ptr().cast_mut(),
        text: P4TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 15,
        background: c"RROCK13".as_ptr().cast_mut(),
        text: P5TEXT,
    },
    TextScreen {
        mission: d_mode::pack_plut,
        episode: 1,
        level: 31,
        background: c"RROCK19".as_ptr().cast_mut(),
        text: P6TEXT,
    },
];

/// Ordered list of enemies shown in the Doom II cast-of-characters roll.
///
/// The last entry is a sentinel with a null `name` pointer.  C origin:
/// `castorder[]` in f_finale.c.
pub(super) const CASTORDER: [CastInfo; 18] = [
    CastInfo {
        name: CC_ZOMBIE,
        type_: MT_POSSESSED,
    },
    CastInfo {
        name: CC_SHOTGUN,
        type_: MT_SHOTGUY,
    },
    CastInfo {
        name: CC_HEAVY,
        type_: MT_CHAINGUY,
    },
    CastInfo {
        name: CC_IMP,
        type_: MT_TROOP,
    },
    CastInfo {
        name: CC_DEMON,
        type_: MT_SERGEANT,
    },
    CastInfo {
        name: CC_LOST,
        type_: MT_SKULL,
    },
    CastInfo {
        name: CC_CACO,
        type_: MT_HEAD,
    },
    CastInfo {
        name: CC_HELL,
        type_: MT_KNIGHT,
    },
    CastInfo {
        name: CC_BARON,
        type_: MT_BRUISER,
    },
    CastInfo {
        name: CC_ARACH,
        type_: MT_BABY,
    },
    CastInfo {
        name: CC_PAIN,
        type_: MT_PAIN,
    },
    CastInfo {
        name: CC_REVEN,
        type_: MT_UNDEAD,
    },
    CastInfo {
        name: CC_MANCU,
        type_: MT_FATSO,
    },
    CastInfo {
        name: CC_ARCH,
        type_: MT_VILE,
    },
    CastInfo {
        name: CC_SPIDER,
        type_: MT_SPIDER,
    },
    CastInfo {
        name: CC_CYBER,
        type_: MT_CYBORG,
    },
    CastInfo {
        name: CC_HERO,
        type_: MT_PLAYER,
    },
    CastInfo {
        name: std::ptr::null_mut(),
        type_: 0,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textspeed_is_3() {
        assert_eq!(TEXTSPEED, 3);
    }

    #[test]
    fn textwait_is_250() {
        assert_eq!(TEXTWAIT, 250);
    }

    #[test]
    fn textwait_longer_than_textspeed() {
        assert!(TEXTWAIT > TEXTSPEED);
    }
}
