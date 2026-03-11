use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(crate) const MAX_TOP_LEVEL_DIRECTORIES: usize = 6;
pub(crate) const MAX_EVIDENCE_FILES: usize = 8;
const MAX_SNIPPET_LINES: usize = 24;
const MAX_SNIPPET_CHARS: usize = 1_200;

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
    pub component_id: String,
    pub summary: String,
}

pub fn collect_repo_evidence(repo_path: &Path) -> Result<RepoEvidence, String> {
    if !repo_path.is_dir() {
        return Err(format!("Not a directory: {}", repo_path.display()));
    }

    let root_entries = read_sorted_dir(repo_path)?;
    let top_level_directories = root_entries
        .iter()
        .filter(|path| path.is_dir())
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
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            is_metadata_file(name).then(|| path.to_path_buf())
        })
        .collect::<Vec<_>>();
    metadata_paths.sort_by(|left, right| {
        metadata_file_rank(left)
            .cmp(&metadata_file_rank(right))
            .then_with(|| relative_path(repo_path, left).cmp(&relative_path(repo_path, right)))
    });

    for path in metadata_paths {
        if files.len() >= MAX_EVIDENCE_FILES {
            break;
        }
        push_evidence_file(&mut files, &mut seen_paths, repo_path, &path);
    }

    for directory in &top_level_directories {
        if files.len() >= MAX_EVIDENCE_FILES {
            break;
        }

        let path = repo_path.join(directory);
        if let Some(file) = best_source_file_in_dir(repo_path, &path)? {
            push_evidence_file(&mut files, &mut seen_paths, repo_path, &file);
        }
    }

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
                    "### {}\n```text\n{}\n```",
                    file.path,
                    file.snippet.trim_end()
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
- Every component id must appear as a Mermaid node id.\n\
- evidence_paths and evidence_snippets must cite only the files shown below.\n\
- Output JSON only. No prose, no markdown fences.\n\n\
Top-level directories:\n{directories}\n\n\
Repository evidence:\n{file_sections}\n"
    )
}

pub fn generate_architecture_blocking(repo_path: &Path) -> Result<ArchitectureResult, String> {
    let evidence = collect_repo_evidence(repo_path)?;
    let prompt = build_architecture_prompt(repo_path, &evidence);
    let output = std::process::Command::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg(&prompt)
        .current_dir(repo_path)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run codex exec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codex exec failed: {}", stderr.trim()));
    }

    parse_architecture_output(&String::from_utf8_lossy(&output.stdout))
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
    let json = extract_json(output).ok_or("No JSON found in output")?;
    let parsed: ArchitectureResult =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {}", e))?;
    sanitize_architecture_result(parsed)
}

fn sanitize_architecture_result(result: ArchitectureResult) -> Result<ArchitectureResult, String> {
    let title = result.title.trim().to_string();
    if title.is_empty() {
        return Err("Architecture title cannot be empty".to_string());
    }

    let mermaid = result.mermaid.trim().to_string();
    if mermaid.is_empty() {
        return Err("Architecture Mermaid cannot be empty".to_string());
    }

    let mermaid_node_ids = extract_mermaid_node_ids(&mermaid);
    let mut components = Vec::with_capacity(result.components.len());
    for (component_index, component) in result.components.into_iter().enumerate() {
        components.push(sanitize_component(
            component,
            component_index,
            &mermaid_node_ids,
        )?);
    }
    validate_component_references(&components)?;

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
        evidence_paths: trim_string_list(component.evidence_paths),
        evidence_snippets: trim_string_list(component.evidence_snippets),
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

    while index < bytes.len() && is_mermaid_edge_char(bytes[index]) {
        index += 1;
    }

    if index == skip_ascii_whitespace(bytes, start) {
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
    matches!(byte, b'-' | b'.' | b'=' | b'<' | b'>' | b'o' | b'x')
}

fn read_sorted_dir(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(entries)
}

fn push_evidence_file(
    files: &mut Vec<RepoEvidenceFile>,
    seen_paths: &mut HashSet<String>,
    repo_path: &Path,
    path: &Path,
) {
    let relative = relative_path(repo_path, path);
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
    let contents = std::fs::read_to_string(path).ok()?;
    let mut snippet = contents
        .lines()
        .take(MAX_SNIPPET_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if snippet.chars().count() > MAX_SNIPPET_CHARS {
        snippet = snippet.chars().take(MAX_SNIPPET_CHARS).collect();
    }
    let snippet = snippet.trim().to_string();
    (!snippet.is_empty()).then_some(snippet)
}

fn best_source_file_in_dir(repo_path: &Path, directory: &Path) -> Result<Option<PathBuf>, String> {
    let mut candidates = Vec::new();
    collect_source_candidates(repo_path, directory, &mut candidates)?;
    candidates.sort_by(|left, right| {
        source_file_rank(repo_path, left)
            .cmp(&source_file_rank(repo_path, right))
            .then_with(|| relative_path(repo_path, left).cmp(&relative_path(repo_path, right)))
    });
    Ok(candidates.into_iter().next())
}

fn collect_source_candidates(
    repo_path: &Path,
    current_dir: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for path in read_sorted_dir(current_dir)? {
        if path.is_dir() {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if is_ignored_dir(name) {
                continue;
            }
            collect_source_candidates(repo_path, &path, candidates)?;
            continue;
        }

        if path.is_file() && is_source_file(&path) {
            candidates.push(path);
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
    let relative = relative_path(repo_path, path);
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

fn relative_path(repo_path: &Path, path: &Path) -> String {
    path.strip_prefix(repo_path)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{collect_repo_evidence, parse_architecture_output, MAX_EVIDENCE_FILES};

    fn write_repo_file(repo: &TempDir, relative_path: &str, contents: &str) {
        let path = repo.path().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(path, contents).expect("write repo fixture file");
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
            paths.iter().any(|path| *path == "src/main.rs"),
            "representative root source file should be included"
        );
        assert!(
            paths.iter().any(|path| *path == "web/app.ts"),
            "representative file from another top-level directory should be included"
        );
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
}
