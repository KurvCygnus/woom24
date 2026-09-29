//! The automap raster: framebuffer clear, Cohen-Sutherland map-line
//! clip, Bresenham frame-line draw, grid/walls/players/things/marks/
//! crosshair renderers, and the per-frame drawer (with the reseat
//! detect-by-diff block).

use std::ffi::c_int;
use std::os::raw::c_uint;

use super::consts::{
    fline_t, fpoint_t, mline_t, AM_NUMMARKPOINTS, BACKGROUND, CDWALLCOLORS, CHEAT_ARROW,
    FDWALLCOLORS, GRIDCOLORS, GRAYS, OC_BOTTOM, OC_LEFT, OC_RIGHT, OC_TOP, PLAYER_ARROW,
    SECRETWALLCOLORS, THINGCOLORS, THINTRIANGLE_GUY, TSWALLCOLORS, WALLCOLORS, WALLRANGE, WHITE,
    XHAIRCOLORS,
};
use super::state::{
    amclock, automapactive, cheating, f_h, f_w, f_x, f_y, fb, finit_height, finit_width, grid,
    lightlev, marknums, markpoints, m_h, m_w, m_x, m_x2, m_y, m_y2, plr,
};
use super::view::{cxmtof, cymtof};
use crate::doom::c_ffi::{mobj_t, sector_t, LinedefFlag, MAPBLOCKUNITS};
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::g_game::{deathmatch, netgame, playeringame, players, singledemo};
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH, I_VideoBuffer};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACBITS};
use crate::doom::p_setup::{bmaporgx, bmaporgy, lines, numlines, numsectors, sectors};
use crate::doom::tables::ANGLETOFINESHIFT;
use crate::doom::tables::{finecosine, finesine};
use crate::doom::v_video::{V_DrawPatch, V_MarkRect};

/// Advance the `lightlev` animation to the next level in the table.
///
/// This function is compiled but currently disabled (call site is commented
/// out in [`super::lifecycle::ticker`]).  It steps through `LITELEVELS` every 6 ticks.
/// C origin: `AM_updateLightLev` in am_map.c.
///
/// # Safety
///
/// Reads and writes mutable statics; must only be called with the automap
/// active.
#[doc(alias = "AM_updateLightLev")]
#[allow(dead_code)]
pub(super) unsafe fn update_light_lev() {
    static mut nexttic: c_int = 0;
    static LITELEVELS: [c_int; 8] = [0, 4, 7, 10, 12, 14, 15, 15];
    static mut litelevelscnt: c_int = 0;

    if amclock > nexttic {
        lightlev = LITELEVELS[litelevelscnt as usize];
        litelevelscnt += 1;
        if litelevelscnt == LITELEVELS.len() as c_int {
            litelevelscnt = 0;
        }
        nexttic = amclock + 6 - (amclock % 6);
    }
}

/// Fill the automap framebuffer rectangle with `color`.
///
/// Uses `ptr::write_bytes` to set `f_w * f_h` bytes starting at `fb`.
/// C origin: `AM_clearFB` in am_map.c.
///
/// # Safety
///
/// `fb` must point to a buffer of at least `f_w * f_h` bytes.
#[doc(alias = "AM_clearFB")]
unsafe fn clear_fb(color: c_int) {
    std::ptr::write_bytes(fb, color as u8, (f_w * f_h) as usize);
}

/// Compute the Cohen-Sutherland outcode for frame-coordinate point `(mx, my)`.
///
/// Returns a bitmask of [`OC_LEFT`], [`OC_RIGHT`], [`OC_TOP`], [`OC_BOTTOM`]
/// indicating which clip edges the point lies outside.
///
/// # Safety
///
/// Reads `f_h` and `f_w` mutable statics.
#[inline(always)]
unsafe fn dooutcode(mx: c_int, my: c_int) -> c_int {
    let mut oc = 0;
    if my < 0 {
        oc |= OC_TOP;
    } else if my >= f_h {
        oc |= OC_BOTTOM;
    }
    if mx < 0 {
        oc |= OC_LEFT;
    } else if mx >= f_w {
        oc |= OC_RIGHT;
    }
    oc
}

