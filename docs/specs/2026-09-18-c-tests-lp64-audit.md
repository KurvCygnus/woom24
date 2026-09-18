# Audit: `c_long` / `c_ulong` occurrences and wasm determinism

**Date:** 2026-09-18
**Status:** Complete — satisfies decision **D3** of `2026-09-17-c-tests-lp64-design.md`.
**Note on path:** D3's text names the artifact `2026-09-17-c-tests-lp64-audit.md`; the
implementation plan fixes the path as this file (`2026-09-18-…`). Policy-doc sync (design-spec
step 3) should reconcile the filename when citing it.

## Conclusion (up front)

**All 28 occurrences of `c_long`/`c_ulong` in the audited trees are CRT-adjacent; zero are
simulation-facing.** The only `c_long` *values* in engine code are file lengths and stream
positions (`M_FileLength`, `ftell`) plus the `SAVEGAMESIZE` byte cap — every value originates
from or terminates at `fopen`/`fseek`/`ftell`/`fclose`. Simulation state is fixed-width
(`c_int`/`u32`/`fixed_t` = `c_int`); the RNG (`m_random.rs`) and the demo record/playback paths
(`g_game.rs` demo buffers, byte-wise `lmp` I/O) contain no `c_long` at all. **Demo determinism
is therefore data-model independent**: the wasm32 data model (`long` = 4 bytes, defined by our
own `shells/web` CRT shim) cannot change any observable simulation behavior.

This matches D3's expected conclusion, so no escalation is required.

## Method

Exhaustive (no sampling) grep over the five engine/platform trees:

```bash
grep -rn "c_long\|c_ulong" room/src/doom/ room/src/audio/ room/src/types/ shells/native/src/ shells/web/src/
```

Hit counts per tree:

| Tree | Hits |
|---|---|
| `room/src/doom/` | 22 |
| `room/src/audio/` | 0 |
| `room/src/types/` | 0 |
| `shells/native/src/` | 0 |
| `shells/web/src/` | 6 |
| **Total** | **28** |

Occurrences found outside the audited trees are listed under "Supplementary occurrences" for
completeness; they are either comments or the wasm CRT declaration layer itself.

## Classification scheme (from design spec D3)

- **CRT-adjacent** — the type crosses the platform I/O boundary (fseek/ftell offsets, file
  lengths, libc shims). Values originate from or terminate at the VFS/CRT layer; wasm's 4-byte
  `long` is what the wasm CRT shim itself defines, so behavior is self-consistent.
- **Simulation-facing** — the value reaches game state, RNG, physics, or demo playback.
  Any hit here means STOP and escalate to the user (no autonomous fix).

## Results

Every occurrence, in file/line order. `K` = kind (declaration / use).

### `room/src/doom/g_game.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `g_game.rs:74` | decl | CRT-adjacent | Type alias introduced solely to type the savegame size cap; its only consumer is the `vanilla_savegame_limit` check at `g_game.rs:2282`. |
| `g_game.rs:189` | decl | CRT-adjacent | `SAVEGAMESIZE = 0x2c000` is a byte-count cap compared against `ftell(save_stream)` to raise `I_Error("Savegame buffer overrun")`; it never enters game state and the constant is representable at any `long` width. |

### `room/src/doom/m_argv.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `m_argv.rs:11` | decl | CRT-adjacent | Import of the CRT-boundary type, used only by this file's response-file loader. |
| `m_argv.rs:115` | use | CRT-adjacent | `size` is the response-file byte length from `M_FileLength` (fseek/ftell round-trip), consumed only for `malloc` sizing and NUL-termination of the file buffer. |
| `m_argv.rs:150` | use | CRT-adjacent | `(k as c_long) < size` is a scan bound over response-file text, mirroring upstream C's `long` loop index; the value terminates at argv string pointers. |
| `m_argv.rs:151` | use | CRT-adjacent | Same scan bound (whitespace skip) over the CRT file buffer. |
| `m_argv.rs:154` | use | CRT-adjacent | Same scan bound (loop exit check) over the CRT file buffer. |
| `m_argv.rs:162` | use | CRT-adjacent | Same scan bound (quoted-argument scan) over the CRT file buffer. |
| `m_argv.rs:168` | use | CRT-adjacent | Same scan bound (unclosed-quote check) over the CRT file buffer. |
| `m_argv.rs:179` | use | CRT-adjacent | Same scan bound (argument-end scan) over the CRT file buffer. |

### `room/src/doom/m_misc.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `m_misc.rs:16` | decl | CRT-adjacent | Import of the CRT-boundary type for the stdio `extern "C"` block below. |
| `m_misc.rs:50` | decl | CRT-adjacent | Declaration of libc `fseek` — `offset: c_long` is libc's own signature at the platform I/O boundary. |
| `m_misc.rs:52` | decl | CRT-adjacent | Declaration of libc `ftell` returning `c_long`, verbatim libc semantics. |
| `m_misc.rs:131` | use | CRT-adjacent | `M_FileLength` is an fseek/ftell round-trip returning a file length — a pure CRT quantity. |

### `room/src/doom/p_saveg.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `p_saveg.rs:121` | decl | CRT-adjacent | Import used only by the two stream-alignment padding helpers. |
| `p_saveg.rs:199` | use | CRT-adjacent | `saveg_read_pad` reduces a `ftell` position via `pos & 3` to a 0..3 padding count — width-invariant arithmetic on a CRT stream position; all savegame fields themselves travel through byte-explicit `saveg_read8/32` (`u32`), never `c_long`. |
| `p_saveg.rs:211` | use | CRT-adjacent | `saveg_write_pad` is the identical reduction on the write path. |

