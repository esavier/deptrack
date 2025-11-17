use deptrack::GitOps;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn create_git_repo_with_commits(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // initialize git repo
    let _repo = gix::init(dir)?;

    // create a simple file
    let test_file = dir.join("README.md");
    fs::write(&test_file, "# Test Repository\n\nThis is a test.")?;

    // create some directory structure
    let src_dir = dir.join("src");
    fs::create_dir(&src_dir)?;
    fs::write(
        src_dir.join("main.rs"),
        "fn main() {\n    println!(\"Hello, world!\");\n}",
    )?;

    let docs_dir = dir.join("docs");
    fs::create_dir(&docs_dir)?;
    fs::write(
        docs_dir.join("guide.md"),
        "# User Guide\n\nHow to use this.",
    )?;

    Ok(())
}

fn create_nested_git_repos(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // create main repo
    create_git_repo_with_commits(base_dir)?;

    // create a subdirectory with another repo (nested repo scenario)
    let nested_dir = base_dir.join("nested_project");
    fs::create_dir(&nested_dir)?;
    create_git_repo_with_commits(&nested_dir)?;

    Ok(())
}

#[test]
fn test_integration_detect_repository_root_from_various_depths() {
    let temp_dir = TempDir::new().unwrap();
    create_git_repo_with_commits(temp_dir.path()).unwrap();

    // test from repo root
    let root_result = GitOps::detect_repository_root(temp_dir.path());
    assert!(root_result.is_ok());
    let expected_root = root_result.unwrap();

    // test from src directory
    let src_result = GitOps::detect_repository_root(temp_dir.path().join("src"));
    assert!(src_result.is_ok());
    assert_eq!(expected_root, src_result.unwrap());

    // test from docs directory
    let docs_result = GitOps::detect_repository_root(temp_dir.path().join("docs"));
    assert!(docs_result.is_ok());
    assert_eq!(expected_root, docs_result.unwrap());

    // test from a file
    let file_result = GitOps::detect_repository_root(temp_dir.path().join("README.md"));
    assert!(file_result.is_ok());
    assert_eq!(expected_root, file_result.unwrap());
}

#[test]
fn test_integration_is_repository_various_paths() {
    let temp_dir = TempDir::new().unwrap();
    create_git_repo_with_commits(temp_dir.path()).unwrap();

    // repo root should be a repository
    assert!(GitOps::is_repository(temp_dir.path()).unwrap());

    // subdirectories should also be in a repository
    assert!(GitOps::is_repository(temp_dir.path().join("src")).unwrap());
    assert!(GitOps::is_repository(temp_dir.path().join("docs")).unwrap());

    // files should also be in a repository
    assert!(GitOps::is_repository(temp_dir.path().join("README.md")).unwrap());
    assert!(GitOps::is_repository(temp_dir.path().join("src").join("main.rs")).unwrap());

    // create a directory outside the repo
    let outside_dir = TempDir::new().unwrap();
    assert!(!GitOps::is_repository(outside_dir.path()).unwrap());
}

#[test]
fn test_integration_is_repository_root_precision() {
    let temp_dir = TempDir::new().unwrap();
    create_git_repo_with_commits(temp_dir.path()).unwrap();

    // only the actual root should return true
    assert!(GitOps::is_repository_root(temp_dir.path()).unwrap());

    // subdirectories should return false
    assert!(!GitOps::is_repository_root(temp_dir.path().join("src")).unwrap());
    assert!(!GitOps::is_repository_root(temp_dir.path().join("docs")).unwrap());

    // files should return false
    assert!(!GitOps::is_repository_root(temp_dir.path().join("README.md")).unwrap());

    // non-repo directory should return false
    let outside_dir = TempDir::new().unwrap();
    assert!(!GitOps::is_repository_root(outside_dir.path()).unwrap());
}

