//! Automap constants and shape tables: palette colour ranges, zoom
//! scales, `AM_MSG*` event values, the player-arrow/thing shape tables,
//! the Cohen-Sutherland outcodes, and the point/line types.

use std::ffi::c_int;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};

/// Maximum number of player-placed mark points on the automap.
///
/// C origin: `AM_NUMMARKPOINTS` in am_map.c.
pub const AM_NUMMARKPOINTS: usize = 10;

/// Initial map-to-frame scale factor (approximately 0.2 in fixed-point).
///
/// The automap starts zoomed out to show roughly 20 % of the map height per
/// screen height.  C origin: `INITSCALEMTOF` in am_map.c.
pub const INITSCALEMTOF: c_int = (0.2 * FRACUNIT as f64) as c_int;

/// Pan increment in screen pixels per tick when the player holds a pan key.
///
/// C origin: `F_PANINC` in am_map.c.
pub const F_PANINC: c_int = 4;

/// Scale multiplier applied to `scale_mtof` each tick while zooming in.
///
/// 1.02 in fixed-point; chosen to give a smooth zoom feel.
/// C origin: `M_ZOOMIN` in am_map.c.
pub const M_ZOOMIN: c_int = (1.02 * FRACUNIT as f64) as c_int;

/// Scale multiplier applied to `scale_mtof` each tick while zooming out.
///
/// Reciprocal of [`M_ZOOMIN`] in fixed-point.
/// C origin: `M_ZOOMOUT` in am_map.c.
pub const M_ZOOMOUT: c_int = (FRACUNIT as f64 / 1.02) as c_int;

/// Half-diameter of the player object in map units, used for arrow scaling.
///
/// C origin: `PLAYERRADIUS` in am_map.c.
pub(super) const PLAYERRADIUS: c_int = 16 * FRACUNIT;

// Colour palette indices.

/// Starting palette index of the red colour range.
pub(super) const REDS: c_int = 256 - 5 * 16;
/// Number of palette entries in the red range.
pub(super) const REDRANGE: c_int = 16;
/// Starting palette index of the blue colour range.
const BLUES: c_int = 256 - 4 * 16 + 8;
/// Number of palette entries in the blue colour range.
const BLUERANGE: c_int = 8;
/// Starting palette index of the green colour range.
pub(super) const GREENS: c_int = 7 * 16;
/// Number of palette entries in the green colour range.
pub(super) const GREENRANGE: c_int = 16;
/// Starting palette index of the grey colour range.
pub(super) const GRAYS: c_int = 6 * 16;
/// Number of palette entries in the grey colour range.
pub(super) const GRAYSRANGE: c_int = 16;
/// Starting palette index of the brown colour range.
pub(super) const BROWNS: c_int = 4 * 16;
/// Number of palette entries in the brown colour range.
pub(super) const BROWNRANGE: c_int = 16;
/// Starting palette index of the yellow colour range.
pub(super) const YELLOWS: c_int = 256 - 32 + 7;
/// Number of palette entries in the yellow colour range.
pub(super) const YELLOWRANGE: c_int = 1;
/// Palette index for black (the automap background).
pub(super) const BLACK: c_int = 0;
/// Palette index for white (the player arrow in single-player).
pub(super) const WHITE: c_int = 256 - 47;

// Automap colour assignments.

/// Background fill colour index.
pub(super) const BACKGROUND: c_int = BLACK;
/// Colour used for solid (one-sided) walls.
pub(super) const WALLCOLORS: c_int = REDS;
/// Colour range width for solid walls.
pub(super) const WALLRANGE: c_int = REDRANGE;
/// Colour used for two-sided walls where both sides have the same floor and
/// ceiling height (transparent / passable walls).
pub(super) const TSWALLCOLORS: c_int = GRAYS;
/// Colour range width for transparent walls.
const TSWALLRANGE: c_int = GRAYSRANGE;
/// Colour used for floor-height-change linedefs.
pub(super) const FDWALLCOLORS: c_int = BROWNS;
/// Colour range width for floor-height-change walls.
const FDWALLRANGE: c_int = BROWNRANGE;
/// Colour used for ceiling-height-change linedefs.
pub(super) const CDWALLCOLORS: c_int = YELLOWS;
/// Colour range width for ceiling-height-change walls.
const CDWALLRANGE: c_int = YELLOWRANGE;
/// Colour used for thing triangles.
pub(super) const THINGCOLORS: c_int = GREENS;
/// Colour range width for thing triangles.
pub(super) const THINGRANGE: c_int = GREENRANGE;
/// Colour used for secret walls (same as solid walls; they look identical
/// unless cheating).
pub(super) const SECRETWALLCOLORS: c_int = WALLCOLORS;
/// Colour range width for secret walls.
const SECRETWALLRANGE: c_int = WALLRANGE;
/// Colour used for the background grid.
pub(super) const GRIDCOLORS: c_int = GRAYS + GRAYSRANGE / 2;
/// Colour used for the central crosshair dot.
pub(super) const XHAIRCOLORS: c_int = GRAYS;

