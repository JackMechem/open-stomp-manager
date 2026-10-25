//! Tauri's build step (icons, Windows resources, permissions), plus build metadata: the git commit for
//! the version display (`STOMP_GIT_HASH`, empty outside a git checkout, e.g. in Nix builds) and the
//! open-stomp library version from `Cargo.lock` (`STOMP_LIB_VERSION`).

use std::process::Command;

fn main() {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let hash = git(&["rev-parse", "--short=8", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=STOMP_GIT_HASH={hash}");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/refs/heads");
    }

    // the library is a path dependency, so its version is whatever Cargo.lock resolved
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    let lib = std::fs::read_to_string("../../Cargo.lock")
        .ok()
        .and_then(|lock| {
            let mut lines = lock.lines().skip_while(|l| *l != r#"name = "open-stomp""#).skip(1);
            lines.next()?.strip_prefix("version = \"")?.strip_suffix('"').map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=STOMP_LIB_VERSION={lib}");

    tauri_build::build()
}
