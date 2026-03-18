use crate::emitter::EventEmitter;
use crate::models::{GithubIssue, GithubLabel};
use crate::pty_manager::PtyManager;
use crate::storage::Storage;
use crate::voice::VoicePipeline;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;

const ISSUE_CACHE_TTL_SECS: u64 = 60;

pub struct CacheEntry {
    pub issues: Vec<GithubIssue>,
    pub fetched_at: Instant,
}

impl CacheEntry {
    pub fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < Duration::from_secs(ISSUE_CACHE_TTL_SECS)
    }
}

#[derive(Default)]
pub struct IssueCache {
    pub entries: HashMap<String, CacheEntry>,
}

impl IssueCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, repo_path: &str) -> Option<&CacheEntry> {
        self.entries.get(repo_path)
    }

    pub fn insert(&mut self, repo_path: String, issues: Vec<GithubIssue>) {
        tracing::debug!(repo = %repo_path, count = issues.len(), "caching issues for repo");
        self.entries.insert(
            repo_path,
            CacheEntry {
                issues,
                fetched_at: Instant::now(),
            },
        );
    }

    pub fn invalidate(&mut self, repo_path: &str) {
        tracing::debug!(repo = %repo_path, "invalidating issue cache for repo");
        self.entries.remove(repo_path);
    }

    /// Add a newly created issue to the cache for a repo (if cached).
    pub fn add_issue(&mut self, repo_path: &str, issue: GithubIssue) {
        if let Some(entry) = self.entries.get_mut(repo_path) {
            entry.issues.push(issue);
        }
    }

    /// Add a label to a cached issue.
    pub fn add_label(&mut self, repo_path: &str, issue_number: u64, label: &str) {
        if let Some(entry) = self.entries.get_mut(repo_path) {
            if let Some(issue) = entry.issues.iter_mut().find(|i| i.number == issue_number) {
                if !issue.labels.iter().any(|l| l.name == label) {
                    issue.labels.push(GithubLabel {
                        name: label.to_string(),
                    });
                }
            }
        }
    }

    /// Remove an issue from the cache.
    pub fn remove_issue(&mut self, repo_path: &str, issue_number: u64) {
        if let Some(entry) = self.entries.get_mut(repo_path) {
            entry.issues.retain(|i| i.number != issue_number);
        }
    }

    /// Remove a label from a cached issue.
    pub fn remove_label(&mut self, repo_path: &str, issue_number: u64, label: &str) {
        if let Some(entry) = self.entries.get_mut(repo_path) {
            if let Some(issue) = entry.issues.iter_mut().find(|i| i.number == issue_number) {
                issue.labels.retain(|l| l.name != label);
            }
        }
    }
}

pub struct AppState {
    pub storage: Arc<Mutex<Storage>>,
    pub pty_manager: Arc<Mutex<PtyManager>>,
    pub issue_cache: Arc<Mutex<IssueCache>>,
    pub(crate) secure_env_request: Mutex<Option<crate::secure_env::ActiveSecureEnvRequest>>,
    pub emitter: Arc<dyn EventEmitter>,
    pub staging_lock: TokioMutex<()>,
    pub voice_pipeline: Arc<TokioMutex<Option<VoicePipeline>>>,
    pub frontend_log: std::sync::Mutex<Option<std::fs::File>>,
    /// Incremented each time stop_voice_pipeline is called. start_voice_pipeline
    /// reads this before init and checks again after — if it changed, a stop was
    /// requested during init so the new pipeline is dropped instead of stored.
    pub voice_generation: AtomicU64,
}

