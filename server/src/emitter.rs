use std::sync::Arc;

pub trait EventEmitter: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String>;
}

/// No-op implementation for tests and headless contexts.
pub struct NoopEmitter;

impl NoopEmitter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Arc<dyn EventEmitter> {
        Arc::new(Self)
    }
}

impl EventEmitter for NoopEmitter {
    fn emit(&self, _event: &str, _payload: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Broadcast-based emitter that sends events to WebSocket clients.
pub struct WsBroadcastEmitter {
    tx: tokio::sync::broadcast::Sender<String>,
}

impl WsBroadcastEmitter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (
        Arc<dyn EventEmitter>,
        tokio::sync::broadcast::Sender<String>,
    ) {
        let (tx, _) = tokio::sync::broadcast::channel(4096);
        let sender = tx.clone();
        (Arc::new(Self { tx }), sender)
    }
}

impl EventEmitter for WsBroadcastEmitter {
    fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
        let msg = serde_json::json!({ "event": event, "payload": payload }).to_string();
        let _ = self.tx.send(msg); // Ok if no receivers
        Ok(())
    }
}
