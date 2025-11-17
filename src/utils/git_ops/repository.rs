use super::types::{ChangeType, ChangedFiles, FileChange, GitRef, GitRepository};
use crate::error::{Error, Result};
use gix;
use gix::bstr::ByteSlice;
use std::path::{Path, PathBuf};

pub struct GitOps;

impl GitOps {
    pub fn new() -> Self {
        Self
    }

    /// detect root of the repository (path as a result)
    pub fn detect_repository_root<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
        let path = path.as_ref();

        // if it's a file, use its parent directory for discovery
        let check_path = if path.is_file() {
            match path.parent() {
                Some(parent) => parent,
                None => {
                    return Err(Error::RepositoryNotFound {
                        path: path.to_path_buf(),
                    });
                }
            }
        } else {
            path
        };

        // use gix to discover the repository
        let repo = gix::discover(check_path)?;

        // get the working directory (repo root) or git dir for bare repos
        let root_path = if let Some(work_dir) = repo.work_dir() {
            work_dir.to_path_buf()
        } else {
            // bare repository - return the git directory
            repo.git_dir().to_path_buf()
        };

        Ok(root_path)
    }

    /// verify if the path is in the root of the repo
    pub fn is_repository_root<P: AsRef<Path>>(path: P) -> Result<bool> {
        let path = path.as_ref();

        // first check if it's a repository at all
        if !Self::is_repository(path)? {
            return Ok(false);
        }

        // get the actual root
        let root = Self::detect_repository_root(path)?;

        // canonicalize both paths for accurate comparison
        let canonical_path = path.canonicalize().map_err(Error::IoError)?;
        let canonical_root = root.canonicalize().map_err(Error::IoError)?;

        Ok(canonical_path == canonical_root)
    }

    /// detect if the given path is a repository (also if its a root)
    pub fn is_repository<P: AsRef<Path>>(path: P) -> Result<bool> {
        let path = path.as_ref();

        // if it's a file, check its parent directory
        let check_path = if path.is_file() {
            match path.parent() {
                Some(parent) => parent,
                None => return Ok(false),
            }
        } else {
            path
        };

        match gix::discover(check_path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // any error means it's not a repository
        }
    }

    /// get detailed repository information
    pub fn get_repository_info<P: AsRef<Path>>(path: P) -> Result<GitRepository> {
        let path = path.as_ref();

        // if it's a file, use its parent directory for discovery
        let check_path = if path.is_file() {
            match path.parent() {
                Some(parent) => parent,
                None => {
                    return Err(Error::RepositoryNotFound {
                        path: path.to_path_buf(),
                    });
                }
            }
        } else {
            path
        };

        let repo = gix::discover(check_path)?;

        let is_bare = repo.is_bare();
        let git_dir = repo.git_dir().to_path_buf();

        let root_path = if let Some(work_dir) = repo.work_dir() {
            work_dir.to_path_buf()
        } else {
            git_dir.clone()
        };

        Ok(GitRepository::new(root_path, is_bare, git_dir))
    }

    /// resolve git reference to commit hash
    pub fn resolve_ref<P: AsRef<Path>>(repo_path: P, git_ref: &GitRef) -> Result<String> {
        let repo_path = repo_path.as_ref();
        let repo = gix::discover(repo_path)?;

        let commit_id = match git_ref {
            GitRef::Hash(hash) => {
                // validate hash exists
                let object_id =
                    gix::ObjectId::from_hex(hash.as_bytes()).map_err(|_| Error::InvalidRef {
                        ref_name: hash.clone(),
                    })?;

                // verify object exists in repo
                if repo.find_object(object_id).is_ok() {
                    hash.clone()
                } else {
                    return Err(Error::RefNotFound {
                        ref_name: hash.clone(),
                    });
                }
            }
            GitRef::Head => {
                let head_commit = repo
                    .head_commit()
                    .map_err(|e| Error::GitError(Box::new(e)))?;
                head_commit.id().to_string()
            }
            GitRef::Branch(branch_name) => {
                // try to resolve branch reference
                let reference = repo
                    .find_reference(&format!("refs/heads/{}", branch_name))
                    .or_else(|_| {
                        repo.find_reference(&format!("refs/remotes/origin/{}", branch_name))
                    })
                    .or_else(|_| repo.find_reference(branch_name))
                    .map_err(|_| Error::RefNotFound {
                        ref_name: branch_name.clone(),
                    })?;

                // try to peel to commit
                let commit = reference
                    .into_fully_peeled_id()
                    .map_err(|e| Error::GitError(Box::new(e)))?;
                commit.to_string()
            }
            GitRef::Tag(tag_name) => {
                let reference = repo
                    .find_reference(&format!("refs/tags/{}", tag_name))
                    .map_err(|_| Error::RefNotFound {
                        ref_name: tag_name.clone(),
                    })?;

                let commit = reference
                    .into_fully_peeled_id()
                    .map_err(|e| Error::GitError(Box::new(e)))?;
                commit.to_string()
            }
        };

        Ok(commit_id)
    }

    /// list changed files between two git references
    pub fn list_changed_files<P: AsRef<Path>>(
        repo_path: P,
        from_ref: &GitRef,
        to_ref: &GitRef,
    ) -> Result<ChangedFiles> {
        let repo_path = repo_path.as_ref();
        let repo = gix::discover(repo_path)?;

        let from_hash = Self::resolve_ref(repo_path, from_ref)?;
        let to_hash = Self::resolve_ref(repo_path, to_ref)?;

        let mut changed_files = ChangedFiles::new(from_hash.clone(), to_hash.clone());

        // for now, implement a simple version that compares commit hashes
        // if they're the same, no changes
        if from_hash == to_hash {
            return Ok(changed_files);
        }

        // get commits
        let from_commit_id =
            gix::ObjectId::from_hex(from_hash.as_bytes()).map_err(|_| Error::InvalidRef {
                ref_name: from_hash.clone(),
            })?;
        let to_commit_id =
            gix::ObjectId::from_hex(to_hash.as_bytes()).map_err(|_| Error::InvalidRef {
                ref_name: to_hash.clone(),
            })?;

        let from_commit = repo
            .find_object(from_commit_id)
            .map_err(|e| Error::GitError(Box::new(e)))?
            .try_into_commit()
            .map_err(|_| {
                Error::GitError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Not a commit",
                )))
            })?;
        let to_commit = repo
            .find_object(to_commit_id)
            .map_err(|e| Error::GitError(Box::new(e)))?
            .try_into_commit()
            .map_err(|_| {
                Error::GitError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Not a commit",
                )))
            })?;

        // get tree ids
        let from_tree_id = from_commit
            .tree_id()
            .map_err(|e| Error::GitError(Box::new(e)))?;
        let to_tree_id = to_commit
            .tree_id()
            .map_err(|e| Error::GitError(Box::new(e)))?;

        // if tree ids are the same, no changes
        if from_tree_id == to_tree_id {
            return Ok(changed_files);
        }

        // use git command to get file changes (simpler than gix diff API for now)
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("diff")
            .arg("--name-status")
            .arg(&from_hash)
            .arg(&to_hash)
            .output()
            .map_err(Error::IoError)?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::other(
                "git diff failed",
            ))));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // parse git diff output format: "M\tpath/to/file"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let status = parts[0];
                let path = PathBuf::from(parts[1]);

                let change_type = match status.chars().next() {
                    Some('A') => ChangeType::Added,
                    Some('D') => ChangeType::Deleted,
                    Some('M') => ChangeType::Modified,
                    _ => ChangeType::Modified, // default to modified for unknown types
                };

                let is_binary = Self::_is_binary_file_path(&path).unwrap_or(false);
                changed_files.add_change(FileChange::new(path, change_type, is_binary));
            }
        }

        Ok(changed_files)
    }

    /// list files changed uniquely in the 'to' ref compared to 'from' ref
    /// uses three-dot syntax (from...to) to find changes since the merge base
    /// this excludes changes from 'from' that were merged into 'to'
    pub fn list_unique_changes<P: AsRef<Path>>(
        repo_path: P,
        from_ref: &GitRef,
        to_ref: &GitRef,
    ) -> Result<ChangedFiles> {
        let repo_path = repo_path.as_ref();
        let _repo = gix::discover(repo_path)?;

        let from_hash = Self::resolve_ref(repo_path, from_ref)?;
        let to_hash = Self::resolve_ref(repo_path, to_ref)?;

        let mut changed_files =
            ChangedFiles::new(format!("{}...{}", from_hash, to_hash), to_hash.clone());

        // if refs are the same, no changes
        if from_hash == to_hash {
            return Ok(changed_files);
        }

        // use git diff with three-dot syntax to compare from merge-base
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("diff")
            .arg("--name-status")
            .arg(format!("{}...{}", from_hash, to_hash))
            .output()
            .map_err(Error::IoError)?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::other(
                "git diff failed",
            ))));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // parse git diff output format: "M\tpath/to/file"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let status = parts[0];
                let path = PathBuf::from(parts[1]);

                let change_type = match status.chars().next() {
                    Some('A') => ChangeType::Added,
                    Some('D') => ChangeType::Deleted,
                    Some('M') => ChangeType::Modified,
                    _ => ChangeType::Modified, // default to modified for unknown types
                };

                let is_binary = Self::_is_binary_file_path(&path).unwrap_or(false);
                changed_files.add_change(FileChange::new(path, change_type, is_binary));
            }
        }

        Ok(changed_files)
    }

    /// list files changed in working directory (staged and unstaged)
    pub fn list_working_directory_changes<P: AsRef<Path>>(repo_path: P) -> Result<ChangedFiles> {
        let repo_path = repo_path.as_ref();
        let repo = gix::discover(repo_path)?;

        let head_commit = if let Ok(head_commit) = repo.head_commit() {
            head_commit.id().to_string()
        } else {
            "HEAD".to_string()
        };

        let mut changed_files = ChangedFiles::new(head_commit, "WORKING_DIR".to_string());

        // use git status --porcelain to get working directory changes
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .map_err(Error::IoError)?;

        if !output.status.success() {
            return Err(Error::GitError(Box::new(std::io::Error::other(
                "git status failed",
            ))));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.len() < 3 {
                continue;
            }

            // git status --porcelain format: "XY filename"
            // X = staged status, Y = unstaged status
            let status_chars = &line[0..2];
            let path = PathBuf::from(line[3..].trim());

            // determine change type based on status
            // we look at both staged (X) and unstaged (Y) status
            let change_type = if status_chars.contains('A') {
                ChangeType::Added
            } else if status_chars.contains('D') {
                ChangeType::Deleted
            } else if status_chars.contains('M') || status_chars.contains('?') {
                ChangeType::Modified
            } else {
                continue; // skip unknown status
            };

            let is_binary = Self::_is_binary_file_path(&path).unwrap_or(false);
            changed_files.add_change(FileChange::new(path, change_type, is_binary));
        }

        Ok(changed_files)
    }

    /// helper to detect if a file is binary
    fn _is_binary_file(
        _repo: &gix::Repository,
        path: &Path,
        _commit_id: &gix::ObjectId,
    ) -> Result<bool> {
        // simple heuristic: check file extension
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            let binary_extensions = [
                "png", "jpg", "jpeg", "gif", "pdf", "zip", "tar", "gz", "exe", "dll", "so",
            ];
            return Ok(binary_extensions.contains(&ext.as_str()));
        }
        Ok(false)
    }

    /// helper to detect if a file path is binary based on extension
    fn _is_binary_file_path(path: &Path) -> Result<bool> {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            let binary_extensions = [
                "png", "jpg", "jpeg", "gif", "pdf", "zip", "tar", "gz", "exe", "dll", "so",
            ];
            return Ok(binary_extensions.contains(&ext.as_str()));
        }
        Ok(false)
    }

    /// convenience method to compare current branch with another ref
    pub fn compare_with_branch<P: AsRef<Path>>(
        repo_path: P,
        target_branch: &str,
        include_working_dir: bool,
    ) -> Result<Vec<ChangedFiles>> {
        let mut results = Vec::new();

        // compare HEAD with target branch
        let head_ref = GitRef::Head;
        let target_ref = GitRef::from_string(target_branch);

        let branch_changes = Self::list_changed_files(&repo_path, &target_ref, &head_ref)?;
        results.push(branch_changes);

        // include working directory changes if requested
        if include_working_dir {
            let wd_changes = Self::list_working_directory_changes(&repo_path)?;
            if !wd_changes.changes.is_empty() {
                results.push(wd_changes);
            }
        }

        Ok(results)
    }

    /// list all branches in the repository
    pub fn list_branches<P: AsRef<Path>>(repo_path: P) -> Result<Vec<String>> {
        let repo_path = repo_path.as_ref();
        let repo = gix::discover(repo_path)?;

        let mut branches = Vec::new();

        // get all references using the iter platform
        let references = repo.references().map_err(Error::from_git_error)?;

        // iterate through all references
        for reference_result in references.all().map_err(Error::from_git_error)? {
            if let Ok(reference) = reference_result
                && let Ok(name) = reference.name().as_bstr().to_str()
            {
                // filter for branch references
                if name.starts_with("refs/heads/")
                    && let Some(branch_name) = name.strip_prefix("refs/heads/")
                {
                    branches.push(branch_name.to_string());
                }
            }
        }

        Ok(branches)
    }

    /// get the current branch name
    pub fn get_current_branch<P: AsRef<Path>>(repo_path: P) -> Result<String> {
        let repo_path = repo_path.as_ref();
        let repo = gix::discover(repo_path)?;

        let head = repo.head().map_err(Error::from_git_error)?;

        let head_ref = head
            .try_into_referent()
            .ok_or_else(|| Error::from_git_error(std::io::Error::other("HEAD is detached")))?;

        let name = head_ref.name().as_bstr().to_str().map_err(|_| {
            Error::from_git_error(std::io::Error::other("invalid branch name encoding"))
        })?;

        // strip refs/heads/ prefix if present
        if let Some(branch_name) = name.strip_prefix("refs/heads/") {
            Ok(branch_name.to_string())
        } else {
            Ok(name.to_string())
        }
    }
}

impl Default for GitOps {
    fn default() -> Self {
        Self::new()
    }
}