/// Clip map line `ml` to the current viewport and convert the result to frame
/// coordinates in `*fl`.
///
/// First performs a trivial reject in map coordinates (both endpoints outside
/// the same edge), then transforms to frame coordinates and applies a
/// Cohen-Sutherland iterative clip.  Returns 1 if the clipped segment is
/// visible, 0 if it was entirely clipped away.
///
/// C origin: `AM_clipMline` in am_map.c.
///
/// # Safety
///
/// `ml` and `fl` must be valid non-null pointers; mutable statics are read.
#[doc(alias = "AM_clipMline")]
unsafe fn clip_mline(ml: *mut mline_t, fl: *mut fline_t) -> c_int {
    let mut outcode1: c_int = 0;
    let mut outcode2: c_int = 0;
    let mut outside: c_int;
    let mut tmp: fpoint_t = fpoint_t { x: 0, y: 0 };
    let mut dx: c_int;
    let mut dy: c_int;

    // Trivial rejects and outcodes in map coords.
    if (*ml).a.y > m_y2 {
        outcode1 = OC_TOP;
    } else if (*ml).a.y < m_y {
        outcode1 = OC_BOTTOM;
    }
    if (*ml).b.y > m_y2 {
        outcode2 = OC_TOP;
    } else if (*ml).b.y < m_y {
        outcode2 = OC_BOTTOM;
    }
    if outcode1 & outcode2 != 0 {
        return 0;
    }

    if (*ml).a.x < m_x {
        outcode1 |= OC_LEFT;
    } else if (*ml).a.x > m_x2 {
        outcode1 |= OC_RIGHT;
    }
    if (*ml).b.x < m_x {
        outcode2 |= OC_LEFT;
    } else if (*ml).b.x > m_x2 {
        outcode2 |= OC_RIGHT;
    }
    if outcode1 & outcode2 != 0 {
        return 0;
    }

    // Transform to frame-buffer coordinates.
    (*fl).a.x = cxmtof((*ml).a.x);
    (*fl).a.y = cymtof((*ml).a.y);
    (*fl).b.x = cxmtof((*ml).b.x);
    (*fl).b.y = cymtof((*ml).b.y);

    outcode1 = dooutcode((*fl).a.x, (*fl).a.y);
    outcode2 = dooutcode((*fl).b.x, (*fl).b.y);

    if outcode1 & outcode2 != 0 {
        return 0;
    }

    while outcode1 | outcode2 != 0 {
        if outcode1 != 0 {
            outside = outcode1;
        } else {
            outside = outcode2;
        }

        if outside & OC_TOP != 0 {
            dy = (*fl).a.y - (*fl).b.y;
            dx = (*fl).b.x - (*fl).a.x;
            tmp.x = (*fl).a.x + (dx * (*fl).a.y) / dy;
            tmp.y = 0;
        } else if outside & OC_BOTTOM != 0 {
            dy = (*fl).a.y - (*fl).b.y;
            dx = (*fl).b.x - (*fl).a.x;
            tmp.x = (*fl).a.x + (dx * ((*fl).a.y - f_h)) / dy;
            tmp.y = f_h - 1;
        } else if outside & OC_RIGHT != 0 {
            dy = (*fl).b.y - (*fl).a.y;
            dx = (*fl).b.x - (*fl).a.x;
            tmp.y = (*fl).a.y + (dy * (f_w - 1 - (*fl).a.x)) / dx;
            tmp.x = f_w - 1;
        } else if outside & OC_LEFT != 0 {
            dy = (*fl).b.y - (*fl).a.y;
            dx = (*fl).b.x - (*fl).a.x;
            tmp.y = (*fl).a.y + (dy * (-(*fl).a.x)) / dx;
            tmp.x = 0;
        } else {
            tmp.x = 0;
            tmp.y = 0;
        }

        if outside == outcode1 {
            (*fl).a = tmp;
            outcode1 = dooutcode((*fl).a.x, (*fl).a.y);
        } else {
            (*fl).b = tmp;
            outcode2 = dooutcode((*fl).b.x, (*fl).b.y);
        }

        if outcode1 & outcode2 != 0 {
            return 0;
        }
    }

    1
}

