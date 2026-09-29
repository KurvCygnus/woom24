//! The C-header constants: blockmap math, slope types, WAD map lump
//! indices (`MapLump`), linedef flag bits (`LinedefFlag`), and the
//! per-subsystem tunables the freeze-zone consumers read through the
//! hub, all names kept byte-for-byte.

#![allow(non_snake_case)]

use std::ffi::{c_int, c_uint};

use crate::doom::i_timer::TICRATE;
use crate::doom::i_video::SCREENHEIGHT;
use crate::doom::m_fixed::{FRACBITS, FRACUNIT};
use crate::doom::tables::ANG90;

// ---------------------------------------------------------------------------
// Constants from C headers
// ---------------------------------------------------------------------------

/// Width/height of one blockmap cell in map units (`MAPBLOCKUNITS` in
/// `p_local.h`).
pub const MAPBLOCKUNITS: c_int = 128;
/// Width/height of one blockmap cell in fixed-point units (`MAPBLOCKSIZE`
/// in `p_local.h`).
pub const MAPBLOCKSIZE: c_int = MAPBLOCKUNITS * FRACUNIT;
/// Right-shift count converting a fixed-point coordinate to a blockmap
/// cell index (`MAPBLOCKSHIFT` in `p_local.h`).
pub const MAPBLOCKSHIFT: c_int = FRACBITS as c_int + 7;
/// Mask isolating the in-cell offset of a fixed-point coordinate
/// (`MAPBMASK` in `p_local.h`).
pub const MAPBMASK: c_int = MAPBLOCKSIZE - 1;
/// Shift count converting in-cell offsets back to fixed-point fractions
/// (`MAPBTOFRAC` in `p_local.h`).
pub const MAPBTOFRAC: c_int = MAPBLOCKSHIFT - FRACBITS as c_int;

/// Capacity of the per-tic event queue (`ITEMQUESIZE` analogue used by the
/// engine's input handling).
pub const ITEMQUESIZE: usize = 128;

/// Slope type for a horizontal line segment (`ST_HORIZONTAL` in `r_defs.h`).
pub const ST_HORIZONTAL: c_int = 0;
/// Slope type for a vertical line segment (`ST_VERTICAL` in `r_defs.h`).
pub const ST_VERTICAL: c_int = 1;
/// Slope type for a line with positive gradient (`ST_POSITIVE` in `r_defs.h`).
pub const ST_POSITIVE: c_int = 2;
/// Slope type for a line with negative gradient (`ST_NEGATIVE` in `r_defs.h`).
pub const ST_NEGATIVE: c_int = 3;

// ---------------------------------------------------------------------------
// WAD lump-order indices (doomdata.h anonymous enum).
// These are the fixed positions of each data lump within a map's group of
// WAD lumps.  p_setup.c indexes directly into the lump list using these.
// ---------------------------------------------------------------------------

/// WAD map lump indices (`ML_*` in `doomdata.h`). Used as offsets from the
/// map's header lump to locate each sub-lump.
#[repr(C)]
pub struct MapLump;
/// `ML_*` lump-offset constants used to index a map's group of WAD lumps.
impl MapLump {
    #[doc(alias = "ML_LABEL")]
    pub const LABEL: c_int = 0; // ExMx / MAPxx separator
    #[doc(alias = "ML_THINGS")]
    pub const THINGS: c_int = 1; // Monster/item placement
    #[doc(alias = "ML_LINEDEFS")]
    pub const LINEDEFS: c_int = 2; // Line definitions
    #[doc(alias = "ML_SIDEDEFS")]
    pub const SIDEDEFS: c_int = 3; // Side (texture) definitions
    #[doc(alias = "ML_VERTEXES")]
    pub const VERTEXES: c_int = 4; // Vertex coordinates
    #[doc(alias = "ML_SEGS")]
    pub const SEGS: c_int = 5; // BSP line segments
    #[doc(alias = "ML_SSECTORS")]
    pub const SSECTORS: c_int = 6; // BSP sub-sectors
    #[doc(alias = "ML_NODES")]
    pub const NODES: c_int = 7; // BSP nodes
    #[doc(alias = "ML_SECTORS")]
    pub const SECTORS: c_int = 8; // Sector definitions
    #[doc(alias = "ML_REJECT")]
    pub const REJECT: c_int = 9; // Sector-to-sector visibility table
    #[doc(alias = "ML_BLOCKMAP")]
    pub const BLOCKMAP: c_int = 10; // Motion-clipping blockmap
}

