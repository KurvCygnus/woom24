# Vanilla Workarounds

Catalog of every place in shipped code that deliberately emulates, bounds, or
otherwise deviates from plain vanilla semantics *because of* a vanilla DOOM
bug, memory violation (out-of-bounds write), or legacy undefined behavior.

Per `AGENTS.md` (Documentation Standards, decided 2026-09-19) this catalog is
**binding**: adding or changing a workaround REQUIRES updating this document in
the same change. It also doubles as the **intake checklist** for new findings
from reference-source audits: anything listed in "Known gaps / intake" is
unverified or unimplemented, and an audit that pins its semantics must upgrade
it to a full entry. Code sites are cited as `file:line` inside the named
function; line numbers are current as of `feat/spec2-3` at the
violations-census change and will drift -- the function name is the stable
anchor.

A central census for emulation trigger events lives in
`room/src/doom/violations.rs`: every hook below calls
`violations::record(...)`, and tests/audits read `violations::hits(...)` to
prove which workarounds a given input exercises. Counters are pure
diagnostics -- no simulation code reads them (determinism red line); each
hooked entry's Semantics section names the `VanillaViolation` variant its
hook records.

Definition and exclusions: a workaround is *motivated by a vanilla defect*.
Platform degradations (wasm/CRT shims), browser observability, and render-side
policies that deviate from vanilla for non-vanilla reasons are NOT workarounds
and are listed in "Out of scope" instead. Plain bit-exact replication of
vanilla behavior that is *not* a bug (e.g. `G_CmdChecksum`, the fixed-point
semantics of `m_fixed.rs`) is not cataloged here either.

## Summary

| # | Item | Vanilla root cause (one-liner) | Status | Where (file:function) |
|---|------|-------------------------------|--------|-----------------------|
| 1 | spechit array overrun | `PIT_CheckLine` stores crossed special lines into a fixed `spechit[]` past its 8-slot vanilla bound; the stores trample DOS `.bss` neighbors (`tmbbox`, `crushchange`, `nofit`) | complete | `room/src/doom/p_map/move.rs:pit_check_line` / `p_map/spechit.rs:spechit_overrun`; drains in `p_map/move.rs:try_move` (`P_TryMove`), `p_enemy/chase.rs:move_step` |
| 2 | intercepts array overrun | `P_PathTraverse` stores ray/line and ray/thing hits into `intercepts[]` past 128 entries; stores land on `lowfloor`, `bmapwidth`, `playerstarts`, ... | complete | `room/src/doom/p_maputl/intercepts.rs:InterceptsOverrun` |
| 3 | donut NULL-backsector read | `EV_DoDonut` dereferences a null `s3` sector on malformed maps; vanilla reads DOS address `0000:0000`-adjacent memory | complete | `room/src/doom/p_spec/donut.rs:donut_overrun` |
| 4 | REJECT undersized-lump read | `P_LoadReject` reads a REJECT lump shorter than `ceil(numsectors^2/8)` bytes; the tail falls into the `Z_Malloc` zone block header | complete | `room/src/doom/p_setup.rs:PadRejectArray` |
| 5 | missed-backside null sector | segs of two-sided lines with a missing back sidedef get `backsector = NULL`-adjacent DOS memory; vanilla reads address 0 | complete | `room/src/doom/p_setup.rs:GetSectorAtNullAddress` |
| 6 | teleport-fog signed-angle overrun | `G_CheckSpot` computes `an = (ANG45 * angle/45) >> 19` with a signed shift in the DOS binary; out-of-range indices read `finetangent[]` instead of the sine/cosine tables | complete | `room/src/doom/g_game.rs:G_CheckSpot` / `TeleportFogAngleOverrun` |
| 7 | episode-4 par-time off-by-one | `G_DoCompleted` reads `cpars[gamemap]` (not `[gamemap-1]`) for Doom 1 episode 4 -- an accidental adjacent-array read that statcheck depends on | complete | `room/src/doom/g_game.rs:G_DoCompleted` |
| 8 | Archvile fire spawn coordinates | `A_VileTarget` passes `target->x` for both X and Y of `P_SpawnMobj` (vanilla typo) | complete | `room/src/doom/p_enemy/vile_fire.rs:action_vile_target` |
| 9 | commercial map33 par-time read | `G_DoCompleted` reads one `int` past `cpars[31]` for a map 33 exit; the read lands in the first four bytes of the adjacent `GAMMALVL0` rodata string | complete | `room/src/doom/g_game.rs:G_DoCompleted` / `ParTimeOverrun` |
| 10 | playeringame[-1] overrun | `P_SpawnPlayer` gates the spawn on `playeringame[mthing->type - 1]`; a type-0 mapthing makes the index `-1`, which aliases `players[3].didsecret` in the DOS `.bss` | complete | `room/src/doom/p_mobj/mapthings.rs:spawn_player` / `PlayeringameOverrun` |
| 11 | door/plat `specialdata` aliasing | `EV_VerticalDoor` discriminates the thinker in an aliased `sector->specialdata` slot by `acp1` function-pointer compare and, for a plat, writes `plat->wait` through a `vldoor_t*` cast (the "When is a door not a door?" quirk) | complete | `room/src/doom/p_doors/events.rs:EV_VerticalDoor` |
| 12 | null-target dereference substitution | weapon/monster code dereferences `target`/`tracer` pointers that can legitimately be NULL; vanilla read DOS address `0000:0000`-adjacent memory (low vector area) and "worked" for it | complete | `room/src/doom/p_mobj/spawn.rs:subst_null_mobj` |
| 13 | stair heightlist overrun | `P_FindNextHighestFloor` stores qualifying neighbor floors into `heightlist[MAX_ADJOINING_SECTORS]` (20 slots) on the DOS stack; stores 21 and 22 land past the array -- store 21 hits the stack slot of `height` itself, further stores trample deeper stack until vanilla crashes | complete | `room/src/doom/p_spec/geometry.rs:next_highest_floor` |
| G1 | tmbbox overrun family | `PIT_CheckLine`'s spechit-overrun emulated writes land in `tmbbox[0..3]`; every later collision check in the same move then consumes the trampled bbox -- the emulated writes are the complete trample model | complete | `room/src/doom/p_map/spechit.rs:spechit_overrun` (writes) + `p_map/move.rs:pit_check_line`/`check_position` (consumers) |
| G2 | demo-window / ticdup=0 clobber | a browser-only OOB write zeroed `ticdup` mid-run (`panic_const_div_by_zero`); audit found the spechit store was the sole trampler | resolved (pending human browser re-test) | audit notes below; fix = entry 1 |

Reference clones cited below are read-only under `reference/` (see `AGENTS.md`,
Tooling and Workflow Protocols).

## 1. spechit array overrun (complete)

### DOS-era root cause

Vanilla `p_map.c` declares `line_t* spechit[MAXSPECIALCROSS]` with
`MAXSPECIALCROSS_ORIGINAL = 8`. `PIT_CheckLine` stores every contacted
special line into `spechit[numspechit]` and then increments the counter --
with no bound check on the store. A single move that crosses more than 8
special lines writes past the array into whatever the DOS linker placed
next to it in `.bss`.

### What the trample observably affected

In `doom2.exe` the writes land in a known neighbor set: `tmbbox[4]`
(indices 9-12 of the store sequence), then `crushchange` (13) and `nofit`
(14). The corrupted values change subsequent collision checks and
`P_CrossSpecialLine` behavior within the same move, which is observable in
demos recorded on the DOS binary. Known affected demo families include
compet-n `\hr.wad\hr18*.lmp` and `strain.wad\map07` (per the prboom-plus
note quoted in `reference/dsda-doom/prboom2/src/g_overflow.c:227-229`).
Without emulation these demos desync; without bounding, the same stores
corrupt a *non-vanilla* memory layout (see G2 for what that did in wasm).

### Where we emulate it

Three sites, all in `room/src/doom/`:

- `p_map/move.rs:262-270` in `pit_check_line` (`PIT_CheckLine`; guarded store
  + emulation trigger):

```rust
if numspechit >= 0 && (numspechit as usize) < MAXSPECIALCROSS
{
    spechit[numspechit as usize] = ld as *const _ as *mut _;
}
numspechit += 1;
if numspechit > MAXSPECIALCROSS_ORIGINAL
{
    spechit_overrun(ld as *const _ as *mut _);
}
```

