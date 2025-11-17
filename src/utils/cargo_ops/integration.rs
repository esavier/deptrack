use super::types::{CrateDependencyGraph, CrateId};
use crate::error::Result;
use crate::utils::git_ops::{GitOps, GitRef};
use crate::utils::severity::Issue;
use semver::Version;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MIN_CRATE_NAME_WIDTH: usize = 10;
const MIN_VERSION_WIDTH: usize = 12;

/// represents version bump status for a crate
#[derive(Debug, Clone)]
pub struct VersionBumpStatus {
    pub crate_id: CrateId,
    pub base_version: Version,
    pub current_version: Version,
    pub is_bumped: bool,
    pub is_directly_changed: bool,
    pub issues: Vec<Issue>,
}

impl VersionBumpStatus {
    /// check if version needs a bump
    pub fn needs_bump(&self) -> bool {
        !self.is_bumped
    }

    /// add an issue to this version bump status
    pub fn add_issue(&mut self, issue: Issue) {
        self.issues.push(issue);
    }

    /// check if this status has error-level issues
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.is_error())
    }

    /// get count of error-level issues
    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_error()).count()
    }

    /// get count of warning-level issues
    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_warning()).count()
    }
}

/// represents version bump analysis for all affected crates
#[derive(Debug, Clone)]
pub struct VersionBumpAnalysis {
    /// all affected crates and their version status
    pub crate_versions: HashMap<CrateId, VersionBumpStatus>,
    /// crates that need version bumps but don't have them
    pub crates_needing_bump: Vec<CrateId>,
    /// crates that have been properly bumped
    pub crates_bumped: Vec<CrateId>,
    /// total number of error-level issues
    pub total_errors: usize,
    /// total number of warning-level issues
    pub total_warnings: usize,
}

impl VersionBumpAnalysis {
    /// check if all affected crates have been bumped
    pub fn all_bumped(&self) -> bool {
        self.crates_needing_bump.is_empty()
    }

    /// check if there are any error-level issues
    pub fn has_errors(&self) -> bool {
        self.total_errors > 0
    }

    /// get the percentage of crates that have been bumped
    pub fn bump_percentage(&self) -> f64 {
        let total = self.crate_versions.len();
        if total == 0 {
            return 100.0;
        }
        (self.crates_bumped.len() as f64 / total as f64) * 100.0
    }

    /// display version bump analysis in table format
    pub fn display_table(&self) {
        if self.crate_versions.is_empty() {
            println!("no affected crates found.");
            return;
        }

        println!("version bump analysis:");
        println!("  total affected crates: {}", self.crate_versions.len());
        println!("  crates with bumps: {}", self.crates_bumped.len());
        println!("  crates needing bumps: {}", self.crates_needing_bump.len());
        println!("  bump percentage: {:.1}%", self.bump_percentage());
        println!();

        // calculate column widths
        let name_width = self
            .crate_versions
            .keys()
            .map(|id| id.display_name().len())
            .max()
            .unwrap_or(MIN_CRATE_NAME_WIDTH)
            .max(MIN_CRATE_NAME_WIDTH);

        let version_width = self
            .crate_versions
            .values()
            .map(|s| {
                std::cmp::max(
                    s.base_version.to_string().len(),
                    s.current_version.to_string().len(),
                )
            })
            .max()
            .unwrap_or(MIN_VERSION_WIDTH)
            .max(MIN_VERSION_WIDTH);

        // print table header
        println!(
            "  {:<name_width$}  {:<version_width$}  {:<version_width$}  {:>6}  {:<8}",
            "Crate",
            "Base Version",
            "Curr Version",
            "Status",
            "Changed",
            name_width = name_width,
            version_width = version_width
        );

        // print separator line
        println!(
            "  {}  {}  {}  ------  --------",
            "-".repeat(name_width),
            "-".repeat(version_width),
            "-".repeat(version_width)
        );

        // collect and sort by status (needs bump first, then bumped)
        let mut entries: Vec<_> = self.crate_versions.values().collect();
        entries.sort_by_key(|s| {
            (
                !s.is_bumped,
                !s.is_directly_changed,
                s.crate_id.display_name(),
            )
        });

        // print each crate
        for status in entries {
            let bump_status = if status.is_bumped { "OK" } else { "NEEDED" };

            let change_type = if status.is_directly_changed {
                "direct"
            } else {
                "transitive"
            };

            println!(
                "  {:<name_width$}  {:<version_width$}  {:<version_width$}  {:>6}  {:<8}",
                status.crate_id.display_name(),
                status.base_version,
                status.current_version,
                bump_status,
                change_type,
                name_width = name_width,
                version_width = version_width
            );
        }

        println!();

        // show summary
        if !self.all_bumped() {
            println!(
                "warning: {} crate(s) need version bumps",
                self.crates_needing_bump.len()
            );
        } else {
            println!("all affected crates have been version-bumped");
        }
    }

