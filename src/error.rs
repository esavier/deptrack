use std::fmt;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    RepositoryNotFound {
        path: PathBuf,
    },
    FileReadError {
        path: PathBuf,
        source: std::io::Error,
    },
    TomlParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
    WorkspaceError {
        reason: String,
    },
    CyclicDependency {
        cycle: String,
    },
    GitError(Box<dyn std::error::Error + Send + Sync>),
    GitDiscoverError(Box<gix::discover::Error>),
    IoError(std::io::Error),
    RefNotFound {
        ref_name: String,
    },
    InvalidRef {
        ref_name: String,
    },
    DiffError {
        reason: String,
    },
    ChangelogError {
        reason: String,
    },
    ChangelogParseError {
        path: PathBuf,
        line: usize,
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::RepositoryNotFound { path } => {
                write!(f, "git repository not found in path: {}", path.display())
            }
            Error::FileReadError { path, source } => {
                write!(f, "failed to read file: {} ({})", path.display(), source)
            }
            Error::TomlParseError { path, source } => {
                write!(
                    f,
                    "failed to parse toml file: {} ({})",
                    path.display(),
                    source
                )
            }
            Error::WorkspaceError { reason } => {
                write!(f, "invalid workspace structure: {}", reason)
            }
            Error::CyclicDependency { cycle } => {
                write!(f, "cyclic dependency detected in crates: {}", cycle)
            }
            Error::GitError(err) => {
                write!(f, "git error: {}", err)
            }
            Error::GitDiscoverError(err) => {
                write!(f, "git discover error: {}", err)
            }
            Error::IoError(err) => {
                write!(f, "io error: {}", err)
            }
            Error::RefNotFound { ref_name } => {
                write!(f, "git reference not found: {}", ref_name)
            }
            Error::InvalidRef { ref_name } => {
                write!(f, "invalid git reference: {}", ref_name)
            }
            Error::DiffError { reason } => {
                write!(f, "diff error: {}", reason)
            }
            Error::ChangelogError { reason } => {
                write!(f, "changelog error: {}", reason)
            }
            Error::ChangelogParseError { path, line, reason } => {
                write!(
                    f,
                    "changelog parse error at {}:{}: {}",
                    path.display(),
                    line,
                    reason
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::FileReadError { source, .. } => Some(source),
            Error::TomlParseError { source, .. } => Some(source),
            Error::GitError(err) => Some(err.as_ref()),
            Error::GitDiscoverError(err) => Some(err.as_ref()),
            Error::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<gix::open::Error> for Error {
    fn from(err: gix::open::Error) -> Self {
        Error::GitError(Box::new(err))
    }
}

impl From<gix::discover::Error> for Error {
    fn from(err: gix::discover::Error) -> Self {
        Error::GitDiscoverError(Box::new(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

// Helper function to convert various git errors
impl Error {
    pub fn from_git_error<T: std::error::Error + Send + Sync + 'static>(err: T) -> Self {
        Error::GitError(Box::new(err))
    }
}