- `p_map/move.rs:574-577` in `try_move` (`P_TryMove`) and
  `p_enemy/chase.rs:276-279` in `move_step` (`P_Move`; bounded drain
  reads; both loops decrement `numspechit` first and must skip indices
  the guarded push never stored):

```rust
if numspechit as usize >= MAXSPECIALCROSS
{
    continue;
}
```

- `p_map/spechit.rs:59-89`, `spechit_overrun` (`SpechitOverrun`): computes
  `addr = baseaddr + (ld - lines) * 0x3e` and, per `numspechit`, writes the
  address into `tmbbox[(numspechit - 9)]` for 9..=12, `crushchange` for 13,
  and `nofit` for 14. `baseaddr` defaults to `DEFAULT_SPECHIT_MAGIC`
  (`0x01C09C98`, `room/src/doom/c_ffi.rs:680-683`) overridable with
  `-spechit <n>`. The pure halves are extracted to `p_map/dtmc.rs:48`
  (`spechit_trample_addr`, the addr formula) and `:73` (`trample_target`,
  the case-table target selection); the whole-body baseline drives are
  `p_map/spechit.rs:328` / `:354`.

Constants: `MAXSPECIALCROSS = 20` (`p_map/consts.rs:16`), the room-upstream array
size; `MAXSPECIALCROSS_ORIGINAL = 8` (`p_map/consts.rs:23`), the vanilla bound that
gates the emulation.

### Semantics

Hybrid, deliberately assembled from two references:

- *Store*: always in-bounds (killough-style limit removal). The counter keeps
  advancing past 20 so the emulation trigger and demo-visible timing are
  unchanged, but no real memory is trampled. This mirrors woof/dsda, which use
  a doubling dynamic array instead of a fixed 20.
- *Emulation*: for counts 9..=14 the exact chocolate/prboom-plus writes are
  replayed into the named engine globals, preserving the observable state
  corruption vanilla demos depend on.
- Deviation from plain vanilla: an unguarded store is replaced by an in-bounds
  store; the emulated write set is a *model* of the trample, not a trample.
- Deviations from references worth knowing: dsda
  (`reference/dsda-doom/prboom2/src/g_overflow.c:325-330`) maps `numspechit`
  13 -> `nofit`, 14 -> `crushchange` and normalizes `crushchange` to 10;
  chocolate (`reference/chocolate-doom/src/doom/p_map.c:1437-1441`) and woof
  (`reference/woof/src/p_map.c:2494-2499`) map 13 -> `crushchange`,
  14 -> `nofit` with no normalization. We follow chocolate/woof (doom2.exe
  layout). dsda additionally models a dosdoom/tasdoom variant
  (9 -> `tmfloorz`, 10 -> `tmceilingz`, `g_overflow.c:305-318`); we do not,
  because we do not implement those complevels.
- Census: every emulation trigger records a
  `violations::VanillaViolation::SpechitOverrun` hit
  (`room/src/doom/violations.rs`); the hook test
  `spechit_emulation_records_census_hit` (`p_map/spechit.rs:246`) proves the shared
  fixture below actually drives it.
- Regression pin: `p_map/spechit.rs:239`
  (`pit_check_line_push_stays_in_bounds_beyond_the_array`) drives
  `PIT_CheckLine` at `numspechit` 20 and 25 and asserts the 8 words after
  `spechit` are untouched. It failed (process killed by the overflow) on the
  pre-fix code.

### Status

`complete`.

### Reference derivation

- Emulation: `reference/chocolate-doom/src/doom/p_map.c:1388-1451`
  (`SpechitOverrun`: "-spechit" parse, `baseaddr + (ld - lines) * 0x3E`,
  case table 9-12 `tmbbox` / 13 `crushchange` / 14 `nofit`); originally
  prboom-plus research by Andrey Budko (e6y), per the chocolate comment at
  `p_map.c:1382-1384`. Cross-checked against
  `reference/woof/src/p_map.c:2456-2502` (same case table).
- Store bound: `reference/woof/src/p_map.c:462-467` in `PIT_CheckLine`
  ("1/11/98 killough: remove limit on lines hit, by array doubling";
  `spechit_max = spechit_max ? spechit_max*2 : 8;` before the store) and
  `reference/dsda-doom/prboom2/src/p_map.c:3712-3721` in `P_AppendSpecHit`
  (same doubling). Drains: woof `p_map.c:940-950`, dsda
  `p_map.c:1473-1483` -- both iterate the dynamic array without a fixed cap;
  our fixed-20 guard is the wasm-port equivalent (see G2 for why the guard
  exists at all).

## 2. intercepts array overrun (complete)

### DOS-era root cause

`P_PathTraverse` records every blockmap line/thing the trace crosses into
`intercept_t intercepts[MAXINTERCEPTS]` (vanilla bound:
`MAXINTERCEPTS_ORIGINAL = 128`). Dense traces exceed 128 entries; stores past
the array run down the DOS `.bss` in a fixed layout.

### What the trample observably affected

The trailing entries overwrite `lowfloor`, `openbottom`, `opentop`,
`openrange`, `bulletslope`, `playerstarts` (as 16-bit pairs), `bmapwidth`,
`bmaporgx/y`, `bmapheight`, and more. Corrupted `bmapwidth`/`bmaporgx` in
particular redirect later blockmap walks, which demos can observe.

### Where we emulate it

`room/src/doom/p_maputl/intercepts.rs:103-163`,
`InterceptsMemoryOverrun`, walks the vanilla layout as a sequence of
`skip!` / `write_i32!` / `write_i16_arr!` steps (one per neighbor
variable, in vanilla `.bss` order); `p_maputl/intercepts.rs:175-186`,
`InterceptsOverrun`, is the trigger:

```rust
if num_intercepts <= MAXINTERCEPTS_ORIGINAL as c_int {
    return;
}
let location = (num_intercepts - MAXINTERCEPTS_ORIGINAL as c_int - 1) * 12;
InterceptsMemoryOverrun(location, (*intercept).frac);
InterceptsMemoryOverrun(location + 4, (*intercept).isaline);
InterceptsMemoryOverrun(location + 8, (*intercept).d.thing as usize as c_int);
```

Call sites: after each intercept store, before advancing `intercept_p` --
`p_maputl/intercepts.rs:256` (`PIT_AddLineIntercepts`) and
`p_maputl/intercepts.rs:320` (`PIT_AddThingIntercepts`). Constants at
`p_maputl/intercepts.rs:31-37`
(`MAXINTERCEPTS_ORIGINAL = 128`, `MAXINTERCEPTS = 189`).

### Semantics

Byte-faithful chocolate port: same layout table, same 12-byte
`intercept_t` stride, same trigger. Writes land in *our* live globals at the
same logical positions (e.g. `bmapwidth` really is modified), so observable
behavior matches vanilla overrun demos. No deviation from plain vanilla
semantics beyond relocating the writes into a Rust module layout. Every
trigger past the 128-entry bound also records a
`violations::VanillaViolation::InterceptsOverrun` census hit
(`room/src/doom/violations.rs`).

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/p_maputl.c:802-830` (the
`intercepts_overrun[]` layout table), `:834-876`
(`InterceptsMemoryOverrun`), `:876-898` (`InterceptsOverrun`), and the
call sites at `:605`, `:655`, `:719`. The header comment credits the layout
to prboom-plus research. Our macros expand to the identical table
entry-for-entry (verified side-by-side while writing this entry).

## 3. donut NULL-backsector read (complete)

### DOS-era root cause

`EV_DoDonut` expects the ring sector `s2` to be adjacent to an outer sector
`s3`. On malformed maps `s2`'s line can have no back sector, so `s3` is null
and vanilla reads `s3->floorheight` / `s3->floorpic` from (near) DOS address
`0000:0000`.

### What the trample observably affected

The donut lowers/raises to whatever bytes sat at the start of the DOS data
segment, which differs per DOS/OS loader. The Windows 98 layout yields floor
height 0 and floorpic 0x16, and demos recorded against that layout depend on
it.

### Where we emulate it

`room/src/doom/p_spec/donut.rs:53-91`, `donut_overrun` (export-shaped
as the C `DonutOverrun`, doc-aliased): on first call parses
`-donut <height> <pic>` (defaults 0 and 0x16, rejecting `pic >= numflats`),
then returns the cached values. Hooked at `p_spec/donut.rs:167-174` in
`do_donut` when `s3` is null; the warning at `p_spec/donut.rs:170` prints
"emulating buffer overrun due to NULL back sector".

### Semantics

Identical to chocolate, including the `-donut` overrides and the Win98
defaults. Not parameterized per OS; users needing a different DOS layout pass
`-donut`. Each NULL-backsector trigger records a
`violations::VanillaViolation::DonutOverrun` census hit
(`room/src/doom/violations.rs`), right next to the warning above.

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/p_spec.c:1203-1204`
(`DONUT_FLOORHEIGHT_DEFAULT 0x00000000`, `DONUT_FLOORPIC_DEFAULT 0x16`),
`:1206-1260` (`DonutOverrun`, including the DOS 6.22 / 7.1 / Win98 /
DOSBox memory dump comment), `:1345` (call site in `EV_DoDonut`).