/// Rasterise a frame-coordinate line segment `fl` into the framebuffer using
/// Bresenham's algorithm.
///
/// If either endpoint lies outside the frame bounds, the function increments a
/// debug counter (`fuck`) and returns without drawing; this matches the C
/// behaviour and is intended as an assertion in debug builds.
///
/// C origin: `AM_drawFline` in am_map.c.
///
/// # Safety
///
/// `fl` must be a valid non-null pointer; `fb` must point to a buffer of at
/// least `f_w * f_h` bytes.
#[doc(alias = "AM_drawFline")]
unsafe fn draw_fline(fl: *mut fline_t, color: c_int) {
    static mut fuck: c_int = 0;

    if (*fl).a.x < 0
        || (*fl).a.x >= f_w
        || (*fl).a.y < 0
        || (*fl).a.y >= f_h
        || (*fl).b.x < 0
        || (*fl).b.x >= f_w
        || (*fl).b.y < 0
        || (*fl).b.y >= f_h
    {
        // For debugging only — matches C behaviour of returning without drawing.
        fuck += 1;
        return;
    }

    let dx = (*fl).b.x - (*fl).a.x;
    let ax = 2 * (if dx < 0 { -dx } else { dx });
    let sx = if dx < 0 { -1 } else { 1 };

    let dy = (*fl).b.y - (*fl).a.y;
    let ay = 2 * (if dy < 0 { -dy } else { dy });
    let sy = if dy < 0 { -1 } else { 1 };

    let mut x = (*fl).a.x;
    let mut y = (*fl).a.y;

    if ax > ay {
        let mut d = ay - ax / 2;
        loop {
            *fb.add((y * f_w + x) as usize) = color as u8;
            if x == (*fl).b.x {
                return;
            }
            if d >= 0 {
                y += sy;
                d -= ax;
            }
            x += sx;
            d += ay;
        }
    } else {
        let mut d = ax - ay / 2;
        loop {
            *fb.add((y * f_w + x) as usize) = color as u8;
            if y == (*fl).b.y {
                return;
            }
            if d >= 0 {
                x += sx;
                d -= ay;
            }
            y += sy;
            d += ax;
        }
    }
}

/// Clip and draw map line `ml` in `color`.
///
/// Clips `ml` to the current viewport via [`clip_mline`]; if the result is
/// visible, rasterises it with [`draw_fline`].
///
/// C origin: `AM_drawMline` in am_map.c.
///
/// # Safety
///
/// `ml` must be a valid non-null pointer; mutable statics must be valid.
#[doc(alias = "AM_drawMline")]
unsafe fn draw_mline(ml: *mut mline_t, color: c_int) {
    static mut fl: fline_t = fline_t {
        a: fpoint_t { x: 0, y: 0 },
        b: fpoint_t { x: 0, y: 0 },
    };
    if clip_mline(ml, &raw mut fl) != 0 {
        draw_fline(&raw mut fl, color);
    }
}

/// Draw the blockmap-aligned background grid in `color`.
///
/// Grid lines are spaced `MAPBLOCKUNITS << FRACBITS` apart and are aligned to
/// the blockmap origin (`bmaporgx`, `bmaporgy`) so that the grid matches the
/// collision-detection grid.  Draws vertical lines first, then horizontal.
///
/// C origin: `AM_drawGrid` in am_map.c.
///
/// # Safety
///
/// Reads `bmaporgx` / `bmaporgy` mutable statics; `lines` / `numlines` must
/// be valid.
#[doc(alias = "AM_drawGrid")]
unsafe fn draw_grid(color: c_int) {
    let mut start: fixed_t;
    let mut end: fixed_t;
    let mut ml: mline_t = mline_t {
        a: super::consts::mpoint_t { x: 0, y: 0 },
        b: super::consts::mpoint_t { x: 0, y: 0 },
    };

    // Vertical gridlines.
    start = m_x;
    if (start - bmaporgx) % (MAPBLOCKUNITS << FRACBITS) != 0 {
        start += (MAPBLOCKUNITS << FRACBITS) - ((start - bmaporgx) % (MAPBLOCKUNITS << FRACBITS));
    }
    end = m_x + m_w;

    ml.a.y = m_y;
    ml.b.y = m_y + m_h;
    let mut x = start;
    while x < end {
        ml.a.x = x;
        ml.b.x = x;
        draw_mline(&mut ml, color);
        x += MAPBLOCKUNITS << FRACBITS;
    }

    // Horizontal gridlines.
    start = m_y;
    if (start - bmaporgy) % (MAPBLOCKUNITS << FRACBITS) != 0 {
        start += (MAPBLOCKUNITS << FRACBITS) - ((start - bmaporgy) % (MAPBLOCKUNITS << FRACBITS));
    }
    end = m_y + m_h;

    ml.a.x = m_x;
    ml.b.x = m_x + m_w;
    let mut y = start;
    while y < end {
        ml.a.y = y;
        ml.b.y = y;
        draw_mline(&mut ml, color);
        y += MAPBLOCKUNITS << FRACBITS;
    }
}