const _: () = assert!(
    std::mem::size_of::<c_int>() == std::mem::size_of::<i32>(),
    "MapLump constants are c_int; c_int must be 32-bit on this platform"
);

/// Linedef flag bits (`ML_*` in `doomdata.h`). Stored in `line_t.flags` as
/// a bitmask; each constant is a single-bit mask.
#[repr(C)]
pub struct LinedefFlag;
/// `ML_*` bit-mask constants stored in `line_t.flags`.
impl LinedefFlag {
    #[doc(alias = "ML_BLOCKING")]
    pub const BLOCKING: u16 = 1; // Solid obstacle
    #[doc(alias = "ML_BLOCKMONSTERS")]
    pub const BLOCKMONSTERS: u16 = 2; // Blocks monsters only
    #[doc(alias = "ML_TWOSIDED")]
    pub const TWOSIDED: u16 = 4; // Has a back sector
    #[doc(alias = "ML_DONTPEGTOP")]
    pub const DONTPEGTOP: u16 = 8; // Upper texture is unpegged
    #[doc(alias = "ML_DONTPEGBOTTOM")]
    pub const DONTPEGBOTTOM: u16 = 16; // Lower texture is unpegged
    #[doc(alias = "ML_SECRET")]
    pub const SECRET: u16 = 32; // Secret on automap
    #[doc(alias = "ML_SOUNDBLOCK")]
    pub const SOUNDBLOCK: u16 = 64; // Sound propagation barrier
    #[doc(alias = "ML_DONTDRAW")]
    pub const DONTDRAW: u16 = 128; // Hidden on automap
    #[doc(alias = "ML_MAPPED")]
    pub const MAPPED: u16 = 256; // Already revealed on automap
}

const _: () = assert!(
    std::mem::size_of::<u16>() == std::mem::size_of::<std::os::raw::c_short>(),
    "LinedefFlag masks are u16; u16 must match c_short (line_t.flags)"
);

// ---------------------------------------------------------------------------
// Screen constants (from i_video.h / st_stuff.h)
// ---------------------------------------------------------------------------

/// Status-bar height (ST_HEIGHT from st_stuff.h).
pub const SBARHEIGHT: c_int = 32;

// ---------------------------------------------------------------------------
// r_draw.c constants
// ---------------------------------------------------------------------------

/// Number of entries in the fuzz offset lookup table (FUZZTABLE in r_draw.c).
pub const FUZZTABLE: usize = 50;
/// Direction unit used when rendering the fuzz/spectre effect. Vanilla/boom
/// bake `FUZZOFF = SCREENWIDTH` into the table at compile time; since F1 M2
/// the raster width is runtime, so the table stores +/-1 direction units and
/// the column renderers scale by the live `SCREENWIDTH` (crispy-doom
/// `r_draw.c:325` defines FUZZOFF as 1 and scales at the use site,
/// `r_draw.c:409`) — identical output at any width.
pub const FUZZOFF: c_int = 1;

/// Version code for cph's longtics hack ("v1.91") from `doomdef.h`.
pub const DOOM_191_VERSION: c_int = 111;

// ---------------------------------------------------------------------------
// Additional constants from C headers
// ---------------------------------------------------------------------------

/// Screen width used by the "squash" scale modes (SCREENWIDTH_4_3 in i_video.h).
pub const SCREENWIDTH_4_3: c_int = 256;
/// Screen height used by the "stretch" scale modes (SCREENHEIGHT_4_3 in i_video.h).
pub const SCREENHEIGHT_4_3: c_int = 240;

