//! # Workspace Manager
//!
//! This module provides the WorkspaceManager struct for managing workspace file operations.
//! It handles reading and writing markdown files with atomic operations.
//!
//! # Example
//!
//! ```no_run
//! use automation_workspace::WorkspaceManager;
//! use std::path::Path;
//!
//! let manager = WorkspaceManager::new(Path::new("/workspace"));
//! let todo = manager.read_todo_file().unwrap();
//! manager.write_todo_file(&todo).unwrap();
//! ```

use automation_common::Result;
use std::path::{Path, PathBuf};

/// WorkspaceManager for managing workspace file operations.
///
/// This struct provides convenience methods for reading and writing markdown files
/// (TODO.md, BACKLOG.md, COMPLETED.md, BLOCKERS.md, PRD.md) with atomic operations.
#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    /// Path to the workspace directory
    workspace_path: PathBuf,
}

impl WorkspaceManager {
    /// Creates a new WorkspaceManager with the given workspace path.
    ///
    /// # Arguments
    ///
    /// * `workspace_path` - Path to the workspace directory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    /// use std::path::Path;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// ```
    pub fn new(workspace_path: &Path) -> Self {
        Self {
            workspace_path: workspace_path.to_path_buf(),
        }
    }

    /// Gets the workspace path.
    ///
    /// # Returns
    ///
    /// The workspace path as a Path reference
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    /// use std::path::Path;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let path = manager.get_workspace_path();
    /// assert_eq!(path, Path::new("/workspace"));
    /// ```
    pub fn get_workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    /// Checks if a file exists.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file (relative to workspace or absolute)
    ///
    /// # Returns
    ///
    /// `true` if the file exists, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// if manager.file_exists("TODO.md") {
    ///     println!("TODO.md exists");
    /// }
    /// ```
    pub fn file_exists(&self, file_path: impl AsRef<Path>) -> bool {
        let path = self.resolve_path(file_path);
        path.exists()
    }

    /// Ensures a directory exists, creating it if necessary.
    ///
    /// # Arguments
    ///
    /// * `dir_path` - Path to the directory (relative to workspace or absolute)
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.ensure_directory("nested/path").unwrap();
    /// ```
    pub fn ensure_directory(&self, dir_path: impl AsRef<Path>) -> Result<()> {
        let path = self.resolve_path(dir_path);
        super::files::ensure_dir(&path)
    }

    /// Reads a markdown file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the markdown file (relative to workspace or absolute)
    ///
    /// # Returns
    ///
    /// The content of the file as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let content = manager.read_markdown_file("TODO.md").unwrap();
    /// ```
    pub fn read_markdown_file(&self, file_path: impl AsRef<Path>) -> Result<String> {
        let path = self.resolve_path(file_path);
        super::files::read_file(&path)
    }

    /// Reads TODO.md from workspace.
    ///
    /// # Returns
    ///
    /// The content of TODO.md as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let todo = manager.read_todo_file().unwrap();
    /// ```
    pub fn read_todo_file(&self) -> Result<String> {
        self.read_markdown_file("TODO.md")
    }

    /// Reads BACKLOG.md from workspace.
    ///
    /// # Returns
    ///
    /// The content of BACKLOG.md as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let backlog = manager.read_backlog_file().unwrap();
    /// ```
    pub fn read_backlog_file(&self) -> Result<String> {
        self.read_markdown_file("BACKLOG.md")
    }

    /// Reads COMPLETED.md from workspace.
    ///
    /// # Returns
    ///
    /// The content of COMPLETED.md as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let completed = manager.read_completed_file().unwrap();
    /// ```
    pub fn read_completed_file(&self) -> Result<String> {
        self.read_markdown_file("COMPLETED.md")
    }

    /// Reads BLOCKERS.md from workspace.
    ///
    /// # Returns
    ///
    /// The content of BLOCKERS.md as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let blockers = manager.read_blockers_file().unwrap();
    /// ```
    pub fn read_blockers_file(&self) -> Result<String> {
        self.read_markdown_file("BLOCKERS.md")
    }

    /// Reads PRD.md from workspace.
    ///
    /// # Returns
    ///
    /// The content of PRD.md as a string
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// let prd = manager.read_prd_file().unwrap();
    /// ```
    pub fn read_prd_file(&self) -> Result<String> {
        self.read_markdown_file("PRD.md")
    }

    /// Writes content to a markdown file using atomic operation.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the markdown file (relative to workspace or absolute)
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_markdown_file("TODO.md", "## TODO\n- Task 1").unwrap();
    /// ```
    pub fn write_markdown_file(&self, file_path: impl AsRef<Path>, content: &str) -> Result<()> {
        let path = self.resolve_path(file_path);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            super::files::ensure_dir(parent)?;
        }

