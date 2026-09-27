//! Special sector/line action dispatcher: texture animation, height and
//! lighting changes, line tag handling, sector triggers, and the donut
//! effect, plus the pure map-query geometry family
//! (`get_side`/`get_sector`/`two_sided`/`get_next_sector` and the seven
//! `P_Find*` sweeps) used by the floor/ceiling/platform code --
//! bit-exact with `vendor/doomgeneric/p_spec.c`.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the numeric vocabulary: the animation-table bounds
//!   (`MAXANIMS`/`MAXLINEANIMS`), the adjoining-sector overrun bound
//!   (`MAX_ADJOINING_SECTORS`), the `vld_*` / `floor_*` / `ceiling_*` /
//!   `plat_*` / `stair_*` movement-type codes, and `pw_ironfeet`
//! - `anims.rs` -- the animation data home (`anim_t`/`animdef_t`
//!   layout-pinned types, the `ANIMDEFS` table, the `anims`/`lastanim`
//!   statics), the identity `deh_string` hook, and `init_pic_anims`
//! - `geometry.rs` -- the pure map-query family: `get_side`,
//!   `get_sector`, `two_sided`, `get_next_sector`,
//!   `lowest_floor_surrounding`, `highest_floor_surrounding`,
//!   `next_highest_floor` (with the vanilla adjoining-sector overrun
//!   emulation, catalog entry 13), `lowest_ceiling_surrounding`,
//!   `highest_ceiling_surrounding`, `sector_from_line_tag`,
//!   `min_surrounding_light`
//! - `crossline.rs` -- `cross_special_line` (the one-shot/retriggerable
//!   dispatch with the special-clear writes) and `shoot_special_line`
//! - `ticker.rs` -- the per-tic statics (`numlinespecials`,
//!   `linespeciallist`, `levelTimer`, `levelTimeCount`) and
//!   `update_specials` (level timer, animation phase advance, wall
//!   scroll, button countdown)
//! - `player_sector.rs` -- `player_in_special_sector` (the environmental
//!   damage ladder with the `leveltime & 0x1f` cadence and the
//!   `P_Random() < 5` suit-bypass draw)
//! - `donut.rs` -- the entry-3 workaround unit: `do_donut` plus the
//!   private `donut_overrun` default-value hook
//! - `spawn.rs` -- `spawn_specials` (the once-per-map-load thinker
//!   sweep and registry init)
//! - `anchor.rs` -- `spec_link_anchor`, the C-linkage keep-alive anchor
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! (`anim_frame_pic`) and is covered by Deterministic Aspects. The
//! module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below. Every freeze-zone importer (`p_setup`,
//! `p_floor`, `p_doors`, `p_ceilng`, `p_plats`, `p_lights`, `p_map`,
//! `p_switch`, `p_tick`, `p_user`) still names the upstream identifiers
//! through the root.
//!
//! Cross-module contracts documented once here:
//! - **try_move drain ordering** (shared with p_map): `cross_special_line`
//!   is called INSIDE p_map's `try_move` drain
//!   (`p_map/mod.rs` pins "bounded spechit drain -> P_CrossSpecialLine
//!   ordering"); the root shim forwards to the same function item, so the
//!   call chain shape is untouched by this graduation.
//! - **per-tic order** (shared with p_tick): `update_specials` runs from
//!   `P_Ticker` before `P_RespawnSpecials` (pinned by `p_tick/mod.rs`).
//! - **data home for the cross-subfile statics**: `levelTimer` /
//!   `levelTimeCount` are written by `spawn::spawn_specials` and read by
//!   `ticker::update_specials`; they live in `ticker.rs` next to the
//!   scrolling-wall registry, and `spawn.rs` reaches them via
//!   `super::ticker` -- one source of truth.
//! - **Dependency directions**: the geometry family only reads loaded map
//!   data (`sectors`/`sides`/`numsectors`); the dispatch halves call the
//!   `EV_*` action functions and never write map geometry. No C
//!   translation unit references any of these symbols
//!   (`doomgeneric-sys/build.rs` excludes `p_spec.c`), and NO extern
//!   block anywhere in the tree declares them by symbol -- the
//!   `#[export_name]` pins below are pure symbol-set conservatism (the
//!   2026-09-27 maintainer ruling), not load-bearing links.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the `upstream-name shim` re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. Functions only:
//! statics/consts/tables/types keep their upstream names (data-tier
//! renaming comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_InitPicAnims` | `anims::init_pic_anims` | glue | map-load marshalling over `ANIMDEFS` (lump resolution + skip rules); shim + pin; upstream `p_spec.c:143` |
//! | `getSide` | `geometry::get_side` | dtmc (whole-body; pure map query) | sidenum resolve through `sides`; feeds mover target computation; shim + pin; upstream `p_spec.c:202-203` |
//! | `getSector` | `geometry::get_sector` | dtmc (whole-body; pure map query) | back-side sector resolve; callers guard the one-sided null; shim + pin; upstream `p_spec.c:218-219` |
//! | `twoSided` | `geometry::two_sided` | dtmc (whole-body; pure map query) | raw `ML_TWOSIDED` mask (not a normalized boolean); shim + pin; upstream `p_spec.c:233-234` |
//! | `getNextSector` | `geometry::get_next_sector` | dtmc (whole-body; pure map query) | the neighbor oracle: null arm for one-sided lines, else the opposite sector; the donut and every sweep build on it; shim + pin; upstream `p_spec.c:249-250` |
//! | `P_FindLowestFloorSurrounding` | `geometry::lowest_floor_surrounding` | dtmc (whole-body pure) | own-floor initial value (no sentinel); min neighbor sweep; shim + pin; upstream `p_spec.c:269` |
//! | `P_FindHighestFloorSurrounding` | `geometry::highest_floor_surrounding` | dtmc (whole-body pure) | `-500 * FRACUNIT` sentinel initial value (baseline-pinned); max sweep; shim + pin; upstream `p_spec.c:296` |
//! | `P_FindNextHighestFloor` | `geometry::next_highest_floor` | dtmc (whole-body; contains the adjoining-sectors overrun emulation) | `heightlist[MAX+2]` sweep; the `h == MAX+1` stack-shadow write + `h == MAX+2` `i_error!` are catalog entry 13, interleaved with the sweep -- never split; 20/21/22 boundary baseline-pinned (commit `3841018`, mutation-checked); shim + pin; upstream `p_spec.c:329-330` |
//! | `P_FindLowestCeilingSurrounding` | `geometry::lowest_ceiling_surrounding` | dtmc (whole-body pure) | `INT_MAX` sentinel initial value (baseline-pinned); shim + pin; upstream `p_spec.c:391-392` |
//! | `P_FindHighestCeilingSurrounding` | `geometry::highest_ceiling_surrounding` | dtmc (whole-body pure) | 0 sentinel initial value (baseline-pinned); shim + pin; upstream `p_spec.c:417` |
//! | `P_FindSectorFromLineTag` | `geometry::sector_from_line_tag` | dtmc (whole-body pure) | the `start + 1` tag-scan iterator protocol of every action loop (baseline-pinned incl. the -1 end); shim + pin; upstream `p_spec.c:443-444` |
//! | `P_FindMinSurroundingLight` | `geometry::min_surrounding_light` | dtmc (whole-body pure) | min neighbor light clamped to `max`; p_lights strobe/flash target; shim + pin; upstream `p_spec.c:463-464` |
//! | `P_CrossSpecialLine` | `crossline::cross_special_line` | dtmc (whole-body; dispatch order + special-clear) | projectile/monster filter, one-shot `special = 0` clears vs retriggerable arms, 52/124 exits without clearing, 125/126 monster-only match guards; called inside p_map's `try_move` drain (contract above); shim + pin; upstream `p_spec.c:502` |
//! | `P_ShootSpecialLine` | `crossline::shoot_special_line` | dtmc (whole-body) | impact specials 24/46/47; non-player 46-only gate; `EV_*` then `P_ChangeSwitchTexture` order; shim + pin; upstream `p_spec.c:969` |
//! | `P_PlayerInSpecialSector` | `player_sector::player_in_special_sector` | dtmc (whole-body; `leveltime & 0x1f` gates + `P_Random() < 5` suit bypass draw) | damage ladder 5/7/(4\|16)/9/11; the suit-bypass draw happens only after the suit check short-circuits false -- draw count/order pinned by the goldens; shim + pin; upstream `p_spec.c:1019` |
//! | `P_UpdateSpecials` | `ticker::update_specials` | dtmc (whole-body; anim phase formula extracted to `dtmc`) | level timer, `anim_frame_pic` phase advance over `anims..lastanim`, special-48 scroll, button countdown; per-tic order pinned by `p_tick/mod.rs`; shim + pin; upstream `p_spec.c:1093` |
//! | `EV_DoDonut` | `donut::do_donut` | dtmc (whole-body; thinker wiring + entry-3 workaround hook) | s1/s2/s3 walk, rising-then-lowering thinker order, null-s2 warning + break, null-s3 `donut_overrun` hook; shim + pin; upstream `p_spec.c:1257` |
//! | `P_SpawnSpecials` | `spawn::spawn_specials` | glue | map-load marshalling: deathmatch timer init, sector-special thinker sweep (order demo-relevant but not separable), special-48 registry, registry clears; shim + pin; upstream `p_spec.c:1374` |
//! | `P_Spec_Link_Anchor` | `anchor::spec_link_anchor` | glue | keep-alive anchor taking all 18 addresses; zero callers in this port (absent from the `doomgeneric.rs` anchor list) -- kept dead-but-exported for symbol-set stability, retire with the freeze zone; shim + pin |
//! | `DonutOverrun` (private `unsafe fn`) | `donut::donut_overrun` | glue | entry-3 core: first-call `-donut` parse + cached (0, 0x16) Win98 defaults; never a C symbol -- doc alias only (spechit_overrun precedent); upstream `p_spec.c:1178` |
//! | `DEH_String` (private inline) | `anims::deh_string` | glue | identity shim for the chocolate Dehacked hook; never a C symbol -- doc alias only |
//! | -- (new extraction; no named C counterpart) | `dtmc::anim_frame_pic` | dtmc (extracted) | the `base + ((leveltime / speed + i) % numpics)` phase formula from `P_UpdateSpecials`; integer division/modulo order load-bearing; baseline vectors landed pre-move (commit `3841018`) |
//! | `anims` / `lastanim` (`#[no_mangle]` statics) | `anims` | data | upstream names + `#[no_mangle]` retained; no module-root path consumers, so no path-stability re-export (data-tier renaming comes with freeze-zone retirement) |
//! | `numlinespecials` / `linespeciallist` / `levelTimer` / `levelTimeCount` (`#[no_mangle]` statics) | `ticker` | data | upstream names + `#[no_mangle]` retained; `levelTimer`/`levelTimeCount` written by `spawn`, read by `ticker` (one data home); no root path consumers |
//! | `anim_t` / `animdef_t` / `ANIMDEFS` | `anims` | data | layout-pinned (20/28 bytes, `layout_checks`); `ANIMDEFS` sentinel row baseline-tested |
//! | `MAXANIMS` / `MAXLINEANIMS` / `MAX_ADJOINING_SECTORS` / `vld_*` / `floor_*` / `ceiling_*` / `plat_*` / `stair_*` / `pw_ironfeet` | `consts` | data | `pub(super)`; module-internal vocabulary, no external consumers |
//!
//! ## Deterministic Aspects
//!
//! The module feeds two demo-observable channels: the mover-target
//! geometry (every floor/ceiling/light thinker picks its target through
//! the `geometry.rs` family) and the per-tic special effects
//! (`update_specials`, `player_in_special_sector`). Load-bearing pieces:
//!
//! - **geometry family**: pure queries over loaded map data whose
//!   results set floor/ceiling/light targets every mover thinker
//!   consumes -- the genuinely qualifying part wholesale, moved whole
//!   (unlike p_map there is no marshalling shell to leave behind). The
//!   synthetic-sector baseline vectors (commit `3841018`, retargeted to
//!   `geometry::tests`) pin the sidenum walks, the null arms, the
//!   sentinel initial values, the tag-scan protocol, and the
//!   20/21/22-adjoining-sector overrun boundary.
//! - **adjoining-sectors overrun** (`geometry::next_highest_floor`,
//!   catalog entry 13): the vanilla stack-overflow emulation is
//!   interleaved with the heightlist sweep and kept in the body -- the
//!   `h == MAX_ADJOINING_SECTORS + 1` arm writes the filter threshold
//!   `height` itself (the observable trample: later neighbors qualify
//!   against the raised threshold, pinned by the 23-line drive), and
//!   `h == MAX_ADJOINING_SECTORS + 2` is chocolate's
//!   `I_Error("Sector with more than 22 adjoining sectors. Vanilla will
//!   crash here")`. Never "fix" the array bound: the shadow write IS the
//!   shipped behavior (catalog G2 discipline).
//! - **RNG ledger**: exactly one `P_Random` draw in the module --
//!   `player_in_special_sector`'s `P_Random() < 5` suit bypass for
//!   specials 4/16, drawn only when a radiation suit is worn, and only
//!   after the `||` short-circuits the suit check false. Draw position
//!   in the per-tic stream is pinned by the F9 goldens.
//! - **animation phase** (`ticker::update_specials` +
//!   `dtmc::anim_frame_pic`): the extracted formula decides which frame
//!   of every animated texture/flat is visible per tic; the walk order
//!   (`anims[0..lastanim)`, frames `base..base + numpics`) and the
//!   translation-table write order stay at the call site.
//! - **special-clear order** (`crossline::cross_special_line`): each
//!   one-shot arm runs its `EV_*` handler, then writes
//!   `line->special = 0`; the retriggerable range never clears. The
//!   call site inside p_map's `try_move` drain is order-pinned by
//!   `p_map/mod.rs`.
//! - **map-load order** (`spawn::spawn_specials`): runs once per level;
//!   the thinker spawn order is demo-relevant but not separable (glue),
//!   and the statics it writes live in one data home (`ticker.rs`).

