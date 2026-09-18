//! Initialization pipeline shared by both entries (D5 / AGENTS.md entry
//! contract).
//!
//! Profile → argv (built on the Rust side, no JS argv) → factory install →
//! doomgeneric_Create. The engine keeps `myargv` for the whole process, so the
//! `CString`s and the pointer array must live just as long: both are leaked
//! for the process lifetime (never freed), matching native main.rs's
//! never-freed `App.args` contract.

use std::cell::Cell;
use std::ffi::{c_char, c_int, CString};

use room::audio::AudioBackend;

use crate::profile::BootProfile;
use crate::wasm_vfs;
use crate::web_audio;

thread_local! {
    /// One-shot boot latch (entry contract: the engine is created at most once
    /// per process). Armed only when a start attempt actually reaches the
    /// engine, so a failed validation (e.g. unregistered IWAD) can be retried.
    static CREATED: Cell<bool> = const { Cell::new(false) };
}

/// Build the engine argv from a boot profile.
/// Convention: argv[0] = "woom24"; -iwad <name>; each PWAD via -file <name>;
/// SF2 never enters argv (i_sound's exists() is always false on wasm, see Task 4).
/// Every entry is a single argv token: hosts must pre-split `engine_args`
/// (see `BootProfile` docs). Profile values are JS-reachable input, so an
/// interior NUL maps to Err -- never an unwrap panic (AGENTS: input-reachable
/// code must not panic).
fn build_argv(profile: &BootProfile) -> Result<Vec<CString>, String> {
    fn cstr(s: &str) -> Result<CString, String> {
        CString::new(s).map_err(|_| format!("profile value contains NUL byte: {s:?}"))
    }
    let mut v = vec![cstr("woom24")?];
    v.push(cstr("-iwad")?);
    v.push(cstr(&profile.iwad)?);
    for pwad in &profile.pwads {
        v.push(cstr("-file")?);
        v.push(cstr(pwad)?);
    }
    for a in &profile.engine_args {
        // Engine args pass through verbatim (host profile is trusted-boundary input).
        v.push(cstr(a)?);
    }
    Ok(v)
}

/// Builds the `char *argv[]` array over `strings` (C convention:
/// `argv[argc] = NULL`) and leaks both payloads for the whole process.
/// Returns `(argc, argv)` for `doomgeneric_Create`.
// The engine stores the raw pointers (`m_argv.rs` `myargv`) beyond this call,
// so the strings and the array itself must never be freed (post-boot readers
// dereference the array, e.g. I_Error's `M_ParmExists("-nogui")` scan) --
// here the leak IS the ownership contract. `anchor_argv` runs at most once
// per process (the CREATED latch refuses any later pipeline run).
fn anchor_argv(strings: Vec<CString>) -> (c_int, *mut *mut c_char) {
    let strings: &'static [CString] = Vec::leak(strings);
    let mut array: Vec<*mut c_char> = strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    array.push(std::ptr::null_mut());
    let argc = (array.len() - 1) as c_int;
    let argv = Box::leak(array.into_boxed_slice()).as_mut_ptr();
    (argc, argv)
}

