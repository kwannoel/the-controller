use std::collections::{BinaryHeap, HashSet};
use std::io::{BufReader, Read};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::emitter::EventEmitter;

pub(crate) const MAX_TOP_LEVEL_DIRECTORIES: usize = 6;
pub(crate) const MAX_EVIDENCE_FILES: usize = 8;
const MAX_SNIPPET_LINES: usize = 24;
const MAX_SNIPPET_CHARS: usize = 1_200;
const CODEX_EXEC_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_CAPTURE_BYTES: usize = 256 * 1024;
const CODEX_SANDBOX_MODE: &str = "workspace-write";
const DEFAULT_SCAN_LIMITS: ScanLimits = ScanLimits {
    max_entries: 512,
    max_depth: 6,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScanLimits {
    max_entries: usize,
    max_depth: usize,
}

#[derive(Debug)]
struct ScanBudget {
    remaining_entries: usize,
}

#[derive(Debug)]
struct PipeCapture {
    bytes: Vec<u8>,
    overflowed: bool,
}

struct IsolatedExecDir(PathBuf);

impl IsolatedExecDir {
    fn create() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "the-controller-architecture-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&path).map_err(|e| {
            format!(
                "Failed to create isolated codex dir {}: {}",
                path.display(),
                e
            )
        })?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for IsolatedExecDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(unix)]
const SIGKILL: i32 = 9;

impl ScanBudget {
    fn new(limits: ScanLimits) -> Self {
        Self {
            remaining_entries: limits.max_entries,
        }
    }

    fn remaining_entries(&self) -> usize {
        self.remaining_entries
    }

    fn try_take_entry(&mut self) -> bool {
        if self.remaining_entries == 0 {
            return false;
        }
        self.remaining_entries -= 1;
        true
    }
}

#[derive(Clone, Debug)]
struct CodexExecConfig {
    binary: PathBuf,
    timeout: Duration,
}

