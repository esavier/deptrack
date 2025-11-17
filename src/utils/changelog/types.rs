// changelog data structures

use crate::utils::cargo_ops::CrateId;
use crate::utils::severity::Issue;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// represents a single changelog entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub change_type: String,   // feat, fix, chore, etc.
    pub scope: Option<String>, // crate name or component
    pub description: String,
    pub line_number: usize,
}

impl ChangelogEntry {
    pub fn new(
        change_type: String,
        scope: Option<String>,
        description: String,
        line_number: usize,
    ) -> Self {
        Self {
            change_type,
            scope,
            description,
            line_number,
        }
    }
}

/// represents a version section in the changelog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogVersion {
    pub version: Version,
    pub entries: Vec<ChangelogEntry>,
    pub line_number: usize,
}

impl ChangelogVersion {
    pub fn new(version: Version, line_number: usize) -> Self {
        Self {
            version,
            entries: Vec::new(),
            line_number,
        }
    }

    pub fn add_entry(&mut self, entry: ChangelogEntry) {
        self.entries.push(entry);
    }
}

/// represents a complete changelog file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changelog {
    pub path: PathBuf,
    pub versions: HashMap<Version, ChangelogVersion>,
    pub has_header: bool,
    pub format_issues: Vec<Issue>,
}

impl Changelog {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            versions: HashMap::new(),
            has_header: false,
            format_issues: Vec::new(),
        }
    }

    pub fn add_version(&mut self, version_section: ChangelogVersion) {
        self.versions
            .insert(version_section.version.clone(), version_section);
    }

    pub fn has_version(&self, version: &Version) -> bool {
        self.versions.contains_key(version)
    }

    pub fn get_version(&self, version: &Version) -> Option<&ChangelogVersion> {
        self.versions.get(version)
    }

    pub fn is_valid(&self) -> bool {
        self.has_header && self.format_issues.is_empty()
    }
}

/// changelog status for a single crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogStatus {
    pub crate_id: CrateId,
    pub has_changelog: bool,
    pub changelog_path: Option<PathBuf>,
    pub format_valid: bool,
    pub current_version_has_entry: bool,
    pub changelog_was_updated: bool, // was changed in git
    pub issues: Vec<Issue>,
    pub changelog: Option<Changelog>,
}

impl ChangelogStatus {
    pub fn new(crate_id: CrateId) -> Self {
        Self {
            crate_id,
            has_changelog: false,
            changelog_path: None,
            format_valid: false,
            current_version_has_entry: false,
            changelog_was_updated: false,
            issues: Vec::new(),
            changelog: None,
        }
    }

    pub fn add_issue(&mut self, issue: Issue) {
        self.issues.push(issue);
    }

    pub fn needs_attention(&self) -> bool {
        !self.issues.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_warning()).count()
    }

    pub fn is_complete(&self) -> bool {
        self.has_changelog && self.format_valid && self.current_version_has_entry
    }

    pub fn get_display_status(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        let exists = if self.has_changelog { "OK" } else { "MISS" };

        let format = if !self.has_changelog {
            "N/A"
        } else if self.format_valid {
            "OK"
        } else {
            "FAIL"
        };

        let updated = if !self.has_changelog {
            "N/A"
        } else if self.changelog_was_updated {
            "OK"
        } else {
            "NO"
        };

        let version = if !self.has_changelog {
            "N/A"
        } else if self.current_version_has_entry {
            "OK"
        } else {
            "MISS"
        };

        (exists, format, updated, version)
    }
}

/// overall changelog analysis for all affected crates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogAnalysis {
    pub statuses: HashMap<CrateId, ChangelogStatus>,
    pub crates_with_valid_changelog: Vec<CrateId>,
    pub crates_missing_changelog: Vec<CrateId>,
    pub crates_needing_changelog_update: Vec<CrateId>,
    pub total_issues: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
}

impl ChangelogAnalysis {
    pub fn new() -> Self {
        Self {
            statuses: HashMap::new(),
            crates_with_valid_changelog: Vec::new(),
            crates_missing_changelog: Vec::new(),
            crates_needing_changelog_update: Vec::new(),
            total_issues: 0,
            total_errors: 0,
            total_warnings: 0,
        }
    }

    pub fn add_status(&mut self, status: ChangelogStatus) {
        let crate_id = status.crate_id.clone();

        if !status.has_changelog {
            self.crates_missing_changelog.push(crate_id.clone());
        } else if status.format_valid && status.current_version_has_entry {
            self.crates_with_valid_changelog.push(crate_id.clone());
        } else {
            self.crates_needing_changelog_update.push(crate_id.clone());
        }

        self.total_issues += status.issues.len();
        self.total_errors += status.error_count();
        self.total_warnings += status.warning_count();
        self.statuses.insert(crate_id, status);
    }

    pub fn all_valid(&self) -> bool {
        self.crates_missing_changelog.is_empty() && self.crates_needing_changelog_update.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.total_errors > 0
    }

    pub fn compliance_percentage(&self) -> f64 {
        let total = self.statuses.len();
        if total == 0 {
            return 100.0;
        }
        (self.crates_with_valid_changelog.len() as f64 / total as f64) * 100.0
    }
}

impl Default for ChangelogAnalysis {
    fn default() -> Self {
        Self::new()
    }
}
