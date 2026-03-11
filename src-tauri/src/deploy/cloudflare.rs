use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub proxied: bool,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    result: Option<T>,
    errors: Vec<CloudflareError>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    message: String,
}

#[derive(Debug, Clone)]
pub struct CloudflareClient {
    api_key: String,
    zone_id: String,
    client: Client,
}

impl CloudflareClient {
    pub fn new(api_key: &str, zone_id: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            zone_id: zone_id.to_string(),
            client: Client::new(),
        }
    }

    pub async fn create_dns_record(
        &self,
        subdomain: &str,
        server_ip: &str,
    ) -> Result<DnsRecord, String> {
        let body = serde_json::json!({
            "type": "A",
            "name": subdomain,
            "content": server_ip,
            "proxied": true,
            "ttl": 1,
        });

        let resp = self.client
            .post(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                self.zone_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Cloudflare API request failed: {e}"))?;

        let cf_resp: CloudflareResponse<DnsRecord> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Cloudflare response: {e}"))?;

        if !cf_resp.success {
            let msgs: Vec<String> = cf_resp.errors.iter().map(|e| e.message.clone()).collect();
            return Err(format!("Cloudflare error: {}", msgs.join(", ")));
        }

        cf_resp.result.ok_or_else(|| "No result in Cloudflare response".to_string())
    }

    pub async fn list_dns_records(&self) -> Result<Vec<DnsRecord>, String> {
        let resp = self.client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                self.zone_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("Cloudflare API request failed: {e}"))?;

        let cf_resp: CloudflareResponse<Vec<DnsRecord>> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Cloudflare response: {e}"))?;

        if !cf_resp.success {
            let msgs: Vec<String> = cf_resp.errors.iter().map(|e| e.message.clone()).collect();
            return Err(format!("Cloudflare error: {}", msgs.join(", ")));
        }

        cf_resp.result.ok_or_else(|| "No result in Cloudflare response".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_record_deserialize() {
        let json = r#"{"id":"rec-1","name":"app.example.com","type":"A","content":"1.2.3.4","proxied":true}"#;
        let record: DnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.name, "app.example.com");
        assert_eq!(record.record_type, "A");
        assert!(record.proxied);
    }

    #[test]
    fn test_client_construction() {
        let client = CloudflareClient::new("cf-key", "zone-123");
        assert_eq!(client.api_key, "cf-key");
        assert_eq!(client.zone_id, "zone-123");
    }
}
