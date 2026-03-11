use serde::Serialize;

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
    tokio::task::spawn_blocking(|| DeployCredentials::load())
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
