use crate::utils::filesystem::types::{FsDirectory, FsElement, FsFile};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PredicateContext {
    pub current_file: Option<FsFile>,
    pub current_directory: FsDirectory,
    pub parent_directories: Vec<FsDirectory>,
}

impl PredicateContext {
    pub fn new(current_directory: FsDirectory) -> Self {
        Self {
            current_file: None,
            current_directory,
            parent_directories: Vec::new(),
        }
    }

    pub fn with_file(mut self, file: FsFile) -> Self {
        self.current_file = Some(file);
        self
    }

    pub fn with_parent(mut self, parent: FsDirectory) -> Self {
        self.parent_directories.push(parent);
        self
    }

    pub fn file_path(&self) -> Option<&str> {
        self.current_file.as_ref().map(|f| f.path.as_str())
    }

    pub fn file_extension(&self) -> Option<&str> {
        self.current_file.as_ref()?.extension.as_deref()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.current_file.as_ref()?.name.as_deref()
    }

    pub fn directory_path(&self) -> &str {
        &self.current_directory.path
    }

    pub fn directory_contains_file(&self, filename: &str) -> bool {
        self.current_directory
            .elements
            .iter()
            .any(|element| match element {
                FsElement::File(file) => {
                    if let Some(name) = Path::new(&file.path).file_name() {
                        name.to_string_lossy() == filename
                    } else {
                        false
                    }
                }
                _ => false,
            })
    }

    pub fn directory_contains_directory(&self, dirname: &str) -> bool {
        self.current_directory
            .elements
            .iter()
            .any(|element| match element {
                FsElement::Directory(dir) => {
                    if let Some(name) = Path::new(&dir.path).file_name() {
                        name.to_string_lossy() == dirname
                    } else {
                        false
                    }
                }
                _ => false,
            })
    }

    pub fn get_files_in_directory(&self) -> Vec<&FsFile> {
        self.current_directory
            .elements
            .iter()
            .filter_map(|element| match element {
                FsElement::File(file) => Some(file),
                _ => None,
            })
            .collect()
    }

    pub fn get_directories(&self) -> Vec<&FsDirectory> {
        self.current_directory
            .elements
            .iter()
            .filter_map(|element| match element {
                FsElement::Directory(dir) => Some(dir),
                _ => None,
            })
            .collect()
    }
}