    fn display_version_bump_issue_table(
        title: &str,
        issues: &[(&VersionBumpStatus, &crate::utils::severity::Issue)],
    ) {
        println!("{}:", title);

        let name_width = issues
            .iter()
            .map(|(s, _)| s.crate_id.display_name().len())
            .max()
            .unwrap_or(MIN_CRATE_NAME_WIDTH)
            .max(MIN_CRATE_NAME_WIDTH);

        let version_width = issues
            .iter()
            .map(|(s, _)| {
                s.current_version
                    .to_string()
                    .len()
                    .max(s.base_version.to_string().len())
            })
            .max()
            .unwrap_or(MIN_VERSION_WIDTH)
            .max(MIN_VERSION_WIDTH);

        println!(
            "  {:<name_width$}  {:<version_width$}  {:<version_width$}  Issue",
            "Crate",
            "Current",
            "Base",
            name_width = name_width,
            version_width = version_width
        );
        println!(
            "  {}  {}  {}  -----",
            "-".repeat(name_width),
            "-".repeat(version_width),
            "-".repeat(version_width)
        );

        for (status, issue) in issues {
            println!(
                "  {:<name_width$}  {:<version_width$}  {:<version_width$}  {}",
                status.crate_id.display_name(),
                status.current_version,
                status.base_version,
                issue.message,
                name_width = name_width,
                version_width = version_width
            );
        }
        println!();
    }

    /// display detailed version bump issues split by severity
    pub fn display_issues(&self) {
        use crate::utils::severity::IssueSeverity;

        // collect errors and warnings separately
        let mut errors: Vec<(&VersionBumpStatus, &crate::utils::severity::Issue)> = Vec::new();
        let mut warnings: Vec<(&VersionBumpStatus, &crate::utils::severity::Issue)> = Vec::new();

        for status in self.crate_versions.values() {
            for issue in &status.issues {
                if issue.severity == IssueSeverity::Error {
                    errors.push((status, issue));
                } else {
                    warnings.push((status, issue));
                }
            }
        }

        if errors.is_empty() && warnings.is_empty() {
            return;
        }

        if !errors.is_empty() {
            Self::display_version_bump_issue_table("version bump errors", &errors);
        }

        if !warnings.is_empty() {
            Self::display_version_bump_issue_table("version bump warnings", &warnings);
        }
    }
}

/// represents the impact analysis of changes in a repository
#[derive(Debug, Clone)]
pub struct ChangeImpactAnalysis {
    /// files that were changed between the two git refs
    pub changed_files: Vec<PathBuf>,
    /// crates that directly contain changed files
    pub directly_affected_crates: Vec<CrateId>,
    /// all crates affected by changes (including dependents)
    pub all_affected_crates: Vec<CrateId>,
    /// crates that need to be rebuilt due to changes
    pub needs_rebuild: Vec<CrateId>,
    /// mapping of files to the crates they belong to
    pub file_to_crate_mapping: HashMap<PathBuf, CrateId>,
}