## 4. REJECT undersized-lump read (complete)

### DOS-era root cause

Vanilla allocates the REJECT table with `Z_Malloc` and copies the REJECT lump
in without checking the lump length against the required
`(numsectors * numsectors + 7) / 8` bytes. A short lump reads past its end
into the zone memory block header that precedes the allocation.

### What the trample observably affected

The "missing" reject bits come from the zone header bytes (block size, tag
`PU_LEVEL`, zone id `0x1d4a11`), which produce concrete sight-blocking
decisions. WADs (and demos) exist that depend on this pseudo-random-but-
deterministic reject table.

### Where we emulate it

`room/src/doom/p_setup.rs:1132-1162`, `PadRejectArray`, fills the tail of an
undersized REJECT with the modeled zone header:

```rust
let rejectpad: [u32; 4] = [((totallines * 4 + 3) & !3) as u32 + 24, 0, 50, 0x1d4a11];
```

followed by 0x00 fill (or 0xff with `-reject_pad_with_ff`). Called from
`P_LoadReject` (`p_setup.rs:1184`) when `lumplen < minlength`.

### Semantics

Same padding model as the references, including the four-word header shape
and the escape hatch flag. Any deviation is limited to a diagnostic warning
when the pad is exhausted. The `lumplen < minlength` route in `P_LoadReject`
records a `violations::VanillaViolation::RejectPadOverrun` census hit
(`room/src/doom/violations.rs`) before padding.

### Status

`complete`.

### Reference derivation

`reference/woof/src/p_setup.c:965-984` (the `rejectpad[4]` initializer and
byte-copy loop). Same mechanism in dsda
(`reference/dsda-doom/prboom2/src/p_setup.c`, `P_LoadReject` /
`RejectOverrun`, modeled under `OVERFLOW_REJECT` in
`reference/dsda-doom/prboom2/src/g_overflow.c:353-390`).

## 5. missed-backside null sector (complete)

### DOS-era root cause

A linedef with the two-sided flag but a missing back sidedef produces segs
whose `backsector` cannot be resolved. Vanilla dereferences the null pointer
and reads its sector fields from DOS address 0 and following.

### What the trample observably affected

The "impassible glass" hack: WADs exploit that vanilla reads a zeroed,
load-order-dependent sector at address 0, making the line transparent to
sight and rendering under specific height relationships. Demos on such WADs
(vex6d-family "glass hack" maps) desync without this.

### Where we emulate it

`room/src/doom/p_setup.rs:422-443`, `GetSectorAtNullAddress`, returns a
synthetic zeroed `sector_t` whose `floorheight`/`ceilingheight` are read via
`I_GetMemoryValue(0/4, ...)` on first call; `p_setup.rs:537` in `P_LoadSegs`
stores it as the seg's `backsector` when the opposite `sidenum` is missing or
out of range.

### Semantics

Same substitution model as woof/dsda. Ours substitutes at seg-load time,
while woof also substitutes at sight-check time
(`reference/woof/src/p_sight.c:154-158`); because our `P_CheckSight` reads
`seg.backsector` (`room/src/doom/p_sight/bsp.rs:110` in `P_CrossSubsector`),
the load-time substitution covers both consumers. `line.backsector` stays
null and is checked explicitly (`p_sight/bsp.rs:99`) to keep vanilla's
block-on-null path. Every substitution
records a `violations::VanillaViolation::MissedBackSideOverrun` census hit
(`room/src/doom/violations.rs`).

### Status

`complete`.

### Reference derivation

`reference/woof/src/p_setup.c:204-206` (`GetSectorAtNullAddress`) and call
site `reference/woof/src/p_bsp.c:586` ("this is wrong" comment path);
dsda equivalent `reference/dsda-doom/prboom2/src/g_overflow.c:540-570`
(`MissedBackSideOverrun` + `GetSectorAtNullAddress` under
`OVERFLOW_MISSEDBACKSIDE`).

## 6. teleport-fog signed-angle overrun (complete)

### DOS-era root cause

`G_CheckSpot` computes the teleport-fog offset angle as
`(ANG45 * (angle/45)) >> ANGLETOFINESHIFT`. In the DOS binary the multiply
overflows into the sign bit and the shift is signed, so malformed/large
`mthing->angle` values yield negative table indices; because the trig tables
are adjacent in memory, the lookups land in `finetangent[]` instead of
`finecosine[]`/`finesine[]`.

### What the trample observably affected

The fog spawns at a table-dependent offset -- including the famous "silent
west spawn" (no fog) case. Demo compatibility requires the exact same
mis-indexed values.

### Where we emulate it

`room/src/doom/g_game.rs:G_CheckSpot` delegates to the pure helper
`g_check_spot_fog_offset` (same file), which owns the whole offset switch:

```rust
let an = ((ANG45 >> ANGLETOFINESHIFT) as i32).wrapping_mul(angle / 45);
match an {
    4096 => { /* census */ Some((finetangent[2048], finetangent[0])) }
    5120 => { /* census */ Some((finetangent[3072], finetangent[1024])) }
    6144 => { /* census */ Some((finesine[0],        finetangent[2048])) }
    7168 => { /* census */ Some((finesine[1024],     finetangent[3072])) }
    0 | 1024 | 2048 | 3072 =>
        Some((unsafe { *finecosine.0.add(an as usize) }, finesine[an as usize])),
    8192 => Some((tantoangle[0] as fixed_t, finesine[8192])),
    _ => None, // G_CheckSpot raises I_Error (chocolate's default arm)
}
```

The four overrun arms record the census hook
`violations::record(VanillaViolation::TeleportFogAngleOverrun)`; replaying
one of them *is* the vanilla-defect emulation.

### Semantics

Matches chocolate exactly (transcribed case table):

- `an = (ANG45 >> ANGLETOFINESHIFT) * (angle/45)` = `1024 * angle/45`,
  computed without overflow (chocolate deliberately avoids the vanilla UB).
  The switch cases are therefore on the 1024-scale: `0/1024/2048/3072`
  (angles 0/45/90/135) read `finecosine[an]`/`finesine[an]` in range;
  `4096/5120/6144/7168` (angles 180/225/270/315 -- vanilla's negative
  indices -4096/-3072/-2048/-1024) name the `finetangent[]` overrun reads
  explicitly; `8192` (angle 360) reproduces `finecosine[8192]` overrunning
  one past `finesine` into `tantoangle[0]` in the DOS binary's adjacent
  table layout, with `finesine[8192]` (in-range, sine of 360 deg) for ya.
  Census variant recorded by the overrun arms:
  `VanillaViolation::TeleportFogAngleOverrun`.
- Out-of-table values (e.g. angle > 360) reach chocolate's `default:` arm:
  `I_Error("G_CheckSpot: unexpected angle %d\n", an)`.
- Unit test `doom::g_game::tests::g_check_spot_fog_matches_chocolate_case_table`
  pins every arm plus the no-`I_Error` sweep over `0..=360 step 45`.

History: the block was inherited verbatim from upstream room (commit
`aa242e2`, "Port g_game.c to Rust") in a mis-scaled form -- it computed the
*unshifted* product `0x10000000 * angle/45` but matched chocolate's
1024-scale case constants, so every legal angle except 0 fell into the
default arm and the overrun cases were dead code; the same mis-scale exists
in `upstream/main`. It went unobserved because the fog branch only runs on
respawn (`players[].mo != NULL`), which demo playthroughs never exercise.
(This catalog previously mis-stated the correct scale as 512; `ANG45 >>
ANGLETOFINESHIFT` is 2^29 >> 19 = 1024, confirmed by chocolate's own
`case 8192: // 360 deg` comment.)