        super::files::write_file(&path, content)
    }

    /// Writes content to TODO.md.
    ///
    /// # Arguments
    ///
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_todo_file("## TODO\n- Task 1").unwrap();
    /// ```
    pub fn write_todo_file(&self, content: &str) -> Result<()> {
        self.write_markdown_file("TODO.md", content)
    }

    /// Writes content to BACKLOG.md.
    ///
    /// # Arguments
    ///
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_backlog_file("## BACKLOG\n- Future task").unwrap();
    /// ```
    pub fn write_backlog_file(&self, content: &str) -> Result<()> {
        self.write_markdown_file("BACKLOG.md", content)
    }

    /// Writes content to COMPLETED.md.
    ///
    /// # Arguments
    ///
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_completed_file("## COMPLETED\n- Done task").unwrap();
    /// ```
    pub fn write_completed_file(&self, content: &str) -> Result<()> {
        self.write_markdown_file("COMPLETED.md", content)
    }

    /// Writes content to BLOCKERS.md.
    ///
    /// # Arguments
    ///
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_blockers_file("## BLOCKERS\n- Blocking issue").unwrap();
    /// ```
    pub fn write_blockers_file(&self, content: &str) -> Result<()> {
        self.write_markdown_file("BLOCKERS.md", content)
    }

    /// Writes content to PRD.md.
    ///
    /// # Arguments
    ///
    /// * `content` - Content to write
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::WorkspaceManager;
    ///
    /// let manager = WorkspaceManager::new(Path::new("/workspace"));
    /// manager.write_prd_file("## PRD\nProduct requirements here").unwrap();
    /// ```
    pub fn write_prd_file(&self, content: &str) -> Result<()> {
        self.write_markdown_file("PRD.md", content)
    }

    /// Resolves a path to an absolute path within the workspace.
    ///
    /// If the path is absolute, it is used as-is.
    /// If the path is relative, it is joined with the workspace path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to resolve
    ///
    /// # Returns
    ///
    /// The resolved absolute path
    fn resolve_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path_ref = path.as_ref();
        if path_ref.is_absolute() {
            path_ref.to_path_buf()
        } else {
            self.workspace_path.join(path_ref)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        assert_eq!(manager.get_workspace_path(), temp_dir.path());
    }

    #[test]
    fn test_workspace_manager_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        assert!(!manager.file_exists("TODO.md"));

        super::super::files::write_file(&temp_dir.path().join("TODO.md"), "content").unwrap();
        assert!(manager.file_exists("TODO.md"));
    }

    #[test]
    fn test_workspace_manager_ensure_directory() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let nested = temp_dir.path().join("nested").join("deep");
        assert!(!nested.exists());

        manager.ensure_directory("nested/deep").unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn test_workspace_manager_read_write_todo() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let content = "## TODO\n- Task 1\n- Task 2";
        manager.write_todo_file(content).unwrap();

        let read = manager.read_todo_file().unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_workspace_manager_read_write_backlog() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let content = "## BACKLOG\n- Future task";
        manager.write_backlog_file(content).unwrap();

        let read = manager.read_backlog_file().unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_workspace_manager_read_write_completed() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let content = "## COMPLETED\n- Done task";
        manager.write_completed_file(content).unwrap();

        let read = manager.read_completed_file().unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_workspace_manager_read_write_blockers() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let content = "## BLOCKERS\n- Blocking issue";
        manager.write_blockers_file(content).unwrap();

        let read = manager.read_blockers_file().unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_workspace_manager_read_write_prd() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let content = "## PRD\nProduct requirements here";
        manager.write_prd_file(content).unwrap();

        let read = manager.read_prd_file().unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_workspace_manager_all_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        manager.write_todo_file("## TODO\n- Task 1").unwrap();
        manager.write_backlog_file("## BACKLOG\n- Future task").unwrap();
        manager.write_completed_file("## COMPLETED\n- Done task").unwrap();
        manager.write_blockers_file("## BLOCKERS\n- Blocking issue").unwrap();
        manager.write_prd_file("## PRD\nRequirements").unwrap();

        assert!(manager.file_exists("TODO.md"));
        assert!(manager.file_exists("BACKLOG.md"));
        assert!(manager.file_exists("COMPLETED.md"));
        assert!(manager.file_exists("BLOCKERS.md"));
        assert!(manager.file_exists("PRD.md"));
    }

    #[test]
    fn test_workspace_manager_resolve_path_relative() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let resolved = manager.resolve_path("subdir/file.txt");
        assert!(resolved.starts_with(temp_dir.path()));
        assert!(resolved.ends_with("subdir/file.txt"));
    }

    #[test]
    fn test_workspace_manager_resolve_path_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        // Absolute paths are returned as-is
        let absolute_path = temp_dir.path().join("absolute.md");
        let resolved = manager.resolve_path(&absolute_path);
        assert_eq!(resolved, absolute_path);
    }

    #[test]
    fn test_workspace_manager_overwrite_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        manager.write_todo_file("Initial content").unwrap();
        manager.write_todo_file("Updated content").unwrap();

        let read = manager.read_todo_file().unwrap();
        assert_eq!(read, "Updated content");
    }
}