/// Draw all visible level linedefs with appropriate colours.
///
/// Colour selection rules (in priority order):
/// - Lines with `MAPPED` flag or cheat mode on: drawn; unless `DONTDRAW` and
///   not cheating.
///   - One-sided (no backsector): `WALLCOLORS`.
///   - Special 39 (teleporter): mid-range red.
///   - `SECRET` flag: secret wall colour (red while not cheating; same when
///     cheating).
///   - Floor-height difference: `FDWALLCOLORS` (brown).
///   - Ceiling-height difference: `CDWALLCOLORS` (yellow).
///   - Otherwise cheating: `TSWALLCOLORS` (gray).
/// - Computer-area-map powerup (`powers[4]` / `pw_allmap`): gray (no
///   `DONTDRAW` lines).
///
/// C origin: `AM_drawWalls` in am_map.c.
///
/// # Safety
///
/// `lines`, `numlines`, `sectors`, and `plr` must be valid.
#[doc(alias = "AM_drawWalls")]
unsafe fn draw_walls() {
    static mut l: mline_t = mline_t {
        a: super::consts::mpoint_t { x: 0, y: 0 },
        b: super::consts::mpoint_t { x: 0, y: 0 },
    };

    for i in 0..numlines {
        let li = &*lines.add(i as usize);
        l.a.x = (*li.v1).x;
        l.a.y = (*li.v1).y;
        l.b.x = (*li.v2).x;
        l.b.y = (*li.v2).y;

        let flags = li.flags as c_int;

        if cheating != 0 || (flags & LinedefFlag::MAPPED as c_int) != 0 {
            if (flags & LinedefFlag::DONTDRAW as c_int) != 0 && cheating == 0 {
                continue;
            }
            if li.backsector.is_null() {
                draw_mline(&raw mut l, WALLCOLORS + lightlev);
            } else {
                let back = li.backsector as *mut sector_t;
                let front = li.frontsector as *mut sector_t;
                if li.special == 39 {
                    // teleporters
                    draw_mline(&raw mut l, WALLCOLORS + WALLRANGE / 2);
                } else if (flags & LinedefFlag::SECRET as c_int) != 0 {
                    if cheating != 0 {
                        draw_mline(&raw mut l, SECRETWALLCOLORS + lightlev);
                    } else {
                        draw_mline(&raw mut l, WALLCOLORS + lightlev);
                    }
                } else if (*back).floorheight != (*front).floorheight {
                    draw_mline(&raw mut l, FDWALLCOLORS + lightlev);
                } else if (*back).ceilingheight != (*front).ceilingheight {
                    draw_mline(&raw mut l, CDWALLCOLORS + lightlev);
                } else if cheating != 0 {
                    draw_mline(&raw mut l, TSWALLCOLORS + lightlev);
                }
            }
        } else if (*plr).powers[4] != 0 {
            // pw_allmap
            if (flags & LinedefFlag::DONTDRAW as c_int) == 0 {
                draw_mline(&raw mut l, GRAYS + 3);
            }
        }
    }
}

/// Rotate map-coordinate vector `(*x, *y)` by angle `a`.
///
/// Uses the fine-angle lookup tables (`finecosine`, `finesine`) to apply a 2-D
/// rotation in fixed-point arithmetic.  `a` is a Doom angle (0 = east,
/// increasing counter-clockwise), shifted right by `ANGLETOFINESHIFT` to
/// index the lookup table.
///
/// C origin: `AM_rotate` in am_map.c.
///
/// # Safety
///
/// `x` and `y` must be valid non-null pointers; fine-angle tables must be
/// initialised.
#[doc(alias = "AM_rotate")]
unsafe fn rotate(x: *mut fixed_t, y: *mut fixed_t, a: c_uint) {
    let tmpx = FixedMul(*x, *finecosine.0.add((a >> ANGLETOFINESHIFT) as usize))
        - FixedMul(*y, finesine[(a >> ANGLETOFINESHIFT) as usize]);
    *y = FixedMul(*x, finesine[(a >> ANGLETOFINESHIFT) as usize])
        + FixedMul(*y, *finecosine.0.add((a >> ANGLETOFINESHIFT) as usize));
    *x = tmpx;
}

