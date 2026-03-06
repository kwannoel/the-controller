use std::collections::HashMap;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::models::Project;
use crate::storage::Storage;

const BACKUP_VERSION: u32 = 1;
const MAGIC: &[u8; 4] = b"TCBK";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
struct BackupPayload {
    version: u32,
    created_at: String,
    projects: Vec<Project>,
    agents: HashMap<String, String>,
}

#[derive(Debug)]
pub enum BackupError {
    Io(std::io::Error),
    Encryption(String),
    InvalidFormat(String),
    WrongPassphrase,
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupError::Io(e) => write!(f, "IO error: {}", e),
            BackupError::Encryption(e) => write!(f, "Encryption error: {}", e),
            BackupError::InvalidFormat(e) => write!(f, "Invalid backup format: {}", e),
            BackupError::WrongPassphrase => write!(f, "Wrong passphrase or corrupted backup"),
        }
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::Io(e)
    }
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], BackupError> {
    let mut key = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| BackupError::Encryption(format!("Key derivation failed: {}", e)))?;
    Ok(key)
}

fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, BackupError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| BackupError::Encryption(format!("Cipher init failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| BackupError::Encryption("Encryption failed".to_string()))?;

    // Format: MAGIC(4) + version(4) + salt(16) + nonce(12) + ciphertext(variable)
    let mut output = Vec::with_capacity(4 + 4 + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&BACKUP_VERSION.to_le_bytes());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

fn decrypt(data: &[u8], passphrase: &str) -> Result<Vec<u8>, BackupError> {
    let header_len = 4 + 4 + SALT_LEN + NONCE_LEN;
    if data.len() < header_len {
        return Err(BackupError::InvalidFormat("File too short".to_string()));
    }

    if &data[0..4] != MAGIC {
        return Err(BackupError::InvalidFormat("Invalid magic bytes".to_string()));
    }

    let version = u32::from_le_bytes(
        data[4..8]
            .try_into()
            .map_err(|_| BackupError::InvalidFormat("Invalid version".to_string()))?,
    );
    if version != BACKUP_VERSION {
        return Err(BackupError::InvalidFormat(format!(
            "Unsupported backup version: {}",
            version
        )));
    }

    let salt = &data[8..8 + SALT_LEN];
    let nonce_bytes = &data[8 + SALT_LEN..8 + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[header_len..];

    let key = derive_key(passphrase, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| BackupError::Encryption(format!("Cipher init failed: {}", e)))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| BackupError::WrongPassphrase)
}

pub fn export_backup(storage: &Storage, passphrase: &str) -> Result<Vec<u8>, BackupError> {
    let projects = storage.list_projects()?;

    let mut agents = HashMap::new();
    for project in &projects {
        let content = storage.get_agents_md(project)?;
        if !content.is_empty() {
            agents.insert(project.id.to_string(), content);
        }
    }

    let payload = BackupPayload {
        version: BACKUP_VERSION,
        created_at: chrono::Utc::now().to_rfc3339(),
        projects,
        agents,
    };

    let json = serde_json::to_vec(&payload)
        .map_err(|e| BackupError::Encryption(format!("Serialization failed: {}", e)))?;

    encrypt(&json, passphrase)
}

pub fn import_backup(
    storage: &Storage,
    data: &[u8],
    passphrase: &str,
) -> Result<u32, BackupError> {
    let plaintext = decrypt(data, passphrase)?;

    let payload: BackupPayload = serde_json::from_slice(&plaintext).map_err(|e| {
        BackupError::InvalidFormat(format!("Invalid backup payload: {}", e))
    })?;

    let count = payload.projects.len() as u32;

    for project in &payload.projects {
        storage.save_project(project)?;
    }

    for (project_id_str, content) in &payload.agents {
        if let Ok(project_id) = project_id_str.parse() {
            storage.save_agents_md(project_id, content)?;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SessionConfig;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn make_storage(tmp: &TempDir) -> Storage {
        let storage = Storage::new(tmp.path().to_path_buf());
        storage.ensure_dirs().expect("ensure_dirs");
        storage
    }

    fn make_project(name: &str) -> Project {
        Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            repo_path: "/tmp/repo".to_string(),
            created_at: "2026-03-06T00:00:00Z".to_string(),
            archived: false,
            sessions: vec![SessionConfig {
                id: Uuid::new_v4(),
                label: "main".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: Some("my-secret-api-key".to_string()),
            }],
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let data = b"hello world";
        let encrypted = encrypt(data, "password123").unwrap();
        let decrypted = decrypt(&encrypted, "password123").unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_passphrase_fails() {
        let data = b"hello world";
        let encrypted = encrypt(data, "password123").unwrap();
        let result = decrypt(&encrypted, "wrong-password");
        assert!(matches!(result, Err(BackupError::WrongPassphrase)));
    }

    #[test]
    fn test_corrupted_data_fails() {
        let data = b"hello world";
        let mut encrypted = encrypt(data, "password123").unwrap();
        // Corrupt a byte in the ciphertext
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;
        let result = decrypt(&encrypted, "password123");
        assert!(matches!(result, Err(BackupError::WrongPassphrase)));
    }

    #[test]
    fn test_truncated_data_fails() {
        let result = decrypt(b"short", "password");
        assert!(matches!(result, Err(BackupError::InvalidFormat(_))));
    }

    #[test]
    fn test_invalid_magic_fails() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(b"XXXX");
        let result = decrypt(&data, "password");
        assert!(matches!(result, Err(BackupError::InvalidFormat(_))));
    }

    #[test]
    fn test_export_import_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let p1 = make_project("project-1");
        let p2 = make_project("project-2");
        storage.save_project(&p1).unwrap();
        storage.save_project(&p2).unwrap();
        storage.save_agents_md(p1.id, "agent instructions").unwrap();

        let backup = export_backup(&storage, "test-pass").unwrap();

        // Import into a fresh storage
        let tmp2 = TempDir::new().unwrap();
        let storage2 = make_storage(&tmp2);

        let count = import_backup(&storage2, &backup, "test-pass").unwrap();
        assert_eq!(count, 2);

        let projects = storage2.list_projects().unwrap();
        assert_eq!(projects.len(), 2);

        let agents = storage2.get_agents_md(&p1).unwrap();
        assert_eq!(agents, "agent instructions");
    }

    #[test]
    fn test_export_import_wrong_passphrase() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let project = make_project("project-1");
        storage.save_project(&project).unwrap();

        let backup = export_backup(&storage, "correct-pass").unwrap();

        let tmp2 = TempDir::new().unwrap();
        let storage2 = make_storage(&tmp2);
        let result = import_backup(&storage2, &backup, "wrong-pass");
        assert!(matches!(result, Err(BackupError::WrongPassphrase)));

        // Verify nothing was imported
        let projects = storage2.list_projects().unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_backup_preserves_sensitive_data() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let project = make_project("sensitive-project");
        storage.save_project(&project).unwrap();

        let backup = export_backup(&storage, "pass").unwrap();

        // Verify the plaintext secret is NOT in the encrypted backup
        let backup_str = String::from_utf8_lossy(&backup);
        assert!(!backup_str.contains("my-secret-api-key"));

        // But it IS recoverable after import
        let tmp2 = TempDir::new().unwrap();
        let storage2 = make_storage(&tmp2);
        import_backup(&storage2, &backup, "pass").unwrap();

        let projects = storage2.list_projects().unwrap();
        assert_eq!(
            projects[0].sessions[0].initial_prompt.as_deref(),
            Some("my-secret-api-key")
        );
    }
}