pub mod anchor;
pub mod anims;
pub mod consts;
pub mod crossline;
pub mod donut;
pub mod dtmc;
pub mod geometry;
pub mod player_sector;
pub mod spawn;
pub mod ticker;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_spec::P_*` / `get*` / `EV_*`). The C
//* symbol each shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name set
//* stays byte-identical to the pre-split module. No extern block in the
//* tree declares any of these symbols (verified across all doom extern
//* blocks; `doomgeneric-sys/build.rs` excludes `p_spec.c`), so the pins
//* are pure surface-stability conservatism. Shims die with the freeze
//* zone.
pub use anchor::spec_link_anchor as P_Spec_Link_Anchor;
pub use anims::init_pic_anims as P_InitPicAnims;
pub use crossline::{cross_special_line as P_CrossSpecialLine, shoot_special_line as P_ShootSpecialLine};
pub use donut::do_donut as EV_DoDonut;
pub use geometry::{
    get_next_sector as getNextSector, get_sector as getSector, get_side as getSide,
    highest_ceiling_surrounding as P_FindHighestCeilingSurrounding,
    highest_floor_surrounding as P_FindHighestFloorSurrounding,
    lowest_ceiling_surrounding as P_FindLowestCeilingSurrounding,
    lowest_floor_surrounding as P_FindLowestFloorSurrounding,
    min_surrounding_light as P_FindMinSurroundingLight,
    next_highest_floor as P_FindNextHighestFloor,
    sector_from_line_tag as P_FindSectorFromLineTag, two_sided as twoSided,
};
pub use player_sector::player_in_special_sector as P_PlayerInSpecialSector;
pub use spawn::spawn_specials as P_SpawnSpecials;
pub use ticker::update_specials as P_UpdateSpecials;
