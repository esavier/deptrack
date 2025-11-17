// changelog validator

use super::config::ChangelogConfig;
use super::types::{Changelog, ChangelogEntry};
use semver::Version;

/// validate a changelog against configuration rules
pub fn validate_changelog(changelog: &Changelog, config: &ChangelogConfig) -> Vec<String> {
    let mut issues = Vec::new();

    // check for header
    if config.enforce_format && !changelog.has_header {
        issues.push("missing '# CHANGELOG' header".to_string());
    }

    // add existing format issues (extract messages from Issue objects)
    issues.extend(changelog.format_issues.iter().map(|i| i.message.clone()));

    // validate each version section
    for (version, version_section) in &changelog.versions {
        // check for empty version sections
        if version_section.entries.is_empty() {
            issues.push(format!("version {} has no changelog entries", version));
        }

        // validate entries
        for entry in &version_section.entries {
            if let Some(issue) = validate_entry(entry, config) {
                issues.push(format!("version {}: {}", version, issue));
            }
        }
    }

    issues
}

/// validate a single changelog entry
fn validate_entry(entry: &ChangelogEntry, config: &ChangelogConfig) -> Option<String> {
    // check if change type is allowed
    if !config.is_allowed_change_type(&entry.change_type) {
        return Some(format!(
            "line {}: invalid change type '{}' (allowed: {:?})",
            entry.line_number, entry.change_type, config.allowed_change_types
        ));
    }

    // check if scope is required
    if config.require_scope && entry.scope.is_none() {
        return Some(format!(
            "line {}: scope is required but missing (expected format: type(scope): description)",
            entry.line_number
        ));
    }

    // check for empty description
    if entry.description.trim().is_empty() {
        return Some(format!("line {}: description is empty", entry.line_number));
    }

    None
}

/// check if a version has a changelog entry
pub fn has_version_entry(changelog: &Changelog, version: &Version) -> bool {
    changelog.has_version(version)
}

/// check if a version section has meaningful content
pub fn version_has_content(changelog: &Changelog, version: &Version) -> bool {
    if let Some(version_section) = changelog.get_version(version) {
        !version_section.entries.is_empty()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::changelog::types::ChangelogEntry;

    #[test]
    fn test_validate_entry_valid() {
        let config = ChangelogConfig::default();
        let entry = ChangelogEntry::new(
            "feat".to_string(),
            Some("api".to_string()),
            "add endpoint".to_string(),
            1,
        );

        assert!(validate_entry(&entry, &config).is_none());
    }

    #[test]
    fn test_validate_entry_invalid_type() {
        let config = ChangelogConfig::default();
        let entry = ChangelogEntry::new(
            "invalid".to_string(),
            Some("api".to_string()),
            "add endpoint".to_string(),
            1,
        );

        assert!(validate_entry(&entry, &config).is_some());
    }

    #[test]
    fn test_validate_entry_missing_scope() {
        let config = ChangelogConfig::default().require_scope(true);
        let entry = ChangelogEntry::new("feat".to_string(), None, "add endpoint".to_string(), 1);

        assert!(validate_entry(&entry, &config).is_some());
    }
}
