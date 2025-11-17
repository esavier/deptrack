// builder for creating test repositories

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// represents a test crate to be created
#[derive(Debug, Clone)]
pub struct TestCrate {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub files: HashMap<String, String>, // relative path -> content
}

impl TestCrate {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
            dependencies: Vec::new(),
            files: HashMap::new(),
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.files.insert(path.into(), content.into());
        self
    }
}

/// represents a test workspace
#[derive(Debug, Clone)]
pub struct TestWorkspace {
    pub name: String,
    pub crates: Vec<TestCrate>,
}

impl TestWorkspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            crates: Vec::new(),
        }
    }

    pub fn crate_entry(mut self, crate_def: TestCrate) -> Self {
        self.crates.push(crate_def);
        self
    }
}

/// builder for test repositories
pub struct TestRepoBuilder {
    workspaces: Vec<TestWorkspace>,
    temp_dir: Option<TempDir>,
    use_temp: bool,
    git_init: bool,
}

impl TestRepoBuilder {
    /// create a new builder that uses a temporary directory
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            temp_dir: None,
            use_temp: true,
            git_init: true,
        }
    }

    /// create a builder that uses a specific path (useful for debugging)
    pub fn new_at_path() -> Self {
        Self {
            workspaces: Vec::new(),
            temp_dir: None,
            use_temp: false,
            git_init: true,
        }
    }

    /// disable git initialization
    pub fn no_git(mut self) -> Self {
        self.git_init = false;
        self
    }

    /// add a workspace to the repository
    pub fn workspace(mut self, workspace: TestWorkspace) -> Self {
        self.workspaces.push(workspace);
        self
    }

    /// build the repository and return the path
    pub fn build(mut self) -> Result<TestRepository, Box<dyn std::error::Error>> {
        let repo_path = if self.use_temp {
            let temp_dir = TempDir::new()?;
            let path = temp_dir.path().to_path_buf();
            self.temp_dir = Some(temp_dir);
            path
        } else {
            let path = PathBuf::from("/tmp/deptrack_test_repo");
            if path.exists() {
                fs::remove_dir_all(&path)?;
            }
            fs::create_dir_all(&path)?;
            path
        };

        // initialize git if requested
        if self.git_init {
            Self::init_git(&repo_path)?;
        }

        // create workspaces and crates
        for workspace in &self.workspaces {
            self.create_workspace(&repo_path, workspace)?;
        }

        // create initial commit
        if self.git_init {
            Self::create_commit(&repo_path, "Initial commit")?;
        }

        Ok(TestRepository {
            path: repo_path,
            _temp_dir: self.temp_dir,
            workspaces: self.workspaces,
        })
    }

    fn init_git(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .arg("init")
            .current_dir(path)
            .output()?;

        // configure git for tests
        std::process::Command::new("git")
            .args(["config", "user.email", "test@deptrack.test"])
            .current_dir(path)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.name", "Deptrack Test"])
            .current_dir(path)
            .output()?;

        // disable GPG signing
        std::process::Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(path)
            .output()?;

        Ok(())
    }

    fn create_commit(path: &Path, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;

        std::process::Command::new("git")
            .args(["commit", "-m", message, "--no-gpg-sign"])
            .current_dir(path)
            .output()?;

        Ok(())
    }

    fn create_workspace(
        &self,
        repo_path: &Path,
        workspace: &TestWorkspace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workspace_dir = repo_path.join(&workspace.name);
        fs::create_dir_all(&workspace_dir)?;

        // create workspace Cargo.toml
        let member_paths: Vec<String> = workspace
            .crates
            .iter()
            .map(|c| c.name.to_string())
            .collect();

        let workspace_toml = format!(
            "[workspace]\nmembers = [\n{}\n]\nresolver = \"2\"\n",
            member_paths
                .iter()
                .map(|p| format!("    \"{}\"", p))
                .collect::<Vec<_>>()
                .join(",\n")
        );

        fs::write(workspace_dir.join("Cargo.toml"), workspace_toml)?;

        // create each crate
        for crate_def in &workspace.crates {
            self.create_crate(&workspace_dir, workspace, crate_def)?;
        }

        Ok(())
    }

    fn create_crate(
        &self,
        workspace_dir: &Path,
        workspace: &TestWorkspace,
        crate_def: &TestCrate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let crate_dir = workspace_dir.join(&crate_def.name);
        fs::create_dir_all(&crate_dir)?;

        // create src directory
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // create Cargo.toml
        let mut cargo_toml = format!(
            "[package]\nname = \"{}\"\nversion = \"{}\"\nedition = \"2021\"\n\n",
            crate_def.name, crate_def.version
        );

        if !crate_def.dependencies.is_empty() {
            cargo_toml.push_str("[dependencies]\n");
            for dep in &crate_def.dependencies {
                // determine if this is a local dependency
                let dep_path = self.find_dependency_path(workspace, dep);
                if let Some(path) = dep_path {
                    cargo_toml.push_str(&format!("{} = {{ path = \"{}\" }}\n", dep, path));
                } else {
                    // external dependency
                    cargo_toml.push_str(&format!("{} = \"*\"\n", dep));
                }
            }
        }

        fs::write(crate_dir.join("Cargo.toml"), cargo_toml)?;

        // create default lib.rs if not provided
        if !crate_def.files.contains_key("src/lib.rs") {
            let default_lib = format!(
                "// {}\n\npub fn {}_function() -> String {{\n    \"{}\".to_string()\n}}\n",
                crate_def.name, crate_def.name, crate_def.name
            );
            fs::write(src_dir.join("lib.rs"), default_lib)?;
        }

        // create custom files
        for (rel_path, content) in &crate_def.files {
            let file_path = crate_dir.join(rel_path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(file_path, content)?;
        }

        Ok(())
    }

    fn find_dependency_path(
        &self,
        current_workspace: &TestWorkspace,
        dep_name: &str,
    ) -> Option<String> {
        // first check in current workspace
        if current_workspace.crates.iter().any(|c| c.name == dep_name) {
            return Some(format!("../{}", dep_name));
        }

        // check in other workspaces
        for workspace in &self.workspaces {
            if workspace.name == current_workspace.name {
                continue;
            }

            if workspace.crates.iter().any(|c| c.name == dep_name) {
                return Some(format!("../../{}/{}", workspace.name, dep_name));
            }
        }

        None
    }
}

