//! # File I/O Operations
//!
//! This module provides file I/O operations for workspace management, including
//! atomic writes, file existence checks, directory operations, and path validation.
//!
//! # Atomic Write Strategy
//!
//! All write operations use atomic file operations to prevent data corruption:
//! 1. Write content to a temporary file in the same directory
//! 2. Sync the temporary file to disk
//! 3. Rename the temporary file to the target path (atomic on most filesystems)
//!
//! This ensures that even if the process crashes or the system loses power,
//! the target file is either completely written or unchanged - never corrupted.
//!
//! # Path Validation
//!
//! The `validate_path` function ensures that all file operations are confined
//! within the workspace root, preventing directory traversal attacks.
//!
//! # Examples
//!
//! ```no_run
//! use automation_workspace::files::{read_file, write_file};
//! use std::path::Path;
//!
//! // Read a file
//! let content = read_file(Path::new("/workspace/todo.txt")).unwrap();
//!
//! // Write a file atomically
//! write_file(Path::new("/workspace/output.txt"), &content).unwrap();
//! ```
//!
//! ```no_run
//! use automation_workspace::files::{write_file_with_backup, validate_path};
//! use std::path::Path;
//!
//! // Write with backup
//! let workspace_root = Path::new("/workspace");
//! let file_path = Path::new("/workspace/data.txt");
//!
//! // Validate path is within workspace (security)
//! validate_path(file_path, workspace_root).unwrap();
//!
//! // Write and create backup
//! let backup_path = write_file_with_backup(file_path, "new content").unwrap();
//! println!("Backup created at: {:?}", backup_path);
//! ```

use automation_common::Result;
use chrono::{DateTime, Utc};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Reads a file and returns its content as a string.
///
/// # Arguments
///
/// * `path` - Path to the file to read
///
/// # Returns
///
/// The file content as a string
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The file cannot be read
/// - The content is not valid UTF-8
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::read_file;
/// use std::path::Path;
///
/// let content = read_file(Path::new("/workspace/todo.txt")).unwrap();
/// println!("File content: {}", content);
/// ```
pub fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|e| {
        automation_common::AutomationError::FileSystem(e)
    })
}

/// Writes content to a file atomically.
///
/// This function uses the temporary file + rename pattern to ensure atomic writes:
/// 1. Create a temporary file in the same directory
/// 2. Write content to the temporary file
/// 3. Sync the temporary file to disk
/// 4. Rename the temporary file to the target path
///
/// The parent directory is created if it doesn't exist.
///
/// # Arguments
///
/// * `path` - Target file path
/// * `content` - Content to write
///
/// # Errors
///
/// Returns an error if:
/// - The parent directory cannot be created
/// - The temporary file cannot be created
/// - The content cannot be written
/// - The file cannot be synced to disk
/// - The rename operation fails
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::write_file;
/// use std::path::Path;
///
/// write_file(Path::new("/workspace/output.txt"), "Hello, world!").unwrap();
/// ```
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    write_file_bytes(path, content.as_bytes())
}

/// Writes bytes to a file atomically.
///
/// This is the low-level implementation that all write operations use.
/// It uses the temporary file + rename pattern for atomicity.
///
/// # Arguments
///
/// * `path` - Target file path
/// * `content` - Bytes to write
///
/// # Errors
///
/// Returns an error if the write operation fails at any stage.
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::write_file_bytes;
/// use std::path::Path;
///
/// let data = b"\x48\x65\x6c\x6c\x6f"; // "Hello" in bytes
/// write_file_bytes(Path::new("/workspace/data.bin"), data).unwrap();
/// ```
pub fn write_file_bytes(path: &Path, content: &[u8]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    // Write atomically using temporary file + rename pattern
    atomic_write(path, content).map_err(automation_common::AutomationError::FileSystem)
}

/// Writes content to a file atomically and creates a backup of the existing file.
///
/// If the target file already exists, a backup is created before writing.
/// The backup file is named with a timestamp suffix.
///
/// # Arguments
///
/// * `path` - Target file path
/// * `content` - Content to write
///
/// # Returns
///
/// The path to the backup file (if one was created), otherwise the original path
///
/// # Errors
///
/// Returns an error if:
/// - The backup cannot be created
/// - The write operation fails
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::write_file_with_backup;
/// use std::path::Path;
///
/// let backup_path = write_file_with_backup(
///     Path::new("/workspace/data.txt"),
///     "new content"
/// ).unwrap();
/// println!("Backup created at: {:?}", backup_path);
/// ```
pub fn write_file_with_backup(path: &Path, content: &str) -> Result<PathBuf> {
    // Create backup if file exists
    let backup_path = if path.exists() {
        let backup = create_backup(path)?;
        Some(backup)
    } else {
        None
    };

    // Write new content atomically
    write_file_bytes(path, content.as_bytes())?;

    Ok(backup_path.unwrap_or_else(|| path.to_path_buf()))
}

