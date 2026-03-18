use notify_debouncer_mini::new_debouncer;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct KeybindingsResult {
    pub overrides: HashMap<String, String>,
    pub warnings: Vec<String>,
    /// Which physical modifier "Meta+" maps to: "cmd" (default) or "ctrl".
    pub meta_key: String,
}

/// Commands that are recognized in the keybindings config file.
///
/// Intentionally excludes commands with hardcoded behavior that cannot be
/// meaningfully remapped:
/// - `escape-focus` / `escape-forward` — tied to double-tap Escape logic
/// - `screenshot-picker` — display alias for Shift+screenshot/screenshot-cropped
/// - `switch-workspace` — Space triggers workspace mode picker
const KNOWN_COMMANDS: &[&str] = &[
    "navigate-next",
    "navigate-prev",
    "expand-collapse",
    "fuzzy-finder",
    "create-session",
    "finish-branch",
    "save-prompt",
    "load-prompt",
    "stage",
    "screenshot",
    "screenshot-cropped",
    "toggle-session-provider",
    "new-project",
    "delete",
    "open-issues-modal",
    "generate-architecture",
    "toggle-help",
    "keystroke-visualizer",
    "toggle-agent",
    "trigger-agent-check",
    "clear-agent-reports",
    "toggle-maintainer-view",
    "create-note",
    "delete-note",
    "rename-note",
    "duplicate-note",
    "toggle-note-preview",
    "deploy-project",
];

/// Commands handled externally (outside `buildKeyMap`) that require a `Meta+`
/// prefix to be dispatched via `matchMetaKey` in the frontend. Assigning a bare
/// key to these will silently never fire.
const META_REQUIRED_COMMANDS: &[&str] = &[
    "screenshot",
    "screenshot-cropped",
    "toggle-session-provider",
    "keystroke-visualizer",
];

pub fn parse_keybindings(content: &str) -> KeybindingsResult {
    let mut overrides = HashMap::new();
    let mut warnings = Vec::new();
    let mut meta_key = "cmd".to_string();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip trailing inline comments (e.g. "navigate-next j # vim-style")
        let line = match line.find(" #") {
            Some(pos) => line[..pos].trim(),
            None => line,
        };

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            warnings.push(format!("malformed line: {line}"));
            continue;
        }

        let command = parts[0];
        let key = parts[1];

        // Special directive: "meta cmd" or "meta ctrl"
        if command == "meta" {
            match key {
                "cmd" | "ctrl" => meta_key = key.to_string(),
                _ => warnings.push(format!("invalid meta value: {key} (expected cmd or ctrl)")),
            }
            continue;
        }

        if !KNOWN_COMMANDS.contains(&command) {
            warnings.push(format!("unknown command: {command}"));
            continue;
        }

        overrides.insert(command.to_string(), key.to_string());

        if META_REQUIRED_COMMANDS.contains(&command)
            && !key.starts_with("Meta+")
            && !key.starts_with('⌘')
        {
            warnings.push(format!(
                "command '{command}' requires Meta+ prefix (e.g. Meta+<key>)"
            ));
        }
    }

    KeybindingsResult {
        overrides,
        warnings,
        meta_key,
    }
}

pub fn keybindings_path(base_dir: &Path) -> PathBuf {
    base_dir.join("keybindings")
}

pub fn load_keybindings(base_dir: &Path) -> KeybindingsResult {
    let path = keybindings_path(base_dir);
    match fs::read_to_string(path) {
        Ok(content) => parse_keybindings(&content),
        Err(_) => KeybindingsResult {
            overrides: HashMap::new(),
            warnings: Vec::new(),
            meta_key: "cmd".to_string(),
        },
    }
}

const COMMAND_DEFAULTS: &[(&str, &str, &str)] = &[
    // Navigation
    ("navigate-next", "j", "Navigation"),
    ("navigate-prev", "k", "Navigation"),
    ("expand-collapse", "l", "Navigation"),
    ("fuzzy-finder", "f", "Navigation"),
    // Sessions (development)
    ("create-session", "c", "Sessions (development)"),
    ("finish-branch", "m", "Sessions (development)"),
    ("save-prompt", "P", "Sessions (development)"),
    ("load-prompt", "p", "Sessions (development)"),
    ("stage", "v", "Sessions (development)"),
    ("screenshot", "Meta+s", "Sessions (development)"),
    ("screenshot-cropped", "Meta+d", "Sessions (development)"),
    (
        "toggle-session-provider",
        "Meta+t",
        "Sessions (development)",
    ),
    // Projects (development)
    ("new-project", "n", "Projects (development)"),
    ("delete", "d", "Projects (development)"),
    ("open-issues-modal", "i", "Projects (development)"),
    // Panels
    ("toggle-help", "?", "Panels"),
    ("keystroke-visualizer", "Meta+k", "Panels"),
    // Agents (agents)
    ("toggle-agent", "o", "Agents (agents)"),
    ("trigger-agent-check", "r", "Agents (agents)"),
    ("clear-agent-reports", "c", "Agents (agents)"),
    ("toggle-maintainer-view", "t", "Agents (agents)"),
    // Notes (notes)
    ("create-note", "n", "Notes (notes)"),
    ("delete-note", "d", "Notes (notes)"),
    ("rename-note", "r", "Notes (notes)"),
    ("duplicate-note", "y", "Notes (notes)"),
    ("toggle-note-preview", "p", "Notes (notes)"),
    // Architecture (architecture)
    ("generate-architecture", "r", "Architecture (architecture)"),
    // Infrastructure (infrastructure)
    ("deploy-project", "d", "Infrastructure (infrastructure)"),
];

