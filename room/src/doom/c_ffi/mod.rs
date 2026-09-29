//! Centralized FFI declarations for the C-shaped surface: the mirror
//! types, header constants, extern declaration blocks, and re-export
//! chains that c_tests and the FFI consumers read through one flat
//! namespace (`c_ffi::vertex_t`, `c_ffi::forwardmove`, ...).
//!
//! ## Charter (re-export/annotate-only)
//!
//! This module holds **no logic** and owns **no behavior**: it is the
//! FFI lens over the engine, not part of it. Its graduation therefore
//! renames nothing and restructures nothing beyond the module-directory
//! anatomy:
//!
//! - every item keeps its upstream name byte-for-byte (types, consts,
//!   extern decls, re-export chains);
//! - the four `extern "C"` blocks in `decls.rs` are the freeze-zone
//!   linkage surface: `doomgeneric-sys/build.rs` compiles zero C
//!   sources, so every entry resolves against the engine's
//!   `#[no_mangle]`/`#[export_name]` exports at link time (wasm-lld /
//!   host ld) -- the blocks are never edited or modernised (f1 report
//!   §0.1);
//! - the `pub use` chains in `reexports.rs` are carried verbatim;
//!   c_tests (633 `c_ffi::` references across 92 files at graduation
//!   time) and the web shell's div-probe (`shells/web/src/lib.rs` uses
//!   `c_ffi::mobj_t` / `c_ffi::sector_t`) consume these paths;
//! - a dedup of the shadowed mirror types below into one canonical home
//!   is a post-graduation refactor touching ~90 files and is expressly
//!   out of scope (f1 report §0.4).
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the two opaque decls (`state_t`, `mobjinfo_t`) and
//!   the eighteen `#[repr(C)]` C-layout mirror records
//! - `consts.rs` -- the C-header constants, the `MapLump` /
//!   `LinedefFlag` constant carriers with their size asserts, and
//!   `SP_TIMEY` (the module's only function: the runtime-`SCREENHEIGHT`
//!   replacement for the C `SP_TIMEY` macro, name kept -- it is a
//!   constant-alias in intent)
//! - `decls.rs` -- the four `extern "C"` blocks (r_draw functions,
//!   g_game statics, savegame statics, p_enemy statics)
//! - `reexports.rs` -- the `pub use` chains surfacing ported symbols
//!   from `i_timer`, `i_video`, `m_bbox`, `m_fixed`, `tables`,
//!   `i_scale`, `d_loop`, `d_main`, `p_tick`, `p_pspr`, `r_draw`,
//!   `r_segs`, `r_things`
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below; no content lives here.
//!
//! ## Type shadow map (layout-parallel mirrors)
//!
//! The same C records already have graduated-module-local mirrors.
//! These are structural `#[repr(C)]` copies, so pointer casts between
//! them work today; dedup is post-graduation work (charter above).
//!
//! | c_ffi type | Other homes (layout-parallel mirrors) |
//! |------------|----------------------------------------|
//! | `vertex_t` | `r_bsp/types.rs`, `p_sight/types.rs` |
//! | `divline_t` | `p_sight/types.rs` |
//! | `subsector_t` | `p_sight/types.rs`, `p_telept/types.rs` |
//! | `mapthing_t` | `p_telept/types.rs`, `p_setup/structs.rs` |
//! | `sector_t` | `r_bsp/types.rs`, `p_lights/types.rs`, `p_sight/types.rs`, `p_telept/types.rs` |
//! | `line_t` | `p_lights/types.rs`, `p_sight/types.rs`, `p_telept/types.rs` |
//! | `intercept_t` (+ `intercept_t_d` union) | none -- `p_maputl/intercepts.rs` consumes c_ffi's |
//! | `mobj_t` | `p_telept/types.rs`; prefix-mirror `s_sound::MobjStub` (`s_sound/params.rs`) |
//! | `side_t` | `r_bsp/types.rs`, `p_floor/state.rs` |
//! | `seg_t` | `r_bsp/types.rs`, `p_sight/types.rs` |
//! | `node_t` | `r_bsp/types.rs`, `p_sight/types.rs` |
//! | `drawseg_t` | `r_bsp/types.rs` |
//! | `vissprite_t`, `spriteframe_t` | c_ffi only (`r_things` imports them) |
//! | `screen_mode_t` | none -- consumed by `i_scale` (its fifteen `mode_*` statics are re-exported here) |
//! | `LumpInfo` (`w_checksum/digest.rs`) | mirrors `w_wad`'s graduated `repr(C)` `lumpinfo` |
//!
//! ## Original Fn Name Mapping
//!
//! The module is data/decls wholesale (the `info.rs` / `tables.rs`
//! precedent: no logic to split, `Surface: data`). Every type, constant,
//! extern declaration, and re-export keeps its name byte-for-byte, so
//! there are no shims -- only the path-stability wiring below. One
//! function exists:
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `SP_TIMEY` (C macro, `wi_stuff.h`) | `consts::SP_TIMEY` | glue | runtime-`SCREENHEIGHT` replacement for the compile-time C macro (F1 M2 made the raster height runtime); name kept, arithmetic only |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: c_ffi is the FFI lens over the demo-synchronization
//! surface, not part of it. The mirror types are inert layouts; the
//! constants are boot-time tunables; the extern blocks declare symbols
//! that live in other modules (their dtmc adjudication belongs to the
//! defining modules); `SP_TIMEY` is arithmetic on the runtime raster
//! height (render-side configuration, never sim state). Nothing here
//! executes inside a tic.
//!
//! Baseline: `c_tests/struct_layouts.rs`, `room/src/bin/struct_sizes.rs`,
//! and the per-type size asserts inline in `consts.rs` pin the layouts --
//! that IS the baseline; it reruns unchanged after graduation.

