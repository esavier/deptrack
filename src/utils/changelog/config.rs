// changelog configuration

use serde::{Deserialize, Serialize};

/// configuration for changelog verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogConfig {
    /// name of the changelog file (default: "CHANGELOG.md")
    pub changelog_file_name: String,

    /// require all crates to have a changelog
    pub require_changelog: bool,

    /// strictly enforce changelog format
    pub enforce_format: bool,

    /// allowed change types (e.g., ["feat", "fix", "chore"])
    pub allowed_change_types: Vec<String>,

    /// require type(scope): format in entries
    pub require_scope: bool,

    /// check if changelog file was modified in git changes
    pub check_changelog_updated: bool,

    /// allow missing changelogs for transitive dependencies
    pub allow_missing_for_transitive: bool,
}

impl ChangelogConfig {
    /// create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    pub fn changelog_file_name(mut self, name: impl Into<String>) -> Self {
        self.changelog_file_name = name.into();
        self
    }

    pub fn require_changelog(mut self, required: bool) -> Self {
        self.require_changelog = required;
        self
    }

    pub fn enforce_format(mut self, enforce: bool) -> Self {
        self.enforce_format = enforce;
        self
    }

    pub fn allowed_change_types(mut self, types: Vec<String>) -> Self {
        self.allowed_change_types = types;
        self
    }

    pub fn require_scope(mut self, required: bool) -> Self {
        self.require_scope = required;
        self
    }

    pub fn check_changelog_updated(mut self, check: bool) -> Self {
        self.check_changelog_updated = check;
        self
    }

    pub fn allow_missing_for_transitive(mut self, allow: bool) -> Self {
        self.allow_missing_for_transitive = allow;
        self
    }

    /// check if a change type is allowed
    pub fn is_allowed_change_type(&self, change_type: &str) -> bool {
        if self.allowed_change_types.is_empty() {
            return true; // if no restrictions, allow all
        }
        self.allowed_change_types.iter().any(|t| t == change_type)
    }
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        Self {
            changelog_file_name: "CHANGELOG.md".to_string(),
            require_changelog: true,
            enforce_format: true,
            allowed_change_types: vec![
                "feat".to_string(),
                "fix".to_string(),
                "chore".to_string(),
                "refactor".to_string(),
                "docs".to_string(),
                "test".to_string(),
                "style".to_string(),
                "perf".to_string(),
            ],
            require_scope: false,
            check_changelog_updated: true,
            allow_missing_for_transitive: true,
        }
    }
}
