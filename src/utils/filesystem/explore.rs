use crate::utils::alt::{Evaluable, LogicExpr};
use crate::utils::filesystem::predicates::{PredicateContext, PredicateError};
use crate::utils::filesystem::types::*;
use std::fs;
use std::path::Path;

pub struct FilesystemExplorer {
    pub root_path: String,
}

impl FilesystemExplorer {
    pub fn new(root_path: String) -> Self {
        FilesystemExplorer { root_path }
    }

    pub fn scan_directory(&self, path: &str) -> Result<FsDirectory, std::io::Error> {
        let mut directory = FsDirectory::new(path.to_string());
        Self::scan_directory_recursive(&mut directory)?;
        Ok(directory)
    }

    fn scan_directory_recursive(directory: &mut FsDirectory) -> Result<(), std::io::Error> {
        let entries = fs::read_dir(&directory.path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if path.is_file() {
                let file = FsFile::new(path_str);
                directory.elements.push(FsElement::File(file));
            } else if path.is_dir() {
                let mut subdir = FsDirectory::new(path_str);
                Self::scan_directory_recursive(&mut subdir)?;
                directory.elements.push(FsElement::Directory(subdir));
            }
        }

        Ok(())
    }

    pub fn scan_from_root(&self) -> Result<FsDirectory, std::io::Error> {
        let mut directory = FsDirectory::new_root(self.root_path.clone());
        Self::scan_directory_recursive(&mut directory)?;
        Ok(directory)
    }

    pub fn find_files_by_extension(&self, directory: &FsDirectory, extension: &str) -> Vec<String> {
        let mut files = Vec::new();
        Self::find_files_by_extension_recursive(directory, extension, &mut files);
        files
    }

    fn find_files_by_extension_recursive(
        directory: &FsDirectory,
        extension: &str,
        files: &mut Vec<String>,
    ) {
        for element in &directory.elements {
            match element {
                FsElement::File(file) => {
                    if let Some(path) = Path::new(&file.path).extension()
                        && path.to_string_lossy() == extension
                    {
                        files.push(file.path.clone());
                    }
                }
                FsElement::Directory(dir) => {
                    Self::find_files_by_extension_recursive(dir, extension, files);
                }
            }
        }
    }

    pub fn count_elements(&self, directory: &FsDirectory) -> (usize, usize) {
        let mut file_count = 0;
        let mut dir_count = 0;
        Self::count_elements_recursive(directory, &mut file_count, &mut dir_count);
        (file_count, dir_count)
    }

    fn count_elements_recursive(
        directory: &FsDirectory,
        file_count: &mut usize,
        dir_count: &mut usize,
    ) {
        for element in &directory.elements {
            match element {
                FsElement::File(_) => *file_count += 1,
                FsElement::Directory(dir) => {
                    *dir_count += 1;
                    Self::count_elements_recursive(dir, file_count, dir_count);
                }
            }
        }
    }