### Status

`complete` (switch transcribed arm-for-arm from chocolate, including the
360-degree case; census hook wired; pinned by unit test).

### Reference derivation

`reference/chocolate-doom/src/doom/g_game.c:1223-1268` in `G_CheckSpot`
(comment explaining the released-source signed overflow and the imported
prboom-plus emulation; switch over `an` with cases 4096/5120/6144/7168,
0/1024/2048/3072, 8192, default `I_Error`).

## 7. episode-4 par-time off-by-one (complete)

### DOS-era root cause

Doom 1 has no par times for episode 4 (`pars[]` has 4 rows but the E4 row is
all zeros / unused). `G_DoCompleted` reads `cpars[gamemap]` instead of
indexing a nonexistent E4 table -- one past the "intended" entry, effectively
reading the next map's Doom 2 par value.

### What the trample observably affected

Not a memory-safety issue in our port (Doom 1 maps are 1..9, well inside
`cpars[32]`), but a value-level vanilla bug: the intermission shows the "wrong"
par time. `statcheck` regression tooling parses exactly these values, so
changing them breaks the statcheck demo ecosystem.

### Where we emulate it

`room/src/doom/g_game.rs:2117-2132` in `G_DoCompleted`:

```rust
wminfo.partime = if gamemode == commercial {
    match commercial_partime(gamemap) {
        Some(partime) => partime,
        None => i_error!(
            "G_DoCompleted: commercial map {} has no vanilla par time",
            gamemap
        ),
    }
} else if gameepisode < 4 {
    35 * pars[gameepisode as usize][gamemap as usize]
} else {
    violations::record(VanillaViolation::ParTimeOverrun);
    35 * cpars[gamemap as usize]
};
```

### Semantics

Identical to chocolate's branch structure, including the deliberate
`cpars[gamemap]` for `gameepisode >= 4`. The documented rationale in
`G_DoCompleted`'s doc comment (`g_game.rs:2026-2028`) explicitly claims the
overflow as statcheck-compatibility behavior. The commercial map33 sibling is
handled by entry 9. Census: every episode-4 read records a
`violations::VanillaViolation::ParTimeOverrun` hit
(`room/src/doom/violations.rs`) -- the same variant entry 9's map33
emulation uses, since both are the par-time overrun family.

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/g_game.c:1524-1556` ("Doom episode 4
doesn't have a par time, so this overflows into the cpars array",
`wminfo.partime = TICRATE*cpars[gamemap]`).

## 8. Archvile fire spawn coordinates (complete)

### DOS-era root cause

`A_VileTarget` spawns the `MT_FIRE` object with a copy-paste typo:
`P_SpawnMobj(target->x, target->x, target->z, ...)` -- the Y argument
receives X.

### What the trample observably affected

No memory violation; the fire mobj is placed on the vertical line through the
target's X coordinate. The flame column's position (and therefore the
Archvile attack's observable behavior) differs from the "obviously intended"
`target->y`. Demos and gameplay depend on the buggy placement.

### Where we emulate it

`room/src/doom/p_enemy/vile_fire.rs:125-131` in `action_vile_target`
(`A_VileTarget`):

```rust
let fog: *mut mobj_t = P_SpawnMobj(
    (*(*actor).target).x,
    (*(*actor).target).x,
    (*(*actor).target).z,
    MT_FIRE,
);
```

with the `//!`-style note at `vile_fire.rs:123-124` marking it as a faithful
vanilla-bug reproduction.

### Semantics

Bit-exact with vanilla and both references. This is a deliberate value-level
bug replication, not a memory-safety workaround; it is cataloged because it
is exactly the kind of accidental behavior `AGENTS.md` requires to be
replicated explicitly and marked.

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/p_enemy.c:1288-1304` (`A_VileTarget`,
`P_SpawnMobj (actor->target->x, actor->target->x, actor->target->z, MT_FIRE)`).

## 9. commercial map33 par-time read (complete)

### DOS-era root cause

`G_DoCompleted`'s commercial par-time branch has no map33 case: doom2.exe
computes `TICRATE * cpars[gamemap-1]` unconditionally, and a normal exit from
a PWAD's commercial map 33 evaluates `cpars[32]` -- one `int` past the
32-entry array. The DOS linker placed the `GAMMALVL0` rodata string ("Gamma
correction OFF", `d_englsh.h:67`) directly after `cpars`, so the overrun read
aliases the string's first four bytes. It is a plain read of adjacent
initialized data: no state corruption, a deterministic garbage value.

### What the trample observably affected

The map33 intermission shows a par time derived from the ASCII bytes
`'G','a','m','m'` read as a little-endian int (0x6D6D6147 = 1835884871),
scaled by TICRATE with 32-bit wrap: 35 * 1835884871 wraps to -168538955
tics. Intermission/statcheck-parsing tooling observes exactly this value on
map33 exits. In Rust the same evaluation is a bounds panic (`cpars[32]` on
`[c_int; 32]`) -- a process abort on WAD-reachable input, violating the
`AGENTS.md` no-panic rule before any compat question arises (pre-fix
evidence: the `commercial_map33_exit_uses_gammalvl0_model_not_panic` test
panicked with `index out of bounds: the len is 32 but the index is 32`).

### Where we emulate it

`room/src/doom/g_game.rs:2117-2132` in `G_DoCompleted` delegates the
commercial arm to the pure helper `commercial_partime` (`g_game.rs:2170`);
the map33 case is

```rust
33 => {
    violations::record(VanillaViolation::ParTimeOverrun);
    Some(35i32.wrapping_mul(gammalvl0_prefix_i32()))
}
```

`gammalvl0_prefix_i32` (`g_game.rs:2193`) reads the first four bytes of the
port's GAMMALVL0 equivalent -- `gammamsg[0]` in `room/src/doom/m_menu.rs:284`
("Gamma correction OFF") -- and forms the little-endian i32. Maps outside
`1..=33` return `None` and the call site raises `I_Error`.

### Semantics

- Map 33 value: identical to chocolate. `memcpy(&cpars32,
  DEH_String(GAMMALVL0), sizeof(int))` followed by `LONG(cpars32)`
  interprets the four bytes little-endian on every host;
  `i32::from_le_bytes` is the same host-independent model. Chocolate's
  `TICRATE*cpars32` overflows `int`; the helper uses `wrapping_mul` so
  debug-Rust wraps like the C binary instead of panicking.
- String indirection: we read the live `gammamsg[0]` static rather than a
  frozen copy of the text, mirroring chocolate's `DEH_String` indirection --
  a future DSDHacked string replacement of the message would move map33's
  par time exactly as chocolate's does.
- Guard deviation (documented choice): commercial maps outside `1..=33`
  have no reference-defined value. Chocolate does a plain unguarded
  `cpars[gamemap-1]` (`g_game.c:1536-1539`), which for map <= 0 or > 33
  reads unmodelable DOS memory; our `None` arm aborts with
  `I_Error("G_DoCompleted: commercial map %d has no vanilla par time")`
  instead of panicking or silently inventing a value.
- Census: the map33 arm records a
  `violations::VanillaViolation::ParTimeOverrun` hit
  (`room/src/doom/violations.rs`); the same variant records entry 7's
  episode-4 off-by-one, its family sibling.
- Regression pin:
  `doom::g_game::tests::commercial_map33_exit_uses_gammalvl0_model_not_panic`
  drives maps 1-32 (ordinary `cpars[map-1]` path), map 33 (model value +
  census hit), and pins `gammamsg[0]`'s four-byte prefix against silent
  text changes.

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/g_game.c:1524-1540` (commercial branch:
the map33 comment and `memcpy`/`LONG` model at `:1526-1534`, the plain
`cpars[gamemap-1]` at `:1538`). GAMMALVL0 text "Gamma correction OFF":
`reference/chocolate-doom/src/doom/d_englsh.h:67` (same text at
`vendor/doomgeneric/d_englsh.h:67`, the C tree our port mirrors).

## G1. tmbbox overrun family (complete)

### DOS-era root cause

