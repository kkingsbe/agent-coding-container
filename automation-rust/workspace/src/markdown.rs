//! # Markdown Parsing
//!
//! This module provides markdown parsing utilities for extracting sections
//! from markdown files used in the automation system.
//!
//! # Supported Sections
//!
//! The following sections are recognized in markdown files:
//! - `## TODO` - Active tasks
//! - `## BACKLOG` - Backlogged tasks
//! - `## COMPLETED` - Completed tasks
//! - `## BLOCKERS` - Blocking issues
//! - `## PRD` - Product Requirements Document
//!
//! # Example
//!
//! ```no_run
//! use automation_workspace::markdown::{parse_sections, Section};
//! use std::path::Path;
//!
//! let content = read_file(Path::new("/workspace/TODO.md")).unwrap();
//! let sections = parse_sections(&content).unwrap();
//!
//! if let Some(todo) = sections.get(&Section::Todo) {
//!     println!("TODO section: {}", todo);
//! }
//! ```

use automation_common::Result;
use std::collections::HashMap;

/// Represents a markdown section that can be extracted from files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// ## TODO section - active tasks
    Todo,

    /// ## BACKLOG section - backlogged tasks
    Backlog,

    /// ## COMPLETED section - completed tasks
    Completed,

    /// ## BLOCKERS section - blocking issues
    Blockers,

    /// ## PRD section - Product Requirements Document
    Prd,
}

impl Section {
    /// Returns the markdown header for this section.
    pub fn header(&self) -> &'static str {
        match self {
            Section::Todo => "## TODO",
            Section::Backlog => "## BACKLOG",
            Section::Completed => "## COMPLETED",
            Section::Blockers => "## BLOCKERS",
            Section::Prd => "## PRD",
        }
    }

    /// Returns all supported sections.
    pub fn all() -> &'static [Section] {
        &[
            Section::Todo,
            Section::Backlog,
            Section::Completed,
            Section::Blockers,
            Section::Prd,
        ]
    }
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.header())
    }
}

/// Parses markdown content and extracts all supported sections.
///
/// This function searches for section headers (## SECTION) and extracts
/// the content between each header. Content before the first header is ignored.
///
/// # Arguments
///
/// * `content` - The markdown content to parse
///
/// # Returns
///
/// A map of sections to their content. Sections that don't exist in the
/// content are not present in the map.
///
/// # Example
///
/// ```no_run
/// use automation_workspace::markdown::parse_sections;
///
/// let content = r#"
/// ## TODO
/// - Task 1
/// - Task 2
///
/// ## BACKLOG
/// - Future task
/// "#;
///
/// let sections = parse_sections(content).unwrap();
/// assert!(sections.contains_key(&Section::Todo));
/// ```
pub fn parse_sections(content: &str) -> Result<HashMap<Section, String>> {
    let mut sections: HashMap<Section, String> = HashMap::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_section: Option<Section> = None;
    let mut current_content: Vec<String> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();

        // Check if this is a section header
        if let Some(section) = parse_section_header(trimmed) {
            // Save previous section if any
            if let Some(prev_section) = current_section {
                let content = current_content.join("\n");
                if !content.trim().is_empty() {
                    sections.insert(prev_section, content);
                }
            }

            // Start new section
            current_section = Some(section);
            current_content.clear();
        } else if trimmed.starts_with("## ") {
            // Unknown section header - end current section without adding this line
            if let Some(prev_section) = current_section {
                let content = current_content.join("\n");
                if !content.trim().is_empty() {
                    sections.insert(prev_section, content);
                }
            }
            current_section = None;
            current_content.clear();
        } else if current_section.is_some() {
            // Add line to current section content
            current_content.push(line.to_string());
        }
    }

    // Save last section if any
    if let Some(section) = current_section {
        let content = current_content.join("\n");
        if !content.trim().is_empty() {
            sections.insert(section, content);
        }
    }

    Ok(sections)
}

/// Extracts a specific section from markdown content.
///
/// # Arguments
///
/// * `content` - The markdown content to parse
/// * `section` - The section to extract
///
/// # Returns
///
/// The content of the section, or `None` if the section doesn't exist.
///
/// # Example
///
/// ```no_run
/// use automation_workspace::markdown::{extract_section, Section};
///
/// let content = "## TODO\n- Task 1\n- Task 2";
/// let todo = extract_section(content, Section::Todo).unwrap();
/// assert!(todo.contains("Task 1"));
/// ```
pub fn extract_section(content: &str, section: Section) -> Option<String> {
    let sections = parse_sections(content).ok()?;
    sections.get(&section).cloned()
}

