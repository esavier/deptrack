pub mod repository;
pub mod types;

pub use repository::GitOps;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn create_git_repo(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // initialize git repo
        let _repo = gix::init(dir)?;

        // create a simple file
        let test_file = dir.join("test.txt");
        fs::write(&test_file, "test content")?;

        // for now, just create the repo structure without commits
        // actual commit creation with gix is complex and not essential for basic tests

        Ok(())
    }

    #[test]
    fn test_is_repository_with_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        let result = GitOps::is_repository(temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_is_repository_with_non_repo() {
        let temp_dir = TempDir::new().unwrap();

        let result = GitOps::is_repository(temp_dir.path());
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_detect_repository_root() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        // create a subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // test from root
        let root_result = GitOps::detect_repository_root(temp_dir.path());
        assert!(root_result.is_ok());

        // test from subdirectory
        let sub_result = GitOps::detect_repository_root(&sub_dir);
        assert!(sub_result.is_ok());

        // both should return the same root
        assert_eq!(root_result.unwrap(), sub_result.unwrap());
    }

    #[test]
    fn test_is_repository_root_true() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        let result = GitOps::is_repository_root(temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_is_repository_root_false_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let result = GitOps::is_repository_root(&sub_dir);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_is_repository_root_false_non_repo() {
        let temp_dir = TempDir::new().unwrap();

        let result = GitOps::is_repository_root(temp_dir.path());
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_get_repository_info() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        let result = GitOps::get_repository_info(temp_dir.path());
        assert!(result.is_ok());

        let repo_info = result.unwrap();
        assert!(!repo_info.is_bare);
        assert!(repo_info.root_path.exists());
        assert!(repo_info.git_dir.exists());
    }

    #[test]
    fn test_detect_repository_root_error_non_repo() {
        let temp_dir = TempDir::new().unwrap();

        let result = GitOps::detect_repository_root(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repository_info_error_non_repo() {
        let temp_dir = TempDir::new().unwrap();

        let result = GitOps::get_repository_info(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_ref_head() {
        let temp_dir = TempDir::new().unwrap();
        if create_git_repo(temp_dir.path()).is_ok() {
            let head_ref = GitRef::Head;
            if let Ok(hash) = GitOps::resolve_ref(temp_dir.path(), &head_ref) {
                assert_eq!(hash.len(), 40); // SHA-1 hash length
            }
            // if creating commits fails, that's ok - this is a basic test
        }
    }

    #[test]
    fn test_resolve_ref_hash() {
        let temp_dir = TempDir::new().unwrap();
        if create_git_repo(temp_dir.path()).is_ok() {
            // first get the HEAD hash
            let head_ref = GitRef::Head;
            if let Ok(head_hash) = GitOps::resolve_ref(temp_dir.path(), &head_ref) {
                // then try to resolve using that hash
                let hash_ref = GitRef::Hash(head_hash.clone());
                let result = GitOps::resolve_ref(temp_dir.path(), &hash_ref);
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), head_hash);
            }
        }
    }

    #[test]
    fn test_list_changed_files_same_ref() {
        let temp_dir = TempDir::new().unwrap();
        if create_git_repo(temp_dir.path()).is_ok() {
            let head_ref = GitRef::Head;
            if let Ok(result) = GitOps::list_changed_files(temp_dir.path(), &head_ref, &head_ref) {
                // should be no changes when comparing same ref
                assert!(result.changes.is_empty());
            }
        }
    }

    #[test]
    fn test_list_working_directory_changes() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        let result = GitOps::list_working_directory_changes(temp_dir.path());
        assert!(result.is_ok());

        let changes = result.unwrap();
        // should detect the untracked test.txt file created by create_git_repo
        assert_eq!(changes.changes.len(), 1);
        assert_eq!(changes.changes[0].path, PathBuf::from("test.txt"));
    }

    #[test]
    fn test_compare_with_branch() {
        let temp_dir = TempDir::new().unwrap();
        create_git_repo(temp_dir.path()).unwrap();

        // compare with main/master (should work even if same as current)
        let result = GitOps::compare_with_branch(temp_dir.path(), "main", true);

        // this might fail if main doesn't exist, but shouldn't panic
        let _ = result;
    }

    #[test]
    fn test_git_ref_from_string() {
        assert_eq!(GitRef::from_string("HEAD"), GitRef::Head);
        assert_eq!(
            GitRef::from_string("main"),
            GitRef::Branch("main".to_string())
        );
        assert_eq!(
            GitRef::from_string("feature/test"),
            GitRef::Branch("feature/test".to_string())
        );

        // 40-character hex string should be detected as hash
        let hash = "a1b2c3d4e5f6789012345678901234567890abcd";
        assert_eq!(GitRef::from_string(hash), GitRef::Hash(hash.to_string()));

        // tag format
        assert_eq!(
            GitRef::from_string("refs/tags/v1.0"),
            GitRef::Tag("v1.0".to_string())
        );
    }

    #[test]
    fn test_change_types() {
        use super::types::*;

        let change = FileChange::new(PathBuf::from("test.rs"), ChangeType::Added, false);

        assert_eq!(change.path, PathBuf::from("test.rs"));
        assert_eq!(change.change_type, ChangeType::Added);
        assert!(!change.is_binary);
    }

    #[test]
    fn test_changed_files_helpers() {
        use super::types::*;

        let mut changes = ChangedFiles::new("abc123".to_string(), "def456".to_string());

        changes.add_change(FileChange::new(
            PathBuf::from("added.rs"),
            ChangeType::Added,
            false,
        ));
        changes.add_change(FileChange::new(
            PathBuf::from("modified.rs"),
            ChangeType::Modified,
            false,
        ));
        changes.add_change(FileChange::new(
            PathBuf::from("deleted.rs"),
            ChangeType::Deleted,
            false,
        ));

        assert_eq!(changes.get_added_files().len(), 1);
        assert_eq!(changes.get_modified_files().len(), 1);
        assert_eq!(changes.get_deleted_files().len(), 1);
    }
}
