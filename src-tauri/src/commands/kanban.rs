use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::{AppHandle, Manager};

pub fn order_file_in(dir: &Path) -> PathBuf {
    dir.join("kanban-order.json")
}

fn order_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create app_data_dir: {e}"))?;
    }
    Ok(order_file_in(&dir))
}

pub fn load_order_from(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Default::default()));
    }
    let bytes =
        std::fs::read(path).map_err(|e| format!("failed to read kanban-order.json: {e}"))?;
    if bytes.is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("failed to parse kanban-order.json: {e}"))
}

pub fn save_order_to(path: &Path, order: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid order file path".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("failed to create parent dir: {e}"))?;

    let tmp = parent.join(".kanban-order.json.tmp");
    let bytes =
        serde_json::to_vec_pretty(order).map_err(|e| format!("failed to serialize order: {e}"))?;
    std::fs::write(&tmp, &bytes).map_err(|e| format!("failed to write tmp order: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("failed to rename tmp order: {e}"))?;
    Ok(())
}

pub(crate) async fn kanban_load_order(app: AppHandle) -> Result<Value, String> {
    let path = order_file(&app)?;
    tokio::task::spawn_blocking(move || load_order_from(&path))
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

pub(crate) async fn kanban_save_order(app: AppHandle, order: Value) -> Result<(), String> {
    let path = order_file(&app)?;
    tokio::task::spawn_blocking(move || save_order_to(&path, &order))
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn load_returns_empty_object_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = order_file_in(dir.path());
        assert_eq!(load_order_from(&path).unwrap(), json!({}));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = order_file_in(dir.path());
        let payload = json!({
            "/tmp/repo:todo": [3, 1, 2],
            "/tmp/repo:done": [9],
        });
        save_order_to(&path, &payload).unwrap();
        let loaded = load_order_from(&path).unwrap();
        assert_eq!(loaded, payload);
    }

    #[test]
    fn save_uses_atomic_rename_no_tmp_leftover() {
        let dir = TempDir::new().unwrap();
        let path = order_file_in(dir.path());
        save_order_to(&path, &json!({"k": [1]})).unwrap();
        let tmp = dir.path().join(".kanban-order.json.tmp");
        assert!(!tmp.exists(), "tmp file should have been renamed");
    }

    #[test]
    fn save_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let path = order_file_in(dir.path());
        save_order_to(&path, &json!({"a": [1]})).unwrap();
        save_order_to(&path, &json!({"a": [2, 3]})).unwrap();
        let loaded = load_order_from(&path).unwrap();
        assert_eq!(loaded, json!({"a": [2, 3]}));
    }

    #[test]
    fn load_empty_file_returns_empty_object() {
        let dir = TempDir::new().unwrap();
        let path = order_file_in(dir.path());
        std::fs::write(&path, b"").unwrap();
        assert_eq!(load_order_from(&path).unwrap(), json!({}));
    }
}
