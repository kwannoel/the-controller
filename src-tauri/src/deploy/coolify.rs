use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolifyApp {
    pub uuid: String,
    pub name: String,
    pub fqdn: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolifyDeployment {
    pub id: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CoolifyClient {
    base_url: String,
    api_key: String,
    client: Client,
}

impl CoolifyClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub async fn list_applications(&self) -> Result<Vec<CoolifyApp>, String> {
        let url = format!("{}/api/v1/applications", self.base_url);
        tracing::debug!(url = %url, "GET Coolify applications");

        let resp = self
            .client
            .get(url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Coolify API request failed");
                format!("Coolify API request failed: {e}")
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, "Coolify API error listing applications");
            return Err(format!("Coolify API error {status}: {body}"));
        }

        let apps = resp.json::<Vec<CoolifyApp>>().await.map_err(|e| {
            tracing::error!(error = %e, "failed to parse Coolify response");
            format!("Failed to parse Coolify response: {e}")
        })?;
        tracing::debug!(count = apps.len(), "fetched Coolify applications");
        Ok(apps)
    }

    pub async fn deploy_application(&self, uuid: &str) -> Result<(), String> {
        let url = format!("{}/api/v1/applications/{uuid}/restart", self.base_url);
        tracing::info!(uuid = %uuid, "deploying Coolify application");

        let resp = self
            .client
            .post(url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, uuid = %uuid, "Coolify deploy request failed");
                format!("Coolify deploy request failed: {e}")
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, uuid = %uuid, "Coolify deploy error");
            return Err(format!("Coolify deploy error {status}: {body}"));
        }

        tracing::info!(uuid = %uuid, "Coolify application deploy triggered");
        Ok(())
    }

    pub async fn get_deployments(&self, uuid: &str) -> Result<Vec<CoolifyDeployment>, String> {
        let url = format!("{}/api/v1/applications/{uuid}/deployments", self.base_url);
        tracing::debug!(url = %url, uuid = %uuid, "GET Coolify deployments");

        let resp = self
            .client
            .get(url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, uuid = %uuid, "Coolify API request failed");
                format!("Coolify API request failed: {e}")
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status = %status, uuid = %uuid, "Coolify API error fetching deployments");
            return Err(format!("Coolify API error {status}: {body}"));
        }

        let deployments = resp.json::<Vec<CoolifyDeployment>>().await.map_err(|e| {
            tracing::error!(error = %e, "failed to parse Coolify deployments");
            format!("Failed to parse Coolify deployments: {e}")
        })?;
        tracing::debug!(uuid = %uuid, count = deployments.len(), "fetched Coolify deployments");
        Ok(deployments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction() {
        let client = CoolifyClient::new("https://coolify.example.com/", "test-key");
        assert_eq!(client.base_url, "https://coolify.example.com");
        assert_eq!(client.api_key, "test-key");
    }

    #[test]
    fn test_auth_header_format() {
        let client = CoolifyClient::new("https://coolify.example.com", "my-token");
        assert_eq!(client.auth_header(), "Bearer my-token");
    }

    #[test]
    fn test_coolify_app_deserialize() {
        let json = r#"{"uuid":"abc-123","name":"myapp","fqdn":"https://myapp.example.com","status":"running","description":null}"#;
        let app: CoolifyApp = serde_json::from_str(json).unwrap();
        assert_eq!(app.uuid, "abc-123");
        assert_eq!(app.name, "myapp");
        assert_eq!(app.status, Some("running".to_string()));
    }
}
