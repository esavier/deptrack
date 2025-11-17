// changelog verification module

pub mod config;
pub mod parser;
pub mod types;
pub mod validator;

pub use config::ChangelogConfig;
pub use parser::parse_changelog;
pub use types::{Changelog, ChangelogAnalysis, ChangelogEntry, ChangelogStatus, ChangelogVersion};
pub use validator::{has_version_entry, validate_changelog, version_has_content};
