use super::context::PredicateContext;
use crate::utils::alt::Evaluable;
use crate::utils::toml_ops::{TomlContext, TomlPredicate as TomlOp, TomlReader};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum FilePredicate {
    Extension(FileExtensionPredicate),
    DirectoryContains(DirectoryContainsPredicate),
    FilePath(FilePathPredicate),
    FileSize(FileSizePredicate),
    FileName(FileNamePredicate),
    TomlContent(TomlContentPredicate),
}

impl Evaluable for FilePredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match self {
            FilePredicate::Extension(pred) => pred.evaluate(context),
            FilePredicate::DirectoryContains(pred) => pred.evaluate(context),
            FilePredicate::FilePath(pred) => pred.evaluate(context),
            FilePredicate::FileSize(pred) => pred.evaluate(context),
            FilePredicate::FileName(pred) => pred.evaluate(context),
            FilePredicate::TomlContent(pred) => pred.evaluate(context),
        }
    }
}

impl From<FileExtensionPredicate> for FilePredicate {
    fn from(pred: FileExtensionPredicate) -> Self {
        FilePredicate::Extension(pred)
    }
}

impl From<DirectoryContainsPredicate> for FilePredicate {
    fn from(pred: DirectoryContainsPredicate) -> Self {
        FilePredicate::DirectoryContains(pred)
    }
}

impl From<FilePathPredicate> for FilePredicate {
    fn from(pred: FilePathPredicate) -> Self {
        FilePredicate::FilePath(pred)
    }
}

impl From<FileSizePredicate> for FilePredicate {
    fn from(pred: FileSizePredicate) -> Self {
        FilePredicate::FileSize(pred)
    }
}

impl From<FileNamePredicate> for FilePredicate {
    fn from(pred: FileNamePredicate) -> Self {
        FilePredicate::FileName(pred)
    }
}

impl From<TomlContentPredicate> for FilePredicate {
    fn from(pred: TomlContentPredicate) -> Self {
        FilePredicate::TomlContent(pred)
    }
}

#[derive(Debug, Clone)]
pub enum PredicateError {
    NoFileInContext,
    IoError(String),
}

impl fmt::Display for PredicateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PredicateError::NoFileInContext => write!(f, "No file in predicate context"),
            PredicateError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl Error for PredicateError {}

#[derive(Debug, Clone)]
pub struct FileExtensionPredicate {
    pub extension: String,
}

impl FileExtensionPredicate {
    pub fn new(extension: impl Into<String>) -> Self {
        Self {
            extension: extension.into(),
        }
    }
}

impl Evaluable for FileExtensionPredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.file_extension() {
            Some(ext) => Ok(ext == self.extension),
            None => Ok(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirectoryContainsPredicate {
    pub filename: String,
}

impl DirectoryContainsPredicate {
    pub fn new(filename: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
        }
    }
}

impl Evaluable for DirectoryContainsPredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        Ok(context.directory_contains_file(&self.filename))
    }
}

#[derive(Debug, Clone)]
pub struct FilePathPredicate {
    pub pattern: String,
    pub is_regex: bool,
}

impl FilePathPredicate {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: false,
        }
    }

    pub fn regex(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: true,
        }
    }
}

impl Evaluable for FilePathPredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.file_path() {
            Some(path) => {
                // for now, both regex and literal matching use contains
                // in the future, we can implement proper regex support
                Ok(path.contains(&self.pattern))
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileSizePredicate {
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
}

impl FileSizePredicate {
    pub fn new() -> Self {
        Self {
            min_size: None,
            max_size: None,
        }
    }

    pub fn min_size(mut self, size: u64) -> Self {
        self.min_size = Some(size);
        self
    }

    pub fn max_size(mut self, size: u64) -> Self {
        self.max_size = Some(size);
        self
    }

    pub fn range(min: u64, max: u64) -> Self {
        Self {
            min_size: Some(min),
            max_size: Some(max),
        }
    }
}

impl Default for FileSizePredicate {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluable for FileSizePredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match &context.current_file {
            Some(file) => {
                if let Some(size) = file.size {
                    let mut result = true;

                    if let Some(min) = self.min_size {
                        result = result && size >= min;
                    }

                    if let Some(max) = self.max_size {
                        result = result && size <= max;
                    }

                    Ok(result)
                } else {
                    Ok(false)
                }
            }
            None => Err(PredicateError::NoFileInContext),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileNamePredicate {
    pub name_pattern: String,
    pub exact_match: bool,
}

impl FileNamePredicate {
    pub fn exact(name: impl Into<String>) -> Self {
        Self {
            name_pattern: name.into(),
            exact_match: true,
        }
    }

    pub fn contains(pattern: impl Into<String>) -> Self {
        Self {
            name_pattern: pattern.into(),
            exact_match: false,
        }
    }
}

impl Evaluable for FileNamePredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        match context.file_name() {
            Some(name) => {
                if self.exact_match {
                    Ok(name == self.name_pattern)
                } else {
                    Ok(name.contains(&self.name_pattern))
                }
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TomlContentPredicate {
    pub toml_predicate: TomlOp,
}

impl TomlContentPredicate {
    pub fn new(toml_predicate: TomlOp) -> Self {
        Self { toml_predicate }
    }

    pub fn has_field(field_name: impl Into<String>) -> Self {
        use crate::utils::toml_ops::HasFieldPredicate;
        Self {
            toml_predicate: TomlOp::HasField(HasFieldPredicate::new(field_name)),
        }
    }

    pub fn has_version() -> Self {
        use crate::utils::toml_ops::ExtractVersionPredicate;
        Self {
            toml_predicate: TomlOp::ExtractVersion(ExtractVersionPredicate::new()),
        }
    }

    pub fn has_dependencies() -> Self {
        use crate::utils::toml_ops::HasTablePredicate;
        Self {
            toml_predicate: TomlOp::HasTable(HasTablePredicate::new("dependencies")),
        }
    }

    pub fn version_equals(version: impl Into<String>) -> Self {
        use crate::utils::toml_ops::VersionMatchesPredicate;
        Self {
            toml_predicate: TomlOp::VersionMatches(VersionMatchesPredicate::exact(version)),
        }
    }

    pub fn version_starts_with(prefix: impl Into<String>) -> Self {
        use crate::utils::toml_ops::VersionMatchesPredicate;
        Self {
            toml_predicate: TomlOp::VersionMatches(VersionMatchesPredicate::starts_with(prefix)),
        }
    }
}

impl Evaluable for TomlContentPredicate {
    type Context = PredicateContext;
    type Error = PredicateError;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error> {
        // Only apply to .toml files
        match context.file_path() {
            Some(path) => {
                if !path.ends_with(".toml") {
                    return Ok(false);
                }

                // Try to read and parse the TOML file
                match TomlReader::read_file(path) {
                    Ok(toml_doc) => {
                        let toml_context = TomlContext::new(toml_doc);

                        // Evaluate the TOML predicate
                        match self.toml_predicate.evaluate(&toml_context) {
                            Ok(result) => Ok(result),
                            Err(_) => Ok(false), // If TOML evaluation fails, return false
                        }
                    }
                    Err(_) => Ok(false), // If file can't be read/parsed, return false
                }
            }
            None => Ok(false),
        }
    }
}