impl Default for CodexExecConfig {
    fn default() -> Self {
        Self {
            binary: std::env::var_os("THE_CONTROLLER_CODEX_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("codex")),
            timeout: CODEX_EXEC_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoEvidence {
    pub top_level_directories: Vec<String>,
    pub files: Vec<RepoEvidenceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoEvidenceFile {
    pub path: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureResult {
    pub title: String,
    pub mermaid: String,
    pub components: Vec<ArchitectureComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureComponent {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub contains: Vec<String>,
    #[serde(default)]
    pub incoming_relationships: Vec<ArchitectureRelationship>,
    #[serde(default)]
    pub outgoing_relationships: Vec<ArchitectureRelationship>,
    #[serde(default)]
    pub evidence_paths: Vec<String>,
    #[serde(default)]
    pub evidence_snippets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureRelationship {
    #[serde(default, alias = "target", alias = "target_id", alias = "id")]
    pub component_id: String,
    #[serde(default)]
    pub summary: String,
}

pub fn collect_repo_evidence(repo_path: &Path) -> Result<RepoEvidence, String> {
    tracing::debug!(path = %repo_path.display(), "collecting repo evidence");
    collect_repo_evidence_with_limits(repo_path, DEFAULT_SCAN_LIMITS)
}

fn collect_repo_evidence_with_limits(
    repo_path: &Path,
    scan_limits: ScanLimits,
) -> Result<RepoEvidence, String> {
    if !repo_path.is_dir() {
        tracing::warn!(path = %repo_path.display(), "not a directory");
        return Err(format!("Not a directory: {}", repo_path.display()));
    }
    let repo_path = repo_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve {}: {}", repo_path.display(), e))?;
    let mut scan_budget = ScanBudget::new(scan_limits);
    let root_entries = read_root_entries(&repo_path, &mut scan_budget)?;
    tracing::debug!(count = root_entries.len(), "read root entries");
    let top_level_directories = root_entries
        .iter()
        .filter(|path| path_kind(path) == Some(RepoPathKind::Directory))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            (!is_ignored_dir(name)).then(|| name.to_string())
        })
        .collect::<Vec<_>>();

    let mut top_level_directories = top_level_directories;
    top_level_directories.sort_by(|left, right| {
        preferred_directory_rank(left)
            .cmp(&preferred_directory_rank(right))
            .then_with(|| left.cmp(right))
    });
    top_level_directories.truncate(MAX_TOP_LEVEL_DIRECTORIES);

    let mut files = Vec::new();
    let mut seen_paths = HashSet::new();

    let mut metadata_paths = root_entries
        .iter()
        .filter(|path| path_kind(path) == Some(RepoPathKind::File))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            is_metadata_file(name).then(|| path.to_path_buf())
        })
        .collect::<Vec<_>>();
    metadata_paths.sort_by(|left, right| {
        metadata_file_rank(left)
            .cmp(&metadata_file_rank(right))
            .then_with(|| relative_path(&repo_path, left).cmp(&relative_path(&repo_path, right)))
    });

    for path in metadata_paths {
        if files.len() >= MAX_EVIDENCE_FILES {
            break;
        }
        push_evidence_file(&mut files, &mut seen_paths, &repo_path, &path);
    }

    let mut root_source_paths = root_entries
        .iter()
        .filter(|path| path_kind(path) == Some(RepoPathKind::File))
        .filter(|path| is_source_file(path))
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>();
    root_source_paths.sort_by(|left, right| {
        source_file_rank(&repo_path, left).cmp(&source_file_rank(&repo_path, right))
    });

    for path in root_source_paths {
        if files.len() >= MAX_EVIDENCE_FILES {
            break;
        }
        push_evidence_file(&mut files, &mut seen_paths, &repo_path, &path);
    }

    for directory in &top_level_directories {
        if files.len() >= MAX_EVIDENCE_FILES {
            break;
        }

        let path = repo_path.join(directory);
        if let Some(file) =
            best_source_file_in_dir(&repo_path, &path, scan_limits, &mut scan_budget, 0)?
        {
            push_evidence_file(&mut files, &mut seen_paths, &repo_path, &file);
        }
    }

    tracing::debug!(
        directories = top_level_directories.len(),
        evidence_files = files.len(),
        budget_remaining = scan_budget.remaining_entries(),
        "repo evidence collection complete"
    );
    Ok(RepoEvidence {
        top_level_directories,
        files,
    })
}

pub fn build_architecture_prompt(repo_path: &Path, evidence: &RepoEvidence) -> String {
    let repo_name = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository");
    let directories = if evidence.top_level_directories.is_empty() {
        "- none captured".to_string()
    } else {
        evidence
            .top_level_directories
            .iter()
            .map(|dir| format!("- {dir}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let file_sections = if evidence.files.is_empty() {
        "No file evidence was captured.".to_string()
    } else {
        evidence
            .files
            .iter()
            .map(|file| {
                format!(
                    "### {}\nSnippet lines:\n{}",
                    file.path,
                    format_prompt_snippet(&file.snippet)
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "Analyze the repository \"{repo_name}\" using only the bounded evidence below.\n\
Return exactly one JSON object with these top-level keys: \"title\", \"mermaid\", and \"components\".\n\
Requirements:\n\
- \"title\" must be a short architecture title.\n\
- \"mermaid\" must be a valid Mermaid flowchart using stable component ids.\n\
- \"components\" must be an array of objects with: id, name, summary, contains, incoming_relationships, outgoing_relationships, evidence_paths, evidence_snippets.\n\
- \"contains\" must be an array of component ids (from this same components array) that are children of the component, or an empty array if the component has no children.\n\
- Each entry in incoming_relationships and outgoing_relationships must be an object with exactly two keys: \"component_id\" (the id of the related component) and \"summary\" (a short description of the relationship).\n\
- Every component id must appear as a Mermaid node id.\n\
- evidence_paths and evidence_snippets must cite only the files shown below.\n\
- Output JSON only. No prose, no markdown fences.\n\n\
Top-level directories:\n{directories}\n\n\
Repository evidence:\n{file_sections}\n"
    )
}

fn format_prompt_snippet(snippet: &str) -> String {
    let snippet = snippet.trim_end();
    if snippet.is_empty() {
        return "|".to_string();
    }

    snippet
        .lines()
        .map(|line| format!("| {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn generate_architecture_blocking(repo_path: &Path) -> Result<ArchitectureResult, String> {
    generate_architecture_blocking_with_emitter(repo_path, &crate::emitter::NoopEmitter::new())
}

pub fn generate_architecture_blocking_with_emitter(
    repo_path: &Path,
    emitter: &Arc<dyn EventEmitter>,
) -> Result<ArchitectureResult, String> {
    let config = &CodexExecConfig::default();
    generate_architecture_blocking_with_emitter_and_config(repo_path, config, emitter)
}

fn generate_architecture_blocking_with_emitter_and_config(
    repo_path: &Path,
    config: &CodexExecConfig,
    emitter: &Arc<dyn EventEmitter>,
) -> Result<ArchitectureResult, String> {
    let _ = emitter.emit("architecture-log", "Scanning repository for evidence…");
    let evidence = collect_repo_evidence(repo_path)?;
    if evidence.files.is_empty() {
        tracing::warn!(path = %repo_path.display(), "no usable evidence files found");
        return Err(
            "No usable evidence files were captured for architecture generation".to_string(),
        );
    }
    let _ = emitter.emit(
        "architecture-log",
        &format!(
            "Collected {} evidence files from {} directories",
            evidence.files.len(),
            evidence.top_level_directories.len()
        ),
    );
    for file in &evidence.files {
        let _ = emitter.emit("architecture-log", &format!("  {}", file.path));
    }

    let _ = emitter.emit("architecture-log", "Building prompt…");
    let prompt = build_architecture_prompt(repo_path, &evidence);

    let _ = emitter.emit(
        "architecture-log",
        &format!(
            "Running codex exec (timeout {}s)…",
            config.timeout.as_secs()
        ),
    );
    let output = run_codex_exec(&prompt, config)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(status = ?output.status, "codex exec returned non-zero exit status");
        return Err(format!("codex exec failed: {}", stderr.trim()));
    }

    let _ = emitter.emit("architecture-log", "Parsing architecture output…");
    let result = parse_architecture_output_with_evidence(
        &String::from_utf8_lossy(&output.stdout),
        &evidence,
    )?;
    let _ = emitter.emit(
        "architecture-log",
        &format!("Done — {} components generated", result.components.len()),
    );
    Ok(result)
}

#[cfg(test)]
fn generate_architecture_blocking_with_config(
    repo_path: &Path,
    config: &CodexExecConfig,
) -> Result<ArchitectureResult, String> {
    generate_architecture_blocking_with_emitter_and_config(
        repo_path,
        config,
        &crate::emitter::NoopEmitter::new(),
    )
}

fn run_codex_exec(prompt: &str, config: &CodexExecConfig) -> Result<Output, String> {
    let exec_dir = IsolatedExecDir::create()?;
    let last_message_path = exec_dir.path().join("codex-last-message.txt");
    tracing::debug!(binary = %config.binary.display(), timeout_secs = config.timeout.as_secs(), "spawning codex exec");
    let mut command = Command::new(&config.binary);
    command
        .arg("exec")
        .arg("--sandbox")
        .arg(CODEX_SANDBOX_MODE)
        .arg("--skip-git-repo-check")
        .arg("--output-last-message")
        .arg(&last_message_path)
        .arg(prompt)
        .current_dir(exec_dir.path())
        .env_remove("CLAUDECODE");
    #[cfg(unix)]
    command.process_group(0);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| {
            tracing::error!(binary = %config.binary.display(), error = %e, "failed to spawn codex exec");
            format!("Failed to run codex exec: {}", e)
        })?;
    let stdout_reader = child
        .stdout
        .take()
        .ok_or("Failed to capture codex exec stdout".to_string())?;
    let stderr_reader = child
        .stderr
        .take()
        .ok_or("Failed to capture codex exec stderr".to_string())?;
    let (overflow_tx, overflow_rx) = mpsc::channel();
    let stdout_handle = spawn_pipe_reader(stdout_reader, "stdout", overflow_tx.clone());
    let stderr_handle = spawn_pipe_reader(stderr_reader, "stderr", overflow_tx);
    let started_at = Instant::now();
    let status = loop {
        while let Ok(stream_name) = overflow_rx.try_recv() {
            if stream_name != "stdout" {
                tracing::error!(
                    stream = stream_name,
                    max_bytes = MAX_CAPTURE_BYTES,
                    "codex exec output overflow"
                );
                terminate_codex_process(&mut child);
                let _ = child.wait();
                return Err(format!(
                    "codex exec {} exceeded {} bytes of output",
                    stream_name, MAX_CAPTURE_BYTES
                ));
            }
        }

        if started_at.elapsed() >= config.timeout {
            tracing::error!(
                timeout_secs = config.timeout.as_secs(),
                "codex exec timed out"
            );
            terminate_codex_process(&mut child);
            let _ = child.wait();
            return Err(format!(
                "codex exec timed out after {} seconds",
                config.timeout.as_secs_f32()
            ));
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("Failed to wait for codex exec: {}", e))?
        {
            break status;
        }

        std::thread::sleep(Duration::from_millis(50));
    };

    let stdout = join_pipe_reader(stdout_handle, "stdout")?;
    let stderr = join_pipe_reader(stderr_handle, "stderr")?;
    tracing::debug!(
        status = ?status,
        stdout_bytes = stdout.bytes.len(),
        stderr_bytes = stderr.bytes.len(),
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "codex exec finished"
    );
    if stdout.overflowed && !status.success() {
        return Err(format!(
            "codex exec stdout exceeded {} bytes of output",
            MAX_CAPTURE_BYTES
        ));
    }
    if stderr.overflowed {
        return Err(format!(
            "codex exec stderr exceeded {} bytes of output",
            MAX_CAPTURE_BYTES
        ));
    }

    let stdout = if status.success() {
        std::fs::read(&last_message_path).map_err(|e| {
            format!(
                "Failed to read codex exec last message {}: {}",
                last_message_path.display(),
                e
            )
        })?
    } else {
        stdout.bytes
    };

    Ok(Output {
        status,
        stdout,
        stderr: stderr.bytes,
    })
}

fn terminate_codex_process(child: &mut std::process::Child) {
    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(child.id()) {
        // `process_group(0)` puts the subprocess in its own group, so negative pid targets its subtree.
        unsafe {
            let _ = kill(-pid, SIGKILL);
        }
    }

    // Non-Unix platforms fall back to killing the direct child only.
    let _ = child.kill();
}
pub fn extract_json(output: &str) -> Option<&str> {
    if let Some(start) = output.find("```json") {
        let json_start = start + "```json".len();
        if let Some(end) = output[json_start..].find("```") {
            return Some(output[json_start..json_start + end].trim());
        }
    }

    if let Some(start) = output.find('{') {
        if let Some(end) = output.rfind('}') {
            if end >= start {
                return Some(&output[start..=end]);
            }
        }
    }

    None
}

pub fn parse_architecture_output(output: &str) -> Result<ArchitectureResult, String> {
    parse_architecture_output_with_evidence(
        output,
        &RepoEvidence {
            top_level_directories: Vec::new(),
            files: Vec::new(),
        },
    )
}

fn parse_architecture_output_with_evidence(
    output: &str,
    evidence: &RepoEvidence,
) -> Result<ArchitectureResult, String> {
    let json = extract_json(output).ok_or_else(|| {
        tracing::error!(output_len = output.len(), "no JSON found in codex output");
        "No JSON found in output".to_string()
    })?;
    tracing::debug!(json_len = json.len(), "extracted JSON from codex output");
    let parsed: ArchitectureResult = serde_json::from_str(json).map_err(|e| {
        tracing::error!(error = %e, "failed to deserialize architecture JSON");
        format!("Failed to parse JSON: {}", e)
    })?;
    tracing::debug!(
        components = parsed.components.len(),
        "deserialized architecture result"
    );
    sanitize_architecture_result(parsed, evidence)
}

fn sanitize_architecture_result(
    result: ArchitectureResult,
    evidence: &RepoEvidence,
) -> Result<ArchitectureResult, String> {
    let title = result.title.trim().to_string();
    if title.is_empty() {
        tracing::warn!("architecture title is empty");
        return Err("Architecture title cannot be empty".to_string());
    }

    let mermaid = result.mermaid.trim().to_string();
    if mermaid.is_empty() {
        tracing::warn!("architecture mermaid diagram is empty");
        return Err("Architecture Mermaid cannot be empty".to_string());
    }

    let mermaid_node_ids = extract_mermaid_node_ids(&mermaid);
    tracing::debug!(
        node_count = mermaid_node_ids.len(),
        "extracted mermaid node ids"
    );
    let evidence_paths = evidence
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let mut components = Vec::with_capacity(result.components.len());
    for (component_index, component) in result.components.into_iter().enumerate() {
        components.push(sanitize_component(
            component,
            component_index,
            &mermaid_node_ids,
            &evidence_paths,
            &evidence.files,
        )?);
    }
    validate_component_references(&components)?;
    if !evidence.files.is_empty()
        && components.iter().all(|component| {
            component.evidence_paths.is_empty() && component.evidence_snippets.is_empty()
        })
    {
        tracing::warn!("architecture result has no grounded evidence");
        return Err("Architecture result must include grounded evidence".to_string());
    }

    tracing::debug!(
        components = components.len(),
        "architecture result sanitized"
    );
    Ok(ArchitectureResult {
        title,
        mermaid,
        components,
    })
}

fn sanitize_component(
    component: ArchitectureComponent,
    component_index: usize,
    mermaid_node_ids: &HashSet<String>,
    evidence_paths: &HashSet<&str>,
    evidence_files: &[RepoEvidenceFile],
) -> Result<ArchitectureComponent, String> {
    let id = component.id.trim().to_string();
    if id.is_empty() {
        return Err(format!(
            "Invalid component at index {}: missing id",
            component_index
        ));
    }
    if !mermaid_node_ids.contains(&id) {
        return Err(format!("Mermaid is missing node id for component '{}'", id));
    }

    let name = component.name.trim().to_string();
    if name.is_empty() {
        return Err(format!(
            "Invalid component '{}' at index {}: missing name",
            id, component_index
        ));
    }

    let summary = component.summary.trim().to_string();
    if summary.is_empty() {
        return Err(format!(
            "Invalid component '{}' at index {}: missing summary",
            id, component_index
        ));
    }

    let grounded_paths = grounded_evidence_paths(component.evidence_paths, evidence_paths);
    let grounded_snippets =
        grounded_evidence_snippets(component.evidence_snippets, &grounded_paths, evidence_files);

    Ok(ArchitectureComponent {
        id,
        name,
        summary,
        contains: trim_string_list(component.contains),
        incoming_relationships: sanitize_relationships(
            component.incoming_relationships,
            component_index,
            "incoming",
        )?,
        outgoing_relationships: sanitize_relationships(
            component.outgoing_relationships,
            component_index,
            "outgoing",
        )?,
        evidence_paths: grounded_paths,
        evidence_snippets: grounded_snippets,
    })
}

fn sanitize_relationships(
    relationships: Vec<ArchitectureRelationship>,
    component_index: usize,
    direction: &str,
) -> Result<Vec<ArchitectureRelationship>, String> {
    relationships
        .into_iter()
        .enumerate()
        .map(|(relationship_index, relationship)| {
            let component_id = relationship.component_id.trim().to_string();
            if component_id.is_empty() {
                return Err(format!(
                    "Invalid {} relationship at component index {} relationship index {}: missing component id",
                    direction, component_index, relationship_index
                ));
            }

            let summary = relationship.summary.trim().to_string();
            if summary.is_empty() {
                return Err(format!(
                    "Invalid {} relationship at component index {} relationship index {}: missing summary",
                    direction, component_index, relationship_index
                ));
            }

            Ok(ArchitectureRelationship {
                component_id,
                summary,
            })
        })
        .collect()
}

fn trim_string_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn grounded_evidence_paths(values: Vec<String>, allowed_paths: &HashSet<&str>) -> Vec<String> {
    let mut seen = HashSet::new();

    trim_string_list(values)
        .into_iter()
        .filter(|value| allowed_paths.contains(value.as_str()))
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn grounded_evidence_snippets(
    values: Vec<String>,
    grounded_paths: &[String],
    evidence_files: &[RepoEvidenceFile],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let grounded_path_set = grounded_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    trim_string_list(values)
        .into_iter()
        .filter(|value| {
            evidence_files
                .iter()
                .filter(|file| grounded_path_set.contains(file.path.as_str()))
                .any(|file| snippet_matches_line_window(value, &file.snippet))
        })
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn snippet_matches_line_window(candidate: &str, evidence_snippet: &str) -> bool {
    let candidate_lines = normalized_nonempty_lines(candidate);
    let evidence_lines = normalized_nonempty_lines(evidence_snippet);

    !candidate_lines.is_empty()
        && candidate_lines.len() <= evidence_lines.len()
        && evidence_lines
            .windows(candidate_lines.len())
            .any(|window| window == candidate_lines.as_slice())
}

fn normalized_nonempty_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(normalize_line)
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalize_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_component_references(components: &[ArchitectureComponent]) -> Result<(), String> {
    let mut component_ids = HashSet::with_capacity(components.len());
    for component in components {
        if !component_ids.insert(component.id.as_str()) {
            return Err(format!("Duplicate component id '{}'", component.id));
        }
    }

    for component in components {
        for contained_component_id in &component.contains {
            if !component_ids.contains(contained_component_id.as_str()) {
                return Err(format!(
                    "Component '{}' contains unknown component '{}'",
                    component.id, contained_component_id
                ));
            }
        }

        validate_relationship_targets(
            &component.id,
            &component.incoming_relationships,
            "incoming",
            &component_ids,
        )?;
        validate_relationship_targets(
            &component.id,
            &component.outgoing_relationships,
            "outgoing",
            &component_ids,
        )?;
    }

    Ok(())
}

fn validate_relationship_targets(
    component_id: &str,
    relationships: &[ArchitectureRelationship],
    direction: &str,
    component_ids: &HashSet<&str>,
) -> Result<(), String> {
    for relationship in relationships {
        if !component_ids.contains(relationship.component_id.as_str()) {
            return Err(format!(
                "Component '{}' has {} relationship to unknown component '{}'",
                component_id, direction, relationship.component_id
            ));
        }
    }

    Ok(())
}

fn extract_mermaid_node_ids(mermaid: &str) -> HashSet<String> {
    let mut ids = HashSet::new();

    for line in mermaid.lines() {
        ids.extend(extract_mermaid_node_ids_from_line(line));
    }

    ids
}

fn extract_mermaid_node_ids_from_line(line: &str) -> HashSet<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("%%")
        || matches!(
            trimmed.split_whitespace().next(),
            Some(
                "flowchart"
                    | "graph"
                    | "subgraph"
                    | "end"
                    | "classDef"
                    | "class"
                    | "style"
                    | "linkStyle"
                    | "click"
            )
        )
    {
        return HashSet::new();
    }

    let mut ids = HashSet::new();
    let mut index = 0;

    if let Some((id, next_index)) = parse_mermaid_node_reference(trimmed, index) {
        ids.insert(id);
        index = next_index;
    } else {
        return ids;
    }

    while let Some(next_index) = consume_mermaid_edge(trimmed, index) {
        index = next_index;
        if let Some((id, next_index)) = parse_mermaid_node_reference(trimmed, index) {
            ids.insert(id);
            index = next_index;
        } else {
            break;
        }
    }

    ids
}

fn parse_mermaid_node_reference(line: &str, start: usize) -> Option<(String, usize)> {
    let bytes = line.as_bytes();
    let mut index = skip_ascii_whitespace(bytes, start);

    if index >= bytes.len() || !is_identifier_char(bytes[index]) {
        return None;
    }

    let id_start = index;
    index += 1;
    while index < bytes.len() && is_identifier_char(bytes[index]) {
        index += 1;
    }

    let id = line[id_start..index].to_string();
    let mut cursor = skip_ascii_whitespace(bytes, index);
    if cursor < bytes.len() && matches!(bytes[cursor], b'[' | b'(' | b'{') {
        cursor = consume_balanced_delimiters(bytes, cursor);
    }

    Some((id, cursor))
}

fn consume_mermaid_edge(line: &str, start: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = skip_ascii_whitespace(bytes, start);
    let edge_start = index;

    // An edge may begin with 'o' or 'x' (e.g. `o--o`, `x--x`), but only
    // if immediately followed by a core edge character.
    if index < bytes.len()
        && is_mermaid_edge_terminator(bytes[index])
        && index + 1 < bytes.len()
        && is_mermaid_edge_char(bytes[index + 1])
    {
        index += 1;
    }

    while index < bytes.len() && is_mermaid_edge_char(bytes[index]) {
        index += 1;
    }

    // Must have consumed at least one core edge character.
    if index == edge_start {
        return None;
    }

    // An edge may end with 'o' or 'x' (e.g. `--o`, `--x`), but only when
    // not followed by another identifier character (otherwise it's the start
    // of a node id like `output`).
    if index < bytes.len() && is_mermaid_edge_terminator(bytes[index]) {
        let after = index + 1;
        if after >= bytes.len() || !is_identifier_char(bytes[after]) {
            index += 1;
        }
    }

    // Require at least one core edge character was consumed (not just terminators).
    let has_core = (edge_start..index).any(|i| is_mermaid_edge_char(bytes[i]));
    if !has_core {
        return None;
    }

    index = skip_ascii_whitespace(bytes, index);
    if index < bytes.len() && bytes[index] == b'|' {
        index += 1;
        while index < bytes.len() && bytes[index] != b'|' {
            index += 1;
        }
        if index < bytes.len() {
            index += 1;
        }
    }

    Some(skip_ascii_whitespace(bytes, index))
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    index
}

fn consume_balanced_delimiters(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    let mut depth = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }

    bytes.len()
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn is_mermaid_edge_char(byte: u8) -> bool {
    matches!(byte, b'-' | b'.' | b'=' | b'<' | b'>')
}

/// Returns true if `byte` is `o` or `x`, which are valid Mermaid edge
/// terminators (e.g. `--o`, `--x`, `o--o`, `x--x`) but only when they
/// appear at the boundary of an edge — not when followed by another
/// identifier character (which would indicate the start of a node id).
fn is_mermaid_edge_terminator(byte: u8) -> bool {
    matches!(byte, b'o' | b'x')
}

fn read_sorted_dir(path: &Path, scan_budget: &mut ScanBudget) -> Result<Vec<PathBuf>, String> {
    if scan_budget.remaining_entries() == 0 {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path());
    Ok(sort_entries_with_budget(entries, scan_budget))
}

fn read_root_entries(path: &Path, scan_budget: &mut ScanBudget) -> Result<Vec<PathBuf>, String> {
    if scan_budget.remaining_entries() == 0 {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path());
    Ok(retain_best_entries(
        entries,
        scan_budget,
        root_entry_sort_key,
    ))
}

fn root_entry_sort_key(path: &Path) -> (usize, usize, String) {
    let (bucket_rank, item_rank) = root_entry_rank(path);
    (
        bucket_rank,
        item_rank,
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

fn sort_entries_with_budget<I>(entries: I, scan_budget: &mut ScanBudget) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    retain_best_entries(entries, scan_budget, |path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    })
}

fn retain_best_entries<I, K>(
    entries: I,
    scan_budget: &mut ScanBudget,
    mut sort_key: impl FnMut(&Path) -> K,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
    K: Ord,
{
    let limit = scan_budget.remaining_entries();
    if limit == 0 {
        return Vec::new();
    }

    let mut retained = BinaryHeap::with_capacity(limit);
    for path in entries {
        let candidate = (sort_key(&path), path);
        if retained.len() < limit {
            retained.push(candidate);
            continue;
        }

        if retained
            .peek()
            .is_some_and(|worst| (&candidate.0, &candidate.1) < (&worst.0, &worst.1))
        {
            retained.pop();
            retained.push(candidate);
        }
    }

    let mut retained = retained.into_vec();
    retained.sort();
    let selected = retained.len();
    for _ in 0..selected {
        let _ = scan_budget.try_take_entry();
    }
    retained.into_iter().map(|(_, path)| path).collect()
}

fn root_entry_rank(path: &Path) -> (usize, usize) {
    match path_kind(path) {
        Some(RepoPathKind::File) => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if is_metadata_file(name) {
                (0, metadata_file_rank(path))
            } else if is_source_file(path) {
                let basename = name.split('.').next().unwrap_or(name);
                let rank = match basename {
                    "main" => 0,
                    "app" => 1,
                    "index" => 2,
                    "server" => 3,
                    "lib" => 4,
                    "mod" => 5,
                    _ => 10,
                };
                (2, rank)
            } else {
                (4, 0)
            }
        }
        Some(RepoPathKind::Directory) => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if is_ignored_dir(name) {
                (5, 0)
            } else {
                (1, preferred_directory_rank(name))
            }
        }
        None => (6, 0),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RepoPathKind {
    Directory,
    File,
}

fn path_kind(path: &Path) -> Option<RepoPathKind> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return None;
    }
    if file_type.is_dir() {
        Some(RepoPathKind::Directory)
    } else if file_type.is_file() {
        Some(RepoPathKind::File)
    } else {
        None
    }
}

fn push_evidence_file(
    files: &mut Vec<RepoEvidenceFile>,
    seen_paths: &mut HashSet<String>,
    repo_path: &Path,
    path: &Path,
) {
    let Some(relative) = relative_path(repo_path, path) else {
        return;
    };
    if !seen_paths.insert(relative.clone()) {
        return;
    }

    if let Some(snippet) = read_text_snippet(path) {
        files.push(RepoEvidenceFile {
            path: relative,
            snippet,
        });
    }
}

fn read_text_snippet(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut snippet = String::new();
    let mut char_buffer = Vec::with_capacity(4);
    let mut line_count = 0usize;
    let mut char_count = 0usize;

    for byte in BufReader::new(file).bytes() {
        let byte = byte.ok()?;
        char_buffer.push(byte);

        let decoded = match std::str::from_utf8(&char_buffer) {
            Ok(decoded) => decoded,
            Err(error) if error.error_len().is_none() && char_buffer.len() < 4 => continue,
            Err(_) => break,
        };

        for ch in decoded.chars() {
            if char_count >= MAX_SNIPPET_CHARS || line_count >= MAX_SNIPPET_LINES {
                break;
            }
            snippet.push(ch);
            char_count += 1;
            if ch == '\n' {
                line_count += 1;
            }
        }
        char_buffer.clear();

        if char_count >= MAX_SNIPPET_CHARS || line_count >= MAX_SNIPPET_LINES {
            break;
        }
    }

    let snippet = snippet.trim().to_string();
    (!snippet.is_empty()).then_some(snippet)
}

fn spawn_pipe_reader<T>(
    reader: T,
    stream_name: &'static str,
    overflow_tx: mpsc::Sender<&'static str>,
) -> std::thread::JoinHandle<std::io::Result<PipeCapture>>
where
    T: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut overflowed = false;

        loop {
            let bytes_read = reader.read(&mut chunk)?;
            if bytes_read == 0 {
                break;
            }

            let remaining = MAX_CAPTURE_BYTES.saturating_sub(buffer.len());
            if buffer.len() < MAX_CAPTURE_BYTES {
                let to_copy = remaining.min(bytes_read);
                buffer.extend_from_slice(&chunk[..to_copy]);
            }

            if !overflowed && bytes_read > remaining {
                overflowed = true;
                let _ = overflow_tx.send(stream_name);
            }
        }

        Ok(PipeCapture {
            bytes: buffer,
            overflowed,
        })
    })
}

fn join_pipe_reader(
    handle: std::thread::JoinHandle<std::io::Result<PipeCapture>>,
    stream_name: &str,
) -> Result<PipeCapture, String> {
    handle
        .join()
        .map_err(|_| format!("Failed to join codex exec {} reader thread", stream_name))?
        .map_err(|e| format!("Failed to read codex exec {}: {}", stream_name, e))
}

fn best_source_file_in_dir(
    repo_path: &Path,
    directory: &Path,
    scan_limits: ScanLimits,
    scan_budget: &mut ScanBudget,
    current_depth: usize,
) -> Result<Option<PathBuf>, String> {
    let mut best = None;
    collect_source_candidates(
        repo_path,
        directory,
        scan_limits,
        scan_budget,
        current_depth,
        &mut best,
    )?;
    Ok(best)
}

fn collect_source_candidates(
    repo_path: &Path,
    current_dir: &Path,
    scan_limits: ScanLimits,
    scan_budget: &mut ScanBudget,
    current_depth: usize,
    best_candidate: &mut Option<PathBuf>,
) -> Result<(), String> {
    for path in read_sorted_dir(current_dir, scan_budget)? {
        if path_kind(&path) == Some(RepoPathKind::Directory) {
            if current_depth >= scan_limits.max_depth {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if is_ignored_dir(name) {
                continue;
            }
            collect_source_candidates(
                repo_path,
                &path,
                scan_limits,
                scan_budget,
                current_depth + 1,
                best_candidate,
            )?;
            continue;
        }

        if path_kind(&path) == Some(RepoPathKind::File)
            && is_source_file(&path)
            && read_text_snippet(&path).is_some()
            && best_candidate.as_ref().is_none_or(|best| {
                source_file_rank(repo_path, &path) < source_file_rank(repo_path, best)
            })
        {
            *best_candidate = Some(path);
        }
    }

    Ok(())
}

fn preferred_directory_rank(name: &str) -> usize {
    match name {
        "src" => 0,
        "app" => 1,
        "apps" => 2,
        "web" => 3,
        "frontend" => 4,
        "backend" => 5,
        "server" => 6,
        "client" => 7,
        "lib" => 8,
        "packages" => 9,
        "crates" => 10,
        "cmd" => 11,
        "services" => 12,
        "scripts" => 20,
        "tests" => 40,
        "docs" => 50,
        _ => 30,
    }
}

fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules"
                | "target"
                | "dist"
                | "build"
                | "coverage"
                | "tmp"
                | "vendor"
                | ".git"
                | ".next"
                | "out"
        )
}

fn is_metadata_file(name: &str) -> bool {
    matches!(
        name,
        "README"
            | "README.md"
            | "README.txt"
            | "package.json"
            | "Cargo.toml"
            | "pyproject.toml"
            | "go.mod"
            | "pom.xml"
            | "Gemfile"
    )
}

fn metadata_file_rank(path: &Path) -> usize {
    match path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
    {
        "README.md" | "README" | "README.txt" => 0,
        "package.json" => 1,
        "Cargo.toml" => 2,
        "pyproject.toml" => 3,
        "go.mod" => 4,
        "pom.xml" => 5,
        "Gemfile" => 6,
        _ => 10,
    }
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "svelte"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "rb"
            | "php"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "scala"
            | "sh"
    )
}

fn source_file_rank(repo_path: &Path, path: &Path) -> (usize, usize, String) {
    let relative = relative_path(repo_path, path).unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let basename = file_name.split('.').next().unwrap_or(file_name);
    let basename_rank = match basename {
        "main" => 0,
        "app" => 1,
        "index" => 2,
        "server" => 3,
        "lib" => 4,
        "mod" => 5,
        _ => 10,
    };
    let depth = relative.matches('/').count();

    (basename_rank, depth, relative)
}

fn relative_path(repo_path: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(repo_path)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::{
        build_architecture_prompt, collect_repo_evidence, collect_repo_evidence_with_limits,
        generate_architecture_blocking_with_config, parse_architecture_output,
        parse_architecture_output_with_evidence, read_root_entries, read_sorted_dir,
        read_text_snippet, sort_entries_with_budget, CodexExecConfig, RepoEvidence,
        RepoEvidenceFile, ScanBudget, ScanLimits, MAX_CAPTURE_BYTES, MAX_EVIDENCE_FILES,
        MAX_SNIPPET_CHARS,
    };

    const BUSY_TEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn write_repo_file(repo: &TempDir, relative_path: &str, contents: &str) {
        let path = repo.path().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(path, contents).expect("write repo fixture file");
    }

    #[cfg(unix)]
    fn write_executable_script(temp_dir: &TempDir, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join(name);
        fs::write(&path, body).expect("write test script");
        let mut permissions = fs::metadata(&path).expect("stat script").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod script");
        path
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path, is_dir: bool) {
        if is_dir {
            std::os::unix::fs::symlink(target, link).expect("create directory symlink");
        } else {
            std::os::unix::fs::symlink(target, link).expect("create file symlink");
        }
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path, is_dir: bool) {
        if is_dir {
            std::os::windows::fs::symlink_dir(target, link).expect("create directory symlink");
        } else {
            std::os::windows::fs::symlink_file(target, link).expect("create file symlink");
        }
    }

    #[test]
    fn extracts_json_from_model_output_with_prose_and_fenced_code() {
        let output = r#"Here is the generated architecture.

```json
{
  "title": "Controller backend",
  "mermaid": "flowchart TD\napp[App]\nworker[Worker]\napp --> worker",
  "components": [
    {
      "id": "app",
      "name": "App",
      "summary": " Coordinates requests. ",
      "contains": [],
      "incoming_relationships": [],
      "outgoing_relationships": [
        {
          "component_id": "worker",
          "summary": " Dispatches work. "
        }
      ],
      "evidence_paths": [],
      "evidence_snippets": []
    },
    {
      "id": "worker",
      "name": " Worker ",
      "summary": " Runs jobs. ",
      "contains": [],
      "incoming_relationships": [
        {
          "component_id": "app",
          "summary": " Receives requests. "
        }
      ],
      "outgoing_relationships": [],
      "evidence_paths": [],
      "evidence_snippets": []
    }
  ]
}
```

That should be enough to render the view."#;

        let parsed = parse_architecture_output(output).expect("should parse");

        assert_eq!(parsed.title, "Controller backend");
        assert_eq!(parsed.components.len(), 2);
        assert_eq!(parsed.components[0].summary, "Coordinates requests.");
        assert_eq!(
            parsed.components[0].outgoing_relationships[0].summary,
            "Dispatches work."
        );
        assert_eq!(parsed.components[1].name, "Worker");
    }

    #[test]
    fn extracts_json_from_model_output_without_fenced_code_block() {
        let output = r#"Architecture summary:
{
  "title": "Controller backend",
  "mermaid": "flowchart TD\napi --> worker",
  "components": [
    {
      "id": "api",
      "name": "API",
      "summary": "Handles requests",
      "contains": [],
      "incoming_relationships": [],
      "outgoing_relationships": [
        {
          "component_id": "worker",
          "summary": "Sends jobs"
        }
      ],
      "evidence_paths": [],
      "evidence_snippets": []
    },
    {
      "id": "worker",
      "name": "Worker",
      "summary": "Runs jobs",
      "contains": [],
      "incoming_relationships": [
        {
          "component_id": "api",
          "summary": "Receives jobs"
        }
      ],
      "outgoing_relationships": [],
      "evidence_paths": [],
      "evidence_snippets": []
    }
  ]
}"#;

        let parsed = parse_architecture_output(output).expect("should parse unfenced JSON");

        assert_eq!(parsed.title, "Controller backend");
        assert_eq!(parsed.components.len(), 2);
    }

    #[test]
    fn build_architecture_prompt_formats_evidence_without_markdown_fences() {
        let repo = Path::new("/tmp/fenced-repo");
        let evidence = RepoEvidence {
            top_level_directories: vec!["docs".to_string()],
            files: vec![RepoEvidenceFile {
                path: "README.md".to_string(),
                snippet: "# README\n```bash\npnpm install\n```\n".to_string(),
            }],
        };

        let prompt = build_architecture_prompt(repo, &evidence);

        assert!(
            !prompt.contains("```text"),
            "prompt evidence should not open raw markdown fences"
        );
        assert!(prompt.contains("### README.md"));
        assert!(prompt.contains("Snippet lines:\n| # README\n| ```bash\n| pnpm install\n| ```"));
    }

    #[test]
    fn rejects_architecture_payloads_with_missing_component_ids() {
        let output = r#"{
          "title": "Broken architecture",
          "mermaid": "flowchart TD\napp[App]",
          "components": [
            {
              "id": "   ",
              "name": "App",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let error = parse_architecture_output(output).expect_err("missing id should fail");
        assert!(error.contains("component"));
        assert!(error.contains("id"));
    }

    #[test]
    fn accepts_relationship_with_target_alias_for_component_id() {
        let output = r#"{
          "title": "Aliased relationships",
          "mermaid": "flowchart TD\napi[API]\nworker[Worker]\napi --> worker",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [
                {
                  "target": "worker",
                  "summary": "Sends jobs"
                }
              ],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "worker",
              "name": "Worker",
              "summary": "Processes jobs",
              "contains": [],
              "incoming_relationships": [
                {
                  "target_id": "api",
                  "summary": "Receives jobs"
                }
              ],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let parsed = parse_architecture_output(output).expect("aliased fields should parse");
        assert_eq!(
            parsed.components[0].outgoing_relationships[0].component_id,
            "worker"
        );
        assert_eq!(
            parsed.components[1].incoming_relationships[0].component_id,
            "api"
        );
    }

    #[test]
    fn rejects_relationship_with_missing_component_id_and_no_alias() {
        let output = r#"{
          "title": "Missing relationship target",
          "mermaid": "flowchart TD\napi[API]\nworker[Worker]\napi --> worker",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [
                {
                  "summary": "Sends jobs"
                }
              ],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "worker",
              "name": "Worker",
              "summary": "Processes jobs",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let error = parse_architecture_output(output)
            .expect_err("missing component_id should fail in validation");
        assert!(
            error.contains("missing component id"),
            "error should mention missing component id: {}",
            error
        );
    }

    #[test]
    fn rejects_duplicate_component_ids() {
        let output = r#"{
          "title": "Duplicate ids",
          "mermaid": "flowchart TD\napi[API]\napi --> worker\nworker[Worker]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "api",
              "name": "Second API",
              "summary": "Also handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let error = parse_architecture_output(output).expect_err("duplicate ids should fail");

        assert!(error.contains("Duplicate"));
        assert!(error.contains("api"));
    }

    #[test]
    fn rejects_unresolved_component_references() {
        let contains_output = r#"{
          "title": "Broken contains",
          "mermaid": "flowchart TD\napi[API]\nworker[Worker]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": ["missing"],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "worker",
              "name": "Worker",
              "summary": "Runs jobs",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let contains_error =
            parse_architecture_output(contains_output).expect_err("missing contains should fail");
        assert!(contains_error.contains("contains"));
        assert!(contains_error.contains("missing"));

        let relationship_output = r#"{
          "title": "Broken relationships",
          "mermaid": "flowchart TD\napi[API]\nworker[Worker]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [
                {
                  "component_id": "ghost-in",
                  "summary": "Receives calls"
                }
              ],
              "outgoing_relationships": [
                {
                  "component_id": "ghost-out",
                  "summary": "Sends jobs"
                }
              ],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "worker",
              "name": "Worker",
              "summary": "Runs jobs",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let relationship_error = parse_architecture_output(relationship_output)
            .expect_err("missing relationship component ids should fail");
        assert!(relationship_error.contains("incoming") || relationship_error.contains("outgoing"));
        assert!(
            relationship_error.contains("ghost-in") || relationship_error.contains("ghost-out")
        );
    }

    #[test]
    fn accepts_edge_only_mermaid_when_component_ids_match_edge_endpoints() {
        let output = r#"{
          "title": "Edge only diagram",
          "mermaid": "flowchart TD\napi --> worker",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [
                {
                  "component_id": "worker",
                  "summary": "Sends jobs"
                }
              ],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "worker",
              "name": "Worker",
              "summary": "Runs jobs",
              "contains": [],
              "incoming_relationships": [
                {
                  "component_id": "api",
                  "summary": "Receives jobs"
                }
              ],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let parsed = parse_architecture_output(output).expect("edge-only mermaid should parse");

        assert_eq!(parsed.components.len(), 2);
    }

    #[test]
    fn rejects_mermaid_when_component_id_is_missing_from_node_ids() {
        let output = r#"{
          "title": "Broken diagram",
          "mermaid": "flowchart TD\napi[API]\nworker[Worker]\napi --> worker",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            },
            {
              "id": "db",
              "name": "Database",
              "summary": "Stores data",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;

        let error = parse_architecture_output(output).expect_err("mermaid mismatch should fail");
        assert!(error.contains("Mermaid"));
        assert!(error.contains("db"));
    }

    #[test]
    fn drops_ungrounded_component_evidence_without_repo_context() {
        let output = r#"{
          "title": "Architecture",
          "mermaid": "flowchart TD\napi[API]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": ["src/main.rs", "made/up.rs"],
              "evidence_snippets": ["fn main() {}", "invented snippet"]
            }
          ]
        }"#;

        let parsed = parse_architecture_output(output).expect("payload should parse");

        assert!(
            parsed.components[0].evidence_paths.is_empty(),
            "ungrounded evidence paths should be dropped"
        );
        assert!(
            parsed.components[0].evidence_snippets.is_empty(),
            "ungrounded evidence snippets should be dropped"
        );
    }

    #[test]
    fn collects_bounded_repo_evidence() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(
            &repo,
            "README.md",
            "# Example App\n\nA repo for architecture evidence collection.\n",
        );
        write_repo_file(
            &repo,
            "package.json",
            "{\n  \"name\": \"example-app\",\n  \"scripts\": { \"dev\": \"vite\" }\n}\n",
        );
        write_repo_file(
            &repo,
            "src/main.ts",
            "import { start } from './server';\nstart();\n",
        );
        write_repo_file(
            &repo,
            "src/server.ts",
            "export function start() {\n  return 'ok';\n}\n",
        );
        write_repo_file(&repo, "src/routes/api.ts", "export const route = '/api';\n");
        write_repo_file(&repo, "scripts/build.sh", "#!/bin/sh\necho build\n");
        write_repo_file(&repo, "docs/overview.md", "# Docs\n");
        write_repo_file(&repo, "tests/server.test.ts", "test('server', () => {});\n");
        write_repo_file(
            &repo,
            "node_modules/ignored/index.js",
            "module.exports = 'ignore me';\n",
        );
        write_repo_file(
            &repo,
            ".git/config",
            "[core]\nrepositoryformatversion = 0\n",
        );

        for index in 0..20 {
            write_repo_file(
                &repo,
                &format!("extra/file-{index}.ts"),
                &format!("export const file{index} = {index};\n"),
            );
        }

        let evidence = collect_repo_evidence(repo.path()).expect("collect evidence");

        assert!(
            evidence.files.len() <= MAX_EVIDENCE_FILES,
            "evidence should stay bounded"
        );
        assert!(
            evidence.files.iter().any(|file| file.path == "README.md"),
            "README should be included"
        );
        assert!(
            evidence
                .files
                .iter()
                .any(|file| file.path == "package.json"),
            "package metadata should be included"
        );
        assert!(
            evidence.files.iter().any(|file| file.path == "src/main.ts"),
            "representative source should be included"
        );
        assert!(
            !evidence
                .files
                .iter()
                .any(|file| file.path.starts_with("node_modules/")),
            "ignored directories should stay out of evidence"
        );
        assert!(
            !evidence
                .files
                .iter()
                .any(|file| file.path.starts_with(".git/")),
            "git metadata should stay out of evidence"
        );
        assert!(
            evidence
                .files
                .iter()
                .all(|file| !Path::new(&file.path).is_absolute()),
            "evidence paths should stay repo-relative"
        );
    }

    #[test]
    fn prefers_repo_landmarks_and_representative_source_files() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(
            &repo,
            "README.md",
            "# Preferred Repo\n\nThe readme should win over less important files.\n",
        );
        write_repo_file(
            &repo,
            "package.json",
            "{\n  \"name\": \"preferred-repo\",\n  \"private\": true\n}\n",
        );
        write_repo_file(
            &repo,
            "Cargo.toml",
            "[package]\nname = \"preferred-repo\"\nversion = \"0.1.0\"\n",
        );
        write_repo_file(
            &repo,
            "src/main.rs",
            "fn main() {\n    println!(\"hi\");\n}\n",
        );
        write_repo_file(&repo, "src/lib.rs", "pub fn serve() {}\n");
        write_repo_file(&repo, "web/app.ts", "export const app = true;\n");
        write_repo_file(&repo, "docs/architecture.md", "# Internal docs\n");

        let evidence = collect_repo_evidence(repo.path()).expect("collect evidence");
        let paths: Vec<&str> = evidence
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();

        assert_eq!(paths.first().copied(), Some("README.md"));
        assert!(
            paths.iter().position(|path| *path == "package.json")
                < paths
                    .iter()
                    .position(|path| path.starts_with("docs/"))
                    .or(Some(paths.len())),
            "package metadata should outrank docs"
        );
        assert!(
            evidence
                .top_level_directories
                .starts_with(&["src".to_string(), "web".to_string()]),
            "top-level source directories should be preferred"
        );
        assert!(
            paths.contains(&"src/main.rs"),
            "representative root source file should be included"
        );
        assert!(
            paths.contains(&"web/app.ts"),
            "representative file from another top-level directory should be included"
        );
    }

