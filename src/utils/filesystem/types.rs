#[derive(Debug, Clone, Copy)]
pub enum HashType {
    CRC32,  // general file sum
    SHA256, // gfeneral security file sum
    SHA512, // if you want to be extra sure
    BLAKE3, // advised for general usage, speed + security
    ALL,    // all of the above
    FAST,   // crc32 + blake3 // fast but still usefull
}

#[derive(Debug, Clone)]
pub enum FsElement {
    File(FsFile),
    Directory(FsDirectory),
}

// represents file, abastract form the FS and quite dumbed down

#[derive(Debug, Clone)]
pub struct FsFile {
    pub path: String,

    // optional after scan
    pub size: Option<u64>,
    pub is_read: bool,
    pub is_write: bool,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub accessed: Option<String>,
    pub permissions: Option<Permissions>,

    // optional after scan (extra)
    pub extension: Option<String>, // part after last dot if any
    pub name: Option<String>,      // part before last dot if any
    pub magic: Option<String>, // output of `file` command if available, warning, this is unreliable

    // hashes
    pub crc32: Option<String>,
    pub sha256: Option<String>,
    pub sha512: Option<String>,
    pub blake3: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Permissions {
    pub owner_read: bool,
    pub owner_write: bool,
    pub owner_execute: bool,
    pub group_read: bool,
    pub group_write: bool,
    pub group_execute: bool,
    pub others_read: bool,
    pub others_write: bool,
    pub others_execute: bool,
}

#[derive(Debug, Clone)]
pub struct ExtAttributes {
    pub name: String,
    pub value: String,
}

// defines a generic directory type
// very basic and abstract from the filesystem
#[derive(Debug, Clone)]
pub struct FsDirectory {
    pub path: String,
    pub elements: Vec<FsElement>,
    pub is_root: bool,

    pub is_read: bool,
    pub is_write: bool,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub accessed: Option<String>,
    pub permissions: Option<Permissions>,
}

impl FsDirectory {
    pub fn new(path: String) -> Self {
        FsDirectory {
            path,
            elements: Vec::new(),
            is_root: false,
            is_read: false,
            is_write: false,
            owner: None,
            group: None,
            created: None,
            modified: None,
            accessed: None,
            permissions: None,
        }
    }

    pub fn new_root(path: String) -> Self {
        FsDirectory {
            path,
            elements: Vec::new(),
            is_root: true,
            is_read: false,
            is_write: false,
            owner: None,
            group: None,
            created: None,
            modified: None,
            accessed: None,
            permissions: None,
        }
    }

    pub fn scan(&mut self) -> Result<(), std::io::Error> {
        use std::fs;

        let entries = fs::read_dir(&self.path)?;
        self.elements.clear();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if path.is_file() {
                let file = FsFile::new(path_str);
                self.elements.push(FsElement::File(file));
            } else if path.is_dir() {
                let mut subdir = FsDirectory::new(path_str);
                subdir.scan()?;
                self.elements.push(FsElement::Directory(subdir));
            }
        }
        Ok(())
    }

    pub fn hash(&mut self, hash_type: HashType) -> Result<(), std::io::Error> {
        // hash the directory contents (follows files and directories recursively)
        self.hash_recursive(hash_type)
    }

    fn hash_recursive(&mut self, hash_type: HashType) -> Result<(), std::io::Error> {
        for element in &mut self.elements {
            match element {
                FsElement::File(file) => {
                    file.hash(hash_type)?;
                }
                FsElement::Directory(dir) => {
                    dir.hash_recursive(hash_type)?;
                }
            }
        }
        Ok(())
    }

