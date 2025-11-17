use super::changelog::ChangelogConfig;
use super::severity_config::SeverityConfig;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// main configuration for deptrack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptrackConfig {
    /// changelog-related configuration
    #[serde(default)]
    pub changelog: ChangelogConfig,

    /// severity configuration for direct dependencies
    #[serde(default = "SeverityConfig::default_direct")]
    pub direct_severity: SeverityConfig,

    /// severity configuration for transitive dependencies
    #[serde(default = "SeverityConfig::default_transitive")]
    pub transitive_severity: SeverityConfig,
}

impl Default for DeptrackConfig {
    fn default() -> Self {
        Self {
            changelog: ChangelogConfig::default(),
            direct_severity: SeverityConfig::default_direct(),
            transitive_severity: SeverityConfig::default_transitive(),
        }
    }
}

impl DeptrackConfig {
    /// load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents =
            std::fs::read_to_string(path).map_err(|e| crate::error::Error::FileReadError {
                path: path.to_path_buf(),
                source: e,
            })?;

        let config: DeptrackConfig =
            toml::from_str(&contents).map_err(|e| crate::error::Error::TomlParseError {
                path: path.to_path_buf(),
                source: e,
            })?;

        Ok(config)
    }

    /// find and load configuration file in repository
    ///
    /// looks for `deptrack.toml` in the repository root
    /// returns default config if file is not found
    pub fn load_or_default<P: AsRef<Path>>(repo_path: P) -> Self {
        match Self::find_config_file(&repo_path) {
            Some(config_path) => {
                // if config exists but can't be parsed, use default
                // (errors will be reported separately)
                Self::load_from_file(&config_path).unwrap_or_default()
            }
            None => Self::default(),
        }
    }

    /// find configuration file in repository
    ///
    /// looks for `deptrack.toml` in the repository root
    pub fn find_config_file<P: AsRef<Path>>(repo_path: P) -> Option<PathBuf> {
        let repo_path = repo_path.as_ref();
        let config_path = repo_path.join("deptrack.toml");

        if config_path.exists() && config_path.is_file() {
            Some(config_path)
        } else {
            None
        }
    }
}