#[test]
fn test_integration_nested_repositories() {
    let temp_dir = TempDir::new().unwrap();
    create_nested_git_repos(temp_dir.path()).unwrap();

    // main repo root
    let main_root = GitOps::detect_repository_root(temp_dir.path()).unwrap();
    assert_eq!(main_root, temp_dir.path().canonicalize().unwrap());

    // nested repo should have its own root
    let nested_path = temp_dir.path().join("nested_project");
    let nested_root = GitOps::detect_repository_root(&nested_path).unwrap();
    assert_eq!(nested_root, nested_path.canonicalize().unwrap());

    // verify they are different roots
    assert_ne!(main_root, nested_root);

    // verify repository root detection
    assert!(GitOps::is_repository_root(temp_dir.path()).unwrap());
    assert!(GitOps::is_repository_root(&nested_path).unwrap());

    // verify both are repositories
    assert!(GitOps::is_repository(temp_dir.path()).unwrap());
    assert!(GitOps::is_repository(&nested_path).unwrap());
}

#[test]
fn test_integration_get_repository_info_detailed() {
    let temp_dir = TempDir::new().unwrap();
    create_git_repo_with_commits(temp_dir.path()).unwrap();

    // test from root
    let root_info = GitOps::get_repository_info(temp_dir.path()).unwrap();
    assert!(!root_info.is_bare);
    assert_eq!(root_info.root_path, temp_dir.path().canonicalize().unwrap());
    assert!(root_info.git_dir.exists());
    assert!(root_info.git_dir.join("HEAD").exists()); // basic git structure check

    // test from subdirectory
    let sub_info = GitOps::get_repository_info(temp_dir.path().join("src")).unwrap();
    assert_eq!(root_info.root_path, sub_info.root_path);
    assert_eq!(root_info.git_dir, sub_info.git_dir);
    assert_eq!(root_info.is_bare, sub_info.is_bare);

    // test from file
    let file_info = GitOps::get_repository_info(temp_dir.path().join("README.md")).unwrap();
    assert_eq!(root_info.root_path, file_info.root_path);
}

#[test]
fn test_integration_error_handling() {
    let temp_dir = TempDir::new().unwrap();

    // test all functions with non-repository path
    assert!(!GitOps::is_repository(temp_dir.path()).unwrap());
    assert!(!GitOps::is_repository_root(temp_dir.path()).unwrap());

    // these should return errors
    assert!(GitOps::detect_repository_root(temp_dir.path()).is_err());
    assert!(GitOps::get_repository_info(temp_dir.path()).is_err());

    // test with non-existent path
    let non_existent = temp_dir.path().join("does_not_exist");
    // these might return Ok(false) or Err depending on the specific error
    // but they shouldn't panic
    let _ = GitOps::is_repository(&non_existent);
    let _ = GitOps::detect_repository_root(&non_existent);
}

#[test]
fn test_integration_bare_repository() {
    let temp_dir = TempDir::new().unwrap();

    // create a bare repository
    let _repo = gix::init_bare(temp_dir.path()).unwrap();

    // test bare repository detection
    assert!(GitOps::is_repository(temp_dir.path()).unwrap());
    assert!(GitOps::is_repository_root(temp_dir.path()).unwrap());

    let root = GitOps::detect_repository_root(temp_dir.path()).unwrap();
    assert_eq!(root, temp_dir.path().canonicalize().unwrap());

    let info = GitOps::get_repository_info(temp_dir.path()).unwrap();
    assert!(info.is_bare);
    assert_eq!(info.root_path, info.git_dir); // for bare repos, these should be the same
}

#[test]
fn test_integration_complex_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    create_git_repo_with_commits(temp_dir.path()).unwrap();

    // create a more complex structure
    let deep_path = temp_dir
        .path()
        .join("src")
        .join("modules")
        .join("core")
        .join("utils");
    fs::create_dir_all(&deep_path).unwrap();
    fs::write(deep_path.join("helper.rs"), "// helper functions").unwrap();

    // test from deeply nested directory
    let deep_root = GitOps::detect_repository_root(&deep_path).unwrap();
    let expected_root = temp_dir.path().canonicalize().unwrap();
    assert_eq!(deep_root, expected_root);

    // test repository detection from deep path
    assert!(GitOps::is_repository(&deep_path).unwrap());
    assert!(!GitOps::is_repository_root(&deep_path).unwrap());

    // test from the file in deep path
    let file_path = deep_path.join("helper.rs");
    assert!(GitOps::is_repository(&file_path).unwrap());
    assert!(!GitOps::is_repository_root(&file_path).unwrap());

    let file_root = GitOps::detect_repository_root(&file_path).unwrap();
    assert_eq!(file_root, expected_root);
}
