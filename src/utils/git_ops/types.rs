use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitRepository {
    pub root_path: PathBuf,
    pub is_bare: bool,
    pub git_dir: PathBuf,
}

impl GitRepository {
    pub fn new(root_path: PathBuf, is_bare: bool, git_dir: PathBuf) -> Self {
        Self {
            root_path,
            is_bare,
            git_dir,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: PathBuf },
    Copied { source_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub is_binary: bool,
}

impl FileChange {
    pub fn new(path: PathBuf, change_type: ChangeType, is_binary: bool) -> Self {
        Self {
            path,
            change_type,
            is_binary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GitRef {
    Hash(String),
    Branch(String),
    Tag(String),
    Head,
}

impl GitRef {
    pub fn from_string(s: &str) -> Self {
        if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            GitRef::Hash(s.to_string())
        } else if s == "HEAD" {
            GitRef::Head
        } else if s.starts_with("refs/tags/") {
            GitRef::Tag(s.strip_prefix("refs/tags/").unwrap().to_string())
        } else {
            GitRef::Branch(s.to_string())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangedFiles {
    pub changes: Vec<FileChange>,
    pub from_ref: String,
    pub to_ref: String,
}

impl ChangedFiles {
    pub fn new(from_ref: String, to_ref: String) -> Self {
        Self {
            changes: Vec::new(),
            from_ref,
            to_ref,
        }
    }

    pub fn add_change(&mut self, change: FileChange) {
        self.changes.push(change);
    }

    pub fn get_added_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| matches!(c.change_type, ChangeType::Added))
            .collect()
    }

    pub fn get_modified_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| matches!(c.change_type, ChangeType::Modified))
            .collect()
    }

    pub fn get_deleted_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| matches!(c.change_type, ChangeType::Deleted))
            .collect()
    }
}
