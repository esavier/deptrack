use deptrack::{FilesystemExplorer, FsDirectory, FsElement, HashType};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_integration_full_scan_and_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create a complex directory structure
    create_test_project(&temp_dir);

    let explorer = FilesystemExplorer::new(temp_path);
    let result = explorer.scan_from_root().unwrap();

    // verify structure
    let (file_count, dir_count) = explorer.count_elements(&result);
    assert_eq!(file_count, 8); // all files in test project
    assert_eq!(dir_count, 4); // src, tests, docs, src/utils

    // verify file type detection
    let rs_files = explorer.find_files_by_extension(&result, "rs");
    assert_eq!(rs_files.len(), 4); // main.rs, lib.rs, mod.rs, test.rs

    let md_files = explorer.find_files_by_extension(&result, "md");
    assert_eq!(md_files.len(), 2); // README.md, docs/guide.md

    let toml_files = explorer.find_files_by_extension(&result, "toml");
    assert_eq!(toml_files.len(), 1); // Cargo.toml
}

#[test]
fn test_integration_metadata_scanning() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create files with different sizes
    fs::write(temp_dir.path().join("small.txt"), "tiny").unwrap();
    fs::write(temp_dir.path().join("large.txt"), "x".repeat(1000)).unwrap();

    let mut directory = FsDirectory::new(temp_path);
    directory.scan().unwrap();
    directory.metadata_scan().unwrap();

    // verify metadata was collected
    assert!(directory.permissions.is_some());
    assert!(directory.created.is_some());

    // check file metadata
    for element in &directory.elements {
        if let FsElement::File(file) = element {
            assert!(file.permissions.is_some());
            assert!(file.size.is_some());
            assert!(file.name.is_some());

            if file.path.ends_with("small.txt") {
                assert_eq!(file.size.unwrap(), 4);
            } else if file.path.ends_with("large.txt") {
                assert_eq!(file.size.unwrap(), 1000);
            }
        }
    }
}

#[test]
fn test_integration_deep_nesting() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create deeply nested structure
    let mut current_path = temp_dir.path().to_path_buf();
    for i in 0..5 {
        current_path = current_path.join(format!("level_{}", i));
        fs::create_dir(&current_path).unwrap();
        fs::write(
            current_path.join(format!("file_{}.txt", i)),
            format!("content {}", i),
        )
        .unwrap();
    }

    let explorer = FilesystemExplorer::new(temp_path);
    let result = explorer.scan_from_root().unwrap();

    let (file_count, dir_count) = explorer.count_elements(&result);
    assert_eq!(file_count, 5); // one file per level
    assert_eq!(dir_count, 5); // one dir per level

    let txt_files = explorer.find_files_by_extension(&result, "txt");
    assert_eq!(txt_files.len(), 5);
}