    pub fn scan_with_predicate<T>(
        &self,
        predicate: LogicExpr<T>,
    ) -> Result<FsDirectory, std::io::Error>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        let mut directory = FsDirectory::new_root(self.root_path.clone());
        Self::scan_directory_with_predicate(&mut directory, &predicate)?;
        Ok(directory)
    }

    pub fn filter_directory_with_predicate<T>(
        &self,
        directory: &FsDirectory,
        predicate: &LogicExpr<T>,
    ) -> Result<FsDirectory, PredicateError>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        Self::filter_directory_recursive(directory, predicate)
    }

    fn filter_directory_recursive<T>(
        directory: &FsDirectory,
        predicate: &LogicExpr<T>,
    ) -> Result<FsDirectory, PredicateError>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        let mut filtered = directory.clone();
        filtered.elements.clear();

        for element in &directory.elements {
            match element {
                FsElement::File(file) => {
                    let mut file_with_metadata = file.clone();
                    let _ = file_with_metadata.metadata_scan(); // ensure metadata is populated
                    let context =
                        PredicateContext::new(directory.clone()).with_file(file_with_metadata);

                    if predicate.evaluate(&context)? {
                        filtered.elements.push(element.clone());
                    }
                }
                FsElement::Directory(dir) => {
                    let context = PredicateContext::new(dir.clone());

                    if predicate.evaluate(&context)? {
                        filtered.elements.push(element.clone());
                    } else {
                        let filtered_subdir = Self::filter_directory_recursive(dir, predicate)?;
                        if !filtered_subdir.elements.is_empty() {
                            filtered
                                .elements
                                .push(FsElement::Directory(filtered_subdir));
                        }
                    }
                }
            }
        }

        Ok(filtered)
    }

    fn scan_directory_with_predicate<T>(
        directory: &mut FsDirectory,
        predicate: &LogicExpr<T>,
    ) -> Result<(), std::io::Error>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        let entries = fs::read_dir(&directory.path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if path.is_file() {
                let mut file = FsFile::new(path_str);
                let _ = file.metadata_scan(); // populate file metadata like extension and name
                let context = PredicateContext::new(directory.clone()).with_file(file.clone());

                if let Ok(matches) = predicate.evaluate(&context)
                    && matches
                {
                    directory.elements.push(FsElement::File(file));
                }
            } else if path.is_dir() {
                let mut subdir_for_eval = FsDirectory::new(path_str.clone());
                // scan just the top level to check if directory contains required files
                let entries = fs::read_dir(&path_str)?;
                for entry in entries {
                    let entry = entry?;
                    let entry_path = entry.path();
                    let entry_path_str = entry_path.to_string_lossy().to_string();

                    if entry_path.is_file() {
                        let file = FsFile::new(entry_path_str);
                        subdir_for_eval.elements.push(FsElement::File(file));
                    }
                }

                let mut subdir = FsDirectory::new(path_str);
                Self::scan_directory_with_predicate(&mut subdir, predicate)?;

                let context = PredicateContext::new(subdir_for_eval);
                if let Ok(matches) = predicate.evaluate(&context)
                    && (matches || !subdir.elements.is_empty())
                {
                    directory.elements.push(FsElement::Directory(subdir));
                }
            }
        }

        Ok(())
    }

    pub fn find_files_matching<T>(
        &self,
        directory: &FsDirectory,
        predicate: &LogicExpr<T>,
    ) -> Result<Vec<String>, PredicateError>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        let mut files = Vec::new();
        Self::find_files_matching_recursive(directory, predicate, &mut files)?;
        Ok(files)
    }

    fn find_files_matching_recursive<T>(
        directory: &FsDirectory,
        predicate: &LogicExpr<T>,
        files: &mut Vec<String>,
    ) -> Result<(), PredicateError>
    where
        T: Evaluable<Context = PredicateContext, Error = PredicateError> + Clone,
    {
        for element in &directory.elements {
            match element {
                FsElement::File(file) => {
                    let mut file_with_metadata = file.clone();
                    let _ = file_with_metadata.metadata_scan(); // ensure metadata is populated
                    let context =
                        PredicateContext::new(directory.clone()).with_file(file_with_metadata);

                    if predicate.evaluate(&context)? {
                        files.push(file.path.clone());
                    }
                }
                FsElement::Directory(dir) => {
                    Self::find_files_matching_recursive(dir, predicate, files)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_filesystem_explorer_basic() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("nested.txt"), "nested").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        let (file_count, dir_count) = explorer.count_elements(&result);
        assert_eq!(file_count, 3); // test.txt, test.rs, nested.txt
        assert_eq!(dir_count, 1); // subdir

        let rs_files = explorer.find_files_by_extension(&result, "rs");
        assert_eq!(rs_files.len(), 1);
        assert!(rs_files[0].ends_with("test.rs"));
    }

    #[test]
    fn test_filesystem_explorer_new() {
        let explorer = FilesystemExplorer::new("/test/path".to_string());
        assert_eq!(explorer.root_path, "/test/path");
    }

    #[test]
    fn test_scan_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        fs::write(temp_dir.path().join("file2.md"), "# Header").unwrap();

        let explorer = FilesystemExplorer::new(temp_path.clone());
        let result = explorer.scan_directory(&temp_path).unwrap();

        assert_eq!(result.path, temp_path);
        assert_eq!(result.elements.len(), 2);
    }

    #[test]
    fn test_find_files_by_extension_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create files with different extensions
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        fs::write(temp_dir.path().join("script.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        let txt_files = explorer.find_files_by_extension(&result, "txt");
        assert_eq!(txt_files.len(), 2);

        let rs_files = explorer.find_files_by_extension(&result, "rs");
        assert_eq!(rs_files.len(), 1);

        let md_files = explorer.find_files_by_extension(&result, "md");
        assert_eq!(md_files.len(), 1);

        let nonexistent = explorer.find_files_by_extension(&result, "xyz");
        assert_eq!(nonexistent.len(), 0);
    }

    #[test]
    fn test_find_files_nested() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create nested structure
        fs::create_dir(temp_dir.path().join("dir1")).unwrap();
        fs::create_dir(temp_dir.path().join("dir1").join("dir2")).unwrap();
        fs::write(temp_dir.path().join("top.rs"), "top level").unwrap();
        fs::write(temp_dir.path().join("dir1").join("mid.rs"), "middle level").unwrap();
        fs::write(
            temp_dir.path().join("dir1").join("dir2").join("deep.rs"),
            "deep level",
        )
        .unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        let rs_files = explorer.find_files_by_extension(&result, "rs");
        assert_eq!(rs_files.len(), 3);

        // check all expected files are found
        let paths: Vec<String> = rs_files
            .iter()
            .map(|p| p.split('/').next_back().unwrap().to_string())
            .collect();
        assert!(paths.contains(&"top.rs".to_string()));
        assert!(paths.contains(&"mid.rs".to_string()));
        assert!(paths.contains(&"deep.rs".to_string()));
    }

    #[test]
    fn test_count_elements_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create structure: 4 files, 2 directories
        fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();
        fs::create_dir(temp_dir.path().join("dir1")).unwrap();
        fs::create_dir(temp_dir.path().join("dir2")).unwrap();
        fs::write(temp_dir.path().join("dir1").join("nested1.txt"), "content").unwrap();
        fs::write(temp_dir.path().join("dir2").join("nested2.txt"), "content").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        let (file_count, dir_count) = explorer.count_elements(&result);
        assert_eq!(file_count, 4); // file1.txt, file2.txt, nested1.txt, nested2.txt
        assert_eq!(dir_count, 2); // dir1, dir2
    }

    #[test]
    fn test_count_elements_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        let (file_count, dir_count) = explorer.count_elements(&result);
        assert_eq!(file_count, 0);
        assert_eq!(dir_count, 0);
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let explorer = FilesystemExplorer::new("/nonexistent/path".to_string());
        let result = explorer.scan_directory("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_files_complex_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create files with various extensions
        fs::write(temp_dir.path().join("archive.tar.gz"), "archive").unwrap();
        fs::write(temp_dir.path().join("backup.tar"), "backup").unwrap();
        fs::write(temp_dir.path().join("config.json"), "{}").unwrap();
        fs::write(temp_dir.path().join("noext"), "no extension").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        // only matches final extension
        let gz_files = explorer.find_files_by_extension(&result, "gz");
        assert_eq!(gz_files.len(), 1);

        let tar_files = explorer.find_files_by_extension(&result, "tar");
        assert_eq!(tar_files.len(), 1);

        let json_files = explorer.find_files_by_extension(&result, "json");
        assert_eq!(json_files.len(), 1);
    }

    #[test]
    fn test_scan_from_root_sets_root_flag() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let result = explorer.scan_from_root().unwrap();

        // root directory should have is_root = true
        assert!(result.is_root);

        // subdirectories should have is_root = false
        for element in &result.elements {
            if let FsElement::Directory(dir) = element {
                assert!(!dir.is_root);
            }
        }
    }

    #[test]
    fn test_scan_with_file_extension_predicate() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FileExtensionPredicate, FilePredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files
        fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();
        fs::write(temp_dir.path().join("data.txt"), "content").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let predicate =
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs")));
        let result = explorer.scan_with_predicate(predicate).unwrap();

        // should only contain .rs files
        assert_eq!(result.elements.len(), 1);
        if let FsElement::File(file) = &result.elements[0] {
            assert!(file.path.ends_with("test.rs"));
        } else {
            panic!("Expected file element");
        }
    }

    #[test]
    fn test_scan_with_or_predicate() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FileExtensionPredicate, FilePredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files
        fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();
        fs::write(temp_dir.path().join("data.txt"), "content").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let predicate = LogicExpr::or(
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs"))),
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("md"))),
        );
        let result = explorer.scan_with_predicate(predicate).unwrap();

        // should contain .rs and .md files
        assert_eq!(result.elements.len(), 2);
        let paths: Vec<String> = result
            .elements
            .iter()
            .filter_map(|e| match e {
                FsElement::File(f) => Some(f.path.clone()),
                _ => None,
            })
            .collect();

        assert!(paths.iter().any(|p| p.ends_with("test.rs")));
        assert!(paths.iter().any(|p| p.ends_with("README.md")));
    }

    #[test]
    fn test_scan_with_directory_contains_predicate() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{
            DirectoryContainsPredicate, FileExtensionPredicate, FilePredicate,
        };

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test structure
        fs::create_dir(temp_dir.path().join("rust_project")).unwrap();
        fs::write(
            temp_dir.path().join("rust_project").join("Cargo.toml"),
            "[package]",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("rust_project").join("main.rs"),
            "fn main() {}",
        )
        .unwrap();

        fs::create_dir(temp_dir.path().join("other_dir")).unwrap();
        fs::write(
            temp_dir.path().join("other_dir").join("file.txt"),
            "content",
        )
        .unwrap();

        let explorer = FilesystemExplorer::new(temp_path);

        // files with .rs extension in directories OR directories that contain Cargo.toml
        let predicate = LogicExpr::or(
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs"))),
            LogicExpr::Leaf(FilePredicate::DirectoryContains(
                DirectoryContainsPredicate::new("Cargo.toml"),
            )),
        );

        let result = explorer.scan_with_predicate(predicate).unwrap();

        // should find rust files and directories containing Cargo.toml
        let mut found_rs_files = 0;
        let mut _found_cargo_dirs = 0;

        for element in &result.elements {
            match element {
                FsElement::File(f) => {
                    if f.path.ends_with(".rs") {
                        found_rs_files += 1;
                    }
                }
                FsElement::Directory(dir) => {
                    if dir
                        .elements
                        .iter()
                        .any(|e| matches!(e, FsElement::File(f) if f.path.ends_with("Cargo.toml")))
                    {
                        _found_cargo_dirs += 1;
                    }
                    // count nested .rs files
                    for nested in &dir.elements {
                        if let FsElement::File(f) = nested
                            && f.path.ends_with(".rs")
                        {
                            found_rs_files += 1;
                        }
                    }
                }
            }
        }

        assert!(found_rs_files >= 1, "Should find at least 1 .rs file");
        assert!(
            !result.elements.is_empty(),
            "Should find at least some elements"
        );
    }

    #[test]
    fn test_filter_directory_with_predicate() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FileExtensionPredicate, FilePredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files
        fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();
        fs::write(temp_dir.path().join("data.txt"), "content").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let directory = explorer.scan_from_root().unwrap();

        let predicate =
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs")));
        let filtered = explorer
            .filter_directory_with_predicate(&directory, &predicate)
            .unwrap();

        // should only contain .rs files
        assert_eq!(filtered.elements.len(), 1);
        if let FsElement::File(file) = &filtered.elements[0] {
            assert!(file.path.ends_with("test.rs"));
        }
    }

    #[test]
    fn test_find_files_matching() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FileExtensionPredicate, FilePredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create nested test structure
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::write(temp_dir.path().join("src").join("lib.rs"), "// lib").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let directory = explorer.scan_from_root().unwrap();

        let predicate =
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs")));
        let rust_files = explorer
            .find_files_matching(&directory, &predicate)
            .unwrap();

        assert_eq!(rust_files.len(), 2);
        assert!(rust_files.iter().any(|p| p.ends_with("main.rs")));
        assert!(rust_files.iter().any(|p| p.ends_with("lib.rs")));
    }

    #[test]
    fn test_complex_predicate_expression() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{
            FileExtensionPredicate, FileNamePredicate, FilePredicate,
        };

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("lib.rs"), "// lib").unwrap();
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        fs::write(temp_dir.path().join("main.py"), "print('hello')").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);
        let directory = explorer.scan_from_root().unwrap();

        // (extension = "rs" AND name contains "main") OR extension = "py"
        let predicate = LogicExpr::or(
            LogicExpr::and(
                LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("rs"))),
                LogicExpr::Leaf(FilePredicate::FileName(FileNamePredicate::contains("main"))),
            ),
            LogicExpr::Leaf(FilePredicate::Extension(FileExtensionPredicate::new("py"))),
        );

        let matching_files = explorer
            .find_files_matching(&directory, &predicate)
            .unwrap();

        assert_eq!(matching_files.len(), 2);
        assert!(matching_files.iter().any(|p| p.ends_with("main.rs")));
        assert!(matching_files.iter().any(|p| p.ends_with("main.py")));
        // lib.rs should NOT be included because it doesn't contain "main"
        assert!(!matching_files.iter().any(|p| p.ends_with("lib.rs")));
    }

    #[test]
    fn test_toml_content_predicate_integration() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FilePredicate, TomlContentPredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test TOML files
        let cargo_toml_content = r#"
