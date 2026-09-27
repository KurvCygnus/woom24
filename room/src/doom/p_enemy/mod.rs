//! Monster AI: target acquisition, sound propagation, the chase
//! state machine, every monster attack action, death/pain reactions,
//! boss-death map events, the Archvile resurrection and fire pillar,
//! and the Icon of Sin spawn logic -- bit-exact with
//! `vendor/doomgeneric/p_enemy.c`.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the numeric vocabulary: melee/missile ranges,
//!   floating budgets, the eight-direction chase lattice (`DI_*`),
//!   the game-mode/version/skill gate values, the boss floor/door
//!   special types, `FATSPREAD`/`SKULLSPEED`, and the module-local
//!   C type aliases
//! - `noise.rs` -- sound propagation: `recursive_sound` (the sector
//!   flood) and `noise_alert`, plus the `soundtarget` static
//! - `chase.rs` -- the chase core: `action_look` / `action_chase`,
//!   `check_melee_range` / `check_missile_range`, `move_step`,
//!   `try_walk`, `new_chase_dir`, `look_for_players`, and the
//!   `opposite`/`diags`/`xspeed`/`yspeed` tables
//! - `attacks.rs` -- `action_face_target` and the twelve
//!   hitscan/projectile monster attacks (former human through baron)
//! - `revenant.rs` -- the Revenant: `action_skel_missile`,
//!   `action_tracer` (homing steer), `action_skel_whoosh`,
//!   `action_skel_fist`, and the `TRACEANGLE` static
//! - `mancubus.rs` -- `action_fat_raise` and the three two-shot
//!   `FATSPREAD` phases
//! - `souls.rs` -- the Lost Soul charge (`action_skull_attack`), the
//!   20-skull-capped spawn helper `action_pain_shoot_skull`, and the
//!   Pain Elemental attack/death pair
//! - `vile.rs` -- the Archvile resurrection: `pit_vile_check` (the
//!   blockmap callback) and `action_vile_chase`, with the
//!   `corpsehit`/`vileobj`/`viletryx`/`viletryy` statics
//! - `vile_fire.rs` -- the Archvile fire pillar (startup, sustain,
//!   the entry-8 target lock, the payload)
//! - `death.rs` -- the variant-randomised screams (`action_scream`,
//!   `action_x_scream`, `action_player_scream`), `action_pain`,
//!   `action_fall`, and the boss footsteps
//! - `map_events.rs` -- `action_keen_die`, `action_boss_death` with
//!   its private `check_boss_end` matrix, `action_explode`, and the
//!   shared `is_mobj_thinker` pointer-identity helper
//! - `ssg.rs` -- the Super Shotgun sound trio (close action refires
//!   through p_pspr's root `A_ReFire` re-export)
//! - `brain.rs` -- the Icon of Sin: target collection, the cube spit
//!   (function-local `easy` alternation), the cube flight/arrival,
//!   and the death sequence, with the `braintargets` statics
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below. Every freeze-zone importer (`p_pspr`) still
//! names the upstream identifiers through the root, and the
//! `states` table in `info.rs` resolves all 54 AI actions BY SYMBOL.
//!
//! Cross-module contracts documented once here:
//! - **info.rs linkage (load-bearing pins)**: the `extern "C"` block
//!   at `info.rs:935-1290` declares exactly 54 `A_*` actions from
//!   this module and the states table stores them as
//!   `Some(A_*)` function pointers; touching info.rs is forbidden
//!   (freeze zone). All 54 `#[export_name = "A_*"]` pins are
//!   therefore LOAD-BEARING, not conservatism -- one missed or
//!   misspelled pin breaks the link loudly. `action_pain_shoot_skull`
//!   is direct-call only (absent from the extern block) but was
//!   `#[no_mangle]`, so it is pinned too under the name = symbol =
//!   pin conservatism rule (p_pspr precedent); the 8 `P_*` and
//!   `PIT_VileCheck` pins ride the same rule. The pin set keeps the
//!   wasm/extern symbol surface byte-identical to the pre-split
//!   module.
//! - **Dependency directions**: `p_pspr/engine.rs` imports
//!   `P_NoiseAlert` through the root shim (called in `P_FireWeapon`);
//!   the reverse edge, `ssg.rs` -> `A_ReFire`, imports through
//!   p_pspr's root re-export (never a submodule path). `move_step`
//!   keeps reading `crate::doom::p_map::MAXSPECIALCROSS` by full
//!   path, which rides p_map's landed `pub(crate)` re-export
//!   (p_map/mod.rs), as do the `floatok`/`tmfloorz`/`spechit`/
//!   `numspechit` and `P_TryMove`/`P_TeleportMove`/`P_AimLineAttack`/
//!   `P_LineAttack`/`P_RadiusAttack`/`P_CheckPosition` imports.
//!   The 4x `P_MobjThinker` acp1 compares (Keen/boss/brain scans and
//!   the skull cap) resolve through p_mobj's root shim and are
//!   single-sourced in `map_events::is_mobj_thinker`.
//! - **c_ffi extern statics**: `xspeed`/`yspeed`/`braintargets`/
//!   `numbraintargets`/`braintargeton` are declared by symbol in
//!   `c_ffi.rs`'s extern block (C test harness surface). Statics keep
//!   their upstream names + `#[no_mangle]` (data tier), so those
//!   externs are untouched. c_ffi's `TRACEANGLE`/`FATSPREAD`/
//!   `SKULLSPEED` are c_ffi-local consts, not externs.
//! - No C translation unit references any function symbol here
//!   (`doomgeneric-sys/build.rs` excludes `p_enemy.c`); c_tests read
//!   the module only through c_ffi and are unaffected by the split.
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
//! statics/consts/tables keep their upstream names (data-tier renaming
//! comes with freeze-zone retirement). The `A_*` -> `action_*` prefix
//! renders the C "state-machine action" role (the pit_/ptr_ precedent:
//! the C prefix encoded the call role); moniker fragments
//! (`Bspi`/`CPos`/`SPos`/`Skel`/`Fat`/`Vile`/`Troop`/`Sarg`/`Head`)
//! are kept verbatim so the C mapping stays greppable.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_RecursiveSound` | `noise::recursive_sound` | dtmc (whole-body) | validcount stamp + flood order; `soundtarget` write per sector; shim + pin; upstream `p_enemy.c:98` |
//! | `P_NoiseAlert` | `noise::noise_alert` | dtmc (whole-body) | `soundtarget` set then `validcount += 1` then flood; p_pspr calls it per shot; shim + pin; upstream `p_enemy.c:151` |
//! | `P_CheckMeleeRange` | `chase::check_melee_range` | dtmc (whole-body pure) | `MELEERANGE - 20*FRACUNIT + radius` gate + sight; shim + pin; upstream `p_enemy.c:167` |
//! | `P_CheckMissileRange` | `chase::check_missile_range` | dtmc (whole-body) | JUSTHIT/reactiontime gates, per-monster distance ladders, terminal `P_Random() < dist` draw; shim + pin; upstream `p_enemy.c:190` |
//! | `P_Move` | `chase::move_step` | dtmc (whole-body) | the spechit drain (entry-1/G2 KEEP site; decrement-then-skip order pinned by the catalog), FLOATSPEED float arm, floor snap; shim + pin; upstream `p_enemy.c:260` |
//! | `P_TryWalk` | `chase::try_walk` | dtmc (whole-body) | move-step success then `movecount = P_Random() & 15` (the per-walk RNG-init other AI reads); shim + pin; upstream `p_enemy.c:337` |
//! | `P_NewChaseDir` | `chase::new_chase_dir` | dtmc (whole-body) | swap coin `P_Random() > 200`, sweep coin `P_Random() & 1`, try-walk attempt order; shim + pin; upstream `p_enemy.c:351` |
//! | `P_LookForPlayers` | `chase::look_for_players` | dtmc (whole-body) | `lastlook & 3` cycle + two-player cap + 90-degree cone; shim + pin; upstream `p_enemy.c:486-487` |
//! | `A_Look` | `chase::action_look` | dtmc (whole-body) | seesound variant draws `P_Random() % 3` / `% 2` ordered before the seestate transition; pin LOAD-BEARING (info.rs extern); upstream `p_enemy.c:589` |
//! | `A_Chase` | `chase::action_chase` | dtmc (whole-body) | the per-tic chase ladder (reactiontime/threshold/angle-snap/attack gates/movecount); pin LOAD-BEARING; upstream `p_enemy.c:657` |
//! | `A_FaceTarget` | `attacks::action_face_target` | dtmc (whole-body) | shadow jitter `(P_Random() - P_Random()) << 21`; the shared first step of every attack; pin LOAD-BEARING; upstream `p_enemy.c:767` |
//! | `A_PosAttack` | `attacks::action_pos_attack` | dtmc (whole-body) | aim, then spread pair `<< 20`, then `(rnd%5+1)*3`; pin LOAD-BEARING; upstream `p_enemy.c:787` |
//! | `A_SPosAttack` | `attacks::action_spos_attack` | dtmc (whole-body) | 3 pellets x (spread pair + damage draw) in loop order; pin LOAD-BEARING; upstream `p_enemy.c:806` |
//! | `A_CPosAttack` | `attacks::action_cpos_attack` | dtmc (whole-body) | sound/aim before the spread pair; pin LOAD-BEARING; upstream `p_enemy.c:830` |
//! | `A_CPosRefire` | `attacks::action_cpos_refire` | dtmc (whole-body) | `P_Random() < 40` keep-firing draw before the sight gate; pin LOAD-BEARING; upstream `p_enemy.c:850` |
//! | `A_SpidRefire` | `attacks::action_spid_refire` | dtmc (whole-body) | `P_Random() < 10` variant; pin LOAD-BEARING; upstream `p_enemy.c:867` |
//! | `A_BspiAttack` | `attacks::action_bspi_attack` | dtmc (whole-body) | face then plasma spawn (statement order); pin LOAD-BEARING; upstream `p_enemy.c:883` |
//! | `A_TroopAttack` | `attacks::action_troop_attack` | dtmc (whole-body) | melee `(rnd%8+1)*3` else fireball; pin LOAD-BEARING; upstream `p_enemy.c:898` |
//! | `A_SargAttack` | `attacks::action_sarg_attack` | dtmc (whole-body) | melee-only `(rnd%10+1)*4`; pin LOAD-BEARING; upstream `p_enemy.c:920` |
//! | `A_HeadAttack` | `attacks::action_head_attack` | dtmc (whole-body) | melee `(rnd%6+1)*10` else `MT_HEADSHOT`; pin LOAD-BEARING; upstream `p_enemy.c:935` |
//! | `A_CyberAttack` | `attacks::action_cyber_attack` | dtmc (whole-body) | face then rocket spawn; pin LOAD-BEARING; upstream `p_enemy.c:954` |
//! | `A_BruisAttack` | `attacks::action_bruis_attack` | dtmc (whole-body) | melee `(rnd%8+1)*10` else baron shot; pin LOAD-BEARING; upstream `p_enemy.c:964` |
//! | `A_SkelMissile` | `revenant::action_skel_missile` | dtmc (whole-body) | z raise/spawn/lower order + tracer link; pin LOAD-BEARING; upstream `p_enemy.c:987` |
//! | `A_Tracer` | `revenant::action_tracer` | dtmc (whole-body) | `gametic & 3` gate, smoke spawn + tic jitter draw, angle-step pins, `momz` converge; pin LOAD-BEARING; upstream `p_enemy.c:1006` |
//! | `A_SkelWhoosh` | `revenant::action_skel_whoosh` | dtmc (whole-body) | face + whoosh order; pin LOAD-BEARING; upstream `p_enemy.c:1078` |
//! | `A_SkelFist` | `revenant::action_skel_fist` | dtmc (whole-body) | melee `(rnd%10+1)*6` with sound-before-damage order; pin LOAD-BEARING; upstream `p_enemy.c:1086` |
//! | `PIT_VileCheck` | `vile::pit_vile_check` | dtmc (whole-body) | corpse probe + `height <<= 2` probe order; blockmap-iterator role keeps the PIT_ prefix in the pin; shim + pin; upstream `p_enemy.c:1114` |
//! | `A_VileChase` | `vile::action_vile_chase` | dtmc (whole-body) | blockmap sweep + resurrection sequence (face-borrow, heal state, restore order); pin LOAD-BEARING; upstream `p_enemy.c:1152` |
//! | `A_VileStart` | `vile_fire::action_vile_start` | dtmc (whole-body) | attack-start sound; pin LOAD-BEARING; upstream `p_enemy.c:1218` |
//! | `A_StartFire` | `vile_fire::action_start_fire` | dtmc (whole-body) | sound then reposition; pin LOAD-BEARING; upstream `p_enemy.c:1230` |
//! | `A_FireCrackle` | `vile_fire::action_fire_crackle` | dtmc (whole-body) | sustained sound then reposition; pin LOAD-BEARING; upstream `p_enemy.c:1236` |
//! | `A_Fire` | `vile_fire::action_fire` | dtmc (whole-body) | sight-gated reposition via unset/move/set; catalog entry 12 call site; pin LOAD-BEARING; upstream `p_enemy.c:1242` |
//! | `A_VileTarget` | `vile_fire::action_vile_target` | dtmc (whole-body; entry-8 bug site) | the `(x, x, z)` vanilla typo spawn (catalog entry 8, reproduced faithfully); pin LOAD-BEARING; upstream `p_enemy.c:1273` |
//! | `A_VileAttack` | `vile_fire::action_vile_attack` | dtmc (whole-body) | 20 direct damage, mass-scaled thrust, fire reposition, radius 70; pin LOAD-BEARING; upstream `p_enemy.c:1298` |
//! | `A_FatRaise` | `mancubus::action_fat_raise` | dtmc (whole-body) | face + warn sound; pin LOAD-BEARING; upstream `p_enemy.c:1339` |
//! | `A_FatAttack1` | `mancubus::action_fat_attack1` | dtmc (whole-body) | +FATSPREAD, two spawns, second shot velocity rewrite; pin LOAD-BEARING; upstream `p_enemy.c:1346` |
//! | `A_FatAttack2` | `mancubus::action_fat_attack2` | dtmc (whole-body) | -FATSPREAD / -2*FATSPREAD variant; pin LOAD-BEARING; upstream `p_enemy.c:1366` |
//! | `A_FatAttack3` | `mancubus::action_fat_attack3` | dtmc (whole-body) | symmetric ±FATSPREAD/2 with per-shot recompute; pin LOAD-BEARING; upstream `p_enemy.c:1385` |
//! | `A_SkullAttack` | `souls::action_skull_attack` | dtmc (whole-body) | SKULLFLY flag, sound, face, momentum from flight time; pin LOAD-BEARING; upstream `p_enemy.c:1415` |
//! | `A_PainShootSkull` | `souls::action_pain_shoot_skull` | dtmc (whole-body) | 20-skull thinker walk, prestep, `P_TryMove`-fail 10000-damage telefrag; direct-call only (absent from info.rs extern) but pinned (was `#[no_mangle]`); upstream `p_enemy.c:~1445` |
//! | `A_PainAttack` | `souls::action_pain_attack` | dtmc (whole-body) | face then skull spawn at current angle; pin LOAD-BEARING; upstream `p_enemy.c:1508` |
//! | `A_PainDie` | `souls::action_pain_die` | dtmc (whole-body) | fall + 90/180/270 skull order; pin LOAD-BEARING; upstream `p_enemy.c:1518` |
//! | `A_Scream` | `death::action_scream` | dtmc (whole-body) | deathsound variant draws `% 3` / `% 2`, global-volume bosses; pin LOAD-BEARING; upstream `p_enemy.c:1531` |
//! | `A_XScream` | `death::action_x_scream` | dtmc (whole-body) | gib scream; pin LOAD-BEARING; upstream `p_enemy.c:1568` |
//! | `A_Pain` | `death::action_pain` | dtmc (whole-body) | painsound gate; pin LOAD-BEARING; upstream `p_enemy.c:1573` |
//! | `A_Fall` | `death::action_fall` | dtmc (whole-body) | MF_SOLID clear (corpse walkable); pin LOAD-BEARING; upstream `p_enemy.c:1581` |
//! | `A_Explode` | `map_events::action_explode` | dtmc (whole-body) | radius-128 blast from `target`; pin LOAD-BEARING; upstream `p_enemy.c:1594` |
//! | `CheckBossEnd` (private, not `#[no_mangle]`) | `map_events::check_boss_end` | dtmc (whole-body pure) | pre/post-ultimate episode/map matrix; private before and after -- doc alias only (spechit_overrun precedent); upstream `p_enemy.c:1605` |
//! | `A_BossDeath` | `map_events::action_boss_death` | dtmc (whole-body) | player-alive scan + thinker walk + 666/667 event order + `G_ExitLevel` fall-through; pin LOAD-BEARING; upstream `p_enemy.c:1656` |
//! | `A_Hoof` | `death::action_hoof` | dtmc (whole-body) | footstep then chase; pin LOAD-BEARING; upstream `p_enemy.c:1757` |
//! | `A_Metal` | `death::action_metal` | dtmc (whole-body) | footstep then chase; pin LOAD-BEARING; upstream `p_enemy.c:1763` |
//! | `A_BabyMetal` | `death::action_baby_metal` | dtmc (whole-body) | footstep then chase; pin LOAD-BEARING; upstream `p_enemy.c:1769` |
//! | `A_OpenShotgun2` | `ssg::action_open_shotgun2` | dtmc (whole-body) | breech-open sound; pin LOAD-BEARING; upstream `p_enemy.c:1775` |
//! | `A_LoadShotgun2` | `ssg::action_load_shotgun2` | dtmc (whole-body) | shell-load sound; pin LOAD-BEARING; upstream `p_enemy.c:1783` |
//! | `A_CloseShotgun2` | `ssg::action_close_shotgun2` | dtmc (whole-body) | close sound then `A_ReFire` (cross-module, root re-export); pin LOAD-BEARING; upstream `p_enemy.c:1791` |
//! | `A_BrainAwake` | `brain::action_brain_awake` | dtmc (whole-body) | thinker walk + the UNGUARDED `braintargets[32]` store (catalog G1 row, chocolate-parity KEEP); pin LOAD-BEARING; upstream `p_enemy.c:1811` |
//! | `A_BrainPain` | `brain::action_brain_pain` | dtmc (whole-body) | global pain sound; pin LOAD-BEARING; upstream `p_enemy.c:1841` |
//! | `A_BrainScream` | `brain::action_brain_scream` | dtmc (whole-body) | 8-unit rocket row: z draw, spawn, momz draw, state, tic-jitter draw per column; pin LOAD-BEARING; upstream `p_enemy.c:1847` |
//! | `A_BrainExplode` | `brain::action_brain_explode` | dtmc (whole-body) | spread pair `* 2048`, z draw, momz draw, tic jitter; pin LOAD-BEARING; upstream `p_enemy.c:1873` |
//! | `A_BrainDie` | `brain::action_brain_die` | dtmc (whole-body) | `G_ExitLevel`; pin LOAD-BEARING; upstream `p_enemy.c:1894` |
//! | `A_BrainSpit` | `brain::action_brain_spit` | dtmc (whole-body) | function-local `static mut easy` alternation, round-robin target, travel-time reactiontime; pin LOAD-BEARING; upstream `p_enemy.c:1899` |
//! | `A_SpawnSound` | `brain::action_spawn_sound` | dtmc (whole-body) | cube sound then fly step; pin LOAD-BEARING; upstream `p_enemy.c:1928` |
//! | `A_SpawnFly` | `brain::action_spawn_fly` | dtmc (whole-body) | reactiontime gate, fog spawn + sound, `dtmc::spawn_fly_pick` (one draw), look/telefrag/removal order; pin LOAD-BEARING; upstream `p_enemy.c:1934` |
//! | `A_PlayerScream` | `death::action_player_scream` | dtmc (whole-body) | commercial `< -50` gib variant; pin LOAD-BEARING; upstream `p_enemy.c:1992` |
//! | 13 `#[no_mangle]` statics (`opposite`, `diags`, `soundtarget`, `xspeed`, `yspeed`, `TRACEANGLE`, `corpsehit`, `vileobj`, `viletryx`, `viletryy`, `braintargets`, `numbraintargets`, `braintargeton`) | `chase` / `noise` / `revenant` / `vile` / `brain` | data | upstream names + `#[no_mangle]` retained; every root path held by the `path-stability re-export` blocks below; c_ffi's extern-by-symbol statics (`xspeed`/`yspeed`/`braintargets`/`numbraintargets`/`braintargeton`) untouched |
//! | private `abs` helper | `vile` | data (dead) | the `stdlib.h` substitute carried verbatim; both remaining users are the `.abs()` method form, so it is dead code kept for upstream parity |
//! | -- (new extraction; no named C counterpart) | `dtmc::spawn_fly_pick` | dtmc (extracted) | the 11-threshold weighted monster table of `A_SpawnFly` (`p_enemy.c:1946-1966`); the draw stays at the call site; baseline vectors landed pre-move (commit `7d60051`) |
//! | -- (new helper; no named C counterpart) | `map_events::is_mobj_thinker` | glue (pointer identity) | the `acp1 == P_MobjThinker` compare single-sourced from the 4 upstream sites (Keen/skull-cap/boss/brain scans); not dtmc (pointer identity, not demo arithmetic) |
//! | file-local type aliases (`angle_t`/`statenum_t`/`mobjtype_t`/`mobjinfo_t`/`dirtype_t`/`size_t`/`c_short`/`CffiMobj`/`PLineThing`) | `consts` / `map_events` | data | carried verbatim (`pub(super)`); do not "modernize" the C-shaped casts |
//!
//! ## Deterministic Aspects
//!
//! `p_enemy` is the engine's heaviest `P_Random` consumer; the
//! demo surface is the DRAW COUNT AND ORDER per action, so every
//! exported function is adjudicated `dtmc (whole-body)` with nothing
//! extracted except the pure `spawn_fly_pick` table. Load-bearing
//! pieces:
//!
//! - **RNG ledger** (draw families; every draw keeps its exact
//!   statement position in its body, and the F9 goldens pin the
//!   stream): AI-core -- `try_walk`'s `& 15` movecount init,
//!   `new_chase_dir`'s `> 200` swap coin and `& 1` sweep coin,
//!   `check_missile_range`'s terminal `< dist`, `action_look`'s
//!   seesound `% 3`/`% 2`, `action_chase`'s activesound `< 3`;
//!   attacks -- `action_face_target`'s `<< 21` shadow pair, the
//!   hitscan `(P_Random() - P_Random()) << 20` pairs with
//!   `(rnd%5+1)*3`-family damage draws, `action_cpos_refire`/`spid`'
//!   `< 40`/`< 10` keep-firing draws, `action_skel_fist`/`sarg`/
//!   `troop`/`head`/`bruis` melee damage draws; revenant --
//!   `action_tracer`'s smoke tic jitter (`& 3`) behind the
//!   `gametic & 3` gate; vile -- no draws in the fire pillar (pure
//!   positioning), the resurrection is draw-free; death --
//!   `action_scream`'s `% 3`/`% 2` variant draws; brain --
//!   `action_brain_scream`/`brain_explode`'s z/momz/jitter draws,
//!   `action_spawn_fly`'s single pick draw. No draw was hoisted into
//!   a helper; the extraction takes the already-drawn byte.
//! - **spechit drain** (`chase::move_step`, catalog entry 1 + G2
//!   KEEP): the decrement-then-skip drain order over `spechit[]` is
//!   the emulation surface itself; the `?  Entries beyond the array`
//!   guard and the `MAXSPECIALCROSS` bound are pinned by the catalog.
//! - **boss-death matrix** (`map_events::check_boss_end`): the pure
//!   pre/post-ultimate episode x map x boss-type table decides which
//!   666/667 special (and whether the level exits) on any boss kill;
//!   the full matrix is baseline-pinned in `map_events::tests`.
//! - **brain target store** (`brain::action_brain_awake`, catalog G1
//!   row): the `braintargets[32]` store stays UNGUARDED
//!   (chocolate/crispy parity, KEEP) -- never "fix" the bound.
//! - **cube alternation** (`brain::action_brain_spit`): the
//!   function-local `static mut easy` coin skips every other spit on
//!   easy skill; the static stays function-local (upstream shape).
//! - **statement-order surfaces**: `action_spawn_fly`'s
//!   fog-look-telefrag-remove sequence and `action_vile_chase`'s
//!   raise sequence are order-pinned wholes (no extraction possible
//!   without forking the sequence).