#[test]
fn test_integration_mixed_content() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create mixed content
    fs::write(temp_dir.path().join("script.sh"), "#!/bin/bash\necho hello").unwrap();
    fs::write(temp_dir.path().join("data.json"), r#"{"key": "value"}"#).unwrap();
    fs::write(temp_dir.path().join("image.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap(); // PNG header
    fs::write(temp_dir.path().join("no_extension"), "some content").unwrap();

    // set executable permission on script
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(temp_dir.path().join("script.sh"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(temp_dir.path().join("script.sh"), perms).unwrap();
    }

    let explorer = FilesystemExplorer::new(temp_path);
    let result = explorer.scan_from_root().unwrap();

    // verify different file types are detected
    let sh_files = explorer.find_files_by_extension(&result, "sh");
    assert_eq!(sh_files.len(), 1);

    let json_files = explorer.find_files_by_extension(&result, "json");
    assert_eq!(json_files.len(), 1);

    let png_files = explorer.find_files_by_extension(&result, "png");
    assert_eq!(png_files.len(), 1);

    // test metadata scanning
    let mut directory = FsDirectory::new(result.path.clone());
    directory.scan().unwrap();
    directory.metadata_scan().unwrap();

    // find the script file and check its permissions
    for element in &directory.elements {
        if let FsElement::File(file) = element {
            if file.path.ends_with("script.sh") {
                assert!(file.permissions.is_some());
                let perms = file.permissions.as_ref().unwrap();
                assert!(perms.owner_execute); // should be executable
            }
            if file.path.ends_with("no_extension") {
                assert_eq!(file.extension, None);
                assert_eq!(file.name, Some("no_extension".to_string()));
            }
        }
    }
}

#[test]
fn test_integration_error_handling() {
    let explorer = FilesystemExplorer::new("/definitely/does/not/exist".to_string());
    let result = explorer.scan_from_root();
    assert!(result.is_err());

    // test with empty directory
    let temp_dir = TempDir::new().unwrap();
    let explorer = FilesystemExplorer::new(temp_dir.path().to_string_lossy().to_string());
    let result = explorer.scan_from_root().unwrap();

    let (file_count, dir_count) = explorer.count_elements(&result);
    assert_eq!(file_count, 0);
    assert_eq!(dir_count, 0);
    assert!(result.elements.is_empty());
}

fn create_test_project(temp_dir: &TempDir) {
    let base = temp_dir.path();

    // root files
    fs::write(
        base.join("Cargo.toml"),
        r#"
[package]
name = "test-project"
version = "0.1.0"
"#,
    )
    .unwrap();

    fs::write(base.join("README.md"), "# Test Project").unwrap();

    // src directory
    fs::create_dir(base.join("src")).unwrap();
    fs::write(base.join("src").join("main.rs"), "fn main() {}").unwrap();
    fs::write(base.join("src").join("lib.rs"), "pub mod utils;").unwrap();

    // src/utils
    fs::create_dir(base.join("src").join("utils")).unwrap();
    fs::write(
        base.join("src").join("utils").join("mod.rs"),
        "pub fn helper() {}",
    )
    .unwrap();

    // tests directory
    fs::create_dir(base.join("tests")).unwrap();
    fs::write(
        base.join("tests").join("test.rs"),
        "#[test] fn it_works() {}",
    )
    .unwrap();

    // docs directory
    fs::create_dir(base.join("docs")).unwrap();
    fs::write(base.join("docs").join("guide.md"), "# User Guide").unwrap();
    fs::write(base.join("docs").join("config.yaml"), "key: value").unwrap();
}

#[test]
fn test_integration_hashing_performance() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create files of different sizes
    fs::write(temp_dir.path().join("small.txt"), "small content").unwrap();
    fs::write(temp_dir.path().join("medium.txt"), "x".repeat(1000)).unwrap();
    fs::write(temp_dir.path().join("large.txt"), "y".repeat(10000)).unwrap();

    let explorer = FilesystemExplorer::new(temp_path);
    let mut result = explorer.scan_from_root().unwrap();

    // test FAST hashing (CRC32 + BLAKE3)
    let start = std::time::Instant::now();
    result.hash(HashType::FAST).unwrap();
    let fast_duration = start.elapsed();

    // verify FAST hashing worked
    for element in &result.elements {
        if let FsElement::File(file) = element {
            assert!(file.crc32.is_some());
            assert!(file.blake3.is_some());
            assert!(file.sha256.is_none());
            assert!(file.sha512.is_none());
        }
    }

    // test ALL hashing
    let mut result2 = explorer.scan_from_root().unwrap();
    let start = std::time::Instant::now();
    result2.hash(HashType::ALL).unwrap();
    let all_duration = start.elapsed();

    // verify ALL hashing worked
    for element in &result2.elements {
        if let FsElement::File(file) = element {
            assert!(file.crc32.is_some());
            assert!(file.blake3.is_some());
            assert!(file.sha256.is_some());
            assert!(file.sha512.is_some());
        }
    }

    // FAST should be faster than ALL (though with small files, timing might be unreliable)
    println!("FAST hashing took: {:?}", fast_duration);
    println!("ALL hashing took: {:?}", all_duration);
}

#[test]
fn test_integration_hash_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    let content = "This is test content for hash consistency verification";
    fs::write(temp_dir.path().join("test.txt"), content).unwrap();

    let explorer = FilesystemExplorer::new(temp_path);

    // hash the same file multiple times and verify consistency
    let mut result1 = explorer.scan_from_root().unwrap();
    let mut result2 = explorer.scan_from_root().unwrap();
    let mut result3 = explorer.scan_from_root().unwrap();

    result1.hash(HashType::SHA256).unwrap();
    result2.hash(HashType::SHA256).unwrap();
    result3.hash(HashType::BLAKE3).unwrap();

    // extract hashes from results
    let mut sha256_1 = None;
    let mut sha256_2 = None;
    let mut blake3_hash = None;

    for element in &result1.elements {
        if let FsElement::File(file) = element
            && file.path.ends_with("test.txt")
        {
            sha256_1 = file.sha256.clone();
        }
    }

    for element in &result2.elements {
        if let FsElement::File(file) = element
            && file.path.ends_with("test.txt")
        {
            sha256_2 = file.sha256.clone();
        }
    }

    for element in &result3.elements {
        if let FsElement::File(file) = element
            && file.path.ends_with("test.txt")
        {
            blake3_hash = file.blake3.clone();
        }
    }

    // verify consistency
    assert_eq!(sha256_1, sha256_2);
    assert!(sha256_1.is_some());
    assert!(blake3_hash.is_some());
    // verify they're different hash values (even though same length)
    assert_ne!(sha256_1.unwrap(), blake3_hash.unwrap());
}

#[test]
fn test_integration_hash_different_algorithms() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    fs::write(temp_dir.path().join("file1.txt"), "identical content").unwrap();
    fs::write(temp_dir.path().join("file2.txt"), "identical content").unwrap();
    fs::write(temp_dir.path().join("file3.txt"), "different content").unwrap();

    let explorer = FilesystemExplorer::new(temp_path);
    let mut result = explorer.scan_from_root().unwrap();
    result.hash(HashType::ALL).unwrap();

    let mut file1_hashes = None;
    let mut file2_hashes = None;
    let mut file3_hashes = None;

    for element in &result.elements {
        if let FsElement::File(file) = element {
            if file.path.ends_with("file1.txt") {
                file1_hashes = Some((file.crc32.clone(), file.sha256.clone(), file.blake3.clone()));
            } else if file.path.ends_with("file2.txt") {
                file2_hashes = Some((file.crc32.clone(), file.sha256.clone(), file.blake3.clone()));
            } else if file.path.ends_with("file3.txt") {
                file3_hashes = Some((file.crc32.clone(), file.sha256.clone(), file.blake3.clone()));
            }
        }
    }

    let file1_hashes = file1_hashes.unwrap();
    let file2_hashes = file2_hashes.unwrap();
    let file3_hashes = file3_hashes.unwrap();

    // files with identical content should have identical hashes
    assert_eq!(file1_hashes.0, file2_hashes.0); // CRC32
    assert_eq!(file1_hashes.1, file2_hashes.1); // SHA256
    assert_eq!(file1_hashes.2, file2_hashes.2); // BLAKE3

    // files with different content should have different hashes
    assert_ne!(file1_hashes.0, file3_hashes.0); // CRC32
    assert_ne!(file1_hashes.1, file3_hashes.1); // SHA256
    assert_ne!(file1_hashes.2, file3_hashes.2); // BLAKE3
}