The `spechit` overrun's first victims (entry 1, store sequence 9-12) are the
four `tmbbox` words, then `crushchange` (13) and `nofit` (14). This adjacency
is pinned by the concordant emulation case tables of all three references:
chocolate `reference/chocolate-doom/src/doom/p_map.c:1433-1441`, woof
`reference/woof/src/p_map.c:2493-2501`, and dsda's non-dosdoom branch
`reference/dsda-doom/prboom2/src/g_overflow.c:321-331`. The dosdoom/tasdoom
EXE builds have a *different* neighbor layout (9 -> `tmfloorz`,
10 -> `tmceilingz`, `g_overflow.c:305-318`); those complevels are out of
scope, so the doom2.exe 1.9 layout is the one we model.

### What the trample observably affected

`PIT_CheckLine` consumes `tmbbox` at entry -- the bbox reject test and
`P_BoxOnLineSide` (`reference/chocolate-doom/src/doom/p_map.c:208-218`) --
*before* the spechit push and overrun emulation at `:265-276`. A trample can
therefore never affect its own call; it affects every **later** line tested in
the same move's `P_BlockLinesIterator` walk: the corrupted coordinates change
reject and side decisions for those lines. The window closes at the next
`P_CheckPosition`-family re-initialization (`p_map.c:421-424`), so the trample
is exactly one move wide. Beyond `tmbbox`, `crushchange` (13) alters
`P_ChangeSector` crushing damage and `nofit` (14) alters `P_TryMove`'s
post-move special-line processing. Known demo families: compet-n
`\hr.wad\hr18*.lmp`, `strain.wad\map07` (see entry 1).

### Where we emulate it

No additional site: the writes are entry 1's
(`p_map/spechit.rs:59-89`, `spechit_overrun`), and the consumption is implied by
construction. Our `tmbbox`, `crushchange`, and `nofit` are live engine globals
written at the identical control-flow point with the identical values
(`baseaddr + (ld - lines) * 0x3e`), so every subsequent consumer in our engine
reads the same trampled values vanilla read. Verified against the reference
control flow: our `pit_check_line` (`PIT_CheckLine`) reads `tmbbox` before the
push (`p_map/move.rs:212-219` vs `:253-271`) and `check_position`
(`P_CheckPosition`) re-initializes it
(`p_map/move.rs:443-446`), matching chocolate line for line.

Panic safety of the corrupted values: every consumer is integer fixed-point
arithmetic (`P_PointOnLineSide` `p_maputl/dtmc.rs:72-106` (core, wrapper
`geometry.rs`), `FixedMul` with a
64-bit intermediate), and the emulated addresses stay well inside `i32` range
(~36M max), so the trampled state cannot panic or wrap differently from
vanilla's C arithmetic on real content.

Complevel gating: woof gates the emulation on `demo_compatibility &&
overflow[emu_spechits].enabled` (`reference/woof/src/p_map.c:470-476`); we
emulate unconditionally above count 8, which is correct for the current
vanilla-only surface. Revisiting the gate must wait for F2 (no `Complevel`
enum exists yet).

### Semantics

