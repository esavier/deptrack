use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum TomlError {
    IoError(String),
    ParseError(String),
    FieldNotFound(String),
    InvalidType(String),
}

impl fmt::Display for TomlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TomlError::IoError(msg) => write!(f, "IO error: {}", msg),
            TomlError::ParseError(msg) => write!(f, "TOML parse error: {}", msg),
            TomlError::FieldNotFound(field) => write!(f, "Field '{}' not found", field),
            TomlError::InvalidType(msg) => write!(f, "Invalid type: {}", msg),
        }
    }
}

impl Error for TomlError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlDocument {
    pub file_path: String,
    pub content: toml::Value,
}

impl TomlDocument {
    pub fn new(file_path: String, content: toml::Value) -> Self {
        Self { file_path, content }
    }

    pub fn get_field(&self, field_name: &str) -> Option<&toml::Value> {
        self.content.get(field_name)
    }

    pub fn get_table(&self, table_name: &str) -> Option<&toml::value::Table> {
        self.content.get(table_name)?.as_table()
    }

    pub fn get_string(&self, field_name: &str) -> Option<String> {
        self.content
            .get(field_name)?
            .as_str()
            .map(|s| s.to_string())
    }

    pub fn get_version(&self) -> Option<String> {
        // Try root level version first
        if let Some(version) = self.get_string("version") {
            return Some(version);
        }

        // Try package.version for Cargo.toml files
        if let Some(package_table) = self.get_table("package")
            && let Some(version_str) = package_table.get("version")?.as_str()
        {
            return Some(version_str.to_string());
        }

        None
    }

    pub fn get_dependencies(&self) -> Option<HashMap<String, serde_json::Value>> {
        let deps_table = self.get_table("dependencies")?;
        let mut deps = HashMap::new();

        for (name, value) in deps_table {
            let json_value = toml_value_to_json(value);
            deps.insert(name.clone(), json_value);
        }

        Some(deps)
    }

    pub fn get_dev_dependencies(&self) -> Option<HashMap<String, serde_json::Value>> {
        let deps_table = self.get_table("dev-dependencies")?;
        let mut deps = HashMap::new();

        for (name, value) in deps_table {
            let json_value = toml_value_to_json(value);
            deps.insert(name.clone(), json_value);
        }

        Some(deps)
    }

    pub fn get_build_dependencies(&self) -> Option<HashMap<String, serde_json::Value>> {
        let deps_table = self.get_table("build-dependencies")?;
        let mut deps = HashMap::new();

        for (name, value) in deps_table {
            let json_value = toml_value_to_json(value);
            deps.insert(name.clone(), json_value);
        }

        Some(deps)
    }

    pub fn has_field(&self, field_name: &str) -> bool {
        self.content.get(field_name).is_some()
    }

    pub fn has_table(&self, table_name: &str) -> bool {
        self.get_table(table_name).is_some()
    }
}

fn toml_value_to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        toml::Value::Float(f) => {
            if let Some(num) = serde_json::Number::from_f64(*f) {
                serde_json::Value::Number(num)
            } else {
                serde_json::Value::Null
            }
        }
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(toml_value_to_json).collect();
            serde_json::Value::Array(json_arr)
        }
        toml::Value::Table(table) => {
            let mut json_obj = serde_json::Map::new();
            for (key, val) in table {
                json_obj.insert(key.clone(), toml_value_to_json(val));
            }
            serde_json::Value::Object(json_obj)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TomlContext {
    pub document: TomlDocument,
    pub file_path: String,
}

impl TomlContext {
    pub fn new(document: TomlDocument) -> Self {
        let file_path = document.file_path.clone();
        Self {
            document,
            file_path,
        }
    }
}