impl Default for TestRepoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// represents a built test repository
pub struct TestRepository {
    pub path: PathBuf,
    _temp_dir: Option<TempDir>,
    pub workspaces: Vec<TestWorkspace>,
}

impl TestRepository {
    /// get the path to the repository
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// create a new branch
    pub fn create_branch(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["checkout", "-b", name])
            .current_dir(&self.path)
            .output()?;
        Ok(())
    }

    /// checkout a branch
    pub fn checkout(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["checkout", name])
            .current_dir(&self.path)
            .output()?;
        Ok(())
    }

    /// modify a file in a crate
    pub fn modify_file(
        &self,
        workspace: &str,
        crate_name: &str,
        rel_path: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = self.path.join(workspace).join(crate_name).join(rel_path);
        fs::write(file_path, content)?;
        Ok(())
    }

    /// update crate version
    pub fn update_version(
        &self,
        workspace: &str,
        crate_name: &str,
        new_version: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cargo_toml_path = self
            .path
            .join(workspace)
            .join(crate_name)
            .join("Cargo.toml");

        let content = fs::read_to_string(&cargo_toml_path)?;
        let updated = content
            .lines()
            .map(|line| {
                if line.starts_with("version = ") {
                    format!("version = \"{}\"", new_version)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(cargo_toml_path, updated)?;
        Ok(())
    }

    /// stage changes
    pub fn stage_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&self.path)
            .output()?;
        Ok(())
    }

    /// create a commit
    pub fn commit(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["commit", "-m", message, "--no-gpg-sign"])
            .current_dir(&self.path)
            .output()?;
        Ok(())
    }

    /// get current branch name
    pub fn current_branch(&self) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.path)
            .output()?;

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }
}