Bit-exact with chocolate/woof for the doom2.exe 1.9 layout: same case table
(9-12 -> `tmbbox[0..3]`, 13 -> `crushchange`, 14 -> `nofit`), same address
formula, same `-spechit <n>` override. Deliberate divergence recorded in entry
1 (dsda's 13/14 swap + crushchange normalization and its dosdoom variant are
NOT followed). Census: triggers are recorded by entry 1's
`VanillaViolation::SpechitOverrun` hook. The behavior verified here is that
the emulated writes alone reproduce the post-trample collision decisions --
no separate `TmbBoxOverrun` emulation is needed (the census variant exists for
a future finding of a genuinely separate tmbbox trampler, if one ever
surfaces).

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/p_map.c:206-276` (`PIT_CheckLine`: entry
tests at :208-218, push + emulation trigger at :265-276), `:421-424` and
`:447-450` (`P_CheckPosition` re-init + blockmap walks), `:1433-1441` (case
table); cross-checked woof `src/p_map.c:2493-2501` and dsda
`g_overflow.c:305-331`.

## G2. demo-window / ticdup=0 clobber (resolved -- pending human browser re-test)

What happened: on Firefox + real `doom1.wad`, the engine booted, rendered the
title, ran tics 1-5, then trapped with `panic_const_div_by_zero` at
`I_GetTime() / ticdup` in `TryRunTics`: `ticdup` (a `static mut c_int` in
`.bss`, `room/src/doom/d_loop.rs:111`) was zeroed mid-run by an out-of-bounds
write; no source path writes 0 to it (hunt log:
`.superpowers/sdd/2026-09-17-wasm-shell-implementation/task-8-report.md`).
The wasm link map placed `ticdup` in the `p_map` interaction cluster with
`spechit[20]` ending just before `tmfloorz` / log internals /
wasm-bindgen `GLOBAL_EXNDATA`/`HEAP_SLAB` -- making the unguarded
`spechit[20+]` store the leading suspect, which entry 1 removed.

Shipped interim mitigations (guards, not root-cause emulations; see also the
"Related deliberate deviations" section below): guarded spechit store + drains
(entry 1), `ticdup < 1` -> `I_Error` (`d_loop.rs:419-424`), `I_Error` also
emits through the log facade (`room/src/doom/i_system.rs:295`; wasm has no
stderr), the pre-creation frame-entry latch
(`room/src/doom/doomgeneric.rs:214-222`, consulted at `d_main.rs:923` and
`d_main.rs:977`), the web panic hook (`shells/web/src/console_log.rs:49`), and
the headless tick harness (`shells/web/scripts/node-tick-smoke.mjs`).

### The 2026-09-23 audit (task 2 of the vanilla-violations plan)

Question: can any OTHER vanilla OOB write reach the demo/net state cluster
(`ticdup`/`gametic`/`MAKETIC`/`TICDATA`) in the wasm layout now that the
spechit store is bounded?

Inventory of fixed-size stores reachable from WAD/demo input
(`room/src/doom/` source sweep; `tools/scan-addr-refs.mjs` covers statics
only):

| Store | Classification |
|---|---|
| `spechit[20]` push (`p_map/move.rs:264`) | bounded (guard `< MAXSPECIALCROSS`; counter advances unbounded by design, emulation replays the observable writes) |
| `heightlist[20]` stores (`p_spec/geometry.rs:277-287`) | vanilla adjoining-sector overrun, catalog entry 13: writes at `h == 20`/`h == 21` are in-bounds of the `MAX+2` window, the `h == MAX+1` arm's write lands on `height` (emulated verbatim), and `h == MAX+2` is chocolate's `I_Error`; 20/21/22-boundary baseline-pinned in `geometry::tests` |
| `intercepts[189]` stores (`p_maputl/intercepts.rs:253-259`, `:317-323`) | store unguarded past 189 entries in one trace -- **chocolate parity**: chocolate's store is identical (`reference/chocolate-doom/src/doom/p_maputl.c:601-605`; `MAXINTERCEPTS = 128 + 61`, `p_local.h:152-155`). Silent-trampler possible on pathological traces; crispy/woof/dsda grow the array dynamically (`reference/crispy-doom/src/doom/p_maputl.c:555`, `reference/woof/src/p_maputl.c:591`). Limit-removal (F2) candidate; not a canary target (the brief's canary set is the demo/net cluster). |
| `braintargets[32]` store (`p_enemy/brain.rs:71`, `action_brain_awake`) | store unguarded -- **chocolate/crispy parity** (`reference/chocolate-doom/src/doom/p_enemy.c:1846`); silent-trampler possible on maps with > 32 `MT_BOSSTARGET` things; woof grows it dynamically (`reference/woof/src/p_enemy.c:2570-2575`). Limit-removal (F2) candidate; unreachable from Doom 1 content (no boss-brain state machine) and not exercised by the audit run. |
| `playerstarts[4]` (`p_mobj/mapthings.rs:204-213`) | bounded -- only types 1-4 dispatch here, index `type-1` in 0..3 |
| `deathmatchstarts[10]` (`p_mobj/mapthings.rs:190-197`) | bounded store (`< base.add(10)`); starts beyond 10 are silently dropped (vanilla trampled; deathmatch-only path, F2 concern) |
| `bodyque[32]` (`g_game.rs:1772-1777`) | bounded (`% 32` on both read and write) |
| `TICDATA` / `consistancy` | all slot expressions use `% BACKUPTICS` (`d_loop.rs:276,362,678`) |
| visplanes / openings / drawsegs growth | render-side; in Rust these fail loud (index panic), not silent -- limit-removal (F2) concerns, out of canary scope |

Canary run (temporary instrumentation, commit `a3838cb`, stripped in
`428400b`): guard regions around `ticdup`, `gametic`, `MAKETIC` (8-word
windows, change-fingerprint + hard invariants) and the `TICDATA` ring margins
(8 words each side, no legitimate writer inside the engine), checked at every
`TryRunTics` call. Driven 30,000 Node-harness ticks (clock mode, 20x fake
clock, standard entry, `doom1.wad` attract/demo loop; `gametic` 0 -> ~30,000).

Results:

- `TICDUP TRAMPLED`: 0 (ticdup stayed 1 for every one of the 30,000 ticks;
  the harness progress lines confirm `ticdup = 1` throughout).
- `GAMETIC JUMP` / `MAKETIC JUMP`: 0 (monotonic small-step advance only).
- `TICDATA` margin changes: present but fully attributed to named
  linker-adjacent engine statics in the wasm link map
  (`target/ticdup-audit.map`, produced with
  `cargo rustc -p room-shell-web --target wasm32-unknown-unknown --release --
  -C link-args=--Map=...`): LO margin `[-2]` = `MAKETIC` (`0x136994`), HI
  margin `[+0]` = `LASTTIME` (`0x13b99c`), `[+6]`/`[+7]` = the head of
  `r_interp::MOBJ_SLOTS` (`0x13b9b4`) -- all legitimate per-tic writers. No
  unidentified writer appeared in any window over the whole run.
- Window fingerprints corroborate: every changed word maps to a named engine
  static (e.g. the `ticdup` window in this build is the `p_map` interaction
  cluster again: `offsetms curline frontsector backsector sidedef linedef
  ds_p drawsegs`).

Conclusion: the bounded spechit store (entry 1) was the **sole** trampler
reaching the demo/net state cluster in the wasm layout; no remaining
candidate in the inventory can zero or corrupt `ticdup`/`gametic`/
`MAKETIC`/`TICDATA` on the audit's input. Status: **resolved -- pending human
browser re-test** (the browser remains the only true reproducer of the
original freeze; the audit is headless evidence). The `panic_const_div_by_zero`
trap itself is now additionally fenced by the `ticdup < 1 -> I_Error` guard
and the frame-entry latch.

## 10. playeringame[-1] overrun (complete)

### DOS-era root cause

Vanilla `P_SpawnPlayer` gates the spawn on `if (!playeringame[mthing->type
- 1]) return;`. A type-0 mapthing makes the index `-1`: `playeringame[-1]`
reads the byte immediately before `playeringame[]`, which the DOS linker
placed as the last byte of the preceding `players[]` array -- `players[3]
.didsecret` (dsda carries the alias as a source comment:
`reference/dsda-doom/prboom2/src/g_overflow.c:207`). e6y's research
(referenced from dsda's header comment at `g_overflow.c:198-200`,
<http://www.doom2.net/doom2/research/runningbody.zip>) attributes the
doom2.exe reachability to the THINGS-lump type-0 entry following the
player-start dispatch path, so `P_SpawnPlayer` runs with
`mthing->type - 1 == -1` (the vex6d.wad `bug_wald(toke).lmp` demo family).
The released linuxdoom-1.10 source the room port mirrors does not model
this: room's `P_SpawnPlayer` added a plain `type == 0 -> return` guard at
the same control-flow position, and `P_SpawnMapThing`'s `type <= 0 ->
return` skip (`room/src/doom/p_mobj/mapthings.rs:200-202`) matches dsda's own
`case 0: return NULL` (`reference/dsda-doom/prboom2/src/p_mobj.c:2385-2397`),
so a type-0 mapthing never reaches the spawn machinery from level load in
either port.

### What the overrun observably affected

When `players[3].didsecret` was set (a co-op partner found a secret in an
earlier level of the session -- the flag persists across maps), the
`playeringame[-1]` gate opened and vanilla spawned a player mobj bound to a
fabricated player slot before `players[]` in `.bss` (`p = &players[-1]`),
positioned at the type-0 mapthing's coordinates -- the "running body". The
subsequent `P_SpawnPlayer` writes (`playerstate`, `mo`, `viewheight`, ...)
trample whatever precedes `players[]`; the binary's mapthing path also
stores `playerstarts[-1] = *mthing`. Reproducing that write-through would
require emulating the whole DOS `.bss` shadow. (This is the e6y/dsda
account; we did not independently disassemble doom2.exe for this entry.)

### Where we emulate it

`room/src/doom/p_mobj/mapthings.rs:72-94` in `spawn_player` -- the same
control-flow point dsda chose (`reference/dsda-doom/prboom2/src/
p_mobj.c:2074`): the `type == 0` prologue guard records
`VanillaViolation::PlayeringameOverrun` and returns without spawning. The
live trigger surface in our tree is the respawn/fallback paths that pass
unfilled `playerstarts[]` slots to `P_SpawnPlayer` (`G_DoReborn` /
`G_DeathMatchSpawnPlayer` in `room/src/doom/g_game.rs`); a level-load
type-0 mapthing is stopped earlier by `P_SpawnMapThing`'s skip, as in
dsda.

### Semantics

- Condition shape: `mthing->type == 0` alone, matching dsda's
  `PlayeringameOverrun` (`g_overflow.c:203-217`). The aliased
  `players[MAXPLAYERS-1].didsecret` byte is diagnosed (read in-bounds --
  the `-1` index is never evaluated in Rust), never branched on: dsda's
  `EMULATE` returns true unconditionally under the type-0 condition, so
  flag set and flag clear behave identically here -- census hit, no spawn.
- Deliberate divergence from vanilla (documented choice): the running-body
  spawn itself is NOT reproduced -- we follow dsda's bound-the-UB model,
  which reports the aliased byte and skips. The original gap text's
  "when that flag is set, vanilla respawns a player from the type-0 spot"
  describes vanilla, not the model we ported.
- Chocolate finding: chocolate-doom does NOT model this overrun at all --
  `reference/chocolate-doom/src/doom/p_mobj.c:698-701` is a plain
  `if (mthing->type == 0) { return; }` with no overrun report; no
  `PlayeringameOverrun` equivalent exists anywhere in its tree (grep
  clean), and neither woof! nor crispy match the pattern.
- Census: `violations::VanillaViolation::PlayeringameOverrun`
  (`room/src/doom/violations.rs`), recorded on every type-0 arrival at the
  gate.
- Panic safety: no out-of-bounds indexing anywhere on the path (that is
  the point of the model: the `-1` read is modeled in-bounds, not
  recreated).
- Complevel gating: dsda gates the model behind `overflows_enabled` plus
  per-overflow warn/emulate switches (`g_overflow.h` `PROCESS`/`EMULATE`,
  cvar-driven). We record + skip unconditionally, which is correct for the
  current vanilla-only semantics; complevel gating lands with F2 (no
  `Complevel` enum exists yet).
- Regression pin:
  `doom::p_mobj::mapthings::tests::type0_mapthing_records_playeringame_overrun_and_spawns_nothing`
  (`room/src/doom/p_mobj/mapthings.rs:303-380`) drives the gate with the aliased
  byte set and clear, asserting one census hit per arrival and that no
  `players[i].mo` ever becomes non-null.

### Status

`complete`.

### Reference derivation

`reference/dsda-doom/prboom2/src/g_overflow.c:203-216` (`PlayeringameOverrun`:
type-0 condition at `:205`, the `playeringame[-1] == players[3].didsecret`
alias comment at `:207`, the diagnostic read passed to
`ShowOverflowWarning` at `:208`, `EMULATE` -> early return at `:210-213`);
e6y header comment with the vex6d/runningbody provenance at `:198-200`;
call site `reference/dsda-doom/prboom2/src/p_mobj.c:2074` (prologue return
before the `!playeringame[n]` gate at `:2079`); dsda's own type-0 skip in
`P_SpawnMapThing` at `p_mobj.c:2385-2397`. Chocolate's unmodeled guard:
`reference/chocolate-doom/src/doom/p_mobj.c:698-701`.

## 11. door/plat `specialdata` aliasing (complete)

### DOS-era root cause

Vanilla tags thinkers sharing the `sector_t.specialdata` void* slot with no
type field: `EV_VerticalDoor`'s toggle branch (specials 1/26/27/28/117) casts
the slot to `vldoor_t*` and identifies the occupier only by comparing
`thinker.function.acp1` against `T_VerticalDoor` and `T_PlatRaise`
(`vendor/doomgeneric/p_doors.c:418`, `:422` -- the "When is a door not a
door?" comment). A sector whose use-line expects a door can legitimately host
a *plat* thinker (both specials claim the same `specialdata` slot on such
WADs), and any third thinker type falls into the else branch, which writes
`door->direction = -1` through the door cast regardless of what the pointer
really addresses -- a vanilla type-punning write. The released source carries
the ep1-0500.lmp note: a plat and a door cross-referenced on the same sector;
the door doesn't open on 64-bit builds.

### What the aliasing observably affected

On ep1-0500.lmp (Doom 1 E1M5, recorded on the DOS binary) the toggle branch
meets a plat: vanilla's `T_PlatRaise` compare fires and sets `plat->wait =
-1` instead of closing a "door", so the plat starts descending and the demo
stays in sync -- the compare table IS the compatibility behavior. For any
non-door non-plat occupier, the else-branch `direction = -1` store lands at
`vldoor_t`'s `direction` offset (48) inside an alien thinker struct,
corrupting whatever field lives there; demos recorded against that trample
depend on the corrupted value.

### Where we emulate it

`room/src/doom/p_doors/events.rs:294-316` in `EV_VerticalDoor`: the same two
compares, transmuting `T_VerticalDoor` and `T_PlatRaise` (the latter imported
through the p_plats graduate's root re-export -- the identical single
function item `EV_DoPlat` stores into `acp1`, which is what keeps the pointer
compares meaningful), the `plat->wait = -1` arm through the `plat_t` cast,
and the else branch verbatim:

```rust
if (*door).thinker.function.acp1 == t_vdoor {
    (*door).direction = -1;
} else if (*door).thinker.function.acp1 == t_plat {
    let plat = door as *mut plat_t;
    (*plat).wait = -1;
} else {
    eprintln!("EV_VerticalDoor: Tried to close something that wasn't a door.");
    (*door).direction = -1;
}
```

`sector->specialdata` in our port only ever holds thinkers we spawned, so the
compare targets are in-tree types rather than raw DOS memory; the write
through the cast reproduces the vanilla aliasing on our layout.

### Semantics

Bit-exact with vanilla's compare table (door -> close, plat -> `wait = -1`,
other -> stderr note + blind `direction = -1` write). Known pre-existing
divergence carried deliberately: the else branch prints via `eprintln!` where
C uses `fprintf(stderr, ...)` -- stderr-text only, no sim-state difference.
No census hook (pure replication, entry-8 style). The `T_VerticalDoor` /
`T_PlatRaise` operands must stay single function items per name (the
graduates' one-item-per-name re-export ruling) or the compares silently break.

### Status

`complete`.

### Reference derivation

`vendor/doomgeneric/p_doors.c:414-435` (the comment block, both `acp1`
compares, the `plat->wait = -1` arm, and the ep1-0500.lmp note), `:436-446`
(the else branch with `fprintf(stderr, ...)` at `:440`); cross-referenced
from the
p_plats graduate's mapping table (`room/src/doom/p_plats/mod.rs`,
`T_PlatRaise` row). The upstream sliding-door family around it is `#if 0`
("abandoned to the mists of time") and correctly absent from the port.