/// Size of the body-object circular queue (BODYQUESIZE in g_game.c).
pub const BODYQUESIZE: usize = 32;
/// Number of tics during which slow-turn speed is used before switching to
/// normal turn speed (SLOWTURNTICS in g_game.c).
pub const SLOWTURNTICS: c_int = 6;
/// Speed threshold above which the turbo-cheat detector fires
/// (TURBOTHRESHOLD in g_game.c = 0x32 = 50).
pub const TURBOTHRESHOLD: c_int = 0x32;

/// Weapon-sprite lower speed (LOWERSPEED in p_pspr.c = FRACUNIT × 6).
pub const LOWERSPEED: c_int = FRACUNIT * 6;
/// Weapon-sprite raise speed (RAISESPEED in p_pspr.c = FRACUNIT × 6).
pub const RAISESPEED: c_int = FRACUNIT * 6;
/// Y-position of the weapon at its lowest (off-screen) rest point
/// (WEAPONBOTTOM in p_pspr.c = 128 × FRACUNIT).
pub const WEAPONBOTTOM: c_int = 128 * FRACUNIT;
/// Y-position of the weapon at its highest (ready) position
/// (WEAPONTOP in p_pspr.c = 32 × FRACUNIT).
pub const WEAPONTOP: c_int = 32 * FRACUNIT;

/// Minimum sprite Z distance; sprites closer than this are not projected
/// (MINZ in r_things.c = FRACUNIT × 4).
pub const MINZ: c_int = FRACUNIT * 4;
/// Vertical centre of the screen in pixels, used for sprite projection
/// (BASEYCENTER in r_things.c = 100).
pub const BASEYCENTER: c_int = 100;

// ---------------------------------------------------------------------------
// p_inter.c — ammo and interaction constants
// ---------------------------------------------------------------------------

/// Number of ammo types (NUMAMMO in doomdef.h = 4: clip, shell, cell, misl).
pub const NUMAMMO: usize = 4;
/// Bonus count added per pick-up for the screen flash effect
/// (BONUSADD in p_inter.c = 6).
pub const BONUSADD: c_int = 6;

// ---------------------------------------------------------------------------
// p_mobj.c — map-object physics constants
// ---------------------------------------------------------------------------

/// Velocity below which horizontal momentum is zeroed (STOPSPEED = 0x1000).
pub const STOPSPEED: c_int = 0x1000;
/// Friction factor applied to horizontal velocity each tic (FRICTION = 0xe800).
pub const FRICTION: c_int = 0xe800_u32 as c_int;

// ---------------------------------------------------------------------------
// p_spec.c — sector-special and animation constants / globals
// ---------------------------------------------------------------------------

/// Maximum number of running floor/ceiling/door animations at once
/// (MAXANIMS in p_spec.c = 32).
pub const MAXANIMS: c_int = 32;
/// Maximum number of active line specials in a level
/// (MAXLINEANIMS in p_spec.c = 64).
pub const MAXLINEANIMS: c_int = 64;

/// Glow effect speed (GLOWSPEED in p_spec.h = 8).
pub const GLOWSPEED: c_int = 8;
/// Bright strobe level (STROBEBRIGHT in p_spec.h = 5).
pub const STROBEBRIGHT: c_int = 5;
/// Fast strobe dark duration in tics (FASTDARK in p_spec.h = 15).
pub const FASTDARK: c_int = 15;
/// Slow strobe dark duration in tics (SLOWDARK in p_spec.h = 35).
pub const SLOWDARK: c_int = 35;

/// Vertical door speed (VDOORSPEED = FRACUNIT × 2).
pub const VDOORSPEED: c_int = FRACUNIT * 2;
/// Vertical door wait time in tics (VDOORWAIT = 150).
pub const VDOORWAIT: c_int = 150;