    pub fn metadata_scan(&mut self) -> Result<(), std::io::Error> {
        use chrono::{DateTime, Utc};
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(&self.path)?;
        let permissions = metadata.permissions();

        self.permissions = Some(Permissions {
            owner_read: permissions.mode() & 0o400 != 0,
            owner_write: permissions.mode() & 0o200 != 0,
            owner_execute: permissions.mode() & 0o100 != 0,
            group_read: permissions.mode() & 0o040 != 0,
            group_write: permissions.mode() & 0o020 != 0,
            group_execute: permissions.mode() & 0o010 != 0,
            others_read: permissions.mode() & 0o004 != 0,
            others_write: permissions.mode() & 0o002 != 0,
            others_execute: permissions.mode() & 0o001 != 0,
        });

        if let Ok(created) = metadata.created() {
            let datetime: DateTime<Utc> = created.into();
            self.created = Some(datetime.to_rfc3339());
        }
        if let Ok(modified) = metadata.modified() {
            let datetime: DateTime<Utc> = modified.into();
            self.modified = Some(datetime.to_rfc3339());
        }
        if let Ok(accessed) = metadata.accessed() {
            let datetime: DateTime<Utc> = accessed.into();
            self.accessed = Some(datetime.to_rfc3339());
        }

        // recursively scan elements
        for element in &mut self.elements {
            match element {
                FsElement::File(file) => {
                    let _ = file.metadata_scan();
                }
                FsElement::Directory(dir) => {
                    let _ = dir.metadata_scan();
                }
            }
        }

        Ok(())
    }

    pub fn ext_attributes_scan(&mut self) -> Result<Vec<ExtAttributes>, std::io::Error> {
        use xattr;

        let mut attributes = Vec::new();

        match xattr::list(&self.path) {
            Ok(names) => {
                for name in names {
                    if let Some(name_str) = name.to_str()
                        && let Ok(Some(value)) = xattr::get(&self.path, &name)
                    {
                        let value_str = match String::from_utf8(value) {
                            Ok(s) => s,
                            Err(e) => format!("(binary: {})", hex::encode(e.into_bytes())),
                        };

                        attributes.push(ExtAttributes {
                            name: name_str.to_string(),
                            value: value_str,
                        });
                    }
                }
            }
            Err(_) => return Ok(attributes),
        }

        Ok(attributes)
    }

    pub fn set_ext_attribute(&self, name: &str, value: &[u8]) -> Result<(), std::io::Error> {
        use xattr;

        xattr::set(&self.path, name, value).map_err(std::io::Error::other)
    }

    pub fn remove_ext_attribute(&self, name: &str) -> Result<(), std::io::Error> {
        use xattr;

        xattr::remove(&self.path, name).map_err(std::io::Error::other)
    }

    pub fn get_ext_attribute(&self, name: &str) -> Result<Option<Vec<u8>>, std::io::Error> {
        use xattr;

        xattr::get(&self.path, name).map_err(std::io::Error::other)
    }

    pub fn search_ext_attributes_recursive(&self, attribute_name: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        self.search_ext_attributes_recursive_impl(attribute_name, &mut results);
        results
    }

    fn search_ext_attributes_recursive_impl(
        &self,
        attribute_name: &str,
        results: &mut Vec<(String, String)>,
    ) {
        // check this directory
        if let Ok(Some(value)) = self.get_ext_attribute(attribute_name) {
            let value_str = match String::from_utf8(value) {
                Ok(s) => s,
                Err(e) => format!("(binary: {})", hex::encode(e.into_bytes())),
            };
            results.push((self.path.clone(), value_str));
        }

        // recursively check all elements
        for element in &self.elements {
            match element {
                FsElement::File(file) => {
                    if let Ok(Some(value)) = file.get_ext_attribute(attribute_name) {
                        let value_str = match String::from_utf8(value) {
                            Ok(s) => s,
                            Err(e) => format!("(binary: {})", hex::encode(e.into_bytes())),
                        };
                        results.push((file.path.clone(), value_str));
                    }
                }
                FsElement::Directory(dir) => {
                    dir.search_ext_attributes_recursive_impl(attribute_name, results);
                }
            }
        }
    }

    pub fn list_all_ext_attributes_recursive(&self) -> Vec<(String, Vec<ExtAttributes>)> {
        let mut results = Vec::new();
        self.list_all_ext_attributes_recursive_impl(&mut results);
        results
    }