## 12. null-target dereference substitution (complete)

### DOS-era root cause

Several monster/weapon codepaths dereference a `target` (or tracer)
pointer that can legitimately be NULL: `A_Fire` and the mancubus
trio read `actor->target`, `A_SpawnFly` reads `mo->tracer`, and
`P_LineAttack` re-reads the shoot-through `t1` -- all after logic that
can leave the pointer unset (a monster firing with no target, a
hair-trigger `A_Look` race, a hitscan whose shooter died mid-trace).
The released source acknowledges the hazard in so many words
(`vendor/doomgeneric/p_mobj.c:925-927`: "Doom did not crash because of
the lack of proper memory protection") and funnels the reads through
`P_SubstNullMobj` (`p_local.h:114`): a NULL becomes `&dummy_mobj`, a
function-local `static` in DOS `.bss`, so the dereference lands on
allocated (zero-initialized) memory instead of faulting. Vanilla's
accidental safety net was the DOS arena's absent MMU plus `.bss`
zero-fill -- not a correctness guarantee.

### What the null dereference observably affected

With the substitution, a target-less `A_Fire` proceeds through the
puff/blood spawn path against a mobj at `(0, 0)` -- the shots still
play their sound and draw their puff, and the tic still advances, so
no demo ever desyncs from the "crash" vanilla never had. The model
that matters for us is the shape of the degrade: every consumer that
reaches a NULL target must observe a *zeroed* mobj (x/y/z/flags = 0)
rather than a fault or an early return. Without any substitution the
Rust port would dereference NULL and panic (wasm trap) on the same
inputs vanilla played through.

### Where we emulate it

`room/src/doom/p_mobj/spawn.rs:274-289` in `subst_null_mobj` (export
pinned as `P_SubstNullMobj`): null in -> pointer to a function-local
`static mut DUMMY_MOBJ` whose `x`/`y`/`z`/`flags` fields are re-zeroed
on every substitution (the exact four fields upstream writes),
non-null in -> the pointer passes through unchanged. The call sites
are the upstream set, unchanged by the p_mobj graduation and re-homed
by the p_enemy graduation:
`A_Fire`/`A_FatAttack1`/`A_FatAttack2`/`A_FatAttack3`/`A_SpawnFly`
(`room/src/doom/p_enemy/vile_fire.rs:88`,
`p_enemy/mancubus.rs:48`, `:77`, `:111`, `p_enemy/brain.rs:243`)
and `P_LineAttack`'s `t1` substitution
(`room/src/doom/p_map/attack.rs:394`).

### Semantics

Bit-exact with the vanilla helper: the null arm returns the zeroed
dummy, the non-null arm is the identity, and the zeroing covers
exactly `x`, `y`, `z`, `flags` (upstream `p_mobj.c:929-943` writes
those four). Known modeling limit, documented deliberately: DOS's
`&dummy_mobj` lives in `.bss` (zero-filled once) while the port
re-zeroes the four fields per call -- observably identical because
nothing between calls writes those fields on the dummy (no code takes
the dummy's address for mutation), and any write would be a `.bss`
trample we do not model. No census hook (pure replication, entry-8
style). The dummy pointer is valid only until the next call from the
same thread; callers must not store it across ticks (upstream has the
same lifetime).

### Status

`complete`.

### Reference derivation

`vendor/doomgeneric/p_mobj.c:925-941` (the memory-protection comment
and the full body) and `p_local.h:114`; callers
`vendor/doomgeneric/p_enemy.c:1252` (`A_Fire`), `:1356`/`:1375`/`:1393`
(`A_FatAttack1`/`2`/`3`), `:1945` (`A_SpawnFly`), and
`vendor/doomgeneric/p_map.c:1076` (`P_LineAttack`); chocolate parity:
`reference/chocolate-doom/src/doom/p_mobj.c:952-964` (identical comment
and body) with the same call sites
(`reference/chocolate-doom/src/doom/p_enemy.c:1267`/`:1371`/`:1390`/
`:1408`/`:1963`, `reference/chocolate-doom/src/doom/p_map.c:1072`).

## 13. stair heightlist overrun (complete)

### DOS-era root cause

`P_FindNextHighestFloor` (the stair-builder / raise-floor target scan)
collects every neighboring floor above the current one into
`fixed_t heightlist[MAX_ADJOINING_SECTORS]` -- a 20-slot array living on
the C stack, immediately below other locals. A sector with more than 20
two-sided neighbors above the current floor keeps storing: store 21
lands on the stack slot of the sweep's own `height` filter variable, and
store 22+ tramples deeper stack until the function returns to garbage or
the map's stair build reads nonsense heights.

### What the trample observably affected

Store 21 is the observable one: the trampled `height` is the filter
threshold every later neighbor is tested against, so after the overrun
the sweep only accepts neighbors above the 21st floor height -- a
shifted qualifying set that changes which height the mover targets
(demos on such maps depend on it). Stores past 22 crash vanilla;
chocolate models the boundary as `I_Error("Sector with more than 22
adjoining sectors. Vanilla will crash here")`.

### Where we emulate it

`room/src/doom/p_spec/geometry.rs:262-305` in `next_highest_floor`
(export pinned as `P_FindNextHighestFloor`): the array is declared with
the `MAX_ADJOINING_SECTORS + 2` window exactly as chocolate declares it,
the `h == MAX_ADJOINING_SECTORS + 1` arm performs the shadow write
`height = other->floorheight` verbatim (the 21st store never reaches the
array -- it raises the filter threshold instead, which the 23-line
baseline drive in `geometry::tests` pins as the observable effect), and
the `h == MAX_ADJOINING_SECTORS + 2` arm calls `i_error!` with
chocolate's message. The emulation is interleaved with the sweep and was
kept whole through the p_spec graduation -- never split, never "fixed"
into a bounds-checked store (G2 discipline).

### Semantics

Bit-exact with chocolate for the drives vanilla survives: 20 and 21
qualifying neighbors store normally, 22 neighbors hit the shadow arm as
the last sweep entry and still return the true minimum, and the
shadow-write's threshold effect keeps a 23rd (lower) neighbor out of the
crash arm. A 23rd qualifying neighbor aborts with `i_error!` (vanilla
crashes; we fail loud). The 20/21/22 boundary and the shadow-arm
discrimination are baseline-pinned (`geometry::tests::
baseline_next_highest_floor_20_21_22_overrun_boundary`, written against
the pre-move body in commit `3841018` and mutation-checked there:
disabling the shadow arm aborts the drive through the crash path).

### Status

`complete`.

### Reference derivation

`reference/chocolate-doom/src/doom/p_spec.c:329`
(`MAX_ADJOINING_SECTORS 20`), `:342` (the `MAX + 2` declaration),
`:354-359` ("Emulation of memory (stack) overflow" + the `h == MAX+1`
shadow write), `:360-364` (the `h == MAX+2` `I_Error`); room-port mirror
`vendor/doomgeneric/p_spec.c:329-330`.

## Known gaps / intake

Intake rules: an audit item becomes a full entry only when (a) the vanilla
root cause is pinned to the DOS binary layout or an authoritative reference
line, (b) the observable effect is stated, and (c) our emulation site and
semantics are known. Until then it lives here. G1 and G2 were intake items
until the 2026-09-23 audit promoted them to full entries above; G4 followed
the same day with the map33 par-time fix (entry 9); G3 followed with the
playeringame[-1] overrun model (entry 10). The intake list is currently
empty: no known vanilla-defect gap is unverified or unimplemented.

## Related deliberate deviations (not vanilla-bug workarounds)

These are cataloged so future audits do not misclassify them; none is
motivated by a vanilla DOOM bug, and none changes simulation-observable
state for compatible inputs.

- **`TryRunTics` stall cap + `D_StartNetGame` parity**
  (`room/src/doom/d_loop.rs:103`, `:646-669`, `:408-424`; commit `a297e53`).
  The port originally gave up after a single tic boundary (`I_GetTime() /
  ticdup - entertic > 0` -- the linuxdoom `d_net.c` behavior), which let
  `TryRunTics` return having run zero tics so the renderer drew unsimulated
  state (observed headless as `D_PageDrawer` before `pagename` was set).
  We now replicate chocolate's gated loop: escape only while still short
  (`lowtic < gametic/ticdup + counts`) and after `MAX_NETGAME_STALL_TICS`
  (5) stall tics, matching `reference/chocolate-doom/src/d_loop.c:737-758`
  and `:50-53`. (Chocolate's comment claims "Vanilla Doom used 20" for the
  cap; the released linuxdoom source actually returns after one tic
  boundary -- the discrepancy is chocolate's characterization, inherited
  verbatim into our doc comment at `d_loop.rs:101`.)
  `D_StartNetGame` additionally fills `localplayer`/`local_playeringame[]`
  and rejects `ticdup < 1` with `I_Error`, matching
  `reference/chocolate-doom/src/d_loop.c:402-419`
  ("D_StartNetGame: invalid ticdup value"). This is *chocolate-parity
  robustness*, not a vanilla-defect emulation: vanilla's early return is
  itself deterministic and demo-exact; we deviate from it only in how long
  the host may spin before a frame renders during a stall. The ticdup guard
  doubles as the symptom guard for G2. The revert `54c9f45` records these as
  "mature-practice safety floor pending the explicit vanilla-violations
  design".
- **`pump_tic_cap`** (`room/src/doom/d_loop.rs:131`, applied at
  `d_loop.rs:641-644`): browser-shell catch-up cap so a suspended tab
  cannot trigger a multi-second tic burst. Render-side policy (`AGENTS.md`
  constraint 3), vanilla path unaffected at the default 0. Not a vanilla
  defect workaround.
- **Engine-created frame-entry latch** (`doomgeneric.rs:214-222`,
  `d_main.rs:923`/`:977`): no-op frames before `doomgeneric_Create`
  completes, because calling into a zero-initialized engine divides by
  `ticdup == 0`. Host-lifecycle platform work, not vanilla emulation (the
  ticdup=0 there comes from pre-init zeroing, not an OOB write).
- **Mouse/joystick button accessors** (`room/src/doom/g_game.rs:833-845`,
  `mousebutton`/`joybutton`): vanilla's `mousebuttons = &mousearray[1]`
  negative-index sentinel is legal C, faithfully rewritten without pointer
  aliasing. Not a bug emulation.
- **`vanilla_savegame_limit` / `vanilla_demo_limit`**
  (`room/src/doom/g_game.rs:520-525`, enforced at `:2393` and `:2775`):
  user-configurable enforcement of vanilla's buffer caps (I_Error) versus
  auto-growth. Limit-removal configuration, not bug emulation.
- **Zone sizing policy** (`room/src/doom/i_system.rs`, `DEFAULT_RAM`,
  this commit): vanilla sizes its zone at 6 MiB (`DEFAULT_RAM` in
  `i_system.c`); we ship 32 MiB on every target. Root cause: the vanilla
  budget starved on the restart path (spec 4 defect A --
  `wipe_init_melt`'s 1,280-byte `Z_Malloc` failed after a level's worth of
  allocation), which vanilla never exercises in our test corpora.
  Observable-fidelity argument: zone *size* is not observable by demos --
  the wipe transients are zone-served identically at any size (vanilla
  `f_wipe.c` also uses `Z_Malloc`, `reference/mbf/src/f_wipe.c:47/:102`),
  and `PadRejectArray` (entry 4) models fixed DOS zone-header bytes
  independent of our layout. The `-mb` override keeps vanilla's knob.

Interim-guard dispositions (V3 final pass, 2026-09-25): the guards shipped
around the G2 incident are all **KEEP**; the 2026-09-23 audit obsoleted none
of them and no guard was stripped. Per-guard rationale:

- `ticdup < 1 -> I_Error` (`d_loop.rs:419-424`) -- KEEP: chocolate-parity
  `D_StartNetGame` validation on the live boot path (`D_CheckNetGame` ->
  `D_StartNetGame`) that doubles as the G2 symptom guard; the audit found no
  distinct root cause, so nothing argues the parity guard away.
- `pump_tic_cap` (`d_loop.rs:131`) -- KEEP: F1 render policy (bullet above),
  not a workaround; nothing to strip.
- Engine-created frame-entry latch (`doomgeneric.rs:214-222`) -- KEEP:
  host-lifecycle platform work; the `ticdup == 0` it fences comes from
  pre-init zeroing, not from any remaining OOB writer.
- Guarded spechit store + bounded drains (`p_map/move.rs:262-270`, `:574-577`,
  `p_enemy/chase.rs:276-279`) -- KEEP: they are entry 1's emulation surface itself;
  stripping them would reintroduce the G2 silent trampler.

## Out of scope

Platform/CRT degradations -- `room/src/doom/crt.rs` (e.g. the wasm mkdir
no-op), the per-arity printf/stdio shims it provides, and the wasm
entry/logging surface (panic hook, `I_Error` log facade) -- are platform
work documented with the shells and the wasm FFI decisions
(`docs/decision-wasm-per-arity-crt.md` and the wasm-shell design notes), not
vanilla-behavior emulation, and are deliberately excluded from this catalog.
