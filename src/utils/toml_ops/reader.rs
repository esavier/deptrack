use super::types::{TomlDocument, TomlError};
use std::fs;
use std::path::Path;

pub struct TomlReader;

impl TomlReader {
    pub fn read_file<P: AsRef<Path>>(file_path: P) -> Result<TomlDocument, TomlError> {
        let path = file_path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        let content = fs::read_to_string(path).map_err(|e| {
            TomlError::IoError(format!("Failed to read file '{}': {}", path_str, e))
        })?;

        let toml_value = content.parse::<toml::Value>().map_err(|e| {
            TomlError::ParseError(format!("Failed to parse TOML in '{}': {}", path_str, e))
        })?;

        Ok(TomlDocument::new(path_str, toml_value))
    }

    pub fn parse_string(content: &str, file_path: String) -> Result<TomlDocument, TomlError> {
        let toml_value = content
            .parse::<toml::Value>()
            .map_err(|e| TomlError::ParseError(format!("Failed to parse TOML: {}", e)))?;

        Ok(TomlDocument::new(file_path, toml_value))
    }

    pub fn read_cargo_toml<P: AsRef<Path>>(directory: P) -> Result<TomlDocument, TomlError> {
        let cargo_path = directory.as_ref().join("Cargo.toml");
        Self::read_file(cargo_path)
    }

    pub fn find_and_read_cargo_toml<P: AsRef<Path>>(
        start_path: P,
    ) -> Result<TomlDocument, TomlError> {
        let mut current_path = start_path.as_ref();

        loop {
            let cargo_path = current_path.join("Cargo.toml");

            if cargo_path.exists() {
                return Self::read_file(cargo_path);
            }

            match current_path.parent() {
                Some(parent) => current_path = parent,
                None => {
                    return Err(TomlError::IoError(
                        "Cargo.toml not found in current directory or any parent directory"
                            .to_string(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_string() {
        let toml_content = r#"
            name = "test-project"
            version = "1.0.0"

            [dependencies]
            serde = "1.0"
            tokio = { version = "1.0", features = ["full"] }
        "#;

        let doc = TomlReader::parse_string(toml_content, "test.toml".to_string()).unwrap();

        assert_eq!(doc.get_string("name"), Some("test-project".to_string()));
        assert_eq!(doc.get_version(), Some("1.0.0".to_string()));
        assert!(doc.has_table("dependencies"));

        let deps = doc.get_dependencies().unwrap();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains_key("serde"));
        assert!(deps.contains_key("tokio"));
    }

    #[test]
    fn test_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("test.toml");

        let toml_content = r#"
            name = "file-test"
            version = "2.0.0"

            [dependencies]
            regex = "1.0"
        "#;

        fs::write(&toml_path, toml_content).unwrap();

        let doc = TomlReader::read_file(&toml_path).unwrap();

        assert_eq!(doc.get_string("name"), Some("file-test".to_string()));
        assert_eq!(doc.get_version(), Some("2.0.0".to_string()));
        assert!(doc.has_table("dependencies"));

        let deps = doc.get_dependencies().unwrap();
        assert!(deps.contains_key("regex"));
    }

    #[test]
    fn test_read_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_content = r#"
            [package]
            name = "my-crate"
            version = "0.1.0"

            [dependencies]
            serde = "1.0"
        "#;

        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content).unwrap();

        let doc = TomlReader::read_cargo_toml(temp_dir.path()).unwrap();

        // Note: Cargo.toml has package.name, not just name
        assert!(doc.has_table("package"));
        assert!(doc.has_table("dependencies"));
    }

    #[test]
    fn test_find_and_read_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let cargo_content = r#"
            [package]
            name = "search-test"
            version = "0.2.0"
        "#;

        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content).unwrap();

        // Search from src directory should find the Cargo.toml in parent
        let doc = TomlReader::find_and_read_cargo_toml(&src_dir).unwrap();
        assert!(doc.has_table("package"));
    }

    #[test]
    fn test_toml_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let bad_toml_path = temp_dir.path().join("bad.toml");

        let bad_toml_content = r#"
            name = "test
            version = 1.0.0
        "#; // Missing closing quote and version should be string

        fs::write(&bad_toml_path, bad_toml_content).unwrap();

        let result = TomlReader::read_file(&bad_toml_path);
        assert!(result.is_err());

        if let Err(TomlError::ParseError(_)) = result {
            // Expected error type
        } else {
            panic!("Expected ParseError");
        }
    }

    #[test]
    fn test_nonexistent_file() {
        let result = TomlReader::read_file("/nonexistent/path/test.toml");
        assert!(result.is_err());

        if let Err(TomlError::IoError(_)) = result {
            // Expected error type
        } else {
            panic!("Expected IoError");
        }
    }
}