/// Magic header for automap event messages sent to `ST_Responder`.
///
/// The upper bytes encode `'a'` and `'m'`; the lower bytes identify the
/// specific sub-message.  C origin: `AM_MSGHEADER` in am_map.c.
pub(super) const AM_MSGHEADER: c_int = (('a' as c_int) << 24) + (('m' as c_int) << 16);

/// Event data value sent to `ST_Responder` when the automap is opened.
///
/// C origin: `AM_MSGENTERED` in am_map.c.
pub(super) const AM_MSGENTERED: c_int = AM_MSGHEADER | (('e' as c_int) << 8);

/// Event data value sent to `ST_Responder` when the automap is closed.
///
/// C origin: `AM_MSGEXITED` in am_map.c.
pub(super) const AM_MSGEXITED: c_int = AM_MSGHEADER | (('x' as c_int) << 8);

// Cohen-Sutherland outcode constants.

/// Cohen-Sutherland outcode bit: point is to the left of the clip rectangle.
pub(super) const OC_LEFT: c_int = 1;
/// Cohen-Sutherland outcode bit: point is to the right of the clip rectangle.
pub(super) const OC_RIGHT: c_int = 2;
/// Cohen-Sutherland outcode bit: point is below the clip rectangle (y > f_h).
pub(super) const OC_BOTTOM: c_int = 4;
/// Cohen-Sutherland outcode bit: point is above the clip rectangle (y < 0).
pub(super) const OC_TOP: c_int = 8;

/// Base radius used to scale the player-arrow line segments.
///
/// Chosen as `(8/7) * PLAYERRADIUS` to give the arrow a slightly larger extent
/// than the collision radius.  C origin: `R` in am_map.c.
const R_ARROW: c_int = (8 * PLAYERRADIUS) / 7;

/// A 2-D point in frame (screen-pixel) coordinates.
///
/// C origin: `fpoint_t` in am_map.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct fpoint_t {
    pub x: c_int,
    pub y: c_int,
}

/// A line segment in frame (screen-pixel) coordinates.
///
/// C origin: `fline_t` in am_map.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct fline_t {
    pub a: fpoint_t,
    pub b: fpoint_t,
}

/// A 2-D point in map (fixed-point world) coordinates.
///
/// C origin: `mpoint_t` in am_map.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct mpoint_t {
    pub x: fixed_t,
    pub y: fixed_t,
}

/// A line segment in map (fixed-point world) coordinates.
///
/// C origin: `mline_t` in am_map.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct mline_t {
    pub a: mpoint_t,
    pub b: mpoint_t,
}

/// Reciprocal-slope pair used by the (unused) slope clipping path.
///
/// `slp` is `dy/dx`; `islp` is `dx/dy`, both in fixed-point.
/// C origin: `islope_t` in am_map.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct islope_t {
    pub slp: fixed_t,
    pub islp: fixed_t,
}

/// Line segments that form the normal (non-cheat) player arrow.
///
/// Defined in map coordinates centred on the origin; scaled and rotated by
/// [`super::raster::draw_line_character`] before drawing.  C origin:
/// `player_arrow[]` in am_map.c.
pub(super) const PLAYER_ARROW: [mline_t; 7] = [
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t { x: R_ARROW, y: 0 },
    },
    mline_t {
        a: mpoint_t { x: R_ARROW, y: 0 },
        b: mpoint_t {
            x: R_ARROW - R_ARROW / 2,
            y: R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t { x: R_ARROW, y: 0 },
        b: mpoint_t {
            x: R_ARROW - R_ARROW / 2,
            y: -R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW - R_ARROW / 8,
            y: R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW - R_ARROW / 8,
            y: -R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + 3 * R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + 3 * R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: -R_ARROW / 4,
        },
    },
];

