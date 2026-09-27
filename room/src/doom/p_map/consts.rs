//! Shared constants for the movement/collision module: the spechit
//! bounds, the blockmap query expansion, the Use-action reach, the
//! map-object flag/type values, and the line-slope classification -- all
//! values carried verbatim from the monolithic `p_map.rs`, upstream
//! names retained (data tier; the renaming scope of the F10 wave B2b
//! ruling is functions only).

use std::ffi::c_int;

use crate::doom::m_fixed::FRACUNIT;

/// Maximum number of special lines that can be crossed during a single move.
///
/// The Rust port uses 20 slots (matching `MAXSPECIALCROSS` in the C source)
/// while the original vanilla limit was 8 (`MAXSPECIALCROSS_ORIGINAL`).
pub(crate) const MAXSPECIALCROSS: usize = 20;

/// Original vanilla Doom limit for special lines crossed per move.
///
/// Crossings beyond this count trigger [`super::spechit::spechit_overrun`]
/// to emulate the vanilla memory-corruption behaviour that some demos
/// depend on.
pub(super) const MAXSPECIALCROSS_ORIGINAL: c_int = 8;

/// Maximum radius of any map object, in map units (fixed-point).
///
/// Used to expand blockmap queries so that objects whose origin lies in an
/// adjacent block but overlaps the query region are still found.
pub(super) const MAXRADIUS: c_int = 32 * FRACUNIT;

/// Forward reach of the player's Use action, in map units (fixed-point).
pub(super) const USERANGE: c_int = 64 * FRACUNIT;

/// Map-object flag: the thing is a special pickup item.
pub(super) const MF_SPECIAL: c_int = 1;
/// Map-object flag: the thing blocks movement (solid).
pub(super) const MF_SOLID: c_int = 2;
/// Map-object flag: the thing can be damaged by hitscan or projectile attacks.
pub(super) const MF_SHOOTABLE: c_int = 4;
/// Map-object flag: the thing is not added to the sector thing list.
pub(super) const MF_NOSECTOR: c_int = 8;
/// Map-object flag: the thing is not added to the blockmap.
pub(super) const MF_NOBLOCKMAP: c_int = 16;
/// Map-object flag: the thing is a projectile (missile).
pub(super) const MF_MISSILE: c_int = 65536;
/// Map-object flag: the thing can walk off ledges.
pub(super) const MF_DROPOFF: c_int = 1024;
/// Map-object flag: the thing ignores clipping (no-clip cheat).
pub(super) const MF_NOCLIP: c_int = 4096;
/// Map-object flag: the thing can fly vertically (floats in air).
pub(super) const MF_FLOAT: c_int = 16384;
/// Map-object flag: the thing is mid-teleport and bypasses step/dropoff checks.
pub(super) const MF_TELEPORT: c_int = 32768;
/// Map-object flag: a Lost Soul currently in charge-flight mode.
pub(super) const MF_SKULLFLY: c_int = 16777216;
/// Map-object flag: the thing picks up items on contact.
pub(super) const MF_PICKUP: c_int = 2048;
/// Map-object flag: the thing does not bleed when damaged (spawns puff instead).
pub(super) const MF_NOBLOOD: c_int = 524288;
/// Map-object flag: the thing was dropped by a monster and should be removed when crushed.
pub(super) const MF_DROPPED: c_int = 131072;

/// Thing-type index for the player.
pub(super) const MT_PLAYER: c_int = 0;
/// Thing-type index for the Hell Knight.
pub(super) const MT_KNIGHT: c_int = 17;
/// Thing-type index for the Baron of Hell.
pub(super) const MT_BRUISER: c_int = 15;
/// Thing-type index for the Cyberdemon (immune to splash damage).
pub(super) const MT_CYBORG: c_int = 21;
/// Thing-type index for the Spider Mastermind (immune to splash damage).
pub(super) const MT_SPIDER: c_int = 19;
/// Thing-type index for the blood splat particle spawned during crushing.
pub(super) const MT_BLOOD: c_int = 38;

/// State index for the "gibs" sprite, used when corpses are crushed.
pub(super) const S_GIBS: c_int = 895;

/// Line slope type: perfectly horizontal (dy == 0).
pub(super) const ST_HORIZONTAL: c_int = 0;
/// Line slope type: perfectly vertical (dx == 0).
pub(super) const ST_VERTICAL: c_int = 1;

/// Default value for the DEH species-infighting flag (disabled).
///
/// When 0, monsters of the same species cannot hurt each other with projectiles.
/// A DeHackEd patch can override this.
pub(super) const DEH_DEFAULT_SPECIES_INFIGHTING: c_int = 0;
