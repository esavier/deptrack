use super::types::{TomlContext, TomlError};
use crate::utils::alt::Evaluable;
use serde_json;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TomlPredicate {
    HasField(HasFieldPredicate),
    HasTable(HasTablePredicate),
    ExtractVersion(ExtractVersionPredicate),
    ExtractDependencies(ExtractDependenciesPredicate),
    ExtractDevDependencies(ExtractDevDependenciesPredicate),
    ExtractBuildDependencies(ExtractBuildDependenciesPredicate),
    FieldEquals(FieldEqualsPredicate),
    VersionMatches(VersionMatchesPredicate),
}

impl Evaluable for TomlPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match self {
            TomlPredicate::HasField(pred) => pred.evaluate(context),
            TomlPredicate::HasTable(pred) => pred.evaluate(context),
            TomlPredicate::ExtractVersion(pred) => pred.evaluate(context),
            TomlPredicate::ExtractDependencies(pred) => pred.evaluate(context),
            TomlPredicate::ExtractDevDependencies(pred) => pred.evaluate(context),
            TomlPredicate::ExtractBuildDependencies(pred) => pred.evaluate(context),
            TomlPredicate::FieldEquals(pred) => pred.evaluate(context),
            TomlPredicate::VersionMatches(pred) => pred.evaluate(context),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HasFieldPredicate {
    pub field_name: String,
}

impl HasFieldPredicate {
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
        }
    }
}

impl Evaluable for HasFieldPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        Ok(context.document.has_field(&self.field_name))
    }
}

#[derive(Debug, Clone)]
pub struct HasTablePredicate {
    pub table_name: String,
}

impl HasTablePredicate {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

impl Evaluable for HasTablePredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        Ok(context.document.has_table(&self.table_name))
    }
}

#[derive(Debug, Clone)]
pub struct ExtractVersionPredicate {
    pub extracted_version: Option<String>,
}

impl ExtractVersionPredicate {
    pub fn new() -> Self {
        Self {
            extracted_version: None,
        }
    }

    pub fn get_extracted_version(&self) -> Option<&str> {
        self.extracted_version.as_deref()
    }
}

impl Default for ExtractVersionPredicate {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluable for ExtractVersionPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.document.get_version() {
            Some(_version) => {
                // We can't modify self in evaluate, so we just return true if version exists
                // The caller should use a separate method to extract the version
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtractDependenciesPredicate {
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone)]
pub enum DependencyType {
    Regular,
    Dev,
    Build,
}

impl ExtractDependenciesPredicate {
    pub fn regular() -> Self {
        Self {
            dependency_type: DependencyType::Regular,
        }
    }

    pub fn dev() -> Self {
        Self {
            dependency_type: DependencyType::Dev,
        }
    }

    pub fn build() -> Self {
        Self {
            dependency_type: DependencyType::Build,
        }
    }
}

impl Evaluable for ExtractDependenciesPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        let has_deps = match self.dependency_type {
            DependencyType::Regular => context.document.get_dependencies().is_some(),
            DependencyType::Dev => context.document.get_dev_dependencies().is_some(),
            DependencyType::Build => context.document.get_build_dependencies().is_some(),
        };

        Ok(has_deps)
    }
}

#[derive(Debug, Clone)]
pub struct ExtractDevDependenciesPredicate;

impl ExtractDevDependenciesPredicate {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExtractDevDependenciesPredicate {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluable for ExtractDevDependenciesPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        Ok(context.document.get_dev_dependencies().is_some())
    }
}

#[derive(Debug, Clone)]
pub struct ExtractBuildDependenciesPredicate;

impl ExtractBuildDependenciesPredicate {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExtractBuildDependenciesPredicate {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluable for ExtractBuildDependenciesPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        Ok(context.document.get_build_dependencies().is_some())
    }
}

#[derive(Debug, Clone)]
pub struct FieldEqualsPredicate {
    pub field_name: String,
    pub expected_value: String,
}

impl FieldEqualsPredicate {
    pub fn new(field_name: impl Into<String>, expected_value: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            expected_value: expected_value.into(),
        }
    }
}

impl Evaluable for FieldEqualsPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.document.get_string(&self.field_name) {
            Some(value) => Ok(value == self.expected_value),
            None => Ok(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VersionMatchesPredicate {
    pub version_pattern: String,
    pub match_type: VersionMatchType,
}

#[derive(Debug, Clone)]
pub enum VersionMatchType {
    Exact,
    StartsWith,
    Contains,
    Regex(String),
}

impl VersionMatchesPredicate {
    pub fn exact(version: impl Into<String>) -> Self {
        Self {
            version_pattern: version.into(),
            match_type: VersionMatchType::Exact,
        }
    }

    pub fn starts_with(prefix: impl Into<String>) -> Self {
        Self {
            version_pattern: prefix.into(),
            match_type: VersionMatchType::StartsWith,
        }
    }

    pub fn contains(substring: impl Into<String>) -> Self {
        Self {
            version_pattern: substring.into(),
            match_type: VersionMatchType::Contains,
        }
    }
}

impl Evaluable for VersionMatchesPredicate {
    type Context = TomlContext;
    type Error = TomlError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.document.get_version() {
            Some(version) => {
                let matches = match &self.match_type {
                    VersionMatchType::Exact => version == self.version_pattern,
                    VersionMatchType::StartsWith => version.starts_with(&self.version_pattern),
                    VersionMatchType::Contains => version.contains(&self.version_pattern),
                    VersionMatchType::Regex(_) => {
                        // For now, just do contains matching
                        // In the future, we could add regex crate dependency
                        version.contains(&self.version_pattern)
                    }
                };
                Ok(matches)
            }
            None => Ok(false),
        }
    }
}

// Utility functions for extracting data (not predicates, but useful for the caller)
pub struct TomlExtractor;

impl TomlExtractor {
    pub fn extract_version(context: &TomlContext) -> Option<String> {
        context.document.get_version()
    }

