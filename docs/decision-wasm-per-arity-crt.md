# Decision Record: Per-Arity CRT Symbols on wasm32 (printf family)

- **Date:** 2026-09-18
- **Status:** DECIDED
- **Scope:** `room/src/doom/` FFI declarations touching variadic C functions;
  `shells/web/crt` (woom24-libc); `shells/web/src/wasm_vfs.rs`
- **Spec:** 2026-09-17 wasm shell (spec 2), D1 / plan appendix A

## The trap mechanism (wasm-lld `signature_mismatch`)

On `wasm32-unknown-unknown`, `rust-lld` validates every call against the
 callee's signature **strictly**. A C variadic declaration
(`extern "C" { fn printf(fmt: *const c_char, ...) -> c_int; }`) does not
produce one signature: rustc emits a **different wasm signature per call-site
argument count** (fmt only; fmt + 1 arg; fmt + 2 args; ...). When the link is
resolved against a single fixed-signature implementation, lld does not fail
the build -- it **warns at link time** and replaces the mismatching call with
a `signature_mismatch` **trap stub**, so the module links, validates, and
traps at runtime on the first call. Probe-verified 2026-09-17: link warns,
running the module traps. The failure is therefore invisible to a successful
link check and only surfaces when the engine's first `printf` fires (which,
via `D_DoomMain`'s boot banner, is immediately on every boot).

## The scheme: one symbol per audited call shape

The printf family is declared in fixed-arity shapes covering exactly the call
shapes the engine uses (audited; plan appendix A):

| Family | Shapes | Declaration | Implementation | Rust-side entry |
|---|---|---|---|---|
| `printf` | 0..4 variadic args (`printf0`..`printf4`) | `shells/web/crt/src/lib.rs` | `shells/web/src/wasm_vfs.rs` (`#[no_mangle]` exports) | `room/src/doom/crt.rs` `c_printf`..`c_printf4` |
| `snprintf` | 1..2 conversions (`snprintf1`/`snprintf2`) | same | same | `c_snprintf1`/`c_snprintf2` |
| `sscanf` | single conversion (`sscanf1`, `M_StrToInt`'s only shape) | same | same | `c_sscanf1` |

Routing: every `room/src/doom/` call site goes through the `crt.rs` wrapper
functions, which are `cfg`-split -- host targets keep the real variadic
`libc::printf`/`libc::snprintf`/`libc::sscanf` (natural argument types
preserved); `wasm32` binds the per-arity `woom24-libc` declarations. Unused
argument slots are never read (a specifier absent from the format string is
never fetched by the shim's formatter).

## The rule

**Any future variadic `extern "C"` declaration in `room/src/doom/` must be
routed through `room/src/doom/crt.rs` the same way**: declare fixed-arity
shapes in `woom24-libc`, export same-name implementations from the web shell's
CRT shim, and add a `cfg`-split wrapper in `crt.rs` that every call site uses.
Declaring a variadic function directly (the old `I_Error` pattern) **silently
recreates the trap**: the pinned wasm check and the release link both succeed,
and the first runtime call traps the page. Code review for any new
`room/src/doom/` extern must check for `...` in the declaration.

## Related decisions

- `mkdir` degrades to `-1` on wasm instead of panicking (2026-09-18 final
  review): the engine's only call (`M_MakeDirectory(".")`) is a no-op on every
  host, and the web VFS is immutable after registration.
- `errno` has no wasm route: paths that reach `errno()` on a failed `fopen`
  must be guarded before engine contact (the shell validates every profile
  WAD name against the VFS first).
