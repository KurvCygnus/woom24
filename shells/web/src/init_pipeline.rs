//! 两入口共享的初始化管线 (D5 / AGENTS.md 入口契约).
//!
//! //! 档案 → argv (Rust 侧构造, 无 JS argv) → 工厂安装 → doomgeneric_Create.
//! //! 引擎会永久保存 myargv, CString 与指针数组必须活满整个进程:
//! //! 存进 thread_local, 与 native main.rs 的 App.args 同一手法.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CString};

use room::audio::AudioBackend;

use crate::profile::BootProfile;
use crate::wasm_vfs;
use crate::web_audio;

thread_local! {
    /// argv 的所有权锚点 (引擎保存 myargv 指针, 绝不可释放).
    static ARG_STORAGE: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
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

/// Pipeline body: both entries converge here; no branching outside the export surface.
pub fn run(profile: &BootProfile) -> Result<(), String> {
    // 1. The IWAD must already sit in the VFS (minimal entry just registered it;
    //    standard entry relies on the host pre-registering it).
    if wasm_vfs::vfs_get(&profile.iwad).is_none() {
        return Err(format!(
            "IWAD '{}' 未注册：先经 woom24_register_file 注册",
            profile.iwad
        ));
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
    //    driven by woom24_tick).
    let mut argv: Vec<*mut c_char> = args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut()); // C convention: argv[argc] = NULL
    let argc = (argv.len() - 1) as c_int;
    ARG_STORAGE.with_borrow_mut(|s| *s = args);
    // SAFETY: argv points at NUL-terminated strings owned by ARG_STORAGE and
    // lives for the whole process; doomgeneric_Create runs at most once per
    // process (entry contract guarantee).
    unsafe {
        room::doom::doomgeneric::doomgeneric_Create(argc, argv.as_mut_ptr());
    }
    Ok(())
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
            pwads: vec![format!("pw\u{0}ad.wad")],
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
}
