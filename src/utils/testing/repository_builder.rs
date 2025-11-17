use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use crate::error::{Error, Result};
use super::{TestRepository, TestGitRepo};

#[derive(Debug, Clone)]
pub struct TestCrate {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub path: PathBuf,
    pub workspace: String,
}

impl TestCrate {
    pub fn new(name: String, version: String, path: PathBuf, workspace: String) -> Self {
        Self {
            name,
            version,
            dependencies: Vec::new(),
            path,
            workspace,
        }
    }

    pub fn with_dependency(mut self, dep_name: String) -> Self {
        self.dependencies.push(dep_name);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TestWorkspace {
    pub name: String,
    pub path: PathBuf,
    pub crates: Vec<TestCrate>,
    pub members: Vec<String>,
}

impl TestWorkspace {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            crates: Vec::new(),
            members: Vec::new(),
        }
    }

    pub fn add_crate(&mut self, test_crate: TestCrate) {
        let relative_path = test_crate.path.strip_prefix(&self.path)
            .unwrap_or(&test_crate.path)
            .to_string_lossy()
            .to_string();

        self.members.push(relative_path);
        self.crates.push(test_crate);
    }
}

#[derive(Debug, Clone)]
struct CrateSpec {
    name: String,
    version: String,
    workspace: String,
    dependencies: Vec<String>,
}

/// Builder for creating test repositories with controlled structure
pub struct TestRepositoryBuilder {
    crates: Vec<CrateSpec>,
    current_workspace: Option<String>,
}

impl TestRepositoryBuilder {
    pub fn new() -> Self {
        Self {
            crates: Vec::new(),
            current_workspace: Some("root".to_string()),
        }
    }

    /// Add a new workspace to the repository
    pub fn with_workspace(mut self, name: &str) -> Self {
        self.current_workspace = Some(name.to_string());
        self
    }

    /// Add a crate to the current workspace
    pub fn with_crate(mut self, name: &str, version: &str) -> Self {
        let workspace_name = self.current_workspace
            .as_ref()
            .expect("Must call with_workspace before with_crate")
            .clone();

        let crate_spec = CrateSpec {
            name: name.to_string(),
            version: version.to_string(),
            workspace: workspace_name,
            dependencies: Vec::new(),
        };

        self.crates.push(crate_spec);
        self
    }

    /// Add a dependency between crates
    pub fn with_dependency(mut self, from_crate: &str, to_crate: &str) -> Self {
        if let Some(crate_spec) = self.crates.iter_mut().find(|c| c.name == from_crate) {
            crate_spec.dependencies.push(to_crate.to_string());
        }
        self
    }

    /// Build the actual test repository
    pub fn build(self) -> Result<TestRepository> {
        let temp_dir = TempDir::new()
            .map_err(|e| Error::IoError(e))?;

        let repo_path = temp_dir.path();

        // Initialize git repository
        let git_repo = TestGitRepo::init(repo_path)?;

        // Group crates by workspace
        let mut workspace_map: HashMap<String, Vec<&CrateSpec>> = HashMap::new();
        for crate_spec in &self.crates {
            workspace_map
                .entry(crate_spec.workspace.clone())
                .or_insert_with(Vec::new)
                .push(crate_spec);
        }

        let mut workspaces = Vec::new();

        // Create each workspace
        for (workspace_name, crate_specs) in workspace_map {
            let workspace_path = if workspace_name == "root" {
                repo_path.to_path_buf()
            } else {
                repo_path.join(&workspace_name)
            };

            fs::create_dir_all(&workspace_path)
                .map_err(|e| Error::IoError(e))?;

            let mut workspace = TestWorkspace::new(workspace_name.clone(), workspace_path.clone());

            // Create crates in this workspace
            for crate_spec in crate_specs {
                let crate_path = if workspace_name == "root" {
                    workspace_path.join(&crate_spec.name)
                } else {
                    workspace_path.join("crates").join(&crate_spec.name)
                };

                fs::create_dir_all(&crate_path)
                    .map_err(|e| Error::IoError(e))?;

                // Create Cargo.toml for the crate
                self.create_crate_cargo_toml(&crate_path, crate_spec)?;

                // Create basic lib.rs
                self.create_basic_lib_rs(&crate_path)?;

                let test_crate = TestCrate::new(
                    crate_spec.name.clone(),
                    crate_spec.version.clone(),
                    crate_path,
                    workspace_name.clone()
                );
                workspace.add_crate(test_crate);
            }

            // Create workspace Cargo.toml if needed
            if workspace_name != "root" || workspace.crates.len() > 1 {
                self.create_workspace_cargo_toml(&workspace)?;
            }

            workspaces.push(workspace);
        }

        // Create initial git commit
        git_repo.add_all_and_commit("Initial commit")?;

        Ok(TestRepository {
            temp_dir,
            git_repo,
            workspaces,
        })
    }

    fn create_crate_cargo_toml(
        &self,
        crate_path: &std::path::Path,
        crate_spec: &CrateSpec,
    ) -> Result<()> {
        let mut toml_content = format!(
            r#"[package]
name = "{}"
version = "{}"
edition = "2021"

"#,
            crate_spec.name, crate_spec.version
        );

        if !crate_spec.dependencies.is_empty() {
            toml_content.push_str("[dependencies]\n");
            for dep in &crate_spec.dependencies {
                // For now, use simple relative path
                let dep_path = format!("../{}", dep);
                toml_content.push_str(&format!("{} = {{ path = \"{}\" }}\n", dep, dep_path));
            }
        }

        fs::write(crate_path.join("Cargo.toml"), toml_content)
            .map_err(|e| Error::IoError(e))?;

        Ok(())
    }

    fn create_workspace_cargo_toml(&self, workspace: &TestWorkspace) -> Result<()> {
        let members: Vec<String> = if workspace.name == "root" {
            workspace.crates.iter().map(|c| c.name.clone()).collect()
        } else {
            vec!["crates/*".to_string()]
        };

        let toml_content = format!(
            r#"[workspace]
members = {:?}
resolver = "2"

"#,
            members
        );

        fs::write(workspace.path.join("Cargo.toml"), toml_content)
            .map_err(|e| Error::IoError(e))?;

        Ok(())
    }

    fn create_basic_lib_rs(&self, crate_path: &std::path::Path) -> Result<()> {
        let src_dir = crate_path.join("src");
        fs::create_dir_all(&src_dir)
            .map_err(|e| Error::IoError(e))?;

        let lib_content = "// Auto-generated test library\n\npub fn hello() -> &'static str {\n    \"Hello from test crate!\"\n}\n";

        fs::write(src_dir.join("lib.rs"), lib_content)
            .map_err(|e| Error::IoError(e))?;

        Ok(())
    }
}

impl Default for TestRepositoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}