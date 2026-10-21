//! Build metadata: the git commit for the version display (`STOMP_GIT_HASH`, empty outside a
//! git checkout, e.g. in Nix or crates.io builds) and, for Windows targets, the exe icon and
//! version resource.

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

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../packaging/icons/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icons/icon.ico")
            .set("ProductName", "Open Stomp Manager")
            .set("FileDescription", "Open Stomp Manager")
            .set("CompanyName", "")
            .set("LegalCopyright", "MIT OR Apache-2.0");
        // a missing resource compiler (e.g. cross builds without windres) only costs the icon
        if let Err(e) = res.compile() {
            println!("cargo:warning=Windows icon/version resource not embedded: {e}");
        }
    }
}