/// Scale, rotate, translate, and draw a multi-segment line-character shape.
///
/// For each segment in `lineguy[0..lineguylines]`: optionally scales each
/// endpoint by `scale` (if non-zero), optionally rotates by `angle` (if
/// non-zero), then translates to `(x, y)` and draws the resulting map line.
/// Used for player arrows and thing triangles.
///
/// C origin: `AM_drawLineCharacter` in am_map.c.
///
/// # Safety
///
/// `lineguy` must point to at least `lineguylines` valid `mline_t` entries;
/// mutable statics must be valid.
#[doc(alias = "AM_drawLineCharacter")]
unsafe fn draw_line_character(
    lineguy: *mut mline_t,
    lineguylines: c_int,
    scale: fixed_t,
    angle: c_uint,
    color: c_int,
    x: fixed_t,
    y: fixed_t,
) {
    let mut l: mline_t = mline_t {
        a: super::consts::mpoint_t { x: 0, y: 0 },
        b: super::consts::mpoint_t { x: 0, y: 0 },
    };

    for i in 0..lineguylines {
        l.a.x = (*lineguy.add(i as usize)).a.x;
        l.a.y = (*lineguy.add(i as usize)).a.y;

        if scale != 0 {
            l.a.x = FixedMul(scale, l.a.x);
            l.a.y = FixedMul(scale, l.a.y);
        }

        if angle != 0 {
            rotate(&mut l.a.x, &mut l.a.y, angle);
        }

        l.a.x += x;
        l.a.y += y;

        l.b.x = (*lineguy.add(i as usize)).b.x;
        l.b.y = (*lineguy.add(i as usize)).b.y;

        if scale != 0 {
            l.b.x = FixedMul(scale, l.b.x);
            l.b.y = FixedMul(scale, l.b.y);
        }

        if angle != 0 {
            rotate(&mut l.b.x, &mut l.b.y, angle);
        }

        l.b.x += x;
        l.b.y += y;

        draw_mline(&mut l, color);
    }
}

/// Draw player arrows for all active players.
///
/// In a non-network game, draws the console player's arrow (cheat arrow if
/// `cheating != 0`, normal arrow otherwise) in white.  In a network game,
/// draws each active player in a player-colour (green/grey/brown/red);
/// invisible players are drawn in colour 246.  In deathmatch outside a demo,
/// only the console player is drawn.
///
/// C origin: `AM_drawPlayers` in am_map.c.
///
/// # Safety
///
/// `plr` and `players` must be valid; `mobj_t` pointers within player structs
/// must be valid.
#[doc(alias = "AM_drawPlayers")]
unsafe fn draw_players() {
    let their_colors: [c_int; 4] =
        [super::consts::GREENS, GRAYS, super::consts::BROWNS, super::consts::REDS];
    let mut their_color: c_int = -1;
    let mut color: c_int;

    if netgame == 0 {
        if cheating != 0 {
            draw_line_character(
                CHEAT_ARROW.as_ptr() as *mut mline_t,
                CHEAT_ARROW.len() as c_int,
                0,
                (*((*plr).mo as *mut mobj_t)).angle,
                WHITE,
                (*((*plr).mo as *mut mobj_t)).x,
                (*((*plr).mo as *mut mobj_t)).y,
            );
        } else {
            draw_line_character(
                PLAYER_ARROW.as_ptr() as *mut mline_t,
                PLAYER_ARROW.len() as c_int,
                0,
                (*((*plr).mo as *mut mobj_t)).angle,
                WHITE,
                (*((*plr).mo as *mut mobj_t)).x,
                (*((*plr).mo as *mut mobj_t)).y,
            );
        }
        return;
    }

    for i in 0..MAXPLAYERS {
        their_color += 1;
        let p = std::ptr::addr_of_mut!(players[0]).add(i);

        if deathmatch != 0 && singledemo == 0 && p != plr {
            continue;
        }

        if playeringame[i] == 0 {
            continue;
        }

        if (*p).powers[2] != 0 {
            // pw_invisibility
            color = 246;
        } else {
            color = their_colors[their_color as usize];
        }

        draw_line_character(
            PLAYER_ARROW.as_ptr() as *mut mline_t,
            PLAYER_ARROW.len() as c_int,
            0,
            (*((*p).mo as *mut mobj_t)).angle,
            color,
            (*((*p).mo as *mut mobj_t)).x,
            (*((*p).mo as *mut mobj_t)).y,
        );
    }
}