    #[test]
    fn includes_root_level_source_files_as_evidence_candidates() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(
            &repo,
            "Cargo.toml",
            "[package]\nname = \"root-source\"\nversion = \"0.1.0\"\n",
        );
        write_repo_file(
            &repo,
            "main.rs",
            "fn main() {\n    println!(\"root entrypoint\");\n}\n",
        );
        write_repo_file(&repo, "src/lib.rs", "pub fn serve() {}\n");

        let evidence = collect_repo_evidence(repo.path()).expect("collect evidence");
        let paths: Vec<&str> = evidence
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();

        assert!(
            paths.contains(&"main.rs"),
            "root-level source files should be considered evidence"
        );
    }

    #[test]
    fn ignores_symlinked_files_and_directories_when_collecting_evidence() {
        let repo = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside temp repo");

        write_repo_file(
            &outside,
            "README.md",
            "# Outside Repo\n\nThis file must never become evidence.\n",
        );
        write_repo_file(
            &outside,
            "src/main.rs",
            "fn main() {\n    println!(\"outside\");\n}\n",
        );

        write_repo_file(
            &repo,
            "package.json",
            "{\n  \"name\": \"safe-repo\",\n  \"private\": true\n}\n",
        );
        write_repo_file(&repo, "app/main.ts", "export const safe = true;\n");

        create_symlink(
            &outside.path().join("README.md"),
            &repo.path().join("README.md"),
            false,
        );
        create_symlink(&outside.path().join("src"), &repo.path().join("src"), true);

        let evidence = collect_repo_evidence(repo.path()).expect("collect evidence");

        assert!(
            !evidence
                .files
                .iter()
                .any(|file| file.path == "README.md" || file.path.starts_with("src/")),
            "symlink targets outside the repo should not be collected"
        );
        assert!(
            !evidence
                .top_level_directories
                .iter()
                .any(|dir| dir == "src"),
            "symlinked directories should not appear as top-level evidence"
        );
    }

    #[test]
    fn falls_back_when_best_ranked_source_file_has_no_usable_snippet() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(
            &repo,
            "Cargo.toml",
            "[package]\nname = \"snippet-fallback\"\nversion = \"0.1.0\"\n",
        );
        write_repo_file(&repo, "src/main.rs", "   \n\n\t");
        write_repo_file(&repo, "src/lib.rs", "pub fn serve() {}\n");

        let evidence = collect_repo_evidence(repo.path()).expect("collect evidence");

        assert!(
            evidence.files.iter().any(|file| file.path == "src/lib.rs"),
            "scanner should continue searching after an unusable top-ranked file"
        );
    }

    #[test]
    fn collect_repo_evidence_respects_scan_limits() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(
            &repo,
            "README.md",
            "# Scan budget\n\nKeep scanning bounded.\n",
        );
        write_repo_file(&repo, "src/level1/entry.ts", "export const one = 1;\n");
        write_repo_file(
            &repo,
            "src/level1/level2/deep.ts",
            "export const deep = 2;\n",
        );
        write_repo_file(&repo, "web/app.ts", "export const app = true;\n");

        let evidence = collect_repo_evidence_with_limits(
            repo.path(),
            ScanLimits {
                max_entries: 3,
                max_depth: 1,
            },
        )
        .expect("collect evidence");

        assert!(
            evidence
                .files
                .iter()
                .all(|file| file.path != "src/level1/level2/deep.ts"),
            "scan limits should prevent arbitrarily deep traversal"
        );
    }

    #[test]
    fn root_landmarks_are_prioritized_before_root_budget_is_spent() {
        let repo = TempDir::new().expect("create temp repo");
        write_repo_file(&repo, "a-noise.txt", "ignore me\n");
        write_repo_file(&repo, "b-noise/extra.ts", "export const noise = true;\n");
        write_repo_file(
            &repo,
            "package.json",
            "{\n  \"name\": \"root-priority\",\n  \"private\": true\n}\n",
        );
        write_repo_file(&repo, "src/main.ts", "export const app = true;\n");

        let evidence = collect_repo_evidence_with_limits(
            repo.path(),
            ScanLimits {
                max_entries: 2,
                max_depth: 2,
            },
        )
        .expect("collect evidence");

        assert!(
            evidence
                .files
                .iter()
                .any(|file| file.path == "package.json"),
            "root metadata landmarks should outrank alphabetical noise under tight budget"
        );
        assert!(
            evidence
                .top_level_directories
                .iter()
                .any(|dir| dir == "src"),
            "preferred source directories should survive root-entry budgeting"
        );
    }

    #[test]
    fn sort_entries_applies_budget_after_sorting() {
        let entries = vec![
            PathBuf::from("z-last.ts"),
            PathBuf::from("a-first.ts"),
            PathBuf::from("m-middle.ts"),
        ];
        let mut scan_budget = ScanBudget::new(ScanLimits {
            max_entries: 2,
            max_depth: 1,
        });

        let retained = sort_entries_with_budget(entries, &mut scan_budget);

        assert_eq!(
            retained,
            vec![PathBuf::from("a-first.ts"), PathBuf::from("m-middle.ts")]
        );
    }

    #[test]
    fn read_sorted_dir_skips_io_when_budget_is_exhausted() {
        let mut scan_budget = ScanBudget::new(ScanLimits {
            max_entries: 0,
            max_depth: 1,
        });

        let entries = read_sorted_dir(
            Path::new("/definitely/missing-sorted-dir"),
            &mut scan_budget,
        )
        .expect("exhausted budget should skip directory enumeration");

        assert!(entries.is_empty());
    }

    #[test]
    fn read_root_entries_skips_io_when_budget_is_exhausted() {
        let mut scan_budget = ScanBudget::new(ScanLimits {
            max_entries: 0,
            max_depth: 1,
        });

        let entries =
            read_root_entries(Path::new("/definitely/missing-root-dir"), &mut scan_budget)
                .expect("exhausted budget should skip root enumeration");

        assert!(entries.is_empty());
    }

    #[test]
    fn read_text_snippet_stops_before_invalid_bytes_beyond_cap() {
        let repo = TempDir::new().expect("create temp repo");
        let path = repo.path().join("large.txt");
        let mut bytes = vec![b'a'; MAX_SNIPPET_CHARS];
        bytes.push(0xff);
        bytes.extend_from_slice(b"rest of file");
        fs::write(&path, bytes).expect("write invalid utf8 fixture");

        let snippet = read_text_snippet(&path).expect("snippet should stop before invalid bytes");

        assert_eq!(snippet.len(), MAX_SNIPPET_CHARS);
        assert!(snippet.chars().all(|ch| ch == 'a'));
    }

    #[test]
    fn evidence_snippets_require_normalized_line_windows_from_cited_paths() {
        let output = r#"{
          "title": "Architecture",
          "mermaid": "flowchart TD\napi[API]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": ["src/main.rs"],
              "evidence_snippets": [
                "fn main() {\n println!(\"hi\");\n}",
                "run();",
                "println!(\"hi\")"
              ]
            }
          ]
        }"#;
        let evidence = RepoEvidence {
            top_level_directories: vec!["src".to_string()],
            files: vec![
                RepoEvidenceFile {
                    path: "src/main.rs".to_string(),
                    snippet: "fn main() {\n    println!(\"hi\");\n}\n".to_string(),
                },
                RepoEvidenceFile {
                    path: "src/lib.rs".to_string(),
                    snippet: "pub fn run() {\n    println!(\"lib\");\n}\n".to_string(),
                },
            ],
        };

        let parsed = parse_architecture_output_with_evidence(output, &evidence).expect("parse");

        assert_eq!(
            parsed.components[0].evidence_snippets,
            vec!["fn main() {\n println!(\"hi\");\n}".to_string()],
            "accepted snippets should be normalized line windows from cited evidence files only"
        );
    }

    #[test]
    fn rejects_architecture_results_with_no_grounded_evidence() {
        let output = r#"{
          "title": "Architecture",
          "mermaid": "flowchart TD\napi[API]",
          "components": [
            {
              "id": "api",
              "name": "API",
              "summary": "Handles requests",
              "contains": [],
              "incoming_relationships": [],
              "outgoing_relationships": [],
              "evidence_paths": [],
              "evidence_snippets": []
            }
          ]
        }"#;
        let evidence = RepoEvidence {
            top_level_directories: vec!["src".to_string()],
            files: vec![RepoEvidenceFile {
                path: "src/main.rs".to_string(),
                snippet: "fn main() {}\n".to_string(),
            }],
        };

        let error =
            parse_architecture_output_with_evidence(output, &evidence).expect_err("should fail");

        assert!(error.contains("grounded evidence"));
    }

    #[test]
    fn generate_architecture_command_uses_spawn_blocking() {
        let commands_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs");
        let source = fs::read_to_string(commands_path).expect("read commands source");
        let start = source
            .find("pub async fn generate_architecture")
            .expect("find generate_architecture");
        let rest = &source[start..];
        let end = rest
            .find("\n#[tauri::command]")
            .expect("find end of generate_architecture");
        let function_body = &rest[..end];

        assert!(
            function_body.contains("spawn_blocking"),
            "generate_architecture must offload repo scanning and codex exec with spawn_blocking"
        );
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_times_out_hung_codex_process() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Timeout test\n");
        let script = write_executable_script(&bin, "fake-codex.sh", "#!/bin/sh\nsleep 5\n");

        let started = Instant::now();
        let error = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: Duration::from_millis(100),
            },
        )
        .expect_err("hung codex process should time out");

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout should return promptly"
        );
        assert!(error.contains("timed out"));
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_runs_codex_outside_repo_with_tighter_sandbox() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Isolated exec test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            &format!(
                "#!/bin/sh\nif [ \"$PWD\" = \"{}\" ]; then\n  echo 'ran inside repo' >&2\n  exit 21\nfi\nif [ \"$1\" != \"exec\" ]; then\n  echo \"expected exec subcommand, got: $1\" >&2\n  exit 22\nfi\nif [ \"$2\" != \"--sandbox\" ] || [ \"$3\" != \"workspace-write\" ]; then\n  echo \"unexpected sandbox args: $2 $3\" >&2\n  exit 23\nfi\nif [ \"$4\" != \"--skip-git-repo-check\" ]; then\n  echo \"missing skip-git-repo-check: $4\" >&2\n  exit 24\nfi\nif [ \"$5\" != \"--output-last-message\" ] || [ -z \"$6\" ]; then\n  echo \"missing output-last-message contract: $5 $6\" >&2\n  exit 25\nfi\ncase \"$7\" in\n  *'Analyze the repository'* ) ;;\n  *)\n    echo 'prompt missing expected architecture instructions' >&2\n    exit 26\n    ;;\nesac\necho 'banner noise that should not be parsed'\nprintf '%s\\n' '{{\"title\":\"Architecture\",\"mermaid\":\"flowchart TD\\napi[API]\",\"components\":[{{\"id\":\"api\",\"name\":\"API\",\"summary\":\"Handles requests\",\"contains\":[],\"incoming_relationships\":[],\"outgoing_relationships\":[],\"evidence_paths\":[\"README.md\"],\"evidence_snippets\":[\"# Isolated exec test\"]}}]}}' > \"$6\"\n",
                repo.path().display()
            ),
        );

        let result = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: BUSY_TEST_TIMEOUT,
            },
        )
        .expect("codex should run outside the repo with a tighter sandbox");

        assert_eq!(result.title, "Architecture");
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_fails_closed_when_no_usable_evidence_exists() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        let marker = repo.path().join("codex-ran.txt");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            &format!("#!/bin/sh\ntouch \"{}\"\nexit 0\n", marker.display()),
        );

        let error = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: Duration::from_secs(1),
            },
        )
        .expect_err("generation should fail before codex runs when evidence is empty");

        assert!(error.contains("No usable evidence files"));
        assert!(
            !marker.exists(),
            "codex should not run when evidence is missing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_timeout_does_not_wait_for_descendants_holding_pipes_open() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Timeout descendant test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            "#!/bin/sh\n(sleep 5) >&2 &\nsleep 5\n",
        );

        let started = Instant::now();
        let error = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: Duration::from_millis(100),
            },
        )
        .expect_err("timeout should not wait for descendant-held pipes");

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout should return promptly even if descendants keep pipes open"
        );
        assert!(error.contains("timed out"));
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_timeout_kills_descendants_in_same_process_group() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        let marker = repo.path().join("descendant-alive.txt");
        write_repo_file(&repo, "README.md", "# Process group timeout test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            &format!(
                "#!/bin/sh\nnohup sh -c 'sleep 1; echo survived > \"{}\"' >/dev/null 2>&1 &\nsleep 5\n",
                marker.display()
            ),
        );

        let error = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: Duration::from_millis(100),
            },
        )
        .expect_err("timeout should kill the whole codex process group");

        std::thread::sleep(Duration::from_millis(1300));

        assert!(error.contains("timed out"));
        assert!(
            !marker.exists(),
            "descendant process should be terminated with the timed-out codex process group"
        );
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_handles_verbose_codex_output_without_deadlock() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Verbose output test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            "#!/bin/sh\ndd if=/dev/zero bs=1024 count=256 2>/dev/null | tr '\\000' 'x' >&2\nif [ \"$5\" != \"--output-last-message\" ] || [ -z \"$6\" ]; then\n  echo 'missing output-last-message contract' >&2\n  exit 18\nfi\nprintf '%s\\n' '{\"title\":\"Architecture\",\"mermaid\":\"flowchart TD\\napi[API]\",\"components\":[{\"id\":\"api\",\"name\":\"API\",\"summary\":\"Handles requests\",\"contains\":[],\"incoming_relationships\":[],\"outgoing_relationships\":[],\"evidence_paths\":[\"README.md\"],\"evidence_snippets\":[\"# Verbose output test\"]}]}' > \"$6\"\n",
        );

        let result = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: BUSY_TEST_TIMEOUT,
            },
        )
        .expect("verbose codex output should be drained without deadlock");

        assert_eq!(result.title, "Architecture");
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_prefers_last_message_when_stdout_is_noisy() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Noisy stdout test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            &format!(
                "#!/bin/sh\nif [ \"$5\" != \"--output-last-message\" ] || [ -z \"$6\" ]; then\n  echo 'missing output-last-message contract' >&2\n  exit 18\nfi\nprintf '%s\\n' '{{\"title\":\"Architecture\",\"mermaid\":\"flowchart TD\\napi[API]\",\"components\":[{{\"id\":\"api\",\"name\":\"API\",\"summary\":\"Handles requests\",\"contains\":[],\"incoming_relationships\":[],\"outgoing_relationships\":[],\"evidence_paths\":[\"README.md\"],\"evidence_snippets\":[\"# Noisy stdout test\"]}}]}}' > \"$6\"\ndd if=/dev/zero bs=1024 count={} 2>/dev/null | tr '\\000' 'x'\n",
                (MAX_CAPTURE_BYTES / 1024) + 1
            ),
        );

        let result = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: BUSY_TEST_TIMEOUT,
            },
        )
        .expect("successful codex runs should use the last-message file even if stdout is noisy");

        assert_eq!(result.title, "Architecture");
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_strips_claudecode_from_codex_environment() {
        struct EnvGuard(Option<std::ffi::OsString>);

        impl Drop for EnvGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(value) => env::set_var("CLAUDECODE", value),
                    None => env::remove_var("CLAUDECODE"),
                }
            }
        }

        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Env strip test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            "#!/bin/sh\nif [ -n \"${CLAUDECODE+x}\" ]; then\n  echo 'CLAUDECODE still set' >&2\n  exit 17\nfi\nif [ \"$5\" != \"--output-last-message\" ] || [ -z \"$6\" ]; then\n  echo 'missing output-last-message contract' >&2\n  exit 18\nfi\nprintf '%s\\n' '{\"title\":\"Architecture\",\"mermaid\":\"flowchart TD\\napi[API]\",\"components\":[{\"id\":\"api\",\"name\":\"API\",\"summary\":\"Handles requests\",\"contains\":[],\"incoming_relationships\":[],\"outgoing_relationships\":[],\"evidence_paths\":[\"README.md\"],\"evidence_snippets\":[\"# Env strip test\"]}]}' > \"$6\"\n",
        );
        let _guard = EnvGuard(env::var_os("CLAUDECODE"));
        env::set_var("CLAUDECODE", "nested-session");

        let result = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: BUSY_TEST_TIMEOUT,
            },
        )
        .expect("CLAUDECODE should be removed for codex exec");

        assert_eq!(result.title, "Architecture");
    }

    #[cfg(unix)]
    #[test]
    fn generate_architecture_returns_codex_stderr_on_failure() {
        let repo = TempDir::new().expect("create temp repo");
        let bin = TempDir::new().expect("create temp bin");
        write_repo_file(&repo, "README.md", "# Error test\n");
        let script = write_executable_script(
            &bin,
            "fake-codex.sh",
            "#!/bin/sh\necho 'codex exploded' >&2\nexit 9\n",
        );

        let error = generate_architecture_blocking_with_config(
            repo.path(),
            &CodexExecConfig {
                binary: script,
                timeout: BUSY_TEST_TIMEOUT,
            },
        )
        .expect_err("non-zero codex exit should fail");

        assert!(error.contains("codex exploded"));
    }
}
