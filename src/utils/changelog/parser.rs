// changelog parser

use super::types::{Changelog, ChangelogEntry, ChangelogVersion};
use crate::error::{Error, Result};
use crate::utils::severity::{Issue, IssueSeverity, IssueType};
use semver::Version;
use std::fs;
use std::path::Path;

fn extract_version_from_header(version_header: &str) -> Option<&str> {
    if let Some(bracketed) = version_header.strip_prefix('#') {
        let bracketed = bracketed.trim();

        if bracketed.starts_with('#') {
            return None; // subsection like "### Added"
        }

        if let Some(inside_brackets) = bracketed.strip_prefix('[') {
            return inside_brackets
                .split(']')
                .next()
                .map(|s| s.trim_start_matches('v'));
        }

        return Some(bracketed.trim_start_matches('v'));
    }

    Some(version_header.trim_start_matches('v'))
}

/// parse a changelog file
pub fn parse_changelog<P: AsRef<Path>>(path: P) -> Result<Changelog> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|e| Error::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut changelog = Changelog::new(path.to_path_buf());

    // parse line by line
    let lines: Vec<&str> = content.lines().collect();
    let mut current_version: Option<ChangelogVersion> = None;
    let mut line_number = 0;

    for line in lines {
        line_number += 1;
        let trimmed = line.trim();

        // skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // check for top-level header
        if trimmed == "# CHANGELOG" || trimmed == "#CHANGELOG" {
            changelog.has_header = true;
            continue;
        }

        if let Some(header) = trimmed.strip_prefix('#') {
            let version_str = match extract_version_from_header(header.trim()) {
                Some(s) => s,
                None => continue,
            };

            if let Some(prev) = current_version.take() {
                changelog.add_version(prev);
            }

            match Version::parse(version_str) {
                Ok(version) => {
                    current_version = Some(ChangelogVersion::new(version, line_number));
                }
                Err(_) => {
                    if !changelog.has_header && trimmed.to_uppercase().contains("CHANGELOG") {
                        changelog.has_header = true;
                    } else {
                        let msg = format!(
                            "line {}: could not parse version from '{}'",
                            line_number, trimmed
                        );
                        changelog.format_issues.push(Issue::new(
                            IssueSeverity::Error,
                            IssueType::BadFormat,
                            msg,
                        ));
                    }
                }
            }
            continue;
        }

        // check for changelog entry (* type(scope): description)
        if let Some(entry_text) = trimmed.strip_prefix('*') {
            if let Some(ref mut version_section) = current_version {
                match parse_entry(entry_text.trim(), line_number) {
                    Ok(entry) => {
                        version_section.add_entry(entry);
                    }
                    Err(e) => {
                        let message = format!("line {}: {}", line_number, e);
                        let issue = Issue::new(IssueSeverity::Error, IssueType::BadFormat, message);
                        changelog.format_issues.push(issue);
                    }
                }
            } else {
                let message = format!(
                    "line {}: entry found outside of version section",
                    line_number
                );
                let issue = Issue::new(IssueSeverity::Error, IssueType::BadFormat, message);
                changelog.format_issues.push(issue);
            }
            continue;
        }

        // check for entry with - instead of *
        if let Some(entry_text) = trimmed.strip_prefix('-') {
            if let Some(ref mut version_section) = current_version {
                match parse_entry(entry_text.trim(), line_number) {
                    Ok(entry) => {
                        version_section.add_entry(entry);
                    }
                    Err(e) => {
                        let message = format!(
                            "line {}: {} (note: prefer '*' over '-' for entries)",
                            line_number, e
                        );
                        let issue = Issue::new(IssueSeverity::Error, IssueType::BadFormat, message);
                        changelog.format_issues.push(issue);
                    }
                }
            }
            continue;
        }

        // unrecognized line format (might be continuation or description)
        // be lenient and skip
    }

    // save last version if exists
    if let Some(version_section) = current_version {
        changelog.add_version(version_section);
    }

    Ok(changelog)
}

fn parse_prefix_and_description(
    prefix: &str,
    description: String,
    line_number: usize,
) -> std::result::Result<ChangelogEntry, String> {
    let paren_pos = match prefix.find('(') {
        Some(pos) => pos,
        None => {
            let change_type = prefix.trim().to_string();
            if change_type.is_empty() {
                return Err("empty change type".to_string());
            }
            return Ok(ChangelogEntry::new(
                change_type,
                None,
                description,
                line_number,
            ));
        }
    };

    let change_type = prefix[..paren_pos].trim().to_string();
    let rest = &prefix[paren_pos + 1..];

    let close_paren = rest.find(')').ok_or("unclosed parenthesis in scope")?;
    let scope = rest[..close_paren].trim().to_string();

    if change_type.is_empty() {
        return Err("empty change type".to_string());
    }

    Ok(ChangelogEntry::new(
        change_type,
        Some(scope),
        description,
        line_number,
    ))
}

/// parse a single changelog entry
fn parse_entry(text: &str, line_number: usize) -> std::result::Result<ChangelogEntry, String> {
    // expected format: type(scope): description
    // or: type: description
    // or: simple description (for Keep a Changelog format)

    let colon_pos = match text.find(':') {
        Some(pos) => pos,
        None => {
            let description = text.trim();
            if description.is_empty() {
                return Err("empty entry".to_string());
            }
            return Ok(ChangelogEntry::new(
                "chore".to_string(),
                None,
                description.to_string(),
                line_number,
            ));
        }
    };

    let prefix = &text[..colon_pos];
    let description = text[colon_pos + 1..].trim().to_string();

    if description.is_empty() {
        return Err("empty description".to_string());
    }

    parse_prefix_and_description(prefix, description, line_number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entry_with_scope() {
        let entry = parse_entry("feat(api): add new endpoint", 1).unwrap();
        assert_eq!(entry.change_type, "feat");
        assert_eq!(entry.scope, Some("api".to_string()));
        assert_eq!(entry.description, "add new endpoint");
    }

    #[test]
    fn test_parse_entry_without_scope() {
        let entry = parse_entry("fix: resolve bug", 1).unwrap();
        assert_eq!(entry.change_type, "fix");
        assert_eq!(entry.scope, None);
        assert_eq!(entry.description, "resolve bug");
    }

    #[test]
    fn test_parse_entry_no_colon() {
        // entries without colon are now treated as simple descriptions (Keep a Changelog format)
        let result = parse_entry("simple entry", 1);
        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.change_type, "chore");
        assert_eq!(entry.description, "simple entry");
        assert_eq!(entry.scope, None);
    }

    #[test]
    fn test_parse_entry_empty_description() {
        let result = parse_entry("feat(api):", 1);
        assert!(result.is_err());
    }
}