### `room/src/doom/w_file.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `w_file.rs:15` | decl | CRT-adjacent | Import of the CRT-boundary type for the stdio WAD backend. |
| `w_file.rs:133` | use | CRT-adjacent | WAD lump read offset handed to CRT `fseek`; `c_uint → c_long` zero-extends on LP64 and is identity on wasm32, so observable reads are identical on both data models. |

### `room/src/doom/c_tests/harness.rs` (test-only, `#[cfg(all(test, unix))]`)

| Location | K | Classification | Justification |
|---|---|---|---|
| `harness.rs:5` | decl | CRT-adjacent | Import for the test-only type-size assertions. |
| `harness.rs:17` | decl | CRT-adjacent | `size_of::<c_long>() == 8` is the LP64 pin asserting the differential oracle's data model — compiled only under the unix-gated test module, never on wasm or in the shipping cdylib. |
| `harness.rs:18` | decl | CRT-adjacent | The same LP64 pin for `c_ulong`. |

### `shells/web/src/wasm_vfs.rs`

| Location | K | Classification | Justification |
|---|---|---|---|
| `wasm_vfs.rs:24` | decl | CRT-adjacent | Import typing the wasm CRT shim signatures. |
| `wasm_vfs.rs:609` | use | CRT-adjacent | The wasm `fseek` shim implements the CRT seek over the in-memory VFS — this is where wasm's own `long` (4 bytes) is defined, so every engine-side `c_long` use binds to a self-consistent definition. |
| `wasm_vfs.rs:614` | use | CRT-adjacent | Descriptor length used in seek arithmetic inside the shim; never leaves the CRT layer. |
| `wasm_vfs.rs:617` | use | CRT-adjacent | `SEEK_CUR` stream-position arithmetic inside the shim. |
| `wasm_vfs.rs:631` | use | CRT-adjacent | The wasm `ftell` shim returning the stream position per CRT semantics. |
| `wasm_vfs.rs:636` | use | CRT-adjacent | Position cast inside the shim; its only consumers are the engine's fseek/ftell/`M_FileLength` plumbing. |

### Supplementary occurrences (outside the audited trees)

| Location | Classification | Justification |
|---|---|---|
| `room/src/doom.rs:104` | n/a | Comment text only (c_tests gating comment citing `c_tests/harness.rs`); no type use. |
| `shells/web/crt/src/lib.rs:12` | CRT-adjacent | Re-export of `std::ffi::c_long` in the wasm libc stand-in crate — the declaration layer of the CRT shim. |
| `shells/web/crt/src/lib.rs:27` | CRT-adjacent | Declaration of `fseek(offset: c_long, …)` with a libc-verbatim signature for wasm32 linking. |
| `shells/web/crt/src/lib.rs:28` | CRT-adjacent | Declaration of `ftell() -> c_long`, ditto. |

## Width-sensitivity analysis

Why no occurrence can change observable behavior under a different data model:

1. **Values are positions and lengths, never state.** Every non-declaration use is driven by
   `ftell`/`M_FileLength` results or feeds `fseek`. None of these values is stored in game
   state, fed to the RNG, or written into a demo stream. Savegame contents are serialized
   through byte-explicit little-endian `saveg_read8/32`/`saveg_write32` (`u32`) — the save
   format is width-independent by construction; `c_long` appears only in the padding
   arithmetic, and `pos & 3` yields the same 0..3 count at any width.
2. **`SAVEGAMESIZE` is width-safe.** `0x2c000` fits any `long`; the check
   `ftell(save_stream) > SAVEGAMESIZE` (`g_game.rs:2282`) compares like-typed values on every
   data model and only gates an `I_Error`, not state. On wasm the VFS `fwrite` is a documented
   no-op stub (`wasm_vfs.rs:599-601`), so savegame write paths never persist there anyway.
3. **The response-file scan mirrors upstream C exactly.** `k as c_long` casts reproduce
   upstream's `long` arithmetic; the loop bounds derive from the file length of a host-supplied
   text file and produce only argv strings — startup input, not simulation state.
4. **The wasm model is self-consistent by construction.** `shells/web/crt` declares
   `fseek`/`ftell` with `c_long`, and `shells/web/src/wasm_vfs.rs` implements those exact
   signatures, so the engine's `extern "C"` uses bind to one coherent wasm-side `long` = 4
   bytes; there is no cross-model ABI seam on wasm.
5. **Zero hits in simulation machinery.** `m_random.rs` (RNG), the thinker/physics modules, and
   the demo record/playback code contain no `c_long`; `room/src/audio/`, `room/src/types/`, and
   `shells/native/src/` have none either.

## Policy statement (testing anchors)

The C-vs-Rust differential suite stays gated `#[cfg(all(test, unix))]`
(`room/src/doom.rs:103-107`, comment citing `c_tests/harness.rs`). Its oracle is the compiled C
build under LP64 (`long` = 8 bytes, pinned by `harness.rs:17-18`); wasm has no C oracle, so
that gate is permanent per design-spec decision **D1**. Wasm determinism is anchored instead by
the golden demo tests on the host (LP64 native) — the same simulation code — which is sound
precisely because this audit shows simulation state never touches `c_long`: demo-exact
behavior is defined by the LP64 Linux reference and is reproducible bit-for-bit on wasm32's
ILP32-ish data model.