/// Deletes a file if it exists.
///
/// This function does not return an error if the file does not exist.
///
/// # Arguments
///
/// * `path` - Path to the file to delete
///
/// # Errors
///
/// Returns an error if the file cannot be deleted (except if it doesn't exist)
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::delete_file;
/// use std::path::Path;
///
/// delete_file(Path::new("/workspace/temp.txt")).unwrap();
/// ```
pub fn delete_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).map_err(automation_common::AutomationError::FileSystem)?;
    }
    Ok(())
}

/// Ensures a directory exists, creating it if necessary.
///
/// This function creates all parent directories as needed.
///
/// # Arguments
///
/// * `path` - Path to the directory
///
/// # Errors
///
/// Returns an error if the directory cannot be created
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::ensure_dir;
/// use std::path::Path;
///
/// ensure_dir(Path::new("/workspace/nested/path")).unwrap();
/// ```
pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(automation_common::AutomationError::FileSystem)
}

/// Checks if a file exists.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// `true` if the file exists, `false` otherwise
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::file_exists;
/// use std::path::Path;
///
/// if file_exists(Path::new("/workspace/config.txt")) {
///     println!("Config file exists");
/// }
/// ```
pub fn file_exists(path: &Path) -> bool {
    path.exists()
}

/// Gets the size of a file in bytes.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// The file size in bytes
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The file size cannot be determined
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::get_file_size;
/// use std::path::Path;
///
/// let size = get_file_size(Path::new("/workspace/data.txt")).unwrap();
/// println!("File size: {} bytes", size);
/// ```
pub fn get_file_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path).map_err(automation_common::AutomationError::FileSystem)?;
    Ok(metadata.len())
}

/// Gets the modification time of a file.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// The file modification time as a UTC DateTime
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The modification time cannot be determined
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::get_file_mtime;
/// use std::path::Path;
///
/// let mtime = get_file_mtime(Path::new("/workspace/data.txt")).unwrap();
/// println!("Last modified: {}", mtime);
/// ```
pub fn get_file_mtime(path: &Path) -> Result<DateTime<Utc>> {
    let metadata = fs::metadata(path).map_err(automation_common::AutomationError::FileSystem)?;
    let modified = metadata.modified().map_err(automation_common::AutomationError::FileSystem)?;
    let datetime: DateTime<Utc> = modified.into();
    Ok(datetime)
}