pub mod consts;
pub mod decls;
pub mod reexports;
pub mod types;

//* path-stability wiring: every item keeps its flat `c_ffi::` path
//* exactly as before the split (no renames happened, so this is wiring,
//* not shims). The lists are explicit on purpose -- a missing name
//* breaks a consumer at compile time instead of silently glob-vanishing.

pub use consts::{
    MapLump, LinedefFlag, SP_TIMEY, MAPBLOCKUNITS, MAPBLOCKSIZE, MAPBLOCKSHIFT, MAPBMASK,
    MAPBTOFRAC, ITEMQUESIZE, ST_HORIZONTAL, ST_VERTICAL, ST_POSITIVE, ST_NEGATIVE, SBARHEIGHT,
    FUZZTABLE, FUZZOFF, SCREENWIDTH_4_3, SCREENHEIGHT_4_3, BODYQUESIZE, SLOWTURNTICS,
    TURBOTHRESHOLD, LOWERSPEED, RAISESPEED, WEAPONBOTTOM, WEAPONTOP, MINZ, BASEYCENTER, NUMAMMO,
    BONUSADD, STOPSPEED, FRICTION, MAXANIMS, MAXLINEANIMS, GLOWSPEED, STROBEBRIGHT, FASTDARK,
    SLOWDARK, VDOORSPEED, VDOORWAIT, CEILSPEED, MAXCEILINGS, CEILWAIT, PLATSPEED, MAXPLATS,
    PLATWAIT, FLOORSPEED, DEFAULT_SPECHIT_MAGIC, SAVEGAME_EOF, VERSIONSIZE, TRACEANGLE,
    FATSPREAD, SKULLSPEED, WI_NUMEPISODES, WI_NUMMAPS, WI_TITLEY, WI_SPACINGY, SP_STATSX,
    SP_STATSY, SP_TIMEX, SHOWNEXTLOCDELAY, DM_SPACINGX, WI_EPSD0_NANIM, WI_EPSD1_NANIM,
    WI_EPSD2_NANIM, STARTREDPALS, STARTBONUSPALS, NUMREDPALS, NUMBONUSPALS, RADIATIONPAL,
    ST_FACEPROBABILITY, ST_NUMPAINFACES, ST_NUMSTRAIGHTFACES, ST_NUMTURNFACES,
    ST_NUMSPECIALFACES, ST_NUMEXTRAFACES, ST_NUMFACES, ST_EVILGRINCOUNT, ST_STRAIGHTFACECOUNT,
    ST_TURNCOUNT, ST_OUCHCOUNT, ST_RAMPAGEDELAY, ST_MUCHPAIN, ST_X, ST_X2, DOOM_191_VERSION,
};

pub use decls::{
    R_DrawViewBorder, R_FillBackScreen, R_InitBuffer, R_InitTranslationTables, R_VideoErase,
    angleturn, bodyqueslot, braintargeton, braintargets, forwardmove, levelstarttic, numbraintargets,
    precache, savegame_error, savegamelength, sidemove, testcontrols, totalsecret, totalitems,
    totalkills, vanilla_demo_limit, vanilla_savegame_limit, xspeed, yspeed,
};

pub use reexports::{
    ANG180, ANG270, ANG45, ANG90, ANGLETOFINESHIFT, BACKUPTICS, BBox, FINEMASK, FRACBITS,
    FRACUNIT, SCREENHEIGHT, SCREENWIDTH, TICRATE, advancedemo, autostart, bfgedition,
    bottomfrac, bottomstep, bottomtexture, dc_colormap, dc_iscale, dc_source, dc_texturemid,
    dc_x, dc_yh, dc_yl, devparm, ds_x1, ds_x2, ds_xfrac, ds_xstep, ds_y, ds_yfrac, ds_ystep,
    fastparm, fuzzoffset, fuzzpos, gametic, iwadfile, main_loop_started, markceiling, markfloor,
    maskedtexture, maskedtexturecol, maxframe, midtexture, mode_scale_1x, mode_scale_2x,
    mode_scale_3x, mode_scale_4x, mode_scale_5x, mode_squash_1x, mode_squash_2x, mode_squash_3x,
    mode_squash_4x, mode_squash_5x, mode_stretch_1x, mode_stretch_2x, mode_stretch_3x,
    mode_stretch_4x, mode_stretch_5x, negonearray, nomonsters, numsprites, offsetms,
    pixhigh, pixhighstep, pixlow, pixlowstep, pspriteiscale, pspritescale, respawnparm,
    rw_angle1, rw_bottomtexturemid, rw_centerangle, rw_distance, rw_midtexturemid, rw_normalangle,
    rw_offset, rw_scale, rw_scalestep, rw_stopx, rw_toptexturemid, rw_x, savegamedir,
    scaledviewwidth, screenheightarray, segtextured, show_endoom, singletics, spritelights,
    spritename, sprites, sprtemp, startepisode, startmap, storedemo, swingx, swingy, ticdup,
    topfrac, topstep, toptexture, viewheight, viewwidth, viewwindowx, viewwindowy, walllights,
    worldbottom, worldhigh, worldlow, worldtop, leveltime,
};

pub use types::{
    drawseg_t, divline_t, intercept_t, intercept_t_d, line_t, mobj_t, mobjinfo_t, node_t,
    screen_mode_t, sector_t, seg_t, side_t, spriteframe_t, state_t, subsector_t, mapthing_t,
    vertex_t, vissprite_t,
};
