//! Keep Cargo's build directory from silently growing until builds slow down.
//!
//! Cargo never deletes superseded artifacts. Every feature set, profile and
//! source change leaves dependency objects and incremental sessions behind; on
//! 2026-10-04 `target/debug` held 80 GiB, and a sibling project's stale objects
//! had pushed an ordinary rebuild from about 16 s to over six minutes. `verify`
//! measures the Cargo profile directories (those holding `.fingerprint`) and,
//! above the limit, removes artifacts unused for a week with `cargo sweep`.
//! Evidence written under `target/` by other tools is never measured or touched.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};

/// Above this many GiB of Cargo artifacts, `verify` sweeps stale ones.
const DEFAULT_LIMIT_GIB: u64 = 40;
const LIMIT_VARIABLE: &str = "BEASTIE_TARGET_LIMIT_GIB";
const STALE_DAYS: &str = "7";
const GIB: u64 = 1 << 30;

/// Measure Cargo artifacts and sweep stale ones when they exceed the limit.
/// Reports what it did; never fails the gate for disk usage alone.
pub fn maintain(workspace: &Path) -> Result<()> {
    let limit = std::env::var(LIMIT_VARIABLE)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_LIMIT_GIB);
    let target = workspace.join("target");
    let before = artifact_bytes(&target)?;
    if before <= limit * GIB {
        return Ok(());
    }
    println!(
        "target hygiene: {} of Cargo artifacts exceeds {limit} GiB; removing artifacts unused for {STALE_DAYS} days",
        gib(before)
    );
    let swept = Command::new("cargo")
        .args(["sweep", "--time", STALE_DAYS])
        .current_dir(workspace)
        .status();
    match swept {
        Ok(status) if status.success() => {
            let after = artifact_bytes(&target)?;
            println!("target hygiene: {} -> {}", gib(before), gib(after));
            if after > limit * GIB {
                println!(
                    "target hygiene: still above {limit} GiB after sweeping; run `cargo clean` when convenient"
                );
            }
        }
        _ => println!(
            "target hygiene: `cargo sweep` is unavailable (install cargo-sweep, listed in the dotfiles Brewfile) or failed; run `cargo clean` to restore build speed"
        ),
    }
    Ok(())
}

/// Bytes in Cargo profile directories directly under `target`, including
/// cross-compilation triples one level deeper.
fn artifact_bytes(target: &Path) -> Result<u64> {
    let mut total = 0;
    for directory in profile_directories(target)? {
        total += tree_bytes(&directory)?;
    }
    Ok(total)
}

fn profile_directories(target: &Path) -> Result<Vec<PathBuf>> {
    let mut profiles = Vec::new();
    let Ok(entries) = std::fs::read_dir(target) else {
        return Ok(profiles);
    };
    for entry in entries {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".fingerprint").is_dir() {
            profiles.push(path);
            continue;
        }
        // Target triples nest profiles: target/<triple>/<profile>.
        if let Ok(nested) = std::fs::read_dir(&path) {
            for entry in nested.flatten() {
                let nested = entry.path();
                if nested.join(".fingerprint").is_dir() {
                    profiles.push(nested);
                }
            }
        }
    }
    Ok(profiles)
}

fn tree_bytes(root: &Path) -> Result<u64> {
    let mut total = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("failed to read {}", directory.display()))?;
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / GIB as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_cargo_profile_directories_are_measured() -> Result<()> {
        let root = std::env::temp_dir().join(format!("beastie-hygiene-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let target = Scratch(root);
        let debug = target.path().join("debug");
        std::fs::create_dir_all(debug.join(".fingerprint"))?;
        std::fs::create_dir_all(debug.join("deps"))?;
        std::fs::write(debug.join("deps/libcore.rlib"), vec![0; 1000])?;
        let triple = target.path().join("aarch64-apple-darwin/release");
        std::fs::create_dir_all(triple.join(".fingerprint"))?;
        std::fs::write(triple.join("app"), vec![0; 500])?;
        // Evidence from other tools lives beside the profiles.
        std::fs::create_dir_all(target.path().join("feel/run"))?;
        std::fs::write(target.path().join("feel/run/capture.png"), vec![0; 4000])?;
        assert_eq!(artifact_bytes(target.path())?, 1500);
        assert_eq!(artifact_bytes(&target.path().join("missing"))?, 0);
        Ok(())
    }

    /// Removes its directory when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
