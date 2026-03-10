use std::collections::HashSet;

use serde::{Deserialize, Serialize};

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

fn extract_mermaid_node_ids(mermaid: &str) -> HashSet<String> {
    let bytes = mermaid.as_bytes();
    let mut ids = HashSet::new();
    let mut index = 0;

    while index < bytes.len() {
        if is_identifier_char(bytes[index]) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_identifier_char(bytes[index]) {
                index += 1;
            }

            let mut cursor = index;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor < bytes.len() && matches!(bytes[cursor], b'[' | b'(' | b'{') {
                ids.insert(mermaid[start..index].to_string());
            }

            continue;
        }

        index += 1;
    }

    ids
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::parse_architecture_output;

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
}