/// Checks if a specific section exists in markdown content.
///
/// # Arguments
///
/// * `content` - The markdown content to check
/// * `section` - The section to check for
///
/// # Returns
///
/// `true` if the section exists, `false` otherwise
///
/// # Example
///
/// ```no_run
/// use automation_workspace::markdown::{has_section, Section};
///
/// let content = "## TODO\n- Task 1";
/// assert!(has_section(content, Section::Todo));
/// assert!(!has_section(content, Section::Backlog));
/// ```
pub fn has_section(content: &str, section: Section) -> bool {
    extract_section(content, section).is_some()
}

/// Parses a section header line.
///
/// Returns `Some(Section)` if the line is a recognized section header,
/// otherwise returns `None`.
///
/// # Arguments
///
/// * `line` - The trimmed line to parse
///
/// # Returns
///
/// `Some(Section)` if the line matches a section header, `None` otherwise
fn parse_section_header(line: &str) -> Option<Section> {
    // Check for ## followed by section name
    if !line.starts_with("## ") {
        return None;
    }

    let section_name = &line[3..]; // Skip "## "

    match section_name {
        "TODO" => Some(Section::Todo),
        "BACKLOG" => Some(Section::Backlog),
        "COMPLETED" => Some(Section::Completed),
        "BLOCKERS" => Some(Section::Blockers),
        "PRD" => Some(Section::Prd),
        _ => None,
    }
}

/// Extracts all task items from a markdown section content.
///
/// This function parses list items (lines starting with `- ` or `* `) from
/// the provided content and returns them as a vector of strings.
///
/// # Arguments
///
/// * `content` - The markdown section content
///
/// # Returns
///
/// A vector of task items with leading markdown syntax removed
///
/// # Example
///
/// ```no_run
/// use automation_workspace::markdown::extract_tasks;
///
/// let content = "- Task 1\n- Task 2\n  - Nested task";
/// let tasks = extract_tasks(content);
/// assert_eq!(tasks.len(), 3);
/// ```
pub fn extract_tasks(content: &str) -> Vec<String> {
    let mut tasks = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for dash list item marker (-) - match both main and nested tasks
        if trimmed.starts_with("- ") {
            // Extract the task text, removing the marker
            let task = trimmed[2..].trim();

            if !task.is_empty() {
                tasks.push(task.to_string());
            }
        }
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sections_all() {
        let content = r#"
## TODO
- Task 1
- Task 2

## BACKLOG
- Future task

## COMPLETED
- Done task
"#;

        let sections = parse_sections(content).unwrap();

        assert_eq!(sections.len(), 3);
        assert!(sections.contains_key(&Section::Todo));
        assert!(sections.contains_key(&Section::Backlog));
        assert!(sections.contains_key(&Section::Completed));
    }

    #[test]
    fn test_parse_sections_empty() {
        let content = "No sections here";
        let sections = parse_sections(content).unwrap();
        assert_eq!(sections.len(), 0);
    }

    #[test]
    fn test_extract_section() {
        let content = r#"
## TODO
- Task 1
- Task 2
"#;

        let todo = extract_section(content, Section::Todo);
        assert!(todo.is_some());
        assert!(todo.unwrap().contains("Task 1"));

        let backlog = extract_section(content, Section::Backlog);
        assert!(backlog.is_none());
    }

    #[test]
    fn test_has_section() {
        let content = "## TODO\n- Task 1";
        assert!(has_section(content, Section::Todo));
        assert!(!has_section(content, Section::Backlog));
    }

    #[test]
    fn test_extract_tasks() {
        let content = "- Task 1\n- Task 2\n  - Nested task\n* Another task (asterisk)";
        let tasks = extract_tasks(content);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0], "Task 1");
        assert_eq!(tasks[1], "Task 2");
        assert_eq!(tasks[2], "Nested task");
    }

    #[test]
    fn test_section_header_display() {
        assert_eq!(Section::Todo.to_string(), "## TODO");
        assert_eq!(Section::Backlog.to_string(), "## BACKLOG");
        assert_eq!(Section::Completed.to_string(), "## COMPLETED");
        assert_eq!(Section::Blockers.to_string(), "## BLOCKERS");
        assert_eq!(Section::Prd.to_string(), "## PRD");
    }

    #[test]
    fn test_section_header_parsing() {
        assert_eq!(parse_section_header("## TODO"), Some(Section::Todo));
        assert_eq!(parse_section_header("## BACKLOG"), Some(Section::Backlog));
        assert_eq!(parse_section_header("## COMPLETED"), Some(Section::Completed));
        assert_eq!(parse_section_header("## BLOCKERS"), Some(Section::Blockers));
        assert_eq!(parse_section_header("## PRD"), Some(Section::Prd));
        assert_eq!(parse_section_header("## UNKNOWN"), None);
        assert_eq!(parse_section_header("# TODO"), None);
        assert_eq!(parse_section_header("### TODO"), None);
    }
}