    pub fn extract_dependencies(
        context: &TomlContext,
    ) -> Option<HashMap<String, serde_json::Value>> {
        context.document.get_dependencies()
    }

    pub fn extract_dev_dependencies(
        context: &TomlContext,
    ) -> Option<HashMap<String, serde_json::Value>> {
        context.document.get_dev_dependencies()
    }

    pub fn extract_build_dependencies(
        context: &TomlContext,
    ) -> Option<HashMap<String, serde_json::Value>> {
        context.document.get_build_dependencies()
    }

    pub fn extract_field_as_string(context: &TomlContext, field_name: &str) -> Option<String> {
        context.document.get_string(field_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::toml_ops::reader::TomlReader;

    fn create_test_context() -> TomlContext {
        let toml_content = r#"
            name = "test-project"
            version = "1.0.0"
            description = "A test project"

            [dependencies]
            serde = "1.0"
            tokio = { version = "1.0", features = ["full"] }

            [dev-dependencies]
            tempfile = "3.0"

            [build-dependencies]
            cc = "1.0"
        "#;

        let doc = TomlReader::parse_string(toml_content, "test.toml".to_string()).unwrap();
        TomlContext::new(doc)
    }

    #[test]
    fn test_has_field_predicate() {
        let context = create_test_context();

        let name_predicate = HasFieldPredicate::new("name");
        assert!(name_predicate.evaluate(&context).unwrap());

        let missing_predicate = HasFieldPredicate::new("nonexistent");
        assert!(!missing_predicate.evaluate(&context).unwrap());
    }

    #[test]
    fn test_has_table_predicate() {
        let context = create_test_context();

        let deps_predicate = HasTablePredicate::new("dependencies");
        assert!(deps_predicate.evaluate(&context).unwrap());

        let missing_predicate = HasTablePredicate::new("nonexistent-table");
        assert!(!missing_predicate.evaluate(&context).unwrap());
    }

    #[test]
    fn test_extract_version_predicate() {
        let context = create_test_context();

        let version_predicate = ExtractVersionPredicate::new();
        assert!(version_predicate.evaluate(&context).unwrap());

        let version = TomlExtractor::extract_version(&context).unwrap();
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_extract_dependencies_predicate() {
        let context = create_test_context();

        let deps_predicate = ExtractDependenciesPredicate::regular();
        assert!(deps_predicate.evaluate(&context).unwrap());

        let deps = TomlExtractor::extract_dependencies(&context).unwrap();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains_key("serde"));
        assert!(deps.contains_key("tokio"));
    }

    #[test]
    fn test_extract_dev_dependencies_predicate() {
        let context = create_test_context();

        let dev_deps_predicate = ExtractDevDependenciesPredicate::new();
        assert!(dev_deps_predicate.evaluate(&context).unwrap());

        let dev_deps = TomlExtractor::extract_dev_dependencies(&context).unwrap();
        assert_eq!(dev_deps.len(), 1);
        assert!(dev_deps.contains_key("tempfile"));
    }

    #[test]
    fn test_field_equals_predicate() {
        let context = create_test_context();

        let name_equals = FieldEqualsPredicate::new("name", "test-project");
        assert!(name_equals.evaluate(&context).unwrap());

        let name_wrong = FieldEqualsPredicate::new("name", "wrong-name");
        assert!(!name_wrong.evaluate(&context).unwrap());
    }

    #[test]
    fn test_version_matches_predicate() {
        let context = create_test_context();

        let exact_match = VersionMatchesPredicate::exact("1.0.0");
        assert!(exact_match.evaluate(&context).unwrap());

        let starts_with = VersionMatchesPredicate::starts_with("1.0");
        assert!(starts_with.evaluate(&context).unwrap());

        let contains = VersionMatchesPredicate::contains("0.0");
        assert!(contains.evaluate(&context).unwrap());

        let no_match = VersionMatchesPredicate::exact("2.0.0");
        assert!(!no_match.evaluate(&context).unwrap());
    }

    #[test]
    fn test_toml_predicate_enum() {
        let context = create_test_context();

        let has_version = TomlPredicate::HasField(HasFieldPredicate::new("version"));
        assert!(has_version.evaluate(&context).unwrap());

        let has_deps = TomlPredicate::HasTable(HasTablePredicate::new("dependencies"));
        assert!(has_deps.evaluate(&context).unwrap());
    }
}
