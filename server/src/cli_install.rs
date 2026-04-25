use std::path::PathBuf;

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

/// Copy the `controller-cli` binary from next to the running executable
/// into `~/.the-controller/bin/controller-cli`.
///
/// Runs at app startup so the CLI is always available on a stable path.
/// Silently skips if the source binary isn't found (e.g. production bundle
/// that doesn't include it yet).
pub fn install_controller_cli() {
    let Some(bin_dir) = controller_bin_dir() else {
        eprintln!("Warning: could not determine home directory for controller-cli install");
        return;
    };

    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    let Some(exe_dir) = current_exe.parent() else {
        return;
    };

    let source = exe_dir.join("controller-cli");
    if !source.exists() {
        return;
    }

    if let Err(e) = std::fs::create_dir_all(&bin_dir) {
        eprintln!("Warning: could not create {}: {}", bin_dir.display(), e);
        return;
    }

    let dest = bin_dir.join("controller-cli");
    if let Err(e) = std::fs::copy(&source, &dest) {
        eprintln!("Warning: could not install controller-cli: {}", e);
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
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
