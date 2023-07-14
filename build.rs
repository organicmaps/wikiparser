use std::process::Command;

/// Pass git-describe through CARGO_GIT_VERSION env variable
///
/// NOTE: Cargo.toml still needs to be updated on releases
fn set_version_from_git() {
    let cmd = Command::new("git")
        .arg("describe")
        .arg("--always")
        .arg("--dirty")
        .arg("--tags")
        .output();

    match cmd {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            let version = version.trim();
            println!("cargo:rustc-env=CARGO_GIT_VERSION={}", version);
            // rerun when git checks out another ref or any ref changes
            println!("cargo:rerun-if-changed=.git/refs/");
            println!("cargo:rerun-if-changed=.git/HEAD");
        }
        _ => {
            // crates.io builds without git, so ignore here
            eprintln!("git describe failed; ignoring");
        }
    }
}

fn main() {
    set_version_from_git();
}
