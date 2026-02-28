use crate::pty_manager::PtyManager;
use crate::storage::Storage;
use std::sync::Mutex;

pub struct AppState {
    pub storage: Mutex<Storage>,
    pub pty_manager: Mutex<PtyManager>,
}

impl AppState {
    pub fn new() -> Self {
        let storage = Storage::with_default_path();
        storage.ensure_dirs().unwrap();
        Self {
            storage: Mutex::new(storage),
            pty_manager: Mutex::new(PtyManager::new()),
        }
    }
}
