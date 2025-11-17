use super::severity::{IssueSeverity, IssueType};
use serde::{Deserialize, Serialize};

/// configuration for issue severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeverityConfig {
    /// severity for missing changelog file
    pub missing_changelog: IssueSeverity,
    /// severity for missing changelog entry for current version
    pub missing_version_entry: IssueSeverity,
    /// severity for changelog not being updated when crate was modified
    pub changelog_not_updated: IssueSeverity,
    /// severity for invalid changelog format
    pub bad_format: IssueSeverity,
    /// severity for missing version bump when crate was modified
    pub no_version_bump: IssueSeverity,
}

impl SeverityConfig {
    /// get severity for a specific issue type
    pub fn get_severity(&self, issue_type: IssueType) -> IssueSeverity {
        match issue_type {
            IssueType::MissingChangelog => self.missing_changelog,
            IssueType::MissingVersionEntry => self.missing_version_entry,
            IssueType::ChangelogNotUpdated => self.changelog_not_updated,
            IssueType::BadFormat => self.bad_format,
            IssueType::NoVersionBump => self.no_version_bump,
        }
    }

    /// create default severity config for direct dependencies
    ///
    /// defaults:
    /// - error: missing changelog, bad format, no version bump
    /// - warning: missing version entry, changelog not updated
    pub fn default_direct() -> Self {
        Self {
            missing_changelog: IssueSeverity::Error,
            missing_version_entry: IssueSeverity::Warning,
            changelog_not_updated: IssueSeverity::Warning,
            bad_format: IssueSeverity::Error,
            no_version_bump: IssueSeverity::Error,
        }
    }

    /// create default severity config for transitive dependencies
    ///
    /// defaults:
    /// - warning: all issue types
    pub fn default_transitive() -> Self {
        Self {
            missing_changelog: IssueSeverity::Warning,
            missing_version_entry: IssueSeverity::Warning,
            changelog_not_updated: IssueSeverity::Warning,
            bad_format: IssueSeverity::Warning,
            no_version_bump: IssueSeverity::Warning,
        }
    }
}

impl Default for SeverityConfig {
    fn default() -> Self {
        Self::default_direct()
    }
}
