use serde::{Deserialize, Serialize};
use std::fmt;

/// severity level for issues detected during analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    /// error-level issue that should cause validation to fail
    Error,
    /// warning-level issue that is informational only
    Warning,
}

impl fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueSeverity::Error => write!(f, "ERROR"),
            IssueSeverity::Warning => write!(f, "WARN"),
        }
    }
}

impl std::str::FromStr for IssueSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(IssueSeverity::Error),
            "warning" | "warn" => Ok(IssueSeverity::Warning),
            _ => Err(format!("invalid severity: {}, use 'error' or 'warning'", s)),
        }
    }
}

/// type of issue detected during analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueType {
    /// changelog file is missing
    MissingChangelog,
    /// changelog entry for current version is missing
    MissingVersionEntry,
    /// changelog was not updated when crate was modified
    ChangelogNotUpdated,
    /// changelog format is invalid
    BadFormat,
    /// version was not bumped when crate was modified
    NoVersionBump,
}

impl fmt::Display for IssueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueType::MissingChangelog => write!(f, "missing_changelog"),
            IssueType::MissingVersionEntry => write!(f, "missing_version_entry"),
            IssueType::ChangelogNotUpdated => write!(f, "changelog_not_updated"),
            IssueType::BadFormat => write!(f, "bad_format"),
            IssueType::NoVersionBump => write!(f, "no_version_bump"),
        }
    }
}

/// structured issue with severity, type, and message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// severity level of the issue
    pub severity: IssueSeverity,
    /// type of the issue
    pub issue_type: IssueType,
    /// human-readable message describing the issue
    pub message: String,
}

impl Issue {
    /// create a new issue
    pub fn new(severity: IssueSeverity, issue_type: IssueType, message: String) -> Self {
        Self {
            severity,
            issue_type,
            message,
        }
    }

    /// check if this is an error-level issue
    pub fn is_error(&self) -> bool {
        self.severity == IssueSeverity::Error
    }

    /// check if this is a warning-level issue
    pub fn is_warning(&self) -> bool {
        self.severity == IssueSeverity::Warning
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)
    }
}