impl ChangeImpactAnalysis {
    /// creates a new empty change impact analysis
    pub fn new() -> Self {
        Self {
            changed_files: Vec::new(),
            directly_affected_crates: Vec::new(),
            all_affected_crates: Vec::new(),
            needs_rebuild: Vec::new(),
            file_to_crate_mapping: HashMap::new(),
        }
    }

    /// returns the number of directly affected crates
    pub fn direct_impact_count(&self) -> usize {
        self.directly_affected_crates.len()
    }

    /// returns the number of all affected crates (including transitive dependencies)
    pub fn total_impact_count(&self) -> usize {
        self.all_affected_crates.len()
    }

    /// checks if a specific crate is affected by the changes
    pub fn is_crate_affected(&self, crate_id: &CrateId) -> bool {
        self.all_affected_crates.contains(crate_id)
    }

    /// returns the files that belong to a specific crate
    pub fn get_changed_files_for_crate(&self, crate_id: &CrateId) -> Vec<PathBuf> {
        self.changed_files
            .iter()
            .filter(|file| {
                self.file_to_crate_mapping
                    .get(*file)
                    .map(|id| id == crate_id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }
}

impl Default for ChangeImpactAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

impl CrateDependencyGraph {
    /// analyzes the impact of git changes between two refs on the dependency graph
    ///
    /// this method:
    /// 1. detects changed files between the two git refs
    /// 2. maps changed files to their owning crates
    /// 3. finds all crates affected by changes (including transitive dependencies)
    /// 4. determines which crates need to be rebuilt
    pub fn analyze_git_changes<P: AsRef<Path>>(
        &self,
        repo_path: P,
        from_ref: &GitRef,
        to_ref: &GitRef,
    ) -> Result<ChangeImpactAnalysis> {
        let repo_path = repo_path.as_ref();

        // get changed files from git
        let changed_files = GitOps::list_changed_files(repo_path, from_ref, to_ref)?;

        // create file-to-crate mapping for all crates
        let file_mapping = self.build_file_to_crate_mapping()?;

        // convert relative paths to absolute by joining with repo_path
        let absolute_changed_files: Vec<PathBuf> = changed_files
            .changes
            .iter()
            .map(|c| repo_path.join(&c.path))
            .collect();

        // map changed files to affected crates
        let directly_affected = self.map_files_to_crates(&absolute_changed_files, &file_mapping);

        // find all crates that depend on the directly affected crates
        let all_affected = self.find_all_affected_crates(&directly_affected);

        // determine which crates need rebuild (all affected crates)
        let needs_rebuild = all_affected.clone();

        Ok(ChangeImpactAnalysis {
            changed_files: changed_files.changes.into_iter().map(|c| c.path).collect(),
            directly_affected_crates: directly_affected,
            all_affected_crates: all_affected,
            needs_rebuild,
            file_to_crate_mapping: file_mapping,
        })
    }

    /// analyzes changes in the working directory compared to a git ref
    pub fn analyze_working_directory_changes<P: AsRef<Path>>(
        &self,
        repo_path: P,
    ) -> Result<ChangeImpactAnalysis> {
        let repo_path = repo_path.as_ref();

        // get changed files in working directory
        let changed_files = GitOps::list_working_directory_changes(repo_path)?;

        // create file-to-crate mapping for all crates
        let file_mapping = self.build_file_to_crate_mapping()?;

        // convert relative paths to absolute by joining with repo_path
        let absolute_changed_files: Vec<PathBuf> = changed_files
            .changes
            .iter()
            .map(|c| repo_path.join(&c.path))
            .collect();

        // map changed files to affected crates
        let directly_affected = self.map_files_to_crates(&absolute_changed_files, &file_mapping);

        // find all crates that depend on the directly affected crates
        let all_affected = self.find_all_affected_crates(&directly_affected);

        // determine which crates need rebuild (all affected crates)
        let needs_rebuild = all_affected.clone();

        Ok(ChangeImpactAnalysis {
            changed_files: changed_files.changes.into_iter().map(|c| c.path).collect(),
            directly_affected_crates: directly_affected,
            all_affected_crates: all_affected,
            needs_rebuild,
            file_to_crate_mapping: file_mapping,
        })
    }

    /// maps file paths to their owning crates
    ///
    /// returns a list of crate IDs that contain any of the given files
    fn map_files_to_crates(
        &self,
        file_paths: &[PathBuf],
        file_mapping: &HashMap<PathBuf, CrateId>,
    ) -> Vec<CrateId> {
        let mut affected_crates = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for file_path in file_paths {
            // try to find which crate this file belongs to
            if let Some(crate_id) = file_mapping.get(file_path)
                && seen.insert(crate_id.clone())
            {
                affected_crates.push(crate_id.clone());
            }
        }

        affected_crates
    }

    /// builds a mapping from file paths to crate IDs
    ///
    /// this method scans all crates and creates a map of which files belong to which crate
    fn build_file_to_crate_mapping(&self) -> Result<HashMap<PathBuf, CrateId>> {
        let mut mapping = HashMap::new();

        for crate_info in self.crates.values() {
            // get the crate's root directory
            let crate_root = &crate_info.path;

            // find all files in the crate directory
            let files = self.scan_crate_files(crate_root)?;

            // map each file to this crate
            for file in files {
                mapping.insert(file, crate_info.id.clone());
            }
        }

        Ok(mapping)
    }

    /// scans a crate directory for all relevant files
    fn scan_crate_files(&self, crate_root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        // common source directories to scan
        let source_dirs = ["src", "tests", "benches", "examples", "build.rs"];

        for source_dir in &source_dirs {
            let dir_path = crate_root.join(source_dir);

            if !dir_path.exists() {
                continue;
            }

            if dir_path.is_file() {
                // handle build.rs as a file
                files.push(dir_path);
            } else if dir_path.is_dir() {
                // recursively scan directory
                Self::scan_directory_recursive(&dir_path, &mut files)?;
            }
        }

        // also include Cargo.toml
        let cargo_toml = crate_root.join("Cargo.toml");
        if cargo_toml.exists() {
            files.push(cargo_toml);
        }

        Ok(files)
    }

    /// recursively scans a directory for files
    fn scan_directory_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    files.push(path);
                } else if path.is_dir() {
                    Self::scan_directory_recursive(&path, files)?;
                }
            }
        }
        Ok(())
    }

    /// finds all crates affected by changes to the given crates
    ///
    /// this includes the directly affected crates and all crates that depend on them
    /// (transitively following the dependency graph)
    fn find_all_affected_crates(&self, directly_affected: &[CrateId]) -> Vec<CrateId> {
        let mut affected = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for crate_id in directly_affected {
            self.collect_affected_recursive(crate_id, &mut affected, &mut visited);
        }

        affected
    }

    /// recursively collects all crates affected by changes to a given crate
    fn collect_affected_recursive(
        &self,
        crate_id: &CrateId,
        affected: &mut Vec<CrateId>,
        visited: &mut std::collections::HashSet<CrateId>,
    ) {
        // avoid infinite loops in case of cycles
        if !visited.insert(crate_id.clone()) {
            return;
        }

        // add this crate to affected list
        affected.push(crate_id.clone());

        // find all crates that depend on this one
        let dependents = self.get_dependents(crate_id);

        // recursively process dependents
        for dependent in dependents {
            self.collect_affected_recursive(dependent, affected, visited);
        }
    }

    /// analyze version bumps for affected crates
    ///
    /// compares versions of affected crates between two git refs
    /// to determine if they have been properly bumped
    pub fn analyze_version_bumps<P: AsRef<Path>>(
        &self,
        repo_path: P,
        base_ref: &GitRef,
        affected_crates: &[CrateId],
        directly_changed: &[CrateId],
        direct_severity: &crate::utils::severity_config::SeverityConfig,
        transitive_severity: &crate::utils::severity_config::SeverityConfig,
    ) -> Result<VersionBumpAnalysis> {
        let repo_path = repo_path.as_ref();
        let mut crate_versions = HashMap::new();
        let mut crates_needing_bump = Vec::new();
        let mut crates_bumped = Vec::new();
        let mut total_errors = 0;
        let mut total_warnings = 0;

        for crate_id in affected_crates {
            // get crate info from current state
            let crate_info = match self.crates.get(crate_id) {
                Some(info) => info,
                None => continue,
            };

            // current version is what we already have
            let current_version = match Version::parse(&crate_info.version) {
                Ok(v) => v,
                Err(_) => continue, // skip if version can't be parsed
            };

            // get base version from git ref
            let base_version = match Self::read_crate_version_at_ref(
                repo_path,
                base_ref,
                &crate_info.cargo_toml_path,
            ) {
                Ok(Some(v)) => v,
                _ => {
                    // if we can't read base version, assume it's the same as current
                    // (might be a new crate)
                    current_version.clone()
                }
            };

            // check if version was bumped
            let is_bumped = current_version > base_version;
            let is_directly_changed = directly_changed.contains(crate_id);

            // determine which severity config to use
            let severity_config = if is_directly_changed {
                direct_severity
            } else {
                transitive_severity
            };

            let mut status = VersionBumpStatus {
                crate_id: crate_id.clone(),
                base_version: base_version.clone(),
                current_version: current_version.clone(),
                is_bumped,
                is_directly_changed,
                issues: Vec::new(),
            };

            if is_bumped {
                crates_bumped.push(crate_id.clone());
            } else {
                crates_needing_bump.push(crate_id.clone());

                // create issue for missing version bump
                let severity =
                    severity_config.get_severity(crate::utils::severity::IssueType::NoVersionBump);
                let message = format!(
                    "version not bumped (current: {}, base: {})",
                    current_version, base_version
                );
                let issue = Issue::new(
                    severity,
                    crate::utils::severity::IssueType::NoVersionBump,
                    message,
                );
                status.add_issue(issue);

                if severity == crate::utils::severity::IssueSeverity::Error {
                    total_errors += 1;
                } else {
                    total_warnings += 1;
                }
            }

            crate_versions.insert(crate_id.clone(), status);
        }

        Ok(VersionBumpAnalysis {
            crate_versions,
            crates_needing_bump,
            crates_bumped,
            total_errors,
            total_warnings,
        })
    }

    /// read crate version from a specific git ref
    fn read_crate_version_at_ref<P: AsRef<Path>>(
        repo_path: P,
        git_ref: &GitRef,
        cargo_toml_path: &Path,
    ) -> Result<Option<Version>> {
        let repo_path = repo_path.as_ref();

        // get the relative path from repo root
        let relative_path = match cargo_toml_path.strip_prefix(repo_path) {
            Ok(p) => p,
            Err(_) => cargo_toml_path, // already relative
        };

        // use git show to read file at specific ref
        let ref_str = match git_ref {
            GitRef::Head => "HEAD".to_string(),
            GitRef::Branch(name) => name.clone(),
            GitRef::Tag(name) => format!("refs/tags/{}", name),
            GitRef::Hash(hash) => hash.clone(),
        };

        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("show")
            .arg(format!("{}:{}", ref_str, relative_path.display()))
            .output()
            .map_err(crate::error::Error::IoError)?;

        if !output.status.success() {
            // file doesn't exist at this ref (maybe new crate)
            return Ok(None);
        }

        // parse the Cargo.toml content
        let content = String::from_utf8_lossy(&output.stdout);
        let toml_doc: toml::Table = match content.parse() {
            Ok(doc) => doc,
            Err(_) => return Ok(None),
        };

        // extract version from [package] section
        let version_str = toml_doc
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");

        match Version::parse(version_str) {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_impact_analysis_new() {
        let analysis = ChangeImpactAnalysis::new();
        assert_eq!(analysis.direct_impact_count(), 0);
        assert_eq!(analysis.total_impact_count(), 0);
        assert!(analysis.changed_files.is_empty());
    }

    #[test]
    fn test_change_impact_analysis_default() {
        let analysis = ChangeImpactAnalysis::default();
        assert_eq!(analysis.direct_impact_count(), 0);
    }
}
