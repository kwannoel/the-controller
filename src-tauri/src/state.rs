use crate::models::GithubIssue;
use crate::pty_manager::PtyManager;
use crate::storage::Storage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
        self.entries.insert(repo_path, CacheEntry {
            issues,
            fetched_at: Instant::now(),
        });
    }
}

pub struct AppState {
    pub storage: Mutex<Storage>,
    pub pty_manager: Mutex<PtyManager>,
    pub issue_cache: Arc<Mutex<IssueCache>>,
}

impl AppState {
    pub fn new() -> Self {
        let storage = Storage::with_default_path();
        storage.ensure_dirs().unwrap();
        Self {
            storage: Mutex::new(storage),
            pty_manager: Mutex::new(PtyManager::new()),
            issue_cache: Arc::new(Mutex::new(IssueCache::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GithubIssue;

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
}
