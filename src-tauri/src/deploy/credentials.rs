use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployCredentials {
    pub hetzner_api_key: Option<String>,
    pub cloudflare_api_key: Option<String>,
    pub cloudflare_zone_id: Option<String>,
    pub root_domain: Option<String>,
    pub coolify_url: Option<String>,
    pub coolify_api_key: Option<String>,
    pub server_ip: Option<String>,
}

impl DeployCredentials {
    pub fn is_provisioned(&self) -> bool {
        let provisioned = self.hetzner_api_key.is_some()
            && self.cloudflare_api_key.is_some()
            && self.root_domain.is_some()
            && self.coolify_url.is_some()
            && self.coolify_api_key.is_some()
            && self.server_ip.is_some();
        if !provisioned {
            let missing: Vec<&str> = [
                ("hetzner_api_key", &self.hetzner_api_key),
                ("cloudflare_api_key", &self.cloudflare_api_key),
                ("root_domain", &self.root_domain),
                ("coolify_url", &self.coolify_url),
                ("coolify_api_key", &self.coolify_api_key),
                ("server_ip", &self.server_ip),
            ]
            .iter()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| *k)
            .collect();
            tracing::debug!(missing = ?missing, "deploy credentials incomplete");
        }
        provisioned
    }

    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".the-controller")
            .join("deploy-credentials.json")
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();
        if !path.exists() {
            tracing::debug!(path = %path.display(), "credentials file not found, returning defaults");
            return Ok(Self::default());
        }
        tracing::debug!(path = %path.display(), "loading deploy credentials");
        let data = std::fs::read_to_string(&path).map_err(|e| {
            tracing::error!(path = %path.display(), error = %e, "failed to read credentials file");
            e.to_string()
        })?;
        serde_json::from_str(&data).map_err(|e| {
            tracing::error!(error = %e, "failed to parse credentials JSON");
            e.to_string()
        })
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        tracing::debug!(path = %path.display(), "saving deploy credentials");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                tracing::error!(path = %parent.display(), error = %e, "failed to create credentials directory");
                e.to_string()
            })?;
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize credentials");
            e.to_string()
        })?;
        let mut file = open_credentials_file(&path).map_err(|e| {
            tracing::error!(path = %path.display(), error = %e, "failed to open credentials file for writing");
            e.to_string()
        })?;
        file.write_all(data.as_bytes()).map_err(|e| {
            tracing::error!(error = %e, "failed to write credentials file");
            e.to_string()
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(
                |e| {
                    tracing::error!(error = %e, "failed to set credentials file permissions");
                    e.to_string()
                },
            )?;
        }
        tracing::debug!("deploy credentials saved");
        Ok(())
    }
}

fn open_credentials_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::env;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn test_default_credentials_are_not_provisioned() {
        let creds = DeployCredentials::default();
        assert!(!creds.is_provisioned());
    }

    #[test]
    fn test_fully_populated_credentials_are_provisioned() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("hk".into()),
            cloudflare_api_key: Some("cf".into()),
            cloudflare_zone_id: Some("zone".into()),
            root_domain: Some("example.com".into()),
            coolify_url: Some("https://coolify.example.com".into()),
            coolify_api_key: Some("ck".into()),
            server_ip: Some("1.2.3.4".into()),
        };
        assert!(creds.is_provisioned());
    }

    #[test]
    fn test_partial_credentials_are_not_provisioned() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("hk".into()),
            ..Default::default()
        };
        assert!(!creds.is_provisioned());
    }

    #[test]
    fn test_credentials_serialize_roundtrip() {
        let creds = DeployCredentials {
            hetzner_api_key: Some("test-key".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&creds).unwrap();
        let deserialized: DeployCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hetzner_api_key, Some("test-key".into()));
    }

    #[cfg(unix)]
    #[test]
    fn test_open_credentials_file_creates_owner_only_file_on_unix() {
        let tmp = TempDir::new().expect("temp dir");
        let path = tmp.path().join("deploy-credentials.json");
        let original_umask = unsafe { libc::umask(0) };

        let file = open_credentials_file(&path).expect("open credentials file");

        unsafe {
            libc::umask(original_umask);
        }

        drop(file);

        let mode = fs::metadata(&path)
            .expect("stat credentials file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn test_save_restores_owner_only_permissions_on_unix() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = TempDir::new().expect("temp dir");
        let original_home = env::var_os("HOME");
        env::set_var("HOME", tmp.path());

        let creds = DeployCredentials {
            hetzner_api_key: Some("test-key".into()),
            ..Default::default()
        };

        creds.save().expect("initial save");

        let path = DeployCredentials::config_path();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("broaden permissions");

        creds.save().expect("second save");

        let mode = fs::metadata(&path)
            .expect("stat credentials file")
            .permissions()
            .mode()
            & 0o777;

        match original_home {
            Some(home) => env::set_var("HOME", home),
            None => env::remove_var("HOME"),
        }

        assert_eq!(mode, 0o600);
    }
}
