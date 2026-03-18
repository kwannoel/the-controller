use std::path::{Path, PathBuf};

/// Return the path to the controller CLI binary directory (`~/.the-controller/bin`).
pub fn controller_bin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".the-controller").join("bin"))
}

/// Return PATH with `~/.the-controller/bin` prepended, so spawned sessions
/// can invoke `controller-cli` without knowing its exact location.
pub fn path_with_controller_bin() -> Option<String> {
    let bin_dir = controller_bin_dir()?;
    let current_path = std::env::var("PATH").unwrap_or_default();
    Some(format!("{}:{}", bin_dir.display(), current_path))
}

/// Query the build date of an installed binary via `--build-date`.
/// Returns `None` if the binary doesn't exist, can't run, or doesn't support the flag.
fn installed_build_date(binary_path: &Path) -> Option<String> {
    let output = std::process::Command::new(binary_path)
        .arg("--build-date")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let date = String::from_utf8(output.stdout).ok()?;
    let trimmed = date.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Copy the `controller-cli` and `pty-broker` binaries from next to the
/// running executable into `~/.the-controller/bin/`.
///
/// Runs at app startup so both binaries are always available on a stable path.
/// Skips copying if the installed binary already has the same build date.
/// Silently skips any binary that isn't found in the source directory.
pub fn install_controller_cli() {
    let Some(bin_dir) = controller_bin_dir() else {
        tracing::warn!("could not determine home directory for controller-cli install");
        return;
    };

    let Ok(current_exe) = std::env::current_exe() else {
        tracing::error!("could not determine current executable path");
        return;
    };
    let Some(exe_dir) = current_exe.parent() else {
        tracing::error!("current executable has no parent directory");
        return;
    };

    if let Err(e) = std::fs::create_dir_all(&bin_dir) {
        tracing::warn!("could not create {}: {}", bin_dir.display(), e);
        return;
    }

    let our_build_date = env!("BUILD_DATE");
    tracing::debug!(
        build_date = our_build_date,
        "checking CLI binaries for updates"
    );

    for binary_name in &["controller-cli", "pty-broker"] {
        let source = exe_dir.join(binary_name);
        if !source.exists() {
            tracing::warn!(
                "{} not found in bundle ({}), skipping install",
                binary_name,
                exe_dir.display()
            );
            continue;
        }

        // Check if the bundle binary matches this app build
        if let Some(source_date) = installed_build_date(&source) {
            if source_date != our_build_date {
                tracing::error!(
                    binary = binary_name,
                    bundle_date = source_date,
                    app_date = our_build_date,
                    "STALE BINARY: {} in bundle was built on {} but app was built on {}. \
                     Run `cargo build --release --bin {}` and rebuild the app.",
                    binary_name,
                    source_date,
                    our_build_date,
                    binary_name,
                );
                continue;
            }
        }

        let dest = bin_dir.join(binary_name);

        // Skip copy if the installed binary has the same build date
        if dest.exists() {
            if let Some(installed_date) = installed_build_date(&dest) {
                if installed_date == our_build_date {
                    tracing::debug!(binary = binary_name, installed_date, "already up-to-date");
                    continue;
                }
                tracing::debug!(
                    binary = binary_name,
                    installed_date,
                    current_date = our_build_date,
                    "build date mismatch"
                );
            }
        }

        tracing::info!(binary = binary_name, dest = %dest.display(), "installing CLI binary");
        if let Err(e) = std::fs::copy(&source, &dest) {
            tracing::warn!(binary = binary_name, error = %e, "could not install CLI binary");
            continue;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_bin_dir_ends_with_expected_suffix() {
        if let Some(dir) = controller_bin_dir() {
            let path_str = dir.to_string_lossy();
            assert!(
                path_str.ends_with(".the-controller/bin"),
                "expected path ending with .the-controller/bin, got: {}",
                path_str
            );
        }
    }

    #[test]
    fn path_with_controller_bin_prepends_bin_dir() {
        if let Some(path) = path_with_controller_bin() {
            let bin_dir = controller_bin_dir().unwrap();
            let prefix = format!("{}:", bin_dir.display());
            assert!(
                path.starts_with(&prefix),
                "PATH should start with bin_dir, got: {}",
                path
            );
        }
    }

    #[test]
    fn path_with_controller_bin_preserves_original_path() {
        if let Some(path) = path_with_controller_bin() {
            if let Ok(original) = std::env::var("PATH") {
                assert!(
                    path.ends_with(&original),
                    "PATH should end with original PATH"
                );
            }
        }
    }

    #[test]
    fn installed_build_date_returns_none_for_missing_binary() {
        let result = installed_build_date(Path::new("/nonexistent/binary"));
        assert!(result.is_none());
    }

    #[test]
    fn build_date_env_is_set() {
        let date = env!("BUILD_DATE");
        assert!(!date.is_empty(), "BUILD_DATE should not be empty");
        // Should look like YYYY-MM-DD
        assert_eq!(date.len(), 10, "BUILD_DATE should be 10 chars: {}", date);
        assert_eq!(&date[4..5], "-", "BUILD_DATE should have dash at pos 4");
        assert_eq!(&date[7..8], "-", "BUILD_DATE should have dash at pos 7");
    }

    #[test]
    fn install_copies_binary_to_bin_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Create a fake controller-cli binary
        let source = source_dir.join("controller-cli");
        std::fs::write(&source, b"fake-binary-content").unwrap();

        let bin_dir = temp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let dest = bin_dir.join("controller-cli");
        std::fs::copy(&source, &dest).unwrap();

        assert!(dest.exists());
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "fake-binary-content"
        );
    }
}