/// Ceiling movement speed (CEILSPEED = FRACUNIT).
pub const CEILSPEED: c_int = FRACUNIT;
/// Maximum simultaneously active ceilings (MAXCEILINGS = 30).
pub const MAXCEILINGS: c_int = 30;
/// Ceiling movement wait time in tics (CEILWAIT = 150).
pub const CEILWAIT: c_int = 150;

/// Platform movement speed (PLATSPEED = FRACUNIT).
pub const PLATSPEED: c_int = FRACUNIT;
/// Maximum simultaneously active platforms (MAXPLATS = 30).
pub const MAXPLATS: c_int = 30;
/// Platform wait time in seconds (PLATWAIT = 3).
pub const PLATWAIT: c_int = 3;

/// Floor movement speed (FLOORSPEED = FRACUNIT).
pub const FLOORSPEED: c_int = FRACUNIT;

// ---------------------------------------------------------------------------
// p_map.c — collision detection globals and constants
// ---------------------------------------------------------------------------

/// Sentinel magic value used to detect the vanilla spechit overflow bug
/// (DEFAULT_SPECHIT_MAGIC in p_map.c = 0x01C09C98).
pub const DEFAULT_SPECHIT_MAGIC: c_uint = 0x01C09C98;

// All p_map globals are now exported from `room/src/doom/p_map.rs`.

// ---------------------------------------------------------------------------
// p_saveg.c — save-game serialization constants and globals
// ---------------------------------------------------------------------------

/// End-of-file marker byte written at the end of every save game
/// (SAVEGAME_EOF in p_saveg.c = 0x1d).
pub const SAVEGAME_EOF: u8 = 0x1d;
/// Length of the version string embedded in save-game headers
/// (VERSIONSIZE in p_saveg.c = 16).
pub const VERSIONSIZE: usize = 16;

// ---------------------------------------------------------------------------
// p_enemy.c — enemy AI globals and constants
// ---------------------------------------------------------------------------

/// Angle turned per tic when pursuing a lost target
/// (TRACEANGLE in p_enemy.c = 0xc000000).
pub const TRACEANGLE: c_uint = 0xc000000;
/// Arc spread angle for the Mancubus fireball pattern
/// (FATSPREAD = ANG90 / 8).
pub const FATSPREAD: c_uint = ANG90 / 8;
/// Speed of the Lost Soul charge attack in fixed-point units/tic
/// (SKULLSPEED = 20 × FRACUNIT).
pub const SKULLSPEED: c_int = 20 * FRACUNIT;

// ---------------------------------------------------------------------------
// wi_stuff.c — intermission screen constants
// ---------------------------------------------------------------------------

/// Number of episodes (NUMEPISODES in wi_stuff.c = 4).
pub const WI_NUMEPISODES: usize = 4;
/// Number of maps per episode (NUMMAPS in wi_stuff.c = 9).
pub const WI_NUMMAPS: usize = 9;
/// Y-position of the level-name title on the intermission screen (WI_TITLEY = 2).
pub const WI_TITLEY: c_int = 2;
/// Vertical spacing between player stats rows (WI_SPACINGY = 33).
pub const WI_SPACINGY: c_int = 33;
/// X-position of single-player stats (SP_STATSX = 50).
pub const SP_STATSX: c_int = 50;
/// Y-position of single-player stats (SP_STATSY = 50).
pub const SP_STATSY: c_int = 50;
/// X-position of the time display (SP_TIMEX = 16).
pub const SP_TIMEX: c_int = 16;
/// Y-position of the time display (SP_TIMEY = SCREENHEIGHT − 32).
/// F1 M2: `SCREENHEIGHT` is the runtime raster height, so this is a function
/// of the live configuration rather than a compile-time constant.
pub fn SP_TIMEY() -> c_int
{
    let height = unsafe { SCREENHEIGHT };
    height - 32
}
/// Delay in tics before showing the "next level" location (SHOWNEXTLOCDELAY = 4).
pub const SHOWNEXTLOCDELAY: c_int = 4;
/// X spacing between deathmatch matrix columns (DM_SPACINGX = 40).
pub const DM_SPACINGX: c_int = 40;
/// Number of animated elements in episode 0 background (EPSD0ANIMINFO length).
pub const WI_EPSD0_NANIM: usize = crate::doom::wi_stuff::EPSD0_NANIM;
/// Number of animated elements in episode 1 background (EPSD1ANIMINFO length).
pub const WI_EPSD1_NANIM: usize = crate::doom::wi_stuff::EPSD1_NANIM;
/// Number of animated elements in episode 2 background (EPSD2ANIMINFO length).
pub const WI_EPSD2_NANIM: usize = crate::doom::wi_stuff::EPSD2_NANIM;