/// Draw thin-triangle icons for all things in every sector.
///
/// Iterates `sectors[0..numsectors]` and follows each sector's `thinglist`
/// linked list.  Each thing is drawn as a [`THINTRIANGLE_GUY`] scaled to
/// `16 << FRACBITS` map units.  Only used when `cheating == 2`.
///
/// C origin: `AM_drawThings` in am_map.c.
///
/// # Safety
///
/// `sectors` / `numsectors` must be valid; thing linked lists must be
/// properly terminated.
#[doc(alias = "AM_drawThings")]
unsafe fn draw_things(colors: c_int, _colorrange: c_int) {
    for i in 0..numsectors {
        let mut t = (*sectors.add(i as usize)).thinglist as *mut mobj_t;
        while !t.is_null() {
            draw_line_character(
                THINTRIANGLE_GUY.as_ptr() as *mut mline_t,
                THINTRIANGLE_GUY.len() as c_int,
                16 << FRACBITS,
                (*t).angle,
                colors + lightlev,
                (*t).x,
                (*t).y,
            );
            t = (*t).snext as *mut mobj_t;
        }
    }
}

/// Draw the `AMMNUM*` glyph for each placed mark point.
///
/// Only draws marks whose `x` field is not `-1` and whose screen position lies
/// within the frame bounds (with a 5x6 pixel margin for the glyph size).
///
/// C origin: `AM_drawMarks` in am_map.c.
///
/// # Safety
///
/// `marknums` patches must have been loaded by [`super::lifecycle::load_pics`];
/// mutable statics must be valid.
#[doc(alias = "AM_drawMarks")]
unsafe fn draw_marks() {
    for i in 0..AM_NUMMARKPOINTS {
        if markpoints[i].x != -1 {
            let w = 5;
            let h = 6;
            let fx = cxmtof(markpoints[i].x);
            let fy = cymtof(markpoints[i].y);
            if fx >= f_x && fx <= f_w - w && fy >= f_y && fy <= f_h - h {
                V_DrawPatch(fx, fy, marknums[i]);
            }
        }
    }
}

/// Draw a single crosshair pixel at the centre of the automap frame.
///
/// Sets the pixel at `fb[f_w * (f_h + 1) / 2]` to `color`.
/// C origin: `AM_drawCrosshair` in am_map.c.
///
/// # Safety
///
/// `fb` must point to a buffer of at least `f_w * f_h` bytes; the computed
/// index must not overflow.
#[doc(alias = "AM_drawCrosshair")]
unsafe fn draw_crosshair(color: c_int) {
    *fb.add(((f_w * (f_h + 1)) / 2) as usize) = color as u8;
}

/// Render the automap for the current frame.
///
/// Returns immediately if `automapactive == 0`.  Otherwise: clears the
/// framebuffer, optionally draws the grid, draws walls, players, things (if
/// `cheating == 2`), crosshair, and mark points, then marks the dirty
/// rectangle via `V_MarkRect`.
///
/// Called from C code in `g_game.c` once per frame.
/// C origin: `AM_Drawer` in am_map.c.
///
/// # Safety
///
/// All automap state must be valid (automap must be active with a level
/// loaded).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` reaches the upstream name through the root shim.
#[doc(alias = "AM_Drawer")]
#[export_name = "AM_Drawer"]
pub unsafe extern "C" fn drawer() {
    if automapactive == 0 {
        return;
    }

    // Fix round 1 (Critical 2): a video_cfg reconfiguration while the map is
    // open moves the raster under the latched video state; re-seat before
    // the first `AM_clearFB` write. The fb compare catches even an
    // A->B->A pair of swaps between two map frames (finit dims alone would
    // not); detect-by-diff so normal frames pay nothing and user zoom is
    // untouched.
    if fb != I_VideoBuffer || finit_width != SCREENWIDTH || finit_height != SCREENHEIGHT - 32 {
        super::reseat::AM_reseatVideoState();
    }

    clear_fb(BACKGROUND);
    if grid != 0 {
        draw_grid(GRIDCOLORS);
    }
    draw_walls();
    draw_players();
    if cheating == 2 {
        draw_things(THINGCOLORS, super::consts::THINGRANGE);
    }
    draw_crosshair(XHAIRCOLORS);
    draw_marks();

    V_MarkRect(f_x, f_y, f_w, f_h);
}

