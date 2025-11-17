use std::path::Path;
use std::process::Command;
use crate::error::{Error, Result};

/// Test git repository utilities for controlled testing
pub struct TestGitRepo {
    repo_path: std::path::PathBuf,
}

impl TestGitRepo {
    /// Initialize a new git repository at the given path
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let repo_path = path.to_path_buf();

        // Initialize git repository using command line git
        let output = Command::new("git")
            .args(&["init"])
            .current_dir(path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to initialize git repository: {}", String::from_utf8_lossy(&output.stderr))
            ))));
        }

        // Configure user for commits
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        // Disable GPG signing for tests
        Command::new("git")
            .args(&["config", "commit.gpgsign", "false"])
            .current_dir(path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        Ok(Self { repo_path })
    }

    /// Add all files and create a commit
    pub fn add_all_and_commit(&self, message: &str) -> Result<()> {
        // Add all files to staging
        let output = Command::new("git")
            .args(&["add", "."])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to add files: {}", String::from_utf8_lossy(&output.stderr))
            ))));
        }

        // Create commit
        let output = Command::new("git")
            .args(&["commit", "-m", message])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create commit: {}", String::from_utf8_lossy(&output.stderr))
            ))));
        }

        Ok(())
    }

    /// Create a new branch
    pub fn create_branch(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .args(&["branch", branch_name])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create branch '{}': {}", branch_name, String::from_utf8_lossy(&output.stderr))
            ))));
        }

        Ok(())
    }

    /// Checkout a branch
    pub fn checkout_branch(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .args(&["checkout", branch_name])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to checkout branch '{}': {}", branch_name, String::from_utf8_lossy(&output.stderr))
            ))));
        }

        Ok(())
    }

    /// Modify a file and commit the change
    pub fn modify_file_and_commit(&self, file_path: &str, content: &str, commit_message: &str) -> Result<()> {
        let full_path = self.repo_path.join(file_path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::IoError(e))?;
        }

        // Write the file
        std::fs::write(full_path, content)
            .map_err(|e| Error::IoError(e))?;

        // Add and commit
        self.add_all_and_commit(commit_message)
    }

    /// Get the repository path
    pub fn path(&self) -> &Path {
        &self.repo_path
    }

    /// Get current HEAD commit ID
    pub fn head_commit_id(&self) -> Result<String> {
        let output = Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get HEAD commit: {}", String::from_utf8_lossy(&output.stderr))
            ))));
        }

        let commit_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(commit_id)
    }

    /// Get list of branches
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(&["branch", "--format=%(refname:short)"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::IoError(e))?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to list branches: {}", String::from_utf8_lossy(&output.stderr))
            ))));
        }

        let branches = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(branches)
    }
}