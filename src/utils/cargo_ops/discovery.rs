use super::types::{CrateId, CrateInfo, Workspace};
use crate::error::{Error, Result};
use crate::utils::filesystem::FilesystemExplorer;
use crate::utils::toml_ops::TomlReader;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CargoDiscovery;

impl CargoDiscovery {
    /// Discover all workspaces in a repository
    pub fn discover_workspaces<P: AsRef<Path>>(repo_root: P) -> Result<Vec<Workspace>> {
        let repo_root = repo_root.as_ref();
        let explorer = FilesystemExplorer::new(repo_root.to_string_lossy().to_string());

        // Find all Cargo.toml files
        let root_dir = explorer.scan_from_root().map_err(|e| {
            Error::IoError(std::io::Error::other(format!(
                "Failed to scan repository: {}",
                e
            )))
        })?;

        let cargo_toml_files = explorer
            .find_files_by_extension(&root_dir, "toml")
            .into_iter()
            .filter(|path| path.ends_with("Cargo.toml"))
            .collect::<Vec<_>>();

        let mut workspaces = Vec::new();

        for cargo_toml_path in cargo_toml_files {
            let toml_path = PathBuf::from(cargo_toml_path);

            // Try to read and parse the Cargo.toml
            if let Ok(toml_doc) = TomlReader::read_file(&toml_path) {
                // Check if this is a workspace Cargo.toml
                if toml_doc.has_table("workspace")
                    && let Some(workspace) = Self::parse_workspace(&toml_path, &toml_doc)?
                {
                    workspaces.push(workspace);
                }
            }
        }

        Ok(workspaces)
    }

    /// Parse a workspace from a Cargo.toml file
    fn parse_workspace(
        cargo_toml_path: &Path,
        toml_doc: &crate::utils::toml_ops::TomlDocument,
    ) -> Result<Option<Workspace>> {
        let workspace_table =
            toml_doc
                .get_table("workspace")
                .ok_or_else(|| Error::WorkspaceError {
                    reason: "No workspace table found".to_string(),
                })?;

        // Get workspace root directory
        let workspace_root = cargo_toml_path
            .parent()
            .ok_or_else(|| Error::WorkspaceError {
                reason: "Invalid workspace path".to_string(),
            })?
            .to_path_buf();

        // Get workspace name (use directory name)
        let workspace_name = workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Parse members
        let members = if let Some(members_array) = workspace_table.get("members") {
            Self::parse_workspace_members(members_array)?
        } else {
            Vec::new()
        };

        Ok(Some(Workspace::new(
            workspace_name,
            workspace_root,
            members,
        )))
    }

    /// Parse workspace members from TOML array
    fn parse_workspace_members(members_value: &toml::Value) -> Result<Vec<String>> {
        let members_array = members_value
            .as_array()
            .ok_or_else(|| Error::WorkspaceError {
                reason: "Workspace members must be an array".to_string(),
            })?;

        let mut members = Vec::new();
        for member in members_array {
            if let Some(member_str) = member.as_str() {
                members.push(member_str.to_string());
            }
        }

        Ok(members)
    }

    /// Discover all crates in a workspace
    pub fn discover_crates_in_workspace(workspace: &Workspace) -> Result<Vec<CrateInfo>> {
        let mut crates = Vec::new();

        for member_pattern in &workspace.members {
            let member_crates = Self::resolve_workspace_member(
                &workspace.root_path,
                member_pattern,
                &workspace.name,
            )?;
            crates.extend(member_crates);
        }

        Ok(crates)
    }

    /// Resolve a workspace member pattern to actual crates
    fn resolve_workspace_member(
        workspace_root: &Path,
        member_pattern: &str,
        workspace_name: &str,
    ) -> Result<Vec<CrateInfo>> {
        let mut crates = Vec::new();

        // Handle glob patterns (basic support for now)
        if member_pattern.contains('*') {
            // For now, implement basic glob support
            // This could be enhanced with a proper glob library later
            crates.extend(Self::resolve_glob_pattern(
                workspace_root,
                member_pattern,
                workspace_name,
            )?);
        } else {
            // Direct path
            let member_path = workspace_root.join(member_pattern);
            if let Some(crate_info) = Self::parse_crate_at_path(&member_path, workspace_name)? {
                crates.push(crate_info);
            }
        }

        Ok(crates)
    }

