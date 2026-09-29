//! The intermission data tables: level-node screen positions, the
//! per-episode animation configs and their mutable runtime states, and
//! the `anim_config`/`anim_state_ptr` accessors. Data tier -- tables
//! keep their upstream names.

use std::ffi::c_int;

use super::types::{
    anim_config_t, anim_state_t, animenum_t, point_t, AnimStateTable, NUMEPISODES, NUMMAPS,
    ZERO_STATE,
};
use crate::doom::i_timer::TICRATE;

/// Number of animated patches in Episode 1's background (matches C `NUMANIMS[0]`).
pub(crate) const EPSD0_NANIM: usize = 10;
/// Number of animated patches in Episode 2's background (matches C `NUMANIMS[1]`).
pub(crate) const EPSD1_NANIM: usize = 9;
/// Number of animated patches in Episode 3's background (matches C `NUMANIMS[2]`).
pub(crate) const EPSD2_NANIM: usize = 6;

/// Level-node screen positions for Episode 1 (Knee-Deep in the Dead) map graphic.
static LNODESEPSD0: [point_t; NUMMAPS] = [
    point_t { x: 185, y: 164 },
    point_t { x: 148, y: 143 },
    point_t { x: 69, y: 122 },
    point_t { x: 209, y: 102 },
    point_t { x: 116, y: 89 },
    point_t { x: 166, y: 55 },
    point_t { x: 71, y: 56 },
    point_t { x: 135, y: 29 },
    point_t { x: 71, y: 24 },
];

/// Level-node screen positions for Episode 2 (The Shores of Hell) map graphic.
static LNODESEPSD1: [point_t; NUMMAPS] = [
    point_t { x: 254, y: 25 },
    point_t { x: 97, y: 50 },
    point_t { x: 188, y: 64 },
    point_t { x: 128, y: 78 },
    point_t { x: 214, y: 92 },
    point_t { x: 133, y: 130 },
    point_t { x: 208, y: 136 },
    point_t { x: 148, y: 140 },
    point_t { x: 235, y: 158 },
];

/// Level-node screen positions for Episode 3 (Inferno) map graphic.
static LNODESEPSD2: [point_t; NUMMAPS] = [
    point_t { x: 156, y: 168 },
    point_t { x: 48, y: 154 },
    point_t { x: 174, y: 95 },
    point_t { x: 265, y: 75 },
    point_t { x: 130, y: 48 },
    point_t { x: 279, y: 23 },
    point_t { x: 198, y: 48 },
    point_t { x: 140, y: 25 },
    point_t { x: 281, y: 136 },
];

/// Level-node positions for Episode 4 (Thy Flesh Consumed) - all zeroed; no map graphic used.
static LNODESEPSD3: [point_t; NUMMAPS] = [point_t { x: 0, y: 0 }; NUMMAPS];

/// Combined level-node position table indexed by `[episode][map]` (C `lnodes` in `wi_stuff.c`).
pub(super) static LNODES: [[point_t; NUMMAPS]; NUMEPISODES] =
    [LNODESEPSD0, LNODESEPSD1, LNODESEPSD2, LNODESEPSD3];

/// Animation configurations for Episode 1's background overlay (C `epsd0animinfo`).
static EPSD0_CONFIG: [anim_config_t; EPSD0_NANIM] = [
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 224, y: 104 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 184, y: 160 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 112, y: 136 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 72, y: 112 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 88, y: 96 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 64, y: 48 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 192, y: 40 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 136, y: 16 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 80, y: 16 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 64, y: 24 },
        data1: 0,
        data2: 0,
    },
];

/// Animation configurations for Episode 2's background overlay (C `epsd1animinfo`).
static EPSD1_CONFIG: [anim_config_t; EPSD1_NANIM] = [
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 1,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 2,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 3,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 4,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 5,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 6,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 7,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 192, y: 144 },
        data1: 8,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_LEVEL,
        period: TICRATE / 3,
        nanims: 1,
        loc: point_t { x: 128, y: 136 },
        data1: 8,
        data2: 0,
    },
];

/// Animation configurations for Episode 3's background overlay (C `epsd2animinfo`).
static EPSD2_CONFIG: [anim_config_t; EPSD2_NANIM] = [
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 104, y: 168 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 40, y: 136 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 160, y: 96 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 104, y: 80 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 3,
        nanims: 3,
        loc: point_t { x: 120, y: 32 },
        data1: 0,
        data2: 0,
    },
    anim_config_t {
        type_: animenum_t::ANIM_ALWAYS,
        period: TICRATE / 4,
        nanims: 3,
        loc: point_t { x: 40, y: 0 },
        data1: 0,
        data2: 0,
    },
];

