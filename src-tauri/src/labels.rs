/// Canonical label constants for GitHub issue triage.
///
/// All label names use the `key:value` format (no space after the colon).
/// Both the maintainer and auto-worker import from here to prevent drift.

// Priority labels
pub const PRIORITY_LOW: &str = "priority:low";
pub const PRIORITY_HIGH: &str = "priority:high";

// Complexity labels
pub const COMPLEXITY_LOW: &str = "complexity:low";
pub const COMPLEXITY_HIGH: &str = "complexity:high";

// Workflow labels
pub const IN_PROGRESS: &str = "in-progress";
pub const ASSIGNED_TO_AUTO_WORKER: &str = "assigned-to-auto-worker";
pub const FINISHED_BY_WORKER: &str = "finished-by-worker";
pub const FILED_BY_MAINTAINER: &str = "filed-by-maintainer";
pub const TRIAGED: &str = "triaged";

/// All triage labels that use the `key:value` format.
const TRIAGE_LABELS: &[&str] = &[PRIORITY_LOW, PRIORITY_HIGH, COMPLEXITY_LOW, COMPLEXITY_HIGH];

/// Validate that a label string follows the canonical `key:value` format
/// (no space after colon) for priority/complexity labels.
/// Returns the label unchanged if valid, or an error message if not.
pub fn validate_triage_label(label: &str) -> Result<&str, String> {
    if label.starts_with("priority:") || label.starts_with("complexity:") {
        if label.contains(": ") {
            return Err(format!(
                "Label '{}' has a space after the colon. Use the canonical format (e.g. 'priority:high', not 'priority: high')",
                label
            ));
        }
        if !TRIAGE_LABELS.contains(&label) {
            return Err(format!(
                "Unknown triage label '{}'. Valid labels: {:?}",
                label, TRIAGE_LABELS
            ));
        }
    }
    Ok(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_labels_have_no_space_after_colon() {
        for label in TRIAGE_LABELS {
            assert!(
                !label.contains(": "),
                "Label '{}' must not have a space after the colon",
                label
            );
        }
    }

    #[test]
    fn validate_rejects_spaced_labels() {
        assert!(validate_triage_label("priority: high").is_err());
        assert!(validate_triage_label("complexity: low").is_err());
        assert!(validate_triage_label("priority: low").is_err());
        assert!(validate_triage_label("complexity: high").is_err());
    }

    #[test]
    fn validate_accepts_canonical_labels() {
        assert!(validate_triage_label("priority:high").is_ok());
        assert!(validate_triage_label("priority:low").is_ok());
        assert!(validate_triage_label("complexity:high").is_ok());
        assert!(validate_triage_label("complexity:low").is_ok());
    }

    #[test]
    fn validate_accepts_non_triage_labels() {
        assert!(validate_triage_label("in-progress").is_ok());
        assert!(validate_triage_label("filed-by-maintainer").is_ok());
        assert!(validate_triage_label("triaged").is_ok());
    }
}