/// Pipeline body: both entries converge here; no branching outside the export surface.
pub fn run(profile: &BootProfile) -> Result<(), String> {
    // 0. Double-start guard: a second start call after a boot is a no-op --
    //    re-running the pipeline would overwrite myargv/ARG_STORAGE and leak
    //    the engine's screen buffer. The check precedes validation on purpose:
    //    an already-booted engine must never be re-entered, whatever the
    //    profile says. The refusal is reported on the console (not as Err), so
    //    a host that ignores it keeps a running game instead of a dead one.
    if CREATED.with(Cell::get) {
        report_double_start();
        return Ok(());
    }
    // 1. Every WAD name in the profile must already sit in the VFS (minimal
    //    entry just registered the IWAD; standard entry relies on the host
    //    pre-registering the full set). Validated before any engine contact:
    //    an unregistered name would reach D_FindWADByName's fopen miss, whose
    //    errno check panics on wasm (no CRT errno).
    if wasm_vfs::vfs_get(&profile.iwad).is_none() {
        return Err(format!(
            "IWAD '{}' 未注册：先经 woom24_register_file 注册",
            profile.iwad
        ));
    }
    for pwad in &profile.pwads {
        if wasm_vfs::vfs_get(pwad).is_none() {
            return Err(format!(
                "PWAD '{}' 未注册：先经 woom24_register_file 注册",
                pwad
            ));
        }
    }
    // 2. Validate argv before any side effect, so a malformed profile fails
    //    without installing factories or reaching the engine.
    let args = build_argv(profile)?;
    // 3. Framebuffer cap (D5): the engine-side cap belongs to the upcoming
    //    custom-resolution work; for this spec it is recorded through the
    //    presenter's canvas clamp (Tasks 5/6 landed at 640×400).
    if let Some(res) = profile.max_render_res {
        log::info!("宿主最大渲染分辨率: {res}px（引擎侧 cap 待自定义分辨率工作）");
    }
    // 4. Audio: remember the SF2 name (the backend preloads it from the VFS at
    //    construction), then install the factory.
    web_audio::set_pending_sf2(profile.sf2.clone());
    room::audio::set_backend_factory(|| {
        web_audio::WebAudioBackend::new().map(|b| Box::new(b) as Box<dyn AudioBackend>)
    });
    // 5. argv → doomgeneric_Create (D_DoomMain returns from here; ticking is
    //    driven by woom24_tick). Both the strings and the pointer array are
    //    leaked for the process lifetime: the engine keeps `myargv` forever
    //    (post-boot readers include I_Error's `-nogui` scan).
    let (argc, argv) = anchor_argv(args);
    // SAFETY: argv[0..argc] point at NUL-terminated C strings that, together
    // with the array itself, live for the whole process (leaked by
    // anchor_argv); the CREATED latch (armed below, before any engine contact)
    // guarantees doomgeneric_Create runs at most once per process (entry
    // contract guarantee).
    CREATED.with(|c| c.set(true));
    unsafe {
        room::doom::doomgeneric::doomgeneric_Create(argc, argv);
    }
    Ok(())
}

