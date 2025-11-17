use super::cargo_ops::integration::{ChangeImpactAnalysis, VersionBumpAnalysis};
use super::cargo_ops::types::CrateDependencyGraph;
use crate::error::Result;
use crate::utils::changelog::{
    ChangelogAnalysis, ChangelogConfig, ChangelogStatus, has_version_entry, parse_changelog,
    validate_changelog,
};
use crate::utils::severity::{Issue, IssueSeverity, IssueType};
use crate::utils::severity_config::SeverityConfig;
use std::collections::HashMap;
use std::path::Path;

const MIN_CRATE_NAME_WIDTH: usize = 10;

impl ChangelogAnalysis {
    /// display changelog analysis in table format
    pub fn display_table(&self) {
        if self.statuses.is_empty() {
            println!("no crates to analyze for changelog compliance.");
            return;
        }

        println!("changelog analysis:");
        println!("  total crates analyzed: {}", self.statuses.len());
        println!(
            "  crates with valid changelogs: {}",
            self.crates_with_valid_changelog.len()
        );
        println!(
            "  crates missing changelogs: {}",
            self.crates_missing_changelog.len()
        );
        println!(
            "  crates needing updates: {}",
            self.crates_needing_changelog_update.len()
        );
        println!("  compliance: {:.1}%", self.compliance_percentage());
        println!();

        // calculate column widths
        let name_width = self
            .statuses
            .keys()
            .map(|id| id.display_name().len())
            .max()
            .unwrap_or(MIN_CRATE_NAME_WIDTH)
            .max(MIN_CRATE_NAME_WIDTH);

        // print table header
        println!(
            "  {:<name_width$}  {:>6}  {:>6}  {:>7}  {:>13}",
            "Crate",
            "Exists",
            "Format",
            "Updated",
            "Version Entry",
            name_width = name_width
        );

        // print separator line
        println!(
            "  {}  ------  ------  -------  -------------",
            "-".repeat(name_width)
        );

        // collect and sort by status (issues first)
        let mut entries: Vec<_> = self.statuses.values().collect();
        entries.sort_by_key(|s| {
            let has_issues = !s.issues.is_empty();
            (!has_issues, s.is_complete(), s.crate_id.display_name())
        });

        // print each crate
        for status in entries {
            let (exists, format, updated, version) = status.get_display_status();

            println!(
                "  {:<name_width$}  {:>6}  {:>6}  {:>7}  {:>13}",
                status.crate_id.display_name(),
                exists,
                format,
                updated,
                version,
                name_width = name_width
            );
        }

        println!();
    }

    fn display_changelog_issue_table(
        title: &str,
        issues: &[(&ChangelogStatus, &crate::utils::severity::Issue)],
    ) {
        println!("{}:", title);

        let name_width = issues
            .iter()
            .map(|(s, _)| s.crate_id.display_name().len())
            .max()
            .unwrap_or(MIN_CRATE_NAME_WIDTH)
            .max(MIN_CRATE_NAME_WIDTH);

        println!("  {:<name_width$}  Issue", "Crate", name_width = name_width);
        println!("  {}  -----", "-".repeat(name_width));

        for (status, issue) in issues {
            println!(
                "  {:<name_width$}  {}",
                status.crate_id.display_name(),
                issue.message,
                name_width = name_width
            );
        }
        println!();
    }

    /// display detailed changelog issues split by severity
    pub fn display_issues(&self) {
        use crate::utils::severity::IssueSeverity;

        // collect errors and warnings separately
        let mut errors: Vec<(&ChangelogStatus, &crate::utils::severity::Issue)> = Vec::new();
        let mut warnings: Vec<(&ChangelogStatus, &crate::utils::severity::Issue)> = Vec::new();

        for status in self.statuses.values() {
            for issue in &status.issues {
                if issue.severity == IssueSeverity::Error {
                    errors.push((status, issue));
                } else {
                    warnings.push((status, issue));
                }
            }
        }

        if errors.is_empty() && warnings.is_empty() {
            println!("no changelog issues found.");
            return;
        }

        if !errors.is_empty() {
            Self::display_changelog_issue_table("changelog errors", &errors);
        }

        if !warnings.is_empty() {
            Self::display_changelog_issue_table("changelog warnings", &warnings);
        }
    }

