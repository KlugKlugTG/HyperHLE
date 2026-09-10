/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::path::{Path, PathBuf};
use std::process::Command;

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

pub fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = package_root.join("../..");

    // Build the version string shown in the UI (app picker label, window
    // title, etc). Always include the short commit hash so the exact build
    // is identifiable, e.g. "v1.0.35 (a1b2c3d)".
    let toml_version = std::env::var("CARGO_PKG_VERSION").unwrap();

    rerun_if_changed(&workspace_root.join(".git/HEAD"));
    rerun_if_changed(&workspace_root.join(".git/refs"));
    rerun_if_changed(&workspace_root.join("Cargo.toml"));

    let git_output = |args: &[&str]| -> Option<String> {
        let output = Command::new("git").args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        Some(
            std::str::from_utf8(&output.stdout)
                .ok()?
                .trim_end()
                .to_string(),
        )
    };

    let commit_hash = git_output(&["rev-parse", "--short=7", "HEAD"]);
    let dirty = matches!(
        git_output(&["status", "--porcelain"]),
        Some(ref s) if !s.is_empty()
    );

    //  Sanity check: warn if the Cargo.toml version doesn't match the latest
    // tag.
    if let Some(tag) = git_output(&["describe", "--tags", "--abbrev=0"]) {
        if tag
            .strip_prefix('v')
            .is_some_and(|v| !v.starts_with(&toml_version))
        {
            println!("cargo:warning=Cargo.toml version (v{toml_version}) is not a prefix of latest tag ({tag})!");
        }
    }

    let version = match &commit_hash {
        Some(hash) => {
            let mut v = format!("v{toml_version} ({hash}");
            if dirty {
                v.push_str("-dirty");
            }
            v.push(')');
            v
        }
        None => {
            let mut v = format!("v{toml_version} (git rev. unknown");
            if dirty {
                v.push_str("-dirty");
            }
            v.push(')');
            v
        }
    };
    std::fs::write(out_dir.join("version.txt"), version).unwrap();
}