    fn list_all_ext_attributes_recursive_impl(
        &self,
        results: &mut Vec<(String, Vec<ExtAttributes>)>,
    ) {
        // get attributes for this directory
        if let Ok(attrs) = FsDirectory::ext_attributes_scan(&mut self.clone())
            && !attrs.is_empty()
        {
            results.push((self.path.clone(), attrs));
        }

        // recursively check all elements
        for element in &self.elements {
            match element {
                FsElement::File(file) => {
                    if let Ok(attrs) = FsFile::ext_attributes_scan(&mut file.clone())
                        && !attrs.is_empty()
                    {
                        results.push((file.path.clone(), attrs));
                    }
                }
                FsElement::Directory(dir) => {
                    dir.list_all_ext_attributes_recursive_impl(results);
                }
            }
        }
    }
}

impl FsFile {
    pub fn new(path: String) -> Self {
        FsFile {
            path,
            size: None,
            is_read: false,
            is_write: false,
            owner: None,
            group: None,
            created: None,
            modified: None,
            accessed: None,
            permissions: None,
            extension: None,
            name: None,
            magic: None,
            crc32: None,
            sha256: None,
            sha512: None,
            blake3: None,
        }
    }

    pub fn hash(&mut self, hash_type: HashType) -> Result<(), std::io::Error> {
        use blake3::Hasher as Blake3Hasher;
        use crc32fast::Hasher as Crc32Hasher;
        use sha2::{Digest, Sha256, Sha512};
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(&self.path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        match hash_type {
            HashType::CRC32 => {
                let mut hasher = Crc32Hasher::new();
                hasher.update(&buffer);
                self.crc32 = Some(format!("{:08x}", hasher.finalize()));
            }
            HashType::SHA256 => {
                let mut hasher = Sha256::new();
                hasher.update(&buffer);
                self.sha256 = Some(format!("{:x}", hasher.finalize()));
            }
            HashType::SHA512 => {
                let mut hasher = Sha512::new();
                hasher.update(&buffer);
                self.sha512 = Some(format!("{:x}", hasher.finalize()));
            }
            HashType::BLAKE3 => {
                let mut hasher = Blake3Hasher::new();
                hasher.update(&buffer);
                self.blake3 = Some(format!("{}", hasher.finalize().to_hex()));
            }
            HashType::ALL => {
                self.hash(HashType::CRC32)?;
                self.hash(HashType::SHA256)?;
                self.hash(HashType::SHA512)?;
                self.hash(HashType::BLAKE3)?;
            }
            HashType::FAST => {
                self.hash(HashType::CRC32)?;
                self.hash(HashType::BLAKE3)?;
            }
        }

        Ok(())
    }

    pub fn metadata_scan(&mut self) -> Result<(), std::io::Error> {
        use chrono::{DateTime, Utc};
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;

        let metadata = fs::metadata(&self.path)?;
        let permissions = metadata.permissions();

        self.size = Some(metadata.len());
        self.permissions = Some(Permissions {
            owner_read: permissions.mode() & 0o400 != 0,
            owner_write: permissions.mode() & 0o200 != 0,
            owner_execute: permissions.mode() & 0o100 != 0,
            group_read: permissions.mode() & 0o040 != 0,
            group_write: permissions.mode() & 0o020 != 0,
            group_execute: permissions.mode() & 0o010 != 0,
            others_read: permissions.mode() & 0o004 != 0,
            others_write: permissions.mode() & 0o002 != 0,
            others_execute: permissions.mode() & 0o001 != 0,
        });

        if let Ok(created) = metadata.created() {
            let datetime: DateTime<Utc> = created.into();
            self.created = Some(datetime.to_rfc3339());
        }
        if let Ok(modified) = metadata.modified() {
            let datetime: DateTime<Utc> = modified.into();
            self.modified = Some(datetime.to_rfc3339());
        }
        if let Ok(accessed) = metadata.accessed() {
            let datetime: DateTime<Utc> = accessed.into();
            self.accessed = Some(datetime.to_rfc3339());
        }

        // extract file name and extension
        let path = Path::new(&self.path);
        if let Some(file_name) = path.file_name() {
            let file_name_str = file_name.to_string_lossy();
            if let Some(extension) = path.extension() {
                self.extension = Some(extension.to_string_lossy().to_string());
                let name =
                    file_name_str.trim_end_matches(&format!(".{}", extension.to_string_lossy()));
                self.name = Some(name.to_string());
            } else {
                self.name = Some(file_name_str.to_string());
            }
        }

        Ok(())
    }

    pub fn ext_attributes_scan(&mut self) -> Result<Vec<ExtAttributes>, std::io::Error> {
        use xattr;

        let mut attributes = Vec::new();

        match xattr::list(&self.path) {
            Ok(names) => {
                for name in names {
                    if let Some(name_str) = name.to_str()
                        && let Ok(Some(value)) = xattr::get(&self.path, &name)
                    {
                        // convert bytes to string, handling non-UTF8 values
                        let value_str = match String::from_utf8(value) {
                            Ok(s) => s,
                            Err(e) => {
                                // for non-UTF8, show hex representation
                                format!("(binary: {})", hex::encode(e.into_bytes()))
                            }
                        };

                        attributes.push(ExtAttributes {
                            name: name_str.to_string(),
                            value: value_str,
                        });
                    }
                }
            }
            Err(_) => {
                // extended attributes not supported or accessible
                return Ok(attributes);
            }
        }

        Ok(attributes)
    }

    pub fn set_ext_attribute(&self, name: &str, value: &[u8]) -> Result<(), std::io::Error> {
        use xattr;

        xattr::set(&self.path, name, value).map_err(std::io::Error::other)
    }

    pub fn remove_ext_attribute(&self, name: &str) -> Result<(), std::io::Error> {
        use xattr;

        xattr::remove(&self.path, name).map_err(std::io::Error::other)
    }

    pub fn get_ext_attribute(&self, name: &str) -> Result<Option<Vec<u8>>, std::io::Error> {
        use xattr;

        xattr::get(&self.path, name).map_err(std::io::Error::other)
    }

    pub fn open(&mut self) -> Result<std::fs::File, std::io::Error> {
        // also trigger all scans
        std::fs::File::open(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn test_fsfile_new() {
        let file = FsFile::new("test.txt".to_string());
        assert_eq!(file.path, "test.txt");
        assert_eq!(file.size, None);
        assert_eq!(file.extension, None);
        assert_eq!(file.name, None);
        assert!(!file.is_read);
        assert!(!file.is_write);
    }

    #[test]
    fn test_fsfile_metadata_scan() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        assert!(file.size.is_some());
        assert_eq!(file.size.unwrap(), 12); // "test content".len()
        assert_eq!(file.extension, Some("txt".to_string()));
        assert_eq!(file.name, Some("test_file".to_string()));
        assert!(file.permissions.is_some());
        assert!(file.created.is_some());
        assert!(file.modified.is_some());
        assert!(file.accessed.is_some());
    }

    #[test]
    fn test_fsfile_metadata_scan_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("README");
        fs::write(&file_path, "readme content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        assert_eq!(file.extension, None);
        assert_eq!(file.name, Some("README".to_string()));
    }

    #[test]
    fn test_fsfile_open() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        let opened_file = file.open().unwrap();
        assert!(opened_file.metadata().unwrap().is_file());
    }

    #[test]
    fn test_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        // set specific permissions
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644); // rw-r--r--
        fs::set_permissions(&file_path, perms).unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        let permissions = file.permissions.unwrap();
        assert!(permissions.owner_read);
        assert!(permissions.owner_write);
        assert!(!permissions.owner_execute);
        assert!(permissions.group_read);
        assert!(!permissions.group_write);
        assert!(!permissions.group_execute);
        assert!(permissions.others_read);
        assert!(!permissions.others_write);
        assert!(!permissions.others_execute);
    }

    #[test]
    fn test_fsdirectory_new() {
        let dir = FsDirectory::new("/test/path".to_string());
        assert_eq!(dir.path, "/test/path");
        assert!(dir.elements.is_empty());
        assert!(!dir.is_read);
        assert!(!dir.is_write);
        assert!(!dir.is_root);
        assert!(dir.permissions.is_none());
    }

    #[test]
    fn test_fsdirectory_new_root() {
        let dir = FsDirectory::new_root("/test/path".to_string());
        assert_eq!(dir.path, "/test/path");
        assert!(dir.elements.is_empty());
        assert!(!dir.is_read);
        assert!(!dir.is_write);
        assert!(dir.is_root);
        assert!(dir.permissions.is_none());
    }

    #[test]
    fn test_fsdirectory_scan() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test files and subdirectories
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "fn main() {}").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("nested.txt"), "nested").unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();

        assert_eq!(directory.elements.len(), 3); // file1.txt, file2.rs, subdir

        let mut files = 0;
        let mut dirs = 0;
        for element in &directory.elements {
            match element {
                FsElement::File(_) => files += 1,
                FsElement::Directory(dir) => {
                    dirs += 1;
                    assert_eq!(dir.elements.len(), 1); // nested.txt
                }
            }
        }
        assert_eq!(files, 2);
        assert_eq!(dirs, 1);
    }

    #[test]
    fn test_fsdirectory_metadata_scan() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();
        directory.metadata_scan().unwrap();

        assert!(directory.permissions.is_some());
        assert!(directory.created.is_some());
        assert!(directory.modified.is_some());
        assert!(directory.accessed.is_some());

        // check that files were also scanned
        if let FsElement::File(file) = &directory.elements[0] {
            assert!(file.permissions.is_some());
            assert!(file.size.is_some());
        }
    }

    #[test]
    fn test_fselement_debug() {
        let file = FsFile::new("test.txt".to_string());
        let element = FsElement::File(file);
        let debug_str = format!("{:?}", element);
        assert!(debug_str.contains("File"));
        assert!(debug_str.contains("test.txt"));
    }

    #[test]
    fn test_fselement_clone() {
        let file = FsFile::new("test.txt".to_string());
        let element = FsElement::File(file);
        let cloned = element.clone();

        match (&element, &cloned) {
            (FsElement::File(orig), FsElement::File(clone)) => {
                assert_eq!(orig.path, clone.path);
            }
            _ => panic!("Clone should preserve variant"),
        }
    }

    #[test]
    fn test_fsfile_hash_crc32() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::CRC32).unwrap();

        assert!(file.crc32.is_some());
        assert_eq!(file.crc32.unwrap(), "57f4675d"); // CRC32 for "test content"
        assert!(file.sha256.is_none());
        assert!(file.sha512.is_none());
        assert!(file.blake3.is_none());
    }

    #[test]
    fn test_fsfile_hash_sha256() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::SHA256).unwrap();

        assert!(file.sha256.is_some());
        let expected_sha256 = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";
        assert_eq!(file.sha256.unwrap(), expected_sha256);
        assert!(file.crc32.is_none());
        assert!(file.sha512.is_none());
        assert!(file.blake3.is_none());
    }

    #[test]
    fn test_fsfile_hash_sha512() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::SHA512).unwrap();

        assert!(file.sha512.is_some());
        assert!(file.sha512.unwrap().len() == 128); // SHA512 produces 64-byte hash = 128 hex chars
        assert!(file.crc32.is_none());
        assert!(file.sha256.is_none());
        assert!(file.blake3.is_none());
    }

    #[test]
    fn test_fsfile_hash_blake3() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::BLAKE3).unwrap();

        assert!(file.blake3.is_some());
        assert!(file.blake3.unwrap().len() == 64); // BLAKE3 produces 32-byte hash = 64 hex chars
        assert!(file.crc32.is_none());
        assert!(file.sha256.is_none());
        assert!(file.sha512.is_none());
    }

    #[test]
    fn test_fsfile_hash_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::ALL).unwrap();

        assert!(file.crc32.is_some());
        assert!(file.sha256.is_some());
        assert!(file.sha512.is_some());
        assert!(file.blake3.is_some());
    }

    #[test]
    fn test_fsfile_hash_fast() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::FAST).unwrap();

        assert!(file.crc32.is_some());
        assert!(file.blake3.is_some());
        assert!(file.sha256.is_none());
        assert!(file.sha512.is_none());
    }

    #[test]
    fn test_fsfile_hash_different_content() {
        let temp_dir = TempDir::new().unwrap();

        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        fs::write(&file1_path, "content A").unwrap();
        fs::write(&file2_path, "content B").unwrap();

        let mut file1 = FsFile::new(file1_path.to_string_lossy().to_string());
        let mut file2 = FsFile::new(file2_path.to_string_lossy().to_string());

        file1.hash(HashType::SHA256).unwrap();
        file2.hash(HashType::SHA256).unwrap();

        assert_ne!(file1.sha256, file2.sha256);
    }

    #[test]
    fn test_fsfile_hash_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.hash(HashType::SHA256).unwrap();

        assert!(file.sha256.is_some());
        // SHA256 of empty string
        assert_eq!(
            file.sha256.unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_fsdirectory_hash_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test structure
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(
            temp_dir.path().join("subdir").join("nested.txt"),
            "nested content",
        )
        .unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();
        directory.hash(HashType::SHA256).unwrap();

        // verify all files were hashed
        for element in &directory.elements {
            match element {
                FsElement::File(file) => {
                    assert!(file.sha256.is_some());
                }
                FsElement::Directory(dir) => {
                    for nested_element in &dir.elements {
                        if let FsElement::File(nested_file) = nested_element {
                            assert!(nested_file.sha256.is_some());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_hashtype_debug() {
        assert_eq!(format!("{:?}", HashType::CRC32), "CRC32");
        assert_eq!(format!("{:?}", HashType::SHA256), "SHA256");
        assert_eq!(format!("{:?}", HashType::BLAKE3), "BLAKE3");
        assert_eq!(format!("{:?}", HashType::ALL), "ALL");
        assert_eq!(format!("{:?}", HashType::FAST), "FAST");
    }

    #[test]
    fn test_hashtype_clone() {
        let hash_type = HashType::SHA256;
        let cloned = hash_type;
        assert_eq!(format!("{:?}", hash_type), format!("{:?}", cloned));
    }

    #[test]
    fn test_rfc3339_time_format() {
        use chrono::DateTime;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        // verify all time fields are RFC 3339 compliant
        if let Some(created) = &file.created {
            assert!(
                DateTime::parse_from_rfc3339(created).is_ok(),
                "Created time is not RFC 3339: {}",
                created
            );
            // UTC should be either Z or +00:00
            assert!(
                created.ends_with('Z') || created.ends_with("+00:00"),
                "Created time should be UTC: {}",
                created
            );
        }

        if let Some(modified) = &file.modified {
            assert!(
                DateTime::parse_from_rfc3339(modified).is_ok(),
                "Modified time is not RFC 3339: {}",
                modified
            );
            assert!(
                modified.ends_with('Z') || modified.ends_with("+00:00"),
                "Modified time should be UTC: {}",
                modified
            );
        }

        if let Some(accessed) = &file.accessed {
            assert!(
                DateTime::parse_from_rfc3339(accessed).is_ok(),
                "Accessed time is not RFC 3339: {}",
                accessed
            );
            assert!(
                accessed.ends_with('Z') || accessed.ends_with("+00:00"),
                "Accessed time should be UTC: {}",
                accessed
            );
        }
    }

    #[test]
    fn test_directory_rfc3339_time_format() {
        use chrono::DateTime;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();
        directory.metadata_scan().unwrap();

        // verify directory times are RFC 3339 compliant
        if let Some(created) = &directory.created {
            assert!(
                DateTime::parse_from_rfc3339(created).is_ok(),
                "Directory created time is not RFC 3339: {}",
                created
            );
        }

        if let Some(modified) = &directory.modified {
            assert!(
                DateTime::parse_from_rfc3339(modified).is_ok(),
                "Directory modified time is not RFC 3339: {}",
                modified
            );
        }

        if let Some(accessed) = &directory.accessed {
            assert!(
                DateTime::parse_from_rfc3339(accessed).is_ok(),
                "Directory accessed time is not RFC 3339: {}",
                accessed
            );
        }

        // verify nested file times are also RFC 3339 compliant
        for element in &directory.elements {
            if let FsElement::File(file) = element {
                if let Some(created) = &file.created {
                    assert!(
                        DateTime::parse_from_rfc3339(created).is_ok(),
                        "Nested file created time is not RFC 3339: {}",
                        created
                    );
                }
                if let Some(modified) = &file.modified {
                    assert!(
                        DateTime::parse_from_rfc3339(modified).is_ok(),
                        "Nested file modified time is not RFC 3339: {}",
                        modified
                    );
                }
                if let Some(accessed) = &file.accessed {
                    assert!(
                        DateTime::parse_from_rfc3339(accessed).is_ok(),
                        "Nested file accessed time is not RFC 3339: {}",
                        accessed
                    );
                }
            }
        }
    }

    #[test]
    fn test_rfc3339_time_consistency() {
        use chrono::DateTime;
        use std::time::SystemTime;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("consistency_test.txt");

        // record time with some tolerance for filesystem precision
        let before = SystemTime::now() - std::time::Duration::from_secs(1);
        fs::write(&file_path, "consistency test").unwrap();
        let after = SystemTime::now() + std::time::Duration::from_secs(1);

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        // verify the RFC 3339 timestamp falls within our time window
        if let Some(created_str) = &file.created {
            let created_time = DateTime::parse_from_rfc3339(created_str).unwrap();
            let created_system: SystemTime = created_time.into();

            assert!(
                created_system >= before,
                "Created time {} is before operation start",
                created_str
            );
            assert!(
                created_system <= after,
                "Created time {} is after operation end",
                created_str
            );
        }
    }

    #[test]
    fn test_rfc3339_format_structure() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("format_test.txt");
        fs::write(&file_path, "format test").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());
        file.metadata_scan().unwrap();

        if let Some(created) = &file.created {
            // RFC 3339 format should be: YYYY-MM-DDTHH:MM:SS.sssZ
            assert!(
                created.len() >= 19,
                "RFC 3339 timestamp too short: {}",
                created
            ); // minimum: 2023-01-01T00:00:00Z
            assert!(
                created.contains('T'),
                "RFC 3339 should contain 'T' separator: {}",
                created
            );
            assert!(
                created.contains(':'),
                "RFC 3339 should contain ':' in time: {}",
                created
            );
            assert!(
                created.contains('-'),
                "RFC 3339 should contain '-' in date: {}",
                created
            );

            // should be UTC (end with Z) or have timezone offset (+00:00)
            assert!(
                created.ends_with('Z')
                    || created.ends_with("+00:00")
                    || created.contains('+')
                    || created.matches('-').count() > 2,
                "RFC 3339 should end with Z or have timezone offset: {}",
                created
            );
        }
    }

    #[test]
    fn test_ext_attributes_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());

        // test setting extended attribute
        let result = file.set_ext_attribute("user.test_attr", b"test_value");
        // extended attributes may not be supported on all filesystems, so we handle both cases
        match result {
            Ok(_) => {
                // if setting succeeded, test getting it back
                let value = file.get_ext_attribute("user.test_attr").unwrap();
                assert_eq!(value, Some(b"test_value".to_vec()));

                // test listing attributes
                let attrs = file.ext_attributes_scan().unwrap();
                let test_attr = attrs.iter().find(|a| a.name == "user.test_attr");
                assert!(test_attr.is_some());
                assert_eq!(test_attr.unwrap().value, "test_value");

                // test removing attribute
                file.remove_ext_attribute("user.test_attr").unwrap();
                let value_after_remove = file.get_ext_attribute("user.test_attr").unwrap();
                assert_eq!(value_after_remove, None);
            }
            Err(_) => {
                // extended attributes not supported on this filesystem, skip test
                println!("Extended attributes not supported on this filesystem, skipping test");
            }
        }
    }

    #[test]
    fn test_ext_attributes_directory_operations() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        let mut directory = FsDirectory::new(temp_path);

        // test setting extended attribute on directory
        let result = directory.set_ext_attribute("user.dir_attr", b"dir_value");
        match result {
            Ok(_) => {
                // test getting it back
                let value = directory.get_ext_attribute("user.dir_attr").unwrap();
                assert_eq!(value, Some(b"dir_value".to_vec()));

                // test listing attributes
                let attrs = directory.ext_attributes_scan().unwrap();
                let dir_attr = attrs.iter().find(|a| a.name == "user.dir_attr");
                assert!(dir_attr.is_some());
                assert_eq!(dir_attr.unwrap().value, "dir_value");

                // test removing attribute
                directory.remove_ext_attribute("user.dir_attr").unwrap();
                let value_after_remove = directory.get_ext_attribute("user.dir_attr").unwrap();
                assert_eq!(value_after_remove, None);
            }
            Err(_) => {
                println!("Extended attributes not supported on this filesystem, skipping test");
            }
        }
    }

    #[test]
    fn test_ext_attributes_recursive_search() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test structure
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("nested.txt"), "nested").unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();

        // try to set extended attributes on files
        let file1 = FsFile::new(
            temp_dir
                .path()
                .join("file1.txt")
                .to_string_lossy()
                .to_string(),
        );
        let nested_file = FsFile::new(
            temp_dir
                .path()
                .join("subdir")
                .join("nested.txt")
                .to_string_lossy()
                .to_string(),
        );

        let result1 = file1.set_ext_attribute("user.search_test", b"found_file1");
        let result2 = nested_file.set_ext_attribute("user.search_test", b"found_nested");

        if result1.is_ok() && result2.is_ok() {
            // rescan to pick up the structure
            directory.scan().unwrap();

            // test recursive search
            let results = directory.search_ext_attributes_recursive("user.search_test");
            assert!(!results.is_empty());

            // check that we found the files
            let file1_found = results
                .iter()
                .any(|(path, value)| path.ends_with("file1.txt") && value == "found_file1");
            let nested_found = results
                .iter()
                .any(|(path, value)| path.ends_with("nested.txt") && value == "found_nested");

            assert!(file1_found, "Should find file1.txt with its attribute");
            assert!(nested_found, "Should find nested.txt with its attribute");
        } else {
            println!("Extended attributes not supported, skipping recursive search test");
        }
    }

    #[test]
    fn test_ext_attributes_list_all_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // create test structure
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("nested.txt"), "nested").unwrap();

        let mut directory = FsDirectory::new(temp_path);
        directory.scan().unwrap();

        // try to set multiple extended attributes
        let file1 = FsFile::new(
            temp_dir
                .path()
                .join("file1.txt")
                .to_string_lossy()
                .to_string(),
        );
        let result1 = file1.set_ext_attribute("user.attr1", b"value1");
        let result2 = file1.set_ext_attribute("user.attr2", b"value2");

        if result1.is_ok() && result2.is_ok() {
            directory.scan().unwrap();

            // test listing all attributes recursively
            let all_attrs = directory.list_all_ext_attributes_recursive();

            // find our file in the results
            let file1_attrs = all_attrs
                .iter()
                .find(|(path, _)| path.ends_with("file1.txt"));
            if let Some((_, attrs)) = file1_attrs {
                assert!(attrs.len() >= 2);
                let attr1 = attrs.iter().find(|a| a.name == "user.attr1");
                let attr2 = attrs.iter().find(|a| a.name == "user.attr2");

                assert!(attr1.is_some());
                assert!(attr2.is_some());
                assert_eq!(attr1.unwrap().value, "value1");
                assert_eq!(attr2.unwrap().value, "value2");
            }
        } else {
            println!("Extended attributes not supported, skipping list all test");
        }
    }

    #[test]
    fn test_ext_attributes_binary_data() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("binary_test.txt");
        fs::write(&file_path, "binary test").unwrap();

        let mut file = FsFile::new(file_path.to_string_lossy().to_string());

        // test binary data
        let binary_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        let result = file.set_ext_attribute("user.binary_attr", &binary_data);

        if result.is_ok() {
            let value = file.get_ext_attribute("user.binary_attr").unwrap();
            assert_eq!(value, Some(binary_data.clone()));

            // test that ext_attributes_scan handles binary data properly
            let attrs = file.ext_attributes_scan().unwrap();
            let binary_attr = attrs.iter().find(|a| a.name == "user.binary_attr");
            assert!(binary_attr.is_some());

            // should be hex encoded since it's not valid UTF-8
            let attr_value = &binary_attr.unwrap().value;
            assert!(attr_value.starts_with("(binary: "));
            assert!(attr_value.contains("0001"));
        } else {
            println!("Extended attributes not supported, skipping binary data test");
        }
    }
}
