#[cfg(all(unix, not(target_os = "macos")))]
fn main() {
    // add unix dependencies below
    // println!("cargo:rustc-flags=-l readline");
}

#[cfg(target_os = "macos")]
fn main() {
    // add macos dependencies below
    // println!("cargo:rustc-flags=-l edit");
}

// The crate is a nightly-only behavioural reference and is excluded from the
// default workspace build, but the build script itself must compile on every
// target (IntelliJ/RustRover loads the whole workspace, Windows included).
#[cfg(windows)]
fn main() {
    // add Windows dependencies below
}
