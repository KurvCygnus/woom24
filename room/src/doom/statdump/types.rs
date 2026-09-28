//! The trimmed intermission-stat mirror structs, moved as-is from the
//! pre-graduation `statdump.rs` (F10 wave A3) -- see the `//?` record
//! below before assuming they match the C layout.

#![allow(non_snake_case)]

use std::ffi::c_int;

//? Latent layout mismatch, recorded F10 wave A3 (MOVE AS-IS, never fix
//? during graduation): this module's trimmed `wbstartstruct_t` is NOT
//? offset-compatible with the struct it memcpy's from -- the source of
//? `StatCopy` is wi_stuff's FULL `wbstartstruct_t` (`wi_stuff.rs:137-160`,
//? cast in `g_game/actions.rs:do_completed`): `last` (offset 4) reads wminfo.didsecret,
//? `partime` (offset 8) reads wminfo.last, and `plyr` (offset 12) starts
//? at wminfo.next. `wbplayerstruct_t` also omits the C `score` field
//? (36 B vs the C 40 B, `d_player.h:168-180`), so the 156-byte size
//? invariant pinned below enshrines the wrong layout (C: 3*4 + 4*40 =
//? 172). Impact today: none -- `captured_stats` is write-only (`StatDump`
//? is a stub; no reader exists), so the capture path has zero
//? observables. Post-graduation fix (a separate, explicit change): alias
//? wi_stuff's real struct, or reorder the fields here and correct the
//? size test.

/// Snapshot of intermission-screen state captured at the end of every
/// level. Mirrors the C `wbstartstruct_t` layout from `d_player.h`.
/// `#[repr(C)]` so it can be `memcpy`'d directly out of game memory.
///
/// Layout invariant: the struct exposed here only includes the four
/// fields the dump path inspects (`epsd`, `last`, `partime`, `plyr`).
/// The full C struct has additional fields (`pnum`, `maxkills`,
/// `maxitems`, `maxsecret`, etc.); the unit tests in this module assert
/// the trimmed size matches `3 * sizeof(int) + 4 * sizeof(wbplayerstruct_t)`.
///
/// The offsets above are the module's own trim, not the C source
/// struct's -- read the `//?` record at the top of this file before
/// reasoning about what a captured snapshot contains.
#[repr(C)]
pub struct wbstartstruct_t {
    /// Episode number (0-based).
    pub epsd: c_int,
    /// Index of the level just completed.
    pub last: c_int,
    /// Par time for this level in tics.
    pub partime: c_int,
    /// Per-player statistics for up to 4 players.
    pub plyr: [wbplayerstruct_t; 4],
}

/// Per-player end-of-level statistics. Mirrors the C `wbplayerstruct_t`
/// from `d_player.h`. The first field is named `in_` (with trailing
/// underscore) because `in` is a Rust keyword; the original C name is
/// `in` and the `#[repr(C)]` layout is unchanged.
///
/// Note the `//?` record at the top of this file: the C struct carries
/// an additional `score` field this port omits.
#[repr(C)]
pub struct wbplayerstruct_t {
    /// "in game" flag - non-zero if this player slot was active. Named
    /// `in` in C; renamed `in_` here to dodge the Rust keyword.
    pub in_: c_int,
    /// Kills count for this player.
    pub skills: c_int,
    /// Items collected.
    pub sitems: c_int,
    /// Secrets found.
    pub ssecret: c_int,
    /// Total time spent on the level, in tics.
    pub stime: c_int,
    /// Frag counts indexed by victim player.
    pub frags: [c_int; 4],
}

#[cfg(test)]
mod tests
{
    use super::*;

    /// wbplayerstruct_t must be 36 bytes: 5 × i32 (20) + [i32; 4] (16).
    #[test]
    fn wbplayerstruct_t_size_matches_c()
    {
        // in_(4) + skills(4) + sitems(4) + ssecret(4) + stime(4) + frags[4](16) = 36
        assert_eq!(std::mem::size_of::<wbplayerstruct_t>(), 36);
    }

    /// wbstartstruct_t: 3 × sizeof(int) + 4 × sizeof(wbplayerstruct_t).
    //? Knowingly-wrong invariant kept per the `//?` record at the top of
    //? this file: 156 pins the trimmed layout, not the C 172.
    #[test]
    fn wbstartstruct_t_size_matches_c()
    {
        // epsd(4) + last(4) + partime(4) + plyr[4] (4 × 36 = 144) = 156
        let expected =
            3 * std::mem::size_of::<c_int>() + 4 * std::mem::size_of::<wbplayerstruct_t>();
        assert_eq!(std::mem::size_of::<wbstartstruct_t>(), expected);
    }
}
