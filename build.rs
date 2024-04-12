use std::{path::PathBuf, process::Command};

fn main() {
    // include_dir requires the directory to exist
    std::fs::create_dir_all(
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("web")
            .join("dist"),
    )
    .ok();

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .expect("error getting git hash. Does `git rev-parse --short HEAD` work for you?");
    let git_hash = String::from_utf8(output.stdout)
        .expect("Error passing output of `git rev-parse --short HEAD`");
    println!("cargo:rustc-env=GIT_HASH={git_hash}");
}
