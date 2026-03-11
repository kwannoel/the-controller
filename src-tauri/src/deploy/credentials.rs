use serde::{Deserialize, Serialize};
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
        self.hetzner_api_key.is_some()
            && self.cloudflare_api_key.is_some()
            && self.root_domain.is_some()
            && self.coolify_url.is_some()
            && self.coolify_api_key.is_some()
            && self.server_ip.is_some()
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
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