impl AppState {
    pub fn from_storage(storage: Storage, emitter: Arc<dyn EventEmitter>) -> std::io::Result<Self> {
        tracing::info!("initializing app state");
        storage.ensure_dirs()?;
        let frontend_log = match crate::logging::init_frontend_log_writer(&storage.base_dir()) {
            Ok((file, _path)) => {
                tracing::debug!("frontend log writer initialized");
                Mutex::new(Some(file))
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to initialize frontend log writer");
                Mutex::new(None)
            }
        };
        tracing::info!("app state initialized");
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
            pty_manager: Arc::new(Mutex::new(PtyManager::new())),
            issue_cache: Arc::new(Mutex::new(IssueCache::new())),
            secure_env_request: Mutex::new(None),
            emitter,
            staging_lock: TokioMutex::new(()),
            voice_pipeline: Arc::new(TokioMutex::new(None)),
            frontend_log,
            voice_generation: AtomicU64::new(0),
        })
    }

    pub fn new(emitter: Arc<dyn EventEmitter>) -> std::io::Result<Self> {
        Self::from_storage(Storage::with_default_path()?, emitter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GithubIssue, GithubLabel};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_issue_cache_get_returns_none_on_miss() {
        let cache = IssueCache::new();
        assert!(cache.get("/some/repo").is_none());
    }

    #[test]
    fn test_issue_cache_insert_and_get() {
        let mut cache = IssueCache::new();
        let issues = vec![GithubIssue {
            number: 1,
            title: "Test".to_string(),
            url: "https://github.com/owner/repo/issues/1".to_string(),
            body: None,
            labels: vec![],
        }];
        cache.insert("/some/repo".to_string(), issues.clone());
        let entry = cache.get("/some/repo").unwrap();
        assert_eq!(entry.issues.len(), 1);
        assert_eq!(entry.issues[0].number, 1);
    }

    #[test]
    fn test_issue_cache_is_fresh_within_ttl() {
        let mut cache = IssueCache::new();
        cache.insert("/repo".to_string(), vec![]);
        let entry = cache.get("/repo").unwrap();
        assert!(entry.is_fresh());
    }

    #[test]
    fn test_issue_cache_is_stale_after_ttl() {
        let mut cache = IssueCache::new();
        let entry = CacheEntry {
            issues: vec![],
            fetched_at: Instant::now() - Duration::from_secs(120),
        };
        cache.entries.insert("/repo".to_string(), entry);
        let entry = cache.get("/repo").unwrap();
        assert!(!entry.is_fresh());
    }

    #[test]
    fn test_issue_cache_add_issue() {
        let mut cache = IssueCache::new();
        cache.insert("/repo".to_string(), vec![]);
        let issue = GithubIssue {
            number: 5,
            title: "New".to_string(),
            url: "https://github.com/o/r/issues/5".to_string(),
            body: None,
            labels: vec![],
        };
        cache.add_issue("/repo", issue);
        let entry = cache.get("/repo").unwrap();
        assert_eq!(entry.issues.len(), 1);
        assert_eq!(entry.issues[0].number, 5);
    }

    #[test]
    fn test_issue_cache_add_issue_no_entry_is_noop() {
        let mut cache = IssueCache::new();
        let issue = GithubIssue {
            number: 5,
            title: "New".to_string(),
            url: "https://github.com/o/r/issues/5".to_string(),
            body: None,
            labels: vec![],
        };
        cache.add_issue("/repo", issue);
        assert!(cache.get("/repo").is_none());
    }

    #[test]
    fn test_issue_cache_add_label() {
        let mut cache = IssueCache::new();
        cache.insert(
            "/repo".to_string(),
            vec![GithubIssue {
                number: 1,
                title: "Test".to_string(),
                url: "https://github.com/o/r/issues/1".to_string(),
                body: None,
                labels: vec![],
            }],
        );
        cache.add_label("/repo", 1, "in-progress");
        let entry = cache.get("/repo").unwrap();
        assert_eq!(entry.issues[0].labels.len(), 1);
        assert_eq!(entry.issues[0].labels[0].name, "in-progress");
    }

    #[test]
    fn test_issue_cache_add_label_no_duplicates() {
        let mut cache = IssueCache::new();
        cache.insert(
            "/repo".to_string(),
            vec![GithubIssue {
                number: 1,
                title: "Test".to_string(),
                url: "https://github.com/o/r/issues/1".to_string(),
                body: None,
                labels: vec![],
            }],
        );
        cache.add_label("/repo", 1, "triaged");
        cache.add_label("/repo", 1, "triaged");
        let entry = cache.get("/repo").unwrap();
        assert_eq!(entry.issues[0].labels.len(), 1);
    }

    #[test]
    fn test_issue_cache_remove_label() {
        let mut cache = IssueCache::new();
        cache.insert(
            "/repo".to_string(),
            vec![GithubIssue {
                number: 1,
                title: "Test".to_string(),
                url: "https://github.com/o/r/issues/1".to_string(),
                body: None,
                labels: vec![GithubLabel {
                    name: "in-progress".to_string(),
                }],
            }],
        );
        cache.remove_label("/repo", 1, "in-progress");
        let entry = cache.get("/repo").unwrap();
        assert!(entry.issues[0].labels.is_empty());
    }

    #[test]
    fn test_issue_cache_remove_issue() {
        let mut cache = IssueCache::new();
        cache.insert(
            "/repo".to_string(),
            vec![
                GithubIssue {
                    number: 1,
                    title: "First".to_string(),
                    url: "https://github.com/o/r/issues/1".to_string(),
                    body: None,
                    labels: vec![],
                },
                GithubIssue {
                    number: 2,
                    title: "Second".to_string(),
                    url: "https://github.com/o/r/issues/2".to_string(),
                    body: None,
                    labels: vec![],
                },
            ],
        );
        cache.remove_issue("/repo", 1);
        let entry = cache.get("/repo").unwrap();
        assert_eq!(entry.issues.len(), 1);
        assert_eq!(entry.issues[0].number, 2);
    }

    #[test]
    fn test_issue_cache_invalidate_removes_repo_entry() {
        let mut cache = IssueCache::new();
        cache.insert("/repo".to_string(), vec![]);

        cache.invalidate("/repo");

        assert!(cache.get("/repo").is_none());
    }

    #[test]
    fn test_app_state_from_storage_returns_error_when_storage_dirs_cannot_be_created() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("blocked-base-dir");
        fs::write(&file_path, "not a directory").unwrap();

        let emitter = crate::emitter::NoopEmitter::new();
        let error = AppState::from_storage(Storage::new(file_path), emitter)
            .err()
            .expect("app state init should fail when storage dirs cannot be created");

        assert_eq!(error.kind(), std::io::ErrorKind::NotADirectory);
    }
}