    /// display compliance summary
    pub fn display_summary(&self) {
        if self.all_valid() {
            println!("all changelogs are compliant");
        } else {
            println!(
                "changelog compliance: {:.1}% ({}/{} crates)",
                self.compliance_percentage(),
                self.crates_with_valid_changelog.len(),
                self.statuses.len()
            );
            println!("  total issues: {}", self.total_issues);

            if !self.crates_missing_changelog.is_empty() {
                println!(
                    "  missing changelogs: {}",
                    self.crates_missing_changelog.len()
                );
            }
            if !self.crates_needing_changelog_update.is_empty() {
                println!(
                    "  needing updates: {}",
                    self.crates_needing_changelog_update.len()
                );
            }
        }
    }
}

// todo maybe this is too much structuring?
struct ChangelogProcessResult {
    format_valid: bool,
    current_version_has_entry: bool,
    changelog_was_updated: bool,
    changelog_obj: Option<crate::utils::changelog::Changelog>,
}

struct IssueCounters<'a> {
    issues: &'a mut Vec<Issue>,
    total_issues: &'a mut usize,
    total_errors: &'a mut usize,
    total_warnings: &'a mut usize,
}

/// changelog checker for analyzing changelog compliance
pub struct ChangelogChecker;

impl ChangelogChecker {
    fn process_changelog(
        has_changelog: bool,
        changelog_path: &Path,
        config: &ChangelogConfig,
        current_version: &semver::Version,
        severity_config: &SeverityConfig,
        counters: &mut IssueCounters,
        check_git_update: Option<(&Path, &[std::path::PathBuf], bool)>,
    ) -> ChangelogProcessResult {
        if !has_changelog {
            if config.require_changelog {
                let msg = format!("missing {} file", config.changelog_file_name);
                Self::add_issue(
                    counters.issues,
                    counters.total_issues,
                    counters.total_errors,
                    counters.total_warnings,
                    severity_config,
                    IssueType::MissingChangelog,
                    msg,
                );
            }
            return ChangelogProcessResult {
                format_valid: false,
                current_version_has_entry: false,
                changelog_was_updated: false,
                changelog_obj: None,
            };
        }

        let changelog = match parse_changelog(changelog_path) {
            Ok(cl) => cl,
            Err(e) => {
                let msg = format!("failed to parse changelog: {}", e);
                Self::add_issue(
                    counters.issues,
                    counters.total_issues,
                    counters.total_errors,
                    counters.total_warnings,
                    severity_config,
                    IssueType::BadFormat,
                    msg,
                );
                return ChangelogProcessResult {
                    format_valid: false,
                    current_version_has_entry: false,
                    changelog_was_updated: false,
                    changelog_obj: None,
                };
            }
        };

        let validation_messages = validate_changelog(&changelog, config);
        let format_valid = validation_messages.is_empty();

        if !format_valid {
            for message in validation_messages {
                Self::add_issue(
                    counters.issues,
                    counters.total_issues,
                    counters.total_errors,
                    counters.total_warnings,
                    severity_config,
                    IssueType::BadFormat,
                    message,
                );
            }
        }

        let current_version_has_entry = has_version_entry(&changelog, current_version);

        if !current_version_has_entry && config.require_changelog {
            let msg = format!(
                "missing changelog entry for current version {}",
                current_version
            );
            Self::add_issue(
                counters.issues,
                counters.total_issues,
                counters.total_errors,
                counters.total_warnings,
                severity_config,
                IssueType::MissingVersionEntry,
                msg,
            );
        }

        let mut changelog_was_updated = false;

        if let Some((repo_path, changed_files, is_directly_changed)) = check_git_update
            && config.check_changelog_updated
        {
            let relative_path = match changelog_path.strip_prefix(repo_path) {
                Ok(p) => p.to_path_buf(),
                Err(_) => changelog_path.to_path_buf(),
            };

            changelog_was_updated = changed_files.contains(&relative_path);

            if is_directly_changed && !changelog_was_updated {
                let msg = format!(
                    "crate was modified but {} was not updated",
                    config.changelog_file_name
                );
                Self::add_issue(
                    counters.issues,
                    counters.total_issues,
                    counters.total_errors,
                    counters.total_warnings,
                    severity_config,
                    IssueType::ChangelogNotUpdated,
                    msg,
                );
            }
        }

        ChangelogProcessResult {
            format_valid,
            current_version_has_entry,
            changelog_was_updated,
            changelog_obj: Some(changelog),
        }
    }