// ---------------------------------------------------------------------------
// st_stuff.c — status bar constants
// ---------------------------------------------------------------------------

/// Palette index at which red (pain) palette rotations begin (STARTREDPALS = 1).
pub const STARTREDPALS: c_int = 1;
/// Palette index at which bonus (item pick-up) rotations begin (STARTBONUSPALS = 9).
pub const STARTBONUSPALS: c_int = 9;
/// Number of red pain palette entries (NUMREDPALS = 8).
pub const NUMREDPALS: c_int = 8;
/// Number of bonus pick-up palette entries (NUMBONUSPALS = 4).
pub const NUMBONUSPALS: c_int = 4;
/// Palette index used for the radiation suit overlay (RADIATIONPAL = 13).
pub const RADIATIONPAL: c_int = 13;
/// 1-in-N probability of a random face change per tic (ST_FACEPROBABILITY = 96).
pub const ST_FACEPROBABILITY: c_int = 96;
/// Number of pain-level face groups (ST_NUMPAINFACES = 5).
pub const ST_NUMPAINFACES: c_int = 5;
/// Number of straight (non-turning) faces per pain group (ST_NUMSTRAIGHTFACES = 3).
pub const ST_NUMSTRAIGHTFACES: c_int = 3;
/// Number of turning face variants per pain group (ST_NUMTURNFACES = 2).
pub const ST_NUMTURNFACES: c_int = 2;
/// Number of special face frames (ST_NUMSPECIALFACES = 3: ouch, evil-grin, rampage).
pub const ST_NUMSPECIALFACES: c_int = 3;
/// Number of extra face frames beyond the pain table (ST_NUMEXTRAFACES = 2: god, dead).
pub const ST_NUMEXTRAFACES: c_int = 2;
/// Total face sprite count = pain × stride + extra.
pub const ST_NUMFACES: c_int = ST_NUMPAINFACES
    * (ST_NUMSTRAIGHTFACES + ST_NUMTURNFACES + ST_NUMSPECIALFACES)
    + ST_NUMEXTRAFACES;
/// Tics of evil-grin display (ST_EVILGRINCOUNT = 2 × TICRATE).
pub const ST_EVILGRINCOUNT: c_int = 2 * TICRATE;
/// Tics of straight-face display (ST_STRAIGHTFACECOUNT = TICRATE / 2).
pub const ST_STRAIGHTFACECOUNT: c_int = TICRATE / 2;
/// Tics before turn-face times out (ST_TURNCOUNT = 1 × TICRATE).
pub const ST_TURNCOUNT: c_int = TICRATE;
/// Tics of ouch-face display (ST_OUCHCOUNT = 1 × TICRATE).
pub const ST_OUCHCOUNT: c_int = TICRATE;
/// Tics of rampage-face display (ST_RAMPAGEDELAY = 2 × TICRATE).
pub const ST_RAMPAGEDELAY: c_int = 2 * TICRATE;
/// Damage threshold for the rampage face (ST_MUCHPAIN = 20).
pub const ST_MUCHPAIN: c_int = 20;
/// X pixel offset of the status bar (ST_X = 0).
pub const ST_X: c_int = 0;
/// X pixel offset of the arms display (ST_X2 = 104).
pub const ST_X2: c_int = 104;
