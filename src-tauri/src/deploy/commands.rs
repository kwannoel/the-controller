use super::credentials::DeployCredentials;

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