/// Lists files in a directory, optionally filtering by a pattern.
///
/// Only files (not directories) are returned. The pattern uses glob-style
/// matching where `*` matches any sequence of characters.
///
/// # Arguments
///
/// * `dir` - Directory to list files from
/// * `pattern` - Optional pattern to filter files (e.g., "*.txt")
///
/// # Returns
///
/// A vector of file paths
///
/// # Errors
///
/// Returns an error if the directory cannot be read
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::list_files;
/// use std::path::Path;
///
/// // List all files
/// let all_files = list_files(Path::new("/workspace"), None).unwrap();
///
/// // List only markdown files
/// let md_files = list_files(Path::new("/workspace"), Some("*.md")).unwrap();
/// ```
pub fn list_files(dir: &Path, pattern: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let entries = fs::read_dir(dir).map_err(automation_common::AutomationError::FileSystem)?;

    for entry in entries {
        let entry = entry.map_err(automation_common::AutomationError::FileSystem)?;
        let path = entry.path();

        // Only include files, not directories
        if path.is_file() {
            // Apply pattern filter if provided
            if let Some(pattern) = pattern {
                if matches_pattern(&path, pattern) {
                    files.push(path);
                }
            } else {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// Lists files recursively in a directory, optionally filtering by a pattern.
///
/// Only files (not directories) are returned. The pattern uses glob-style
/// matching where `*` matches any sequence of characters.
///
/// # Arguments
///
/// * `dir` - Directory to list files from
/// * `pattern` - Optional pattern to filter files (e.g., "*.txt")
///
/// # Returns
///
/// A vector of file paths
///
/// # Errors
///
/// Returns an error if any directory cannot be read
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::list_files_recursive;
/// use std::path::Path;
///
/// // List all files recursively
/// let all_files = list_files_recursive(Path::new("/workspace"), None).unwrap();
///
/// // List only markdown files recursively
/// let md_files = list_files_recursive(Path::new("/workspace"), Some("*.md")).unwrap();
/// ```
pub fn list_files_recursive(dir: &Path, pattern: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let entries = fs::read_dir(dir).map_err(automation_common::AutomationError::FileSystem)?;

    for entry in entries {
        let entry = entry.map_err(automation_common::AutomationError::FileSystem)?;
        let path = entry.path();

        if path.is_file() {
            // Apply pattern filter if provided
            if let Some(pattern) = pattern {
                if matches_pattern(&path, pattern) {
                    files.push(path);
                }
            } else {
                files.push(path);
            }
        } else if path.is_dir() {
            // Recursively list subdirectories
            let sub_files = list_files_recursive(&path, pattern)?;
            files.extend(sub_files);
        }
    }

    Ok(files)
}

/// Ensures a directory exists and is empty.
///
/// If the directory doesn't exist, it is created.
/// If the directory exists and contains files, all files are removed.
/// Subdirectories are not removed.
///
/// # Arguments
///
/// * `dir` - Directory path
///
/// # Errors
///
/// Returns an error if:
/// - The directory cannot be created
/// - Files cannot be removed from the directory
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::ensure_empty_dir;
/// use std::path::Path;
///
/// ensure_empty_dir(Path::new("/workspace/cache")).unwrap();
/// ```
pub fn ensure_empty_dir(dir: &Path) -> Result<()> {
    // Create directory if it doesn't exist
    if !dir.exists() {
        ensure_dir(dir)?;
        return Ok(());
    }

    // Remove all files in the directory
    let entries = fs::read_dir(dir).map_err(automation_common::AutomationError::FileSystem)?;

    for entry in entries {
        let entry = entry.map_err(automation_common::AutomationError::FileSystem)?;
        let path = entry.path();

        // Only remove files, not subdirectories
        if path.is_file() {
            fs::remove_file(&path).map_err(automation_common::AutomationError::FileSystem)?;
        }
    }

    Ok(())
}

/// Validates that a path is within the workspace root.
///
/// This is a security measure to prevent directory traversal attacks.
/// Both paths are canonicalized before comparison.
///
/// # Arguments
///
/// * `path` - The path to validate
/// * `workspace_root` - The workspace root directory
///
/// # Errors
///
/// Returns an error if:
/// - The path is outside the workspace root
/// - Either path cannot be canonicalized
///
/// # Example
///
/// ```no_run
/// use automation_workspace::files::validate_path;
/// use std::path::Path;
///
/// let workspace_root = Path::new("/workspace");
/// let safe_path = Path::new("/workspace/data.txt");
/// validate_path(safe_path, workspace_root).unwrap(); // OK
///
/// let unsafe_path = Path::new("/etc/passwd");
/// validate_path(unsafe_path, workspace_root).unwrap_err(); // Error
/// ```
pub fn validate_path(path: &Path, workspace_root: &Path) -> Result<()> {
    // Canonicalize the workspace root first
    let canonical_root = fs::canonicalize(workspace_root).map_err(automation_common::AutomationError::FileSystem)?;

    // Try to canonicalize the path if it exists
    let canonical_path = if path.exists() {
        fs::canonicalize(path).map_err(automation_common::AutomationError::FileSystem)?
    } else {
        // For non-existent paths, canonicalize the parent and append the file name
        let parent = path.parent()
            .ok_or_else(|| automation_common::AutomationError::Config(format!("path '{}' has no parent", path.display())))?;

        let canonical_parent = if parent.as_os_str().is_empty() {
            // Current directory case
            canonical_root.clone()
        } else {
            fs::canonicalize(parent).map_err(automation_common::AutomationError::FileSystem)?
        };

        // Reconstruct the full path
        if let Some(file_name) = path.file_name() {
            canonical_parent.join(file_name)
        } else {
            canonical_parent
        }
    };

    // Check if the canonical path starts with the canonical workspace root
    if !canonical_path.starts_with(&canonical_root) {
        return Err(automation_common::AutomationError::Config(format!(
            "path '{}' is outside workspace root '{}'",
            path.display(),
            workspace_root.display()
        )));
    }

    Ok(())
}

// ==================== Helper Functions ====================

/// Writes content to a file atomically using the temporary file + rename pattern.
///
/// This is the low-level function that implements atomic writes.
/// It uses the tempfile crate for safe temporary file handling.
/// It ensures that either the write is complete or the file is unchanged.
///
/// # Arguments
///
/// * `path` - Target file path
/// * `content` - Content to write
///
/// # Errors
///
/// Returns an IO error if the atomic write fails at any stage
fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    // Create temporary file in the same directory as the target file
    // NamedTempFile handles cleanup on error automatically
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    
    let mut temp_file = NamedTempFile::new_in(parent)?;
    
    // Write content to temporary file
    temp_file.write_all(content)?;
    temp_file.flush()?;
    temp_file.as_file().sync_all()?; // Ensure data is written to disk
    
    // Persist the temporary file to the target path (atomic on most filesystems)
    temp_file.persist(path)?;
    
    Ok(())
}

/// Creates a backup of a file with a timestamp suffix.
///
/// The backup file is named: `{original_name}.backup.{timestamp}.bak`
///
/// # Arguments
///
/// * `path` - Path to the file to backup
///
/// # Returns
///
/// The path to the backup file
///
/// # Errors
///
/// Returns an error if the backup cannot be created
fn create_backup(path: &Path) -> Result<PathBuf> {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
    let backup_name = format!("{}.backup.{}.bak", filename, timestamp);
    let backup_path = path.with_file_name(backup_name);

    fs::copy(path, &backup_path).map_err(automation_common::AutomationError::FileSystem)?;

    Ok(backup_path)
}

/// Checks if a file path matches a glob-style pattern.
///
/// The pattern uses `*` to match any sequence of characters.
///
/// # Arguments
///
/// * `path` - Path to check
/// * `pattern` - Pattern to match against (e.g., "*.txt")
///
/// # Returns
///
/// `true` if the path matches the pattern, `false` otherwise
fn matches_pattern(path: &Path, pattern: &str) -> bool {
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        // Simple glob matching for '*' wildcard
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with('*') && pattern.ends_with('*') {
            // *something* - contains
            let middle = &pattern[1..pattern.len()-1];
            filename.contains(middle)
        } else if pattern.starts_with('*') {
            // *suffix - ends with
            let suffix = &pattern[1..];
            filename.ends_with(suffix)
        } else if pattern.ends_with('*') {
            // prefix* - starts with
            let prefix = &pattern[..pattern.len()-1];
            filename.starts_with(prefix)
        } else {
            // exact match
            filename == pattern
        }
    } else {
        false
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_read_and_write_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Write file
        write_file(&file_path, "Hello, world!").unwrap();
        assert!(file_exists(&file_path));

        // Read file
        let content = read_file(&file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[test]
    fn test_write_file_bytes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("data.bin");

        let data = b"\x48\x65\x6c\x6c\x6f\x20\x57\x6f\x72\x6c\x64";
        write_file_bytes(&file_path, data).unwrap();

        let content = fs::read(&file_path).unwrap();
        assert_eq!(content, data);
    }

    #[test]
    fn test_write_file_with_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("data.txt");

        // Write initial content
        write_file(&file_path, "initial content").unwrap();

        // Write with backup
        let backup_path = write_file_with_backup(&file_path, "new content").unwrap();

        // Verify backup exists
        assert!(backup_path.exists());

        // Verify new content
        let content = read_file(&file_path).unwrap();
        assert_eq!(content, "new content");

        // Verify backup contains old content
        let backup_content = read_file(&backup_path).unwrap();
        assert_eq!(backup_content, "initial content");
    }

    #[test]
    fn test_write_file_with_backup_new_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        // Write with backup (file doesn't exist)
        let result_path = write_file_with_backup(&file_path, "content").unwrap();

        // Should return the original path, not a backup
        assert_eq!(result_path, file_path);
        assert!(file_exists(&file_path));
    }

    #[test]
    fn test_delete_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("to_delete.txt");

        write_file(&file_path, "content").unwrap();
        assert!(file_exists(&file_path));

        delete_file(&file_path).unwrap();
        assert!(!file_exists(&file_path));
    }

    #[test]
    fn test_delete_nonexistent_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("does_not_exist.txt");

        // Should not error
        delete_file(&file_path).unwrap();
    }

    #[test]
    fn test_ensure_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path().join("nested").join("path");

        assert!(!dir_path.exists());

        ensure_dir(&dir_path).unwrap();
        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
    }

    #[test]
    fn test_file_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("exists.txt");

        assert!(!file_exists(&file_path));

        write_file(&file_path, "content").unwrap();
        assert!(file_exists(&file_path));
    }

    #[test]
    fn test_get_file_size() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("size.txt");

        write_file(&file_path, "Hello, world!").unwrap();
        let size = get_file_size(&file_path).unwrap();

        assert_eq!(size, 13); // "Hello, world!" is 13 bytes
    }

    #[test]
    fn test_get_file_mtime() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("mtime.txt");

        write_file(&file_path, "content").unwrap();
        let mtime = get_file_mtime(&file_path).unwrap();

        // Should be close to now
        let now = Utc::now();
        let diff = (now - mtime).num_seconds().abs();
        assert!(diff < 5, "Modification time should be recent");
    }

    #[test]
    fn test_list_files() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create some files
        write_file(&temp_dir.path().join("file1.txt"), "content1").unwrap();
        write_file(&temp_dir.path().join("file2.md"), "content2").unwrap();
        write_file(&temp_dir.path().join("file3.txt"), "content3").unwrap();

        // List all files
        let all_files = list_files(temp_dir.path(), None).unwrap();
        assert_eq!(all_files.len(), 3);

        // List only .txt files
        let txt_files = list_files(temp_dir.path(), Some("*.txt")).unwrap();
        assert_eq!(txt_files.len(), 2);

        // List only .md files
        let md_files = list_files(temp_dir.path(), Some("*.md")).unwrap();
        assert_eq!(md_files.len(), 1);
    }

    #[test]
    fn test_list_files_recursive() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create nested directory structure
        let subdir = temp_dir.path().join("subdir");
        ensure_dir(&subdir).unwrap();

        write_file(&temp_dir.path().join("file1.txt"), "content1").unwrap();
        write_file(&subdir.join("file2.txt"), "content2").unwrap();
        write_file(&subdir.join("file3.md"), "content3").unwrap();

        // List all files recursively
        let all_files = list_files_recursive(temp_dir.path(), None).unwrap();
        assert_eq!(all_files.len(), 3);

        // List only .txt files recursively
        let txt_files = list_files_recursive(temp_dir.path(), Some("*.txt")).unwrap();
        assert_eq!(txt_files.len(), 2);
    }

    #[test]
    fn test_ensure_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path().join("cache");

        // Create directory with files
        ensure_dir(&dir_path).unwrap();
        write_file(&dir_path.join("file1.txt"), "content1").unwrap();
        write_file(&dir_path.join("file2.txt"), "content2").unwrap();

        // Ensure empty directory (should remove files)
        ensure_empty_dir(&dir_path).unwrap();

        // Verify directory is empty
        let files = list_files(&dir_path, None).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_ensure_empty_dir_new() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path().join("new_cache");

        assert!(!dir_path.exists());

        // Ensure empty directory (should create it)
        ensure_empty_dir(&dir_path).unwrap();
        assert!(dir_path.exists());
    }

    #[test]
    fn test_validate_path_valid() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Valid path within workspace (create the file first)
        let file_path = temp_dir.path().join("file.txt");
        std::fs::File::create(&file_path).unwrap();
        validate_path(&file_path, temp_dir.path()).unwrap();

        // Valid nested path within workspace (create directory first)
        let nested_dir = temp_dir.path().join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let nested_path = nested_dir.join("file.txt");
        std::fs::File::create(&nested_path).unwrap();
        validate_path(&nested_path, temp_dir.path()).unwrap();
    }

    #[test]
    fn test_validate_path_invalid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let other_dir = tempfile::tempdir().unwrap();

        // Create the file outside workspace
        let outside_path = other_dir.path().join("file.txt");
        std::fs::File::create(&outside_path).unwrap();

        let result = validate_path(&outside_path, temp_dir.path());
        assert!(result.is_err());

        // Check error message
        if let Err(automation_common::AutomationError::Config(msg)) = result {
            assert!(msg.contains("outside workspace root"));
        } else {
            panic!("Expected Config error for invalid path, got: {:?}", result);
        }
    }

    #[test]
    fn test_matches_pattern() {
        let file_path = PathBuf::from("/test/file.txt");

        assert!(matches_pattern(&file_path, "*.txt"));
        assert!(matches_pattern(&file_path, "file.*"));
        assert!(matches_pattern(&file_path, "file.txt"));
        assert!(!matches_pattern(&file_path, "*.md"));

        let file_path2 = PathBuf::from("/test/data.md");
        assert!(matches_pattern(&file_path2, "*.md"));
        assert!(!matches_pattern(&file_path2, "*.txt"));
    }

    #[test]
    fn test_atomic_write_overwrite() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("overwrite.txt");

        // Write initial content
        write_file(&file_path, "initial").unwrap();

        // Overwrite with new content
        write_file(&file_path, "updated").unwrap();

        // Verify content was updated
        let content = read_file(&file_path).unwrap();
        assert_eq!(content, "updated");
    }

    #[test]
    fn test_write_creates_parent_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("new").join("nested").join("file.txt");

        // Parent directory doesn't exist
        assert!(!file_path.parent().unwrap().exists());

        // Write should create parent directory
        write_file(&file_path, "content").unwrap();

        // Verify file exists
        assert!(file_path.exists());
    }
}