pub fn generate_template() -> String {
    tracing::debug!("generating keybindings template");
    let mut out = String::new();
    out.push_str("# Keybindings for The Controller\n");
    out.push_str("# Uncomment and change the key to override a default binding.\n");
    out.push_str("# Format: command-name key  (inline comments with # are supported)\n");
    out.push_str("# Use Meta+ prefix for modifier keys (e.g. Meta+s)\n");
    out.push_str("# Changes are applied automatically — no restart needed.\n");
    out.push_str("#\n");
    out.push_str("# Note: Esc (focus navigation), Esc Esc (forward to terminal),\n");
    out.push_str("# and Space (workspace mode) are not configurable.\n");
    out.push_str("\n# Meta key modifier (cmd or ctrl)\n");
    out.push_str("# Uncomment to remap all Meta+ bindings at once.\n");
    out.push_str("# meta cmd\n");

    let mut current_section = "";
    for &(command, key, section) in COMMAND_DEFAULTS {
        if section != current_section {
            out.push('\n');
            out.push_str(&format!("# {section}\n"));
            current_section = section;
        }
        out.push_str(&format!("# {command} {key}\n"));
    }

    out
}

pub fn ensure_keybindings_file(base_dir: &Path) {
    let path = keybindings_path(base_dir);
    if !path.exists() {
        tracing::debug!(
            path = %path.display(),
            "keybindings file not found, creating template"
        );
        if let Err(e) = fs::write(&path, generate_template()) {
            tracing::error!(
                "failed to write keybindings template to {}: {e}",
                path.display()
            );
        }
    }
}

