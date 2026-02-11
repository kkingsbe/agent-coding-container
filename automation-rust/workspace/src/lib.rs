//! # automation-workspace
//!
//! Workspace management module for the automation-rust project.
//!
//! This module provides functionality for managing workspace files, including:
//! - Reading and writing markdown files (TODO.md, BACKLOG.md, COMPLETED.md, BLOCKERS.md, PRD.md)
//! - Atomic write operations using temp files
//! - Markdown section parsing
//! - Context extraction for template variables
//!
//! # Example
//!
//! ```no_run
//! use automation_workspace::{WorkspaceManager, TemplateEngine, TemplateContext};
//! use std::path::Path;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let manager = WorkspaceManager::new(Path::new("/workspace"));
//!
//!     // Read TODO.md
//!     let todo = manager.read_todo_file()?;
//!
//!     // Write to TODO.md atomically
//!     manager.write_todo_file("## TODO\n- Task 1")?;
//!
//!     // Use template engine
//!     let context = TemplateContext {
//!         todo: Some(todo),
//!         workspace: "/workspace".to_string(),
//!         ..Default::default()
//!     };
//!     let rendered = TemplateEngine::render("Working in {{workspace}}", &context);
//!
//!     Ok(())
//! }
//! ```

pub mod files;
pub mod markdown;
pub mod template;
pub mod manager;

// Re-export for convenience
pub use files::{
    read_file, write_file, write_file_bytes, write_file_with_backup,
    delete_file, ensure_dir, file_exists, get_file_size, get_file_mtime,
    list_files, list_files_recursive, ensure_empty_dir, validate_path
};

pub use markdown::{
    Section, parse_sections, extract_section, has_section, extract_tasks
};

pub use template::{
    TemplateEngine, TemplateContext
};

pub use manager::WorkspaceManager;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