pub mod attacks;
pub mod brain;
pub mod chase;
pub mod consts;
pub mod death;
pub mod dtmc;
pub mod mancubus;
pub mod map_events;
pub mod noise;
pub mod revenant;
pub mod souls;
pub mod ssg;
pub mod vile;
pub mod vile_fire;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_enemy::P_*` / `A_*` / `PIT_*`). The C
//* symbol each shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. The 54 info.rs
//* extern-block pins are LOAD-BEARING (link-by-symbol); the rest ride
//* the name = symbol = pin conservatism rule. Shims die with the
//* freeze zone.
pub use chase::{
    check_melee_range as P_CheckMeleeRange, check_missile_range as P_CheckMissileRange,
    look_for_players as P_LookForPlayers, move_step as P_Move, new_chase_dir as P_NewChaseDir,
    try_walk as P_TryWalk,
};
pub use noise::{noise_alert as P_NoiseAlert, recursive_sound as P_RecursiveSound};

//* path-stability re-export: the A_* action surface keeps its
//* module-root paths (info.rs's states table resolves the symbols; the
//* Rust-side paths stay stable for freeze-zone readers and the
//* p_pspr consumer).
pub use attacks::{
    action_bruis_attack as A_BruisAttack, action_bspi_attack as A_BspiAttack,
    action_cpos_attack as A_CPosAttack, action_cpos_refire as A_CPosRefire,
    action_cyber_attack as A_CyberAttack, action_face_target as A_FaceTarget,
    action_head_attack as A_HeadAttack, action_pos_attack as A_PosAttack,
    action_sarg_attack as A_SargAttack, action_spos_attack as A_SPosAttack,
    action_spid_refire as A_SpidRefire, action_troop_attack as A_TroopAttack,
};
pub use brain::{
    action_brain_awake as A_BrainAwake, action_brain_die as A_BrainDie,
    action_brain_explode as A_BrainExplode, action_brain_pain as A_BrainPain,
    action_brain_scream as A_BrainScream, action_brain_spit as A_BrainSpit,
    action_spawn_fly as A_SpawnFly, action_spawn_sound as A_SpawnSound,
};
pub use death::{
    action_baby_metal as A_BabyMetal, action_fall as A_Fall, action_hoof as A_Hoof,
    action_metal as A_Metal, action_pain as A_Pain, action_player_scream as A_PlayerScream,
    action_scream as A_Scream, action_x_scream as A_XScream,
};
pub use mancubus::{
    action_fat_attack1 as A_FatAttack1, action_fat_attack2 as A_FatAttack2,
    action_fat_attack3 as A_FatAttack3, action_fat_raise as A_FatRaise,
};
pub use map_events::{
    action_boss_death as A_BossDeath, action_explode as A_Explode,
    action_keen_die as A_KeenDie,
};
pub use revenant::{
    action_skel_fist as A_SkelFist, action_skel_missile as A_SkelMissile,
    action_skel_whoosh as A_SkelWhoosh, action_tracer as A_Tracer,
};
pub use souls::{
    action_pain_attack as A_PainAttack, action_pain_die as A_PainDie,
    action_pain_shoot_skull as A_PainShootSkull, action_skull_attack as A_SkullAttack,
};
pub use ssg::{
    action_close_shotgun2 as A_CloseShotgun2, action_load_shotgun2 as A_LoadShotgun2,
    action_open_shotgun2 as A_OpenShotgun2,
};
pub use vile::action_vile_chase as A_VileChase;
pub use vile_fire::{
    action_fire as A_Fire, action_fire_crackle as A_FireCrackle, action_start_fire as A_StartFire,
    action_vile_attack as A_VileAttack, action_vile_start as A_VileStart,
    action_vile_target as A_VileTarget,
};
pub use vile::pit_vile_check as PIT_VileCheck;

//* path-stability re-export: the AI statics keep their module-root
//* paths (data tier, upstream names + #[no_mangle] retained; the
//* c_ffi extern block resolves five of them by symbol).
pub use brain::{braintargeton, braintargets, numbraintargets};
pub use chase::{diags, opposite, xspeed, yspeed};
pub use noise::soundtarget;
pub use revenant::TRACEANGLE;
pub use vile::{corpsehit, vileobj, viletryx, viletryy};