    fn add_issue(
        issues: &mut Vec<Issue>,
        total_issues: &mut usize,
        total_errors: &mut usize,
        total_warnings: &mut usize,
        severity_config: &SeverityConfig,
        issue_type: IssueType,
        message: String,
    ) {
        let severity = severity_config.get_severity(issue_type);
        let issue = Issue::new(severity, issue_type, message);
        issues.push(issue);
        *total_issues += 1;
        if severity == IssueSeverity::Error {
            *total_errors += 1;
        } else {
            *total_warnings += 1;
        }
    }

    fn get_severity_config<'a>(
        is_directly_changed: bool,
        direct_severity: &'a SeverityConfig,
        transitive_severity: &'a SeverityConfig,
    ) -> &'a SeverityConfig {
        if is_directly_changed {
            direct_severity
        } else {
            transitive_severity
        }
    }

    fn should_skip_crate(
        is_directly_changed: bool,
        has_changelog: bool,
        config: &ChangelogConfig,
    ) -> bool {
        !is_directly_changed && config.allow_missing_for_transitive && !has_changelog
    }

    /// analyze changelogs for changed crates
    pub fn analyze_for_changes<P: AsRef<Path>>(
        graph: &CrateDependencyGraph,
        repo_path: P,
        config: &ChangelogConfig,
        direct_severity: &SeverityConfig,
        transitive_severity: &SeverityConfig,
        version_analysis: &VersionBumpAnalysis,
        impact_analysis: &ChangeImpactAnalysis,
    ) -> Result<ChangelogAnalysis> {
        let repo_path = repo_path.as_ref();
        let mut statuses = HashMap::new();
        let mut crates_with_valid_changelog = Vec::new();
        let mut crates_missing_changelog = Vec::new();
        let mut crates_needing_changelog_update = Vec::new();
        let mut total_issues = 0;
        let mut total_errors = 0;
        let mut total_warnings = 0;

        for (crate_id, version_status) in &version_analysis.crate_versions {
            let crate_info = match graph.crates.get(crate_id) {
                Some(info) => info,
                None => continue,
            };

            let changelog_path = crate_info.path.join(&config.changelog_file_name);
            let has_changelog = changelog_path.exists();
            let is_directly_changed = impact_analysis.directly_affected_crates.contains(crate_id);

            if Self::should_skip_crate(is_directly_changed, has_changelog, config) {
                continue;
            }

            let severity_config = Self::get_severity_config(
                is_directly_changed,
                direct_severity,
                transitive_severity,
            );

            let mut issues = Vec::new();
            let mut counters = IssueCounters {
                issues: &mut issues,
                total_issues: &mut total_issues,
                total_errors: &mut total_errors,
                total_warnings: &mut total_warnings,
            };

            let result = Self::process_changelog(
                has_changelog,
                &changelog_path,
                config,
                &version_status.current_version,
                severity_config,
                &mut counters,
                Some((
                    repo_path,
                    &impact_analysis.changed_files,
                    is_directly_changed,
                )),
            );

            let format_valid = result.format_valid;
            let current_version_has_entry = result.current_version_has_entry;
            let changelog_was_updated = result.changelog_was_updated;
            let changelog_obj = result.changelog_obj;

            // determine if this crate needs a changelog update
            let needs_update =
                is_directly_changed && (!has_changelog || !current_version_has_entry);

            // create status for this crate
            let status = ChangelogStatus {
                crate_id: crate_id.clone(),
                has_changelog,
                changelog_path: if has_changelog {
                    Some(changelog_path)
                } else {
                    None
                },
                format_valid,
                current_version_has_entry,
                changelog_was_updated,
                issues: issues.clone(),
                changelog: changelog_obj,
            };

            // categorize the crate
            if has_changelog && format_valid && current_version_has_entry {
                crates_with_valid_changelog.push(crate_id.clone());
            } else if !has_changelog {
                crates_missing_changelog.push(crate_id.clone());
            }

            if needs_update {
                crates_needing_changelog_update.push(crate_id.clone());
            }

            statuses.insert(crate_id.clone(), status);
        }

        Ok(ChangelogAnalysis {
            statuses,
            crates_with_valid_changelog,
            crates_missing_changelog,
            crates_needing_changelog_update,
            total_issues,
            total_errors,
            total_warnings,
        })
    }

    /// analyze all crates (not just changed ones)
    pub fn analyze_all<P: AsRef<Path>>(
        graph: &CrateDependencyGraph,
        _repo_path: P,
        config: &ChangelogConfig,
        severity_config: &SeverityConfig,
    ) -> Result<ChangelogAnalysis> {
        let mut statuses = HashMap::new();
        let mut crates_with_valid_changelog = Vec::new();
        let mut crates_missing_changelog = Vec::new();
        let mut crates_needing_changelog_update = Vec::new();
        let mut total_issues = 0;
        let mut total_errors = 0;
        let mut total_warnings = 0;

        for crate_info in graph.crates.values() {
            let crate_id = &crate_info.id;

            // construct changelog path (crate_info.path is already the crate root)
            let changelog_path = crate_info.path.join(&config.changelog_file_name);
            let has_changelog = changelog_path.exists();

            let mut issues = Vec::new();
            let mut format_valid = false;
            let mut current_version_has_entry = false;
            let changelog_was_updated = false;
            let changelog_obj;

            let mut counters = IssueCounters {
                issues: &mut issues,
                total_issues: &mut total_issues,
                total_errors: &mut total_errors,
                total_warnings: &mut total_warnings,
            };

            match semver::Version::parse(&crate_info.version) {
                Ok(current_version) => {
                    let result = Self::process_changelog(
                        has_changelog,
                        &changelog_path,
                        config,
                        &current_version,
                        severity_config,
                        &mut counters,
                        None,
                    );

                    format_valid = result.format_valid;
                    current_version_has_entry = result.current_version_has_entry;
                    changelog_obj = result.changelog_obj;
                }
                Err(_) => {
                    let msg = format!("invalid version in Cargo.toml: {}", crate_info.version);
                    Self::add_issue(
                        counters.issues,
                        counters.total_issues,
                        counters.total_errors,
                        counters.total_warnings,
                        severity_config,
                        IssueType::BadFormat,
                        msg,
                    );
                    changelog_obj = None;
                }
            }

            // determine if this crate needs a changelog update
            let needs_update = !has_changelog || !current_version_has_entry;

            // create status for this crate
            let status = ChangelogStatus {
                crate_id: crate_id.clone(),
                has_changelog,
                changelog_path: if has_changelog {
                    Some(changelog_path)
                } else {
                    None
                },
                format_valid,
                current_version_has_entry,
                changelog_was_updated,
                issues: issues.clone(),
                changelog: changelog_obj,
            };

            // categorize the crate
            if has_changelog && format_valid && current_version_has_entry {
                crates_with_valid_changelog.push(crate_id.clone());
            } else if !has_changelog {
                crates_missing_changelog.push(crate_id.clone());
            }

            if needs_update {
                crates_needing_changelog_update.push(crate_id.clone());
            }

            statuses.insert(crate_id.clone(), status);
        }

        Ok(ChangelogAnalysis {
            statuses,
            crates_with_valid_changelog,
            crates_missing_changelog,
            crates_needing_changelog_update,
            total_issues,
            total_errors,
            total_warnings,
        })
    }
}
