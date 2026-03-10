use std::sync::Arc;

pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String>;
}

/// No-op implementation for tests and headless contexts.
pub struct NoopEmitter;

impl NoopEmitter {
    pub fn new() -> Arc<dyn EventEmitter> {
        Arc::new(Self)
    }
}

impl EventEmitter for NoopEmitter {
    fn emit(&self, _event: &str, _payload: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Tauri implementation — wraps AppHandle.emit()
pub struct TauriEmitter {
    app_handle: tauri::AppHandle,
}

impl TauriEmitter {
    pub fn new(app_handle: tauri::AppHandle) -> Arc<dyn EventEmitter> {
        Arc::new(Self { app_handle })
    }
}

impl EventEmitter for TauriEmitter {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
        use tauri::Emitter;
        self.app_handle
            .emit(event, payload.to_string())
            .map_err(|e| e.to_string())
    }
}