/// Line segments that form the cheat-mode player arrow (spells "DSGN" in the
/// tail).
///
/// Larger and more detailed than [`PLAYER_ARROW`]; displayed when `cheating`
/// is non-zero.  C origin: `cheat_player_arrow[]` in am_map.c.
pub(super) const CHEAT_ARROW: [mline_t; 16] = [
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t { x: R_ARROW, y: 0 },
    },
    mline_t {
        a: mpoint_t { x: R_ARROW, y: 0 },
        b: mpoint_t {
            x: R_ARROW - R_ARROW / 2,
            y: R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t { x: R_ARROW, y: 0 },
        b: mpoint_t {
            x: R_ARROW - R_ARROW / 2,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW - R_ARROW / 8,
            y: R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW - R_ARROW / 8,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + 3 * R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW + 3 * R_ARROW / 8,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW + R_ARROW / 8,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW / 2,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW / 2,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW / 2,
            y: -R_ARROW / 6,
        },
        b: mpoint_t {
            x: -R_ARROW / 2 + R_ARROW / 6,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW / 2 + R_ARROW / 6,
            y: -R_ARROW / 6,
        },
        b: mpoint_t {
            x: -R_ARROW / 2 + R_ARROW / 6,
            y: R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW / 6,
            y: 0,
        },
        b: mpoint_t {
            x: -R_ARROW / 6,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: -R_ARROW / 6,
            y: -R_ARROW / 6,
        },
        b: mpoint_t {
            x: 0,
            y: -R_ARROW / 6,
        },
    },
    mline_t {
        a: mpoint_t {
            x: 0,
            y: -R_ARROW / 6,
        },
        b: mpoint_t {
            x: 0,
            y: R_ARROW / 4,
        },
    },
    mline_t {
        a: mpoint_t {
            x: R_ARROW / 6,
            y: R_ARROW / 4,
        },
        b: mpoint_t {
            x: R_ARROW / 6,
            y: -R_ARROW / 7,
        },
    },
    mline_t {
        a: mpoint_t {
            x: R_ARROW / 6,
            y: -R_ARROW / 7,
        },
        b: mpoint_t {
            x: R_ARROW / 6 + R_ARROW / 32,
            y: -R_ARROW / 7 - R_ARROW / 32,
        },
    },
    mline_t {
        a: mpoint_t {
            x: R_ARROW / 6 + R_ARROW / 32,
            y: -R_ARROW / 7 - R_ARROW / 32,
        },
        b: mpoint_t {
            x: R_ARROW / 6 + R_ARROW / 10,
            y: -R_ARROW / 7,
        },
    },
];

/// Equilateral-triangle shape used to represent non-player things in cheat mode.
///
/// Vertices at roughly ±0.867 and 0.5 FRACUNIT — an equilateral triangle.
/// C origin: `triangle_guy[]` in am_map.c.
pub(super) const TRIANGLE_GUY: [mline_t; 3] = [
    mline_t {
        a: mpoint_t {
            x: (-0.867f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.5f64 * FRACUNIT as f64) as fixed_t,
        },
        b: mpoint_t {
            x: (0.867f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.5f64 * FRACUNIT as f64) as fixed_t,
        },
    },
    mline_t {
        a: mpoint_t {
            x: (0.867f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.5f64 * FRACUNIT as f64) as fixed_t,
        },
        b: mpoint_t { x: 0, y: FRACUNIT },
    },
    mline_t {
        a: mpoint_t { x: 0, y: FRACUNIT },
        b: mpoint_t {
            x: (-0.867f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.5f64 * FRACUNIT as f64) as fixed_t,
        },
    },
];

/// Thin isosceles triangle shape used to represent things in cheat mode.
///
/// A slender triangle pointing right; used for all things when `cheating == 2`.
/// C origin: `thintriangle_guy[]` in am_map.c.
pub(super) const THINTRIANGLE_GUY: [mline_t; 3] = [
    mline_t {
        a: mpoint_t {
            x: (-0.5f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.7f64 * FRACUNIT as f64) as fixed_t,
        },
        b: mpoint_t { x: FRACUNIT, y: 0 },
    },
    mline_t {
        a: mpoint_t { x: FRACUNIT, y: 0 },
        b: mpoint_t {
            x: (-0.5f64 * FRACUNIT as f64) as fixed_t,
            y: (0.7f64 * FRACUNIT as f64) as fixed_t,
        },
    },
    mline_t {
        a: mpoint_t {
            x: (-0.5f64 * FRACUNIT as f64) as fixed_t,
            y: (0.7f64 * FRACUNIT as f64) as fixed_t,
        },
        b: mpoint_t {
            x: (-0.5f64 * FRACUNIT as f64) as fixed_t,
            y: (-0.7f64 * FRACUNIT as f64) as fixed_t,
        },
    },
];