[package]
name = "test-project"
version = "1.0.0"

[dependencies]
serde = "1.0"
        "#;

        let other_toml_content = r#"
name = "config"
debug = true
        "#;

        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml_content).unwrap();
        fs::write(temp_dir.path().join("config.toml"), other_toml_content).unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Test Project").unwrap();

        let explorer = FilesystemExplorer::new(temp_path);

        // Find TOML files that have a version field
        let version_predicate = LogicExpr::Leaf(FilePredicate::TomlContent(
            TomlContentPredicate::has_version(),
        ));

        let result = explorer.scan_with_predicate(version_predicate).unwrap();

        // Should only find Cargo.toml (has version field)
        assert_eq!(result.elements.len(), 1);
        if let FsElement::File(file) = &result.elements[0] {
            assert!(file.path.ends_with("Cargo.toml"));
        }

        // Find TOML files that have dependencies
        let deps_predicate = LogicExpr::Leaf(FilePredicate::TomlContent(
            TomlContentPredicate::has_dependencies(),
        ));

        let deps_result = explorer.scan_with_predicate(deps_predicate).unwrap();

        // Should only find Cargo.toml (has dependencies table)
        assert_eq!(deps_result.elements.len(), 1);
        if let FsElement::File(file) = &deps_result.elements[0] {
            assert!(file.path.ends_with("Cargo.toml"));
        }
    }

    #[test]
    fn test_toml_version_matching() {
        use crate::utils::alt::LogicExpr;
        use crate::utils::filesystem::predicates::{FilePredicate, TomlContentPredicate};

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        let cargo_toml_content = r#"
[package]
name = "version-test"
version = "2.1.0"
        "#;

        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml_content).unwrap();

        let explorer = FilesystemExplorer::new(temp_path);

        // Test exact version matching
        let exact_predicate = LogicExpr::Leaf(FilePredicate::TomlContent(
            TomlContentPredicate::version_equals("2.1.0"),
        ));

        let exact_result = explorer.scan_with_predicate(exact_predicate).unwrap();
        assert_eq!(exact_result.elements.len(), 1);

        // Test version prefix matching
        let prefix_predicate = LogicExpr::Leaf(FilePredicate::TomlContent(
            TomlContentPredicate::version_starts_with("2."),
        ));

        let prefix_result = explorer.scan_with_predicate(prefix_predicate).unwrap();
        assert_eq!(prefix_result.elements.len(), 1);

        // Test non-matching version
        let nomatch_predicate = LogicExpr::Leaf(FilePredicate::TomlContent(
            TomlContentPredicate::version_equals("1.0.0"),
        ));

        let nomatch_result = explorer.scan_with_predicate(nomatch_predicate).unwrap();
        assert_eq!(nomatch_result.elements.len(), 0);
    }
}
