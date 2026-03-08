use crate::models::GithubIssue;

/// Check if an issue is eligible for auto-worker processing.
pub fn is_eligible(issue: &GithubIssue) -> bool {
    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
    labels.contains(&"priority: high")
        && labels.contains(&"complexity: low")
        && labels.contains(&"triaged")
        && !labels.contains(&"in-progress")
        && !labels.contains(&"finished-by-worker")
}

/// Pick the first eligible issue from a list.
pub fn pick_eligible_issue(issues: &[GithubIssue]) -> Option<&GithubIssue> {
    issues.iter().find(|i| is_eligible(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GithubLabel;

    fn make_issue(labels: &[&str]) -> GithubIssue {
        GithubIssue {
            number: 1,
            title: "Test".to_string(),
            url: "https://github.com/o/r/issues/1".to_string(),
            body: None,
            labels: labels.iter().map(|l| GithubLabel { name: l.to_string() }).collect(),
        }
    }

    #[test]
    fn eligible_issue_has_all_required_labels() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged"]);
        assert!(is_eligible(&issue));
    }

    #[test]
    fn missing_priority_high_not_eligible() {
        let issue = make_issue(&["complexity: low", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn missing_complexity_low_not_eligible() {
        let issue = make_issue(&["priority: high", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn missing_triaged_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn in_progress_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged", "in-progress"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn finished_by_worker_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged", "finished-by-worker"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn pick_eligible_returns_first_match() {
        let issues = vec![
            make_issue(&["priority: low"]),
            make_issue(&["priority: high", "complexity: low", "triaged"]),
        ];
        let picked = pick_eligible_issue(&issues);
        assert!(picked.is_some());
    }

    #[test]
    fn pick_eligible_returns_none_when_no_match() {
        let issues = vec![
            make_issue(&["priority: low"]),
            make_issue(&["in-progress", "priority: high", "complexity: low", "triaged"]),
        ];
        assert!(pick_eligible_issue(&issues).is_none());
    }
}