    /// Basic glob pattern resolution
    fn resolve_glob_pattern(
        workspace_root: &Path,
        pattern: &str,
        workspace_name: &str,
    ) -> Result<Vec<CrateInfo>> {
        let mut crates = Vec::new();

        // Simple glob: handle patterns like "crates/*"
        if let Some(prefix) = pattern.strip_suffix("/*") {
            let base_dir = workspace_root.join(prefix);
            if base_dir.exists()
                && base_dir.is_dir()
                && let Ok(entries) = std::fs::read_dir(&base_dir)
            {
                for entry in entries.flatten() {
                    if entry.path().is_dir()
                        && let Some(crate_info) =
                            Self::parse_crate_at_path(&entry.path(), workspace_name)?
                    {
                        crates.push(crate_info);
                    }
                }
            }
        }

        Ok(crates)
    }

    /// Parse a crate at a specific path
    fn parse_crate_at_path(path: &Path, workspace_name: &str) -> Result<Option<CrateInfo>> {
        let cargo_toml_path = path.join("Cargo.toml");

        if !cargo_toml_path.exists() {
            return Ok(None);
        }

        let toml_doc =
            TomlReader::read_file(&cargo_toml_path).map_err(|e| Error::FileReadError {
                path: cargo_toml_path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to read Cargo.toml: {}", e),
                ),
            })?;

        // Check if this is a crate (has [package]) or workspace-only
        // workspace-only Cargo.toml files don't have a [package] section
        let Some(package_table) = toml_doc.get_table("package") else {
            return Ok(None);
        };

        let crate_name = package_table
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::WorkspaceError {
                reason: "No package name found in Cargo.toml".to_string(),
            })?
            .to_string();

        // version field is optional - some test packages don't have it
        let version = package_table
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0") // default for packages without version
            .to_string();

        let crate_id = CrateId::new(workspace_name.to_string(), crate_name);
        let crate_info = CrateInfo::new(crate_id, version, path.to_path_buf());

        Ok(Some(crate_info))
    }

    /// Discover all crates across all workspaces in a repository
    pub fn discover_all_crates<P: AsRef<Path>>(repo_root: P) -> Result<Vec<CrateInfo>> {
        let workspaces = Self::discover_workspaces(&repo_root)?;
        let mut all_crates = Vec::new();

        for workspace in workspaces {
            let workspace_crates = Self::discover_crates_in_workspace(&workspace)?;
            all_crates.extend(workspace_crates);
        }

        Ok(all_crates)
    }

    /// Parse local dependencies from a crate's Cargo.toml
    pub fn parse_local_dependencies(
        crate_info: &CrateInfo,
        all_crates: &[CrateInfo],
    ) -> Result<Vec<String>> {
        let deps_with_types = Self::parse_local_dependencies_with_types(crate_info, all_crates)?;
        Ok(deps_with_types.into_iter().map(|(name, _)| name).collect())
    }

    /// Parse local dependencies from a crate's Cargo.toml with their dependency types
    pub fn parse_local_dependencies_with_types(
        crate_info: &CrateInfo,
        all_crates: &[CrateInfo],
    ) -> Result<Vec<(String, super::types::DependencyType)>> {
        use super::types::DependencyType;

        let toml_doc = TomlReader::read_file(&crate_info.cargo_toml_path).map_err(|e| {
            Error::FileReadError {
                path: crate_info.cargo_toml_path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to read Cargo.toml: {}", e),
                ),
            }
        })?;

        let mut local_deps = Vec::new();

        // Create a lookup map for all known local crates
        let crate_name_map: HashMap<&str, &CrateInfo> =
            all_crates.iter().map(|c| (c.id.name.as_str(), c)).collect();

        // Check different dependency sections with their types
        let dependency_sections = [
            ("dependencies", DependencyType::Normal),
            ("dev-dependencies", DependencyType::Dev),
            ("build-dependencies", DependencyType::Build),
        ];

        for (section, dep_type) in dependency_sections {
            if let Some(deps_table) = toml_doc.get_table(section) {
                for (dep_name, dep_value) in deps_table {
                    // Check if this dependency refers to a local crate
                    let is_local = if crate_name_map.contains_key(dep_name.as_str()) {
                        // If it's a known local crate name, check if it's a path dependency or just same name
                        match dep_value {
                            toml::Value::String(_) => {
                                // Simple version dependency - only local if it's a known crate
                                true
                            }
                            toml::Value::Table(_dep_table) => {
                                // Complex dependency - assume local if name matches
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };

                    if is_local {
                        local_deps.push((dep_name.clone(), dep_type));
                    }
                }
            }
        }

        Ok(local_deps)
    }
}