/// Second start call after a boot: report and refuse. wasm-bindgen imported
/// functions trap on non-wasm targets, so the host path (tests, native shell)
/// goes through `log` instead.
fn report_double_start() {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
        "woom24: engine already started; second start call ignored (reload the page to boot again)",
    ));
    #[cfg(not(target_arch = "wasm32"))]
    log::error!("woom24: engine already started; second start call ignored");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_names(args: &[CString]) -> Vec<String> {
        args.iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn build_argv_orders_iwad_pwads_engine_args() {
        let p = BootProfile {
            iwad: "doom.wad".to_string(),
            pwads: vec!["a.wad".to_string(), "b.wad".to_string()],
            // SF2 goes to set_pending_sf2 only, never into argv.
            sf2: Some("sc55.sf2".to_string()),
            max_render_res: Some(1080),
            engine_args: vec!["-nomusic".to_string(), "-turbo 2".to_string()],
        };
        assert_eq!(
            argv_names(&build_argv(&p).unwrap()),
            vec![
                "woom24", "-iwad", "doom.wad", "-file", "a.wad", "-file", "b.wad", "-nomusic",
                "-turbo 2",
            ]
        );
    }

    #[test]
    fn build_argv_keeps_engine_args_unsplit() {
        // Args containing spaces pass through verbatim (no M_-style splitting here).
        let p = BootProfile {
            iwad: "d.wad".to_string(),
            pwads: Vec::new(),
            sf2: None,
            max_render_res: None,
            engine_args: vec!["-warp 1 3".to_string()],
        };
        let v = argv_names(&build_argv(&p).unwrap());
        assert_eq!(*v.last().unwrap(), "-warp 1 3");
        assert_eq!(v[0], "woom24");
    }

    #[test]
    fn build_argv_rejects_interior_nul_as_err() {
        // Belt-and-braces for fix round 1: even if a NUL slipped past the profile
        // parser (e.g. a profile built directly in Rust), argv construction must
        // return Err -- never panic on CString::new.
        let p = BootProfile {
            iwad: "doom.wad".to_string(),
            pwads: vec!["pw\u{0}ad.wad".to_string()],
            sf2: None,
            max_render_res: None,
            engine_args: Vec::new(),
        };
        let err = build_argv(&p).unwrap_err();
        assert!(err.contains("NUL"), "unexpected error: {err}");
    }

    #[test]
    fn run_rejects_unregistered_iwad_before_engine_contact() {
        // The test thread's VFS is empty: the pipeline must fail before touching
        // the audio factory or the engine.
        let p = BootProfile {
            iwad: "missing.wad".to_string(),
            pwads: Vec::new(),
            sf2: None,
            max_render_res: None,
            engine_args: Vec::new(),
        };
        let err = run(&p).unwrap_err();
        assert!(
            err.contains("missing.wad"),
            "error should name the IWAD: {err}"
        );
    }

    #[test]
    fn anchor_argv_keeps_array_null_terminated_and_stored() {
        let (argc, argv) = anchor_argv(vec![
            CString::new("woom24").unwrap(),
            CString::new("-iwad").unwrap(),
            CString::new("doom.wad").unwrap(),
        ]);
        assert_eq!(argc, 3);
        // SAFETY: the payloads are leaked for the whole process (exactly the
        // property this test pins down), so the array is valid to read here.
        let slots = unsafe { std::slice::from_raw_parts(argv, argc as usize + 1) };
        assert!(slots[..argc as usize].iter().all(|p| !p.is_null()));
        assert!(
            slots[argc as usize].is_null(),
            "argv[argc] must be NULL (C convention)"
        );
    }

    #[test]
    fn run_rejects_unregistered_pwad_before_engine_contact() {
        // Only the IWAD is registered: an unregistered PWAD name must fail
        // validation like an unregistered IWAD, before any engine contact.
        // Left unguarded it reaches D_FindWADByName's fopen miss, whose errno
        // check panics on wasm (no CRT).
        wasm_vfs::vfs_register("doom.wad", b"registered iwad bytes".to_vec());
        let p = BootProfile {
            iwad: "doom.wad".to_string(),
            pwads: vec!["missing.wad".to_string()],
            sf2: None,
            max_render_res: None,
            engine_args: Vec::new(),
        };
        let err = run(&p).unwrap_err();
        assert!(
            err.contains("missing.wad"),
            "error should name the PWAD: {err}"
        );
    }

    #[test]
    fn run_refuses_second_start_after_engine_created() {
        // Simulate a completed boot by arming the latch directly (the engine
        // itself cannot boot on host: DG_Init needs a DOM canvas). The guard
        // must fire BEFORE validation: with the latch armed, even this
        // unregistered-IWAD profile has to come back Ok(()) -- an Err here
        // would mean the second call reached the pipeline body again.
        CREATED.with(|c| c.set(true));
        let p = BootProfile {
            iwad: "missing.wad".to_string(),
            pwads: Vec::new(),
            sf2: None,
            max_render_res: None,
            engine_args: Vec::new(),
        };
        assert!(
            run(&p).is_ok(),
            "second start after a boot must be a no-op Ok"
        );
        assert!(CREATED.with(Cell::get), "the latch must stay armed");
    }

    #[test]
    fn failed_start_does_not_arm_the_latch() {
        // A start that fails validation has not created the engine: retrying
        // (e.g. the host fixed the profile) must be validated again, never
        // silently swallowed by the guard.
        let p = BootProfile {
            iwad: "missing.wad".to_string(),
            pwads: Vec::new(),
            sf2: None,
            max_render_res: None,
            engine_args: Vec::new(),
        };
        assert!(run(&p).is_err(), "first (failing) attempt must Err");
        assert!(
            run(&p).is_err(),
            "a failed start must not arm the latch; the retry must be validated again"
        );
        assert!(
            !CREATED.with(Cell::get),
            "no engine was created, latch stays off"
        );
    }
}
