use serde::{Deserialize, Serialize};

use super::coolify::CoolifyClient;
use super::credentials::DeployCredentials;

#[derive(Serialize)]
pub struct ProjectSignals {
    pub has_dockerfile: bool,
    pub has_package_json: bool,
    pub has_vite_config: bool,
    pub has_start_script: bool,
    pub has_pyproject: bool,
}

#[tauri::command]
pub async fn detect_project_type(repo_path: String) -> Result<ProjectSignals, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&repo_path);
        let has_package_json = path.join("package.json").exists();
        let has_start_script = if has_package_json {
            std::fs::read_to_string(path.join("package.json"))
                .map(|content| content.contains("\"start\""))
                .unwrap_or(false)
        } else {
            false
        };

        Ok(ProjectSignals {
            has_dockerfile: path.join("Dockerfile").exists(),
            has_package_json,
            has_vite_config: path.join("vite.config.ts").exists()
                || path.join("vite.config.js").exists()
                || path.join("astro.config.mjs").exists()
                || path.join("next.config.js").exists()
                || path.join("next.config.mjs").exists(),
            has_start_script,
            has_pyproject: path.join("pyproject.toml").exists()
                || path.join("requirements.txt").exists(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_deploy_credentials() -> Result<DeployCredentials, String> {
    tokio::task::spawn_blocking(DeployCredentials::load)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_deploy_credentials(credentials: DeployCredentials) -> Result<(), String> {
    tokio::task::spawn_blocking(move || credentials.save())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn is_deploy_provisioned() -> Result<bool, String> {
    tokio::task::spawn_blocking(|| {
        let creds = DeployCredentials::load()?;
        Ok(creds.is_provisioned())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
pub struct DeployRequest {
    pub project_name: String,
    pub repo_path: String,
    pub subdomain: String,
    pub project_type: String,
}

#[derive(Serialize)]
pub struct DeployResult {
    pub url: String,
    pub coolify_uuid: String,
}

#[tauri::command]
pub async fn deploy_project(request: DeployRequest) -> Result<DeployResult, String> {
    let creds = tokio::task::spawn_blocking(DeployCredentials::load)
        .await
        .map_err(|e| e.to_string())??;
    if !creds.is_provisioned() {
        return Err("Deploy not provisioned. Run setup first.".to_string());
    }

    let coolify = CoolifyClient::new(
        creds.coolify_url.as_ref().unwrap(),
        creds.coolify_api_key.as_ref().unwrap(),
    );

    let apps = coolify.list_applications().await?;
    let existing = apps.iter().find(|a| a.name == request.project_name);

    let uuid = if let Some(app) = existing {
        coolify.deploy_application(&app.uuid).await?;
        app.uuid.clone()
    } else {
        return Err("Creating new Coolify applications not yet implemented. Create the app in Coolify UI first.".to_string());
    };

    let domain = format!("{}.{}", request.subdomain, creds.root_domain.unwrap());
    let url = format!("https://{domain}");

    Ok(DeployResult {
        url,
        coolify_uuid: uuid,
    })
}

#[tauri::command]
pub async fn list_deployed_services() -> Result<Vec<serde_json::Value>, String> {
    let creds = tokio::task::spawn_blocking(DeployCredentials::load)
        .await
        .map_err(|e| e.to_string())??;
    if !creds.is_provisioned() {
        return Ok(vec![]);
    }

    let coolify = CoolifyClient::new(
        creds.coolify_url.as_ref().unwrap(),
        creds.coolify_api_key.as_ref().unwrap(),
    );

    let apps = coolify.list_applications().await?;
    let result: Vec<serde_json::Value> = apps
        .iter()
        .map(|app| {
            serde_json::json!({
                "uuid": app.uuid,
                "name": app.name,
                "status": app.status,
                "fqdn": app.fqdn,
            })
        })
        .collect();

    Ok(result)
}