#[test]
fn test_integration_rfc3339_timestamps() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    fs::write(
        temp_dir.path().join("timestamp_test.txt"),
        "timestamp content",
    )
    .unwrap();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(
        temp_dir.path().join("subdir").join("nested.txt"),
        "nested content",
    )
    .unwrap();

    let explorer = FilesystemExplorer::new(temp_path);
    let mut result = explorer.scan_from_root().unwrap();
    result.metadata_scan().unwrap();

    // verify all timestamps in the entire tree are RFC 3339 compliant
    verify_rfc3339_recursive(&result);
}

fn verify_rfc3339_recursive(directory: &FsDirectory) {
    use chrono::DateTime;

    // check directory timestamps
    if let Some(created) = &directory.created {
        assert!(
            DateTime::parse_from_rfc3339(created).is_ok(),
            "Directory created time not RFC 3339: {}",
            created
        );
    }
    if let Some(modified) = &directory.modified {
        assert!(
            DateTime::parse_from_rfc3339(modified).is_ok(),
            "Directory modified time not RFC 3339: {}",
            modified
        );
    }
    if let Some(accessed) = &directory.accessed {
        assert!(
            DateTime::parse_from_rfc3339(accessed).is_ok(),
            "Directory accessed time not RFC 3339: {}",
            accessed
        );
    }

    // recursively check all elements
    for element in &directory.elements {
        match element {
            FsElement::File(file) => {
                if let Some(created) = &file.created {
                    assert!(
                        DateTime::parse_from_rfc3339(created).is_ok(),
                        "File {} created time not RFC 3339: {}",
                        file.path,
                        created
                    );
                }
                if let Some(modified) = &file.modified {
                    assert!(
                        DateTime::parse_from_rfc3339(modified).is_ok(),
                        "File {} modified time not RFC 3339: {}",
                        file.path,
                        modified
                    );
                }
                if let Some(accessed) = &file.accessed {
                    assert!(
                        DateTime::parse_from_rfc3339(accessed).is_ok(),
                        "File {} accessed time not RFC 3339: {}",
                        file.path,
                        accessed
                    );
                }
            }
            FsElement::Directory(dir) => {
                verify_rfc3339_recursive(dir);
            }
        }
    }
}

#[test]
fn test_integration_extended_attributes() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // create test files
    fs::write(temp_dir.path().join("test1.txt"), "test content 1").unwrap();
    fs::write(temp_dir.path().join("test2.txt"), "test content 2").unwrap();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(
        temp_dir.path().join("subdir").join("nested.txt"),
        "nested content",
    )
    .unwrap();

    let explorer = FilesystemExplorer::new(temp_path);
    let mut result = explorer.scan_from_root().unwrap();

    // set extended attributes on files
    for element in &mut result.elements {
        if let FsElement::File(file) = element {
            if file.path.ends_with("test1.txt") {
                let _ = file.set_ext_attribute("user.description", b"First test file");
                let _ = file.set_ext_attribute("user.priority", b"high");
            } else if file.path.ends_with("test2.txt") {
                let _ = file.set_ext_attribute("user.description", b"Second test file");
                let _ = file.set_ext_attribute("user.category", b"testing");
            }
        }
    }

    // test recursive search for extended attributes
    let _descriptions = result.search_ext_attributes_recursive("user.description");
    // may be 0 if xattrs not supported on filesystem, but should not panic

    // test listing all extended attributes
    let _all_attrs = result.list_all_ext_attributes_recursive();
    // may be 0 if xattrs not supported on filesystem, but should not panic

    // verify extended attributes scanning works without errors
    for element in &mut result.elements {
        match element {
            FsElement::File(file) => {
                let attrs = file.ext_attributes_scan();
                assert!(attrs.is_ok());
            }
            FsElement::Directory(dir) => {
                let attrs = dir.ext_attributes_scan();
                assert!(attrs.is_ok());
            }
        }
    }
}
