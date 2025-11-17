use deptrack::{FilesystemExplorer, HashType};

fn main() {
    let explorer = FilesystemExplorer::new(".".to_string());

    match explorer.scan_from_root() {
        Ok(mut root_dir) => {
            let (file_count, dir_count) = explorer.count_elements(&root_dir);
            println!(
                "Scanned filesystem: {} files, {} directories",
                file_count, dir_count
            );

            let rs_files = explorer.find_files_by_extension(&root_dir, "rs");
            println!("Found {} Rust files:", rs_files.len());
            for file in rs_files.iter().take(3) {
                println!("  {}", file);
            }
            if rs_files.len() > 3 {
                println!("  ... and {} more", rs_files.len() - 3);
            }

            // demonstrate hashing functionality
            println!("\nHashing Rust files with FAST algorithm (CRC32 + BLAKE3)...");
            if let Err(e) = root_dir.hash(HashType::FAST) {
                eprintln!("Error hashing files: {}", e);
                return;
            }

            let mut hashed_count = 0;
            for file_path in rs_files.iter().take(3) {
                if let Some(file) = find_file_in_tree(&root_dir, file_path)
                    && let (Some(crc32), Some(blake3)) = (&file.crc32, &file.blake3)
                {
                    println!(
                        "  {} - CRC32: {}, BLAKE3: {}...{}",
                        file_path.split('/').next_back().unwrap_or(file_path),
                        crc32,
                        &blake3[..8],
                        &blake3[blake3.len() - 8..]
                    );
                    hashed_count += 1;
                }
            }
            println!("Successfully hashed {} files", hashed_count);
        }
        Err(e) => eprintln!("Error scanning filesystem: {}", e),
    }
}

use deptrack::{FsDirectory, FsElement, FsFile};

fn find_file_in_tree<'a>(directory: &'a FsDirectory, target_path: &str) -> Option<&'a FsFile> {
    for element in &directory.elements {
        match element {
            FsElement::File(file) => {
                if file.path == target_path {
                    return Some(file);
                }
            }
            FsElement::Directory(dir) => {
                if let Some(found) = find_file_in_tree(dir, target_path) {
                    return Some(found);
                }
            }
        }
    }
    None
}