pub fn start_watcher(base_dir: PathBuf, emitter: Arc<dyn crate::emitter::EventEmitter>) {
    // Emit initial state
    let initial = load_keybindings(&base_dir);
    if let Ok(payload) = serde_json::to_string(&initial) {
        let _ = emitter.emit("keybindings-changed", &payload);
    }

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = match new_debouncer(Duration::from_millis(200), tx) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("failed to create keybindings file watcher: {e}");
                return;
            }
        };

        if let Err(e) = debouncer
            .watcher()
            .watch(&base_dir, notify::RecursiveMode::NonRecursive)
        {
            tracing::error!("failed to watch keybindings directory: {e}");
            return;
        }

        tracing::debug!(
            path = %base_dir.display(),
            "keybindings file watcher started"
        );

        let target = keybindings_path(&base_dir);
        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    let relevant = events.iter().any(|e| e.path == target);
                    if !relevant {
                        continue;
                    }
                    tracing::info!("keybindings file change detected, reloading");
                    let result = load_keybindings(&base_dir);
                    if let Ok(payload) = serde_json::to_string(&result) {
                        let _ = emitter.emit("keybindings-changed", &payload);
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("keybindings watcher error: {e:?}");
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_empty_content() {
        let result = parse_keybindings("");
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_comments_only() {
        let result = parse_keybindings("# this is a comment\n# another comment\n");
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_single_override() {
        let result = parse_keybindings("navigate-next j\n");
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "j");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_meta_prefixed_key() {
        let result = parse_keybindings("fuzzy-finder Meta+p\n");
        assert_eq!(result.overrides.get("fuzzy-finder").unwrap(), "Meta+p");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_unknown_command_warns() {
        let result = parse_keybindings("nonexistent-command x\n");
        assert!(result.overrides.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("nonexistent-command"));
    }

    #[test]
    fn test_parse_malformed_line_warns() {
        let result = parse_keybindings("just-one-token\n");
        assert!(result.overrides.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("just-one-token"));
    }

    #[test]
    fn test_parse_inline_comments() {
        let result = parse_keybindings("navigate-next h # vim-style\n");
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "h");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_multiple_overrides() {
        let content = "navigate-next j\nnavigate-prev k\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 2);
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "j");
        assert_eq!(result.overrides.get("navigate-prev").unwrap(), "k");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_mixed_comments_and_overrides() {
        let content = "# Navigation\nnavigate-next j\n# Finder\nfuzzy-finder ctrl+p\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 2);
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "j");
        assert_eq!(result.overrides.get("fuzzy-finder").unwrap(), "ctrl+p");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_blank_lines_ignored() {
        let content = "\n\nnavigate-next j\n\n";
        let result = parse_keybindings(content);
        assert_eq!(result.overrides.len(), 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        let result = parse_keybindings("  navigate-next   j  \n");
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "j");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_load_keybindings_missing_file() {
        let tmp = TempDir::new().unwrap();
        let result = load_keybindings(tmp.path());
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_load_keybindings_with_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(keybindings_path(tmp.path()), "navigate-next j\n").unwrap();
        let result = load_keybindings(tmp.path());
        assert_eq!(result.overrides.get("navigate-next").unwrap(), "j");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_keybindings_path() {
        let base = Path::new("/tmp/test-base");
        assert_eq!(
            keybindings_path(base),
            PathBuf::from("/tmp/test-base/keybindings")
        );
    }

    #[test]
    fn test_generate_template_contains_all_commands() {
        let template = generate_template();
        for cmd in KNOWN_COMMANDS {
            assert!(template.contains(cmd), "template missing command: {cmd}");
        }
    }

    #[test]
    fn test_generate_template_all_lines_are_comments_or_blank() {
        let template = generate_template();
        for line in template.lines() {
            assert!(
                line.is_empty() || line.starts_with('#'),
                "line is not a comment or blank: {line}"
            );
        }
    }

    #[test]
    fn test_ensure_keybindings_file_creates_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = keybindings_path(tmp.path());
        assert!(!path.exists());
        ensure_keybindings_file(tmp.path());
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Keybindings for The Controller"));
    }

    #[test]
    fn test_ensure_keybindings_file_does_not_overwrite_existing() {
        let tmp = TempDir::new().unwrap();
        let path = keybindings_path(tmp.path());
        fs::write(&path, "navigate-next x\n").unwrap();
        ensure_keybindings_file(tmp.path());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "navigate-next x\n");
    }

    #[test]
    fn test_parse_meta_directive_cmd() {
        let result = parse_keybindings("meta cmd\n");
        assert_eq!(result.meta_key, "cmd");
        assert!(result.overrides.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_meta_directive_ctrl() {
        let result = parse_keybindings("meta ctrl\n");
        assert_eq!(result.meta_key, "ctrl");
    }

    #[test]
    fn test_parse_meta_directive_invalid_warns() {
        let result = parse_keybindings("meta alt\n");
        assert_eq!(result.meta_key, "cmd"); // stays default
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("invalid meta value"));
    }

    #[test]
    fn test_parse_meta_default_is_cmd() {
        let result = parse_keybindings("navigate-next h\n");
        assert_eq!(result.meta_key, "cmd");
    }

    #[test]
    fn test_parse_meta_with_overrides() {
        let content = "meta ctrl\nscreenshot Meta+x\n";
        let result = parse_keybindings(content);
        assert_eq!(result.meta_key, "ctrl");
        assert_eq!(
            result.overrides.get("screenshot"),
            Some(&"Meta+x".to_string())
        );
    }

    #[test]
    fn test_external_command_bare_key_warns() {
        for cmd in META_REQUIRED_COMMANDS {
            let content = format!("{cmd} x\n");
            let result = parse_keybindings(&content);
            // Override is still recorded so the user sees their config
            assert_eq!(result.overrides.get(*cmd).unwrap(), "x");
            // But a warning is emitted
            assert_eq!(
                result.warnings.len(),
                1,
                "expected exactly one warning for bare-key {cmd}"
            );
            assert!(
                result.warnings[0].contains("requires Meta+ prefix"),
                "warning should mention Meta+ prefix for {cmd}: {}",
                result.warnings[0]
            );
        }
    }

    #[test]
    fn test_external_command_meta_key_no_warning() {
        let content = "screenshot Meta+x\nscreenshot-cropped Meta+d\n";
        let result = parse_keybindings(content);
        assert!(
            result.warnings.is_empty(),
            "Meta+ prefixed external commands should not warn"
        );
    }

    #[test]
    fn test_external_command_legacy_symbol_no_warning() {
        let content = "screenshot ⌘x\nscreenshot-cropped ⌘d\n";
        let result = parse_keybindings(content);
        assert!(
            result.warnings.is_empty(),
            "⌘-prefixed external commands should not warn"
        );
    }
}
