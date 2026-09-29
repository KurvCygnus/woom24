//! The re-export chains: the ported symbols from other modules that the
//! hub surfaces under one flat FFI namespace, carried VERBATIM from the
//! flat `c_ffi.rs` -- never reordered, never renamed (c_tests and the
//! web shell's div-probe consume these paths; the chains are the
//! module's re-export contract, not its own items).

// Re-export head: cross-module constants the C-shaped tests read.
pub use crate::doom::i_timer::TICRATE;
pub use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
pub use crate::doom::m_bbox::BBox;
pub use crate::doom::m_fixed::{FRACBITS, FRACUNIT};
pub use crate::doom::tables::{ANG180, ANG270, ANG45, ANG90, ANGLETOFINESHIFT, FINEMASK};

// Re-exported from the Rust port of i_scale.c.
pub use crate::doom::i_scale::{
    mode_scale_1x, mode_scale_2x, mode_scale_3x, mode_scale_4x, mode_scale_5x, mode_squash_1x,
    mode_squash_2x, mode_squash_3x, mode_squash_4x, mode_squash_5x, mode_stretch_1x,
    mode_stretch_2x, mode_stretch_3x, mode_stretch_4x, mode_stretch_5x,
};

// ---------------------------------------------------------------------------
// d_loop.rs — main game-loop state globals and constants
// ---------------------------------------------------------------------------

pub use crate::doom::d_loop::{gametic, offsetms, singletics, ticdup, BACKUPTICS};

// ---------------------------------------------------------------------------
// d_main.rs — startup flag defaults and global state.
// Re-exported from the Rust port so C tests and remaining FFI consumers can
// continue to access everything through one module.
// ---------------------------------------------------------------------------

pub use crate::doom::d_main::{
    advancedemo, autostart, bfgedition, devparm, fastparm, iwadfile, main_loop_started, nomonsters,
    respawnparm, savegamedir, show_endoom, startepisode, startmap, storedemo,
};

// ---------------------------------------------------------------------------
// Re-exports of ported globals so C tests and remaining FFI consumers can
// continue to access everything through one module.
// ---------------------------------------------------------------------------

pub use crate::doom::p_tick::leveltime;

pub use crate::doom::p_pspr::{swingx, swingy};

pub use crate::doom::r_draw::{
    dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_x, dc_yh, dc_yl, ds_x1, ds_x2, ds_xfrac,
    ds_xstep, ds_y, ds_yfrac, ds_ystep, fuzzoffset, fuzzpos, scaledviewwidth, viewheight,
    viewwidth, viewwindowx, viewwindowy,
};

pub use crate::doom::r_segs::{
    bottomfrac, bottomstep, bottomtexture, markceiling, markfloor, maskedtexture, maskedtexturecol,
    midtexture, pixhigh, pixhighstep, pixlow, pixlowstep, rw_angle1, rw_bottomtexturemid,
    rw_centerangle, rw_distance, rw_midtexturemid, rw_normalangle, rw_offset, rw_scale,
    rw_scalestep, rw_stopx, rw_toptexturemid, rw_x, segtextured, topfrac, topstep, toptexture,
    walllights, worldbottom, worldhigh, worldlow, worldtop,
};

pub use crate::doom::r_things::{
    maxframe, negonearray, numsprites, pspriteiscale, pspritescale, screenheightarray,
    spritelights, spritename, sprites, sprtemp,
};