/// Mutable runtime animation states for Episode 1's background animations.
static EPSD0_STATE: AnimStateTable<EPSD0_NANIM> =
    AnimStateTable(std::cell::UnsafeCell::new([ZERO_STATE; EPSD0_NANIM]));
/// Mutable runtime animation states for Episode 2's background animations.
static EPSD1_STATE: AnimStateTable<EPSD1_NANIM> =
    AnimStateTable(std::cell::UnsafeCell::new([ZERO_STATE; EPSD1_NANIM]));
/// Mutable runtime animation states for Episode 3's background animations.
static EPSD2_STATE: AnimStateTable<EPSD2_NANIM> =
    AnimStateTable(std::cell::UnsafeCell::new([ZERO_STATE; EPSD2_NANIM]));

/// Return a reference to the compile-time animation configuration for entry `j` in episode `epsd`.
///
/// Panics if `epsd` is out of range (0-2).
pub(super) fn anim_config(epsd: usize, j: usize) -> &'static anim_config_t {
    match epsd {
        0 => &EPSD0_CONFIG[j],
        1 => &EPSD1_CONFIG[j],
        2 => &EPSD2_CONFIG[j],
        _ => panic!("anim_config: invalid episode {}", epsd),
    }
}

/// Returns a raw pointer to the mutable state for animation `j` in episode `epsd`.
///
/// # Safety
///
/// Callers must uphold all three invariants:
/// 1. `epsd` must be 0, 1, or 2 — any other value panics.
/// 2. `j` must be in bounds for the selected episode's state table.
/// 3. No data races: Doom is single-threaded; callers must not use the returned
///    pointer concurrently with any other access to the same `AnimStateTable`.
///
/// Note: raw pointers produced by this function may alias different elements of
/// the same table — that is intentional and sound as long as invariant 3 holds.
pub(super) unsafe fn anim_state_ptr(epsd: usize, j: usize) -> *mut anim_state_t {
    match epsd {
        0 => {
            debug_assert!(
                j < EPSD0_NANIM,
                "j={j} out of bounds for epsd0 (len={EPSD0_NANIM})"
            );
            (EPSD0_STATE.0.get() as *mut anim_state_t).add(j)
        }
        1 => {
            debug_assert!(
                j < EPSD1_NANIM,
                "j={j} out of bounds for epsd1 (len={EPSD1_NANIM})"
            );
            (EPSD1_STATE.0.get() as *mut anim_state_t).add(j)
        }
        2 => {
            debug_assert!(
                j < EPSD2_NANIM,
                "j={j} out of bounds for epsd2 (len={EPSD2_NANIM})"
            );
            (EPSD2_STATE.0.get() as *mut anim_state_t).add(j)
        }
        _ => panic!("anim_state_ptr: invalid episode {epsd}"),
    }
}

/// Count of background animations per episode, indexed by episode number (C `numanims`).
/// Episode 3 (index 3) has no animations (0).
pub(super) static NUMANIMS: [c_int; NUMEPISODES] = [
    EPSD0_NANIM as c_int,
    EPSD1_NANIM as c_int,
    EPSD2_NANIM as c_int,
    0,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epsd0_config_entry0_fields() {
        let c = &EPSD0_CONFIG[0];
        assert_eq!(c.type_, animenum_t::ANIM_ALWAYS);
        assert_eq!(c.nanims, 3);
        assert_eq!(c.loc.x, 224);
        assert_eq!(c.loc.y, 104);
    }

    #[test]
    fn epsd1_config_entry0_is_level_anim() {
        let c = &EPSD1_CONFIG[0];
        assert_eq!(c.type_, animenum_t::ANIM_LEVEL);
        assert_eq!(c.data1, 1);
    }

    #[test]
    fn epsd2_last_entry_has_quarter_ticrate_period() {
        let last = &EPSD2_CONFIG[EPSD2_NANIM - 1];
        assert_eq!(last.period, TICRATE / 4);
    }

    #[test]
    fn anim_state_initializes_zeroed() {
        unsafe {
            let st = anim_state_ptr(0, 0);
            assert_eq!((*st).ctr, 0);
            assert_eq!((*st).nexttic, 0);
            assert_eq!((*st).lastdrawn, 0);
            assert_eq!((*st).state, 0);
        }
    }
}
