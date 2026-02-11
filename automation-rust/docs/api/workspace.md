# automation-workspace API Reference

The `automation-workspace` crate provides workspace directory management and markdown file operations for the gastown system.

## Table of Contents

- [Overview](#overview)
- [WorkspaceManager](#workspacemanager)
- [File Operations](#file-operations)
- [Markdown Parsing](#markdown-parsing)
- [Template Processing](#template-processing)
- [Usage Examples](#usage-examples)

## Overview

`automation-workspace` provides:

- **Workspace directory management** - Create, verify, and manage workspace directories
- **Markdown file operations** - Read and write markdown files atomically
- **Section extraction** - Extract specific sections (TODO, BACKLOG, etc.) from markdown files
- **Task parsing** - Parse tasks from TODO sections
- **Template processing** - Process template files with variable substitution

## WorkspaceManager

### `WorkspaceManager`

Manages a workspace directory.

```rust
pub struct WorkspaceManager {
    workspace_path: PathBuf,
}
```

### `WorkspaceManager::new()`

Create a new workspace manager.

```rust
impl WorkspaceManager {
    pub fn new(workspace_path: impl Into<PathBuf>) -> Self
}
```

**Parameters:**

- `workspace_path` - Path to the workspace directory

**Returns:**

- `WorkspaceManager` - New workspace manager instance

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
```

### `WorkspaceManager::path()`

Get the workspace path.

```rust
impl WorkspaceManager {
    pub fn path(&self) -> &Path
}
```

**Returns:**

- `&Path` - Reference to the workspace path

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
println!("Workspace: {}", workspace.path().display());
```

### `WorkspaceManager::ensure_exists()`

Ensure the workspace directory exists, creating it if necessary.

```rust
impl WorkspaceManager {
    pub async fn ensure_exists(&self) -> Result<(), AutomationError>
}
```

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
workspace.ensure_exists().await?;
```

### `WorkspaceManager::file_path()`

Get the full path to a file in the workspace.

```rust
impl WorkspaceManager {
    pub fn file_path(&self, file_name: impl AsRef<Path>) -> PathBuf
}
```

**Parameters:**

- `file_name` - Name of the file

**Returns:**

- `PathBuf` - Full path to the file

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
let todo_path = workspace.file_path("TODO.md");
// Returns: /workspace/TODO.md
```

## File Operations

### `read_file()`

Read a file from the workspace.

```rust
impl WorkspaceManager {
    pub async fn read_file(&self, file_name: impl AsRef<Path>) -> Result<String, AutomationError>
}
```

**Parameters:**

- `file_name` - Name of the file to read

**Returns:**

- `Result<String, AutomationError>` - File contents or error

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
let content = workspace.read_file("TODO.md").await?;
```

### `write_file()`

Write to a file atomically (write to temp file, then rename).

```rust
impl WorkspaceManager {
    pub async fn write_file(
        &self,
        file_name: impl AsRef<Path>,
        content: impl AsRef<str>
    ) -> Result<(), AutomationError>
}
```

**Parameters:**

- `file_name` - Name of the file to write
- `content` - Content to write

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
let content = "# TODO\n\n- [ ] Task 1\n- [ ] Task 2\n";
workspace.write_file("TODO.md", content).await?;
```

### `append_file()`

Append content to a file.

```rust
impl WorkspaceManager {
    pub async fn append_file(
        &self,
        file_name: impl AsRef<Path>,
        content: impl AsRef<str>
    ) -> Result<(), AutomationError>
}
```

**Parameters:**

- `file_name` - Name of the file to append to
- `content` - Content to append

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
workspace.append_file("TODO.md", "- [ ] New task\n").await?;
```

### `delete_file()`

Delete a file from the workspace.

```rust
impl WorkspaceManager {
    pub async fn delete_file(&self, file_name: impl AsRef<Path>) -> Result<(), AutomationError>
}
```

**Parameters:**

- `file_name` - Name of the file to delete

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
workspace.delete_file("old-file.md").await?;
```

### `file_exists()`

Check if a file exists in the workspace.

```rust
impl WorkspaceManager {
    pub async fn file_exists(&self, file_name: impl AsRef<Path>) -> bool
}
```

**Parameters:**

- `file_name` - Name of the file to check

**Returns:**

- `bool` - true if file exists, false otherwise

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
if workspace.file_exists("TODO.md").await {
    println!("TODO.md exists");
}
```

### `list_files()`

List files in the workspace matching a pattern.

```rust
impl WorkspaceManager {
    pub async fn list_files(&self, pattern: Option<&str>) -> Result<Vec<PathBuf>, AutomationError>
}
```

**Parameters:**

- `pattern` - Optional glob pattern to filter files

**Returns:**

- `Result<Vec<PathBuf>, AutomationError>` - List of file paths

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
let all_files = workspace.list_files(None).await?;
let md_files = workspace.list_files(Some("*.md")).await?;
```

## Markdown Parsing

### `extract_section()`

Extract a specific section from markdown content.

```rust
pub fn extract_section(content: &str, section_name: &str) -> Result<String, AutomationError>
```

**Parameters:**

- `content` - Markdown content
- `section_name` - Name of the section to extract (e.g., "TODO", "BACKLOG")

**Returns:**

- `Result<String, AutomationError>` - Section content or error

**Example:**

```rust
use automation_workspace::extract_section;

let content = "# TODO\n\n- [ ] Task 1\n- [ ] Task 2\n\n# BACKLOG\n\n- Future task";
let todo_section = extract_section(content, "TODO")?;
// Returns: "- [ ] Task 1\n- [ ] Task 2"
```

### `parse_tasks()`

Parse tasks from a TODO section.

```rust
pub fn parse_tasks(section: &str) -> Result<Vec<Task>, AutomationError>
```

**Returns:**

- `Result<Vec<Task>, AutomationError>` - List of parsed tasks

### `Task`

Represents a task parsed from markdown.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub completed: bool,
    pub text: String,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `completed` | `bool` | Whether the task is completed |
| `text` | `String` | Task description |

**Example:**

```rust
use automation_workspace::parse_tasks;

let todo_section = "- [ ] Task 1\n- [x] Task 2";
let tasks = parse_tasks(todo_section)?;

for task in tasks {
    if task.completed {
        println!("✓ {}", task.text);
    } else {
        println!("  {}", task.text);
    }
}
```

### `format_tasks()`

Format tasks as markdown.

```rust
pub fn format_tasks(tasks: &[Task]) -> String
```

**Parameters:**

- `tasks` - List of tasks to format

**Returns:**

- `String` - Formatted markdown

**Example:**

```rust
use automation_workspace::{Task, format_tasks};

let tasks = vec![
    Task { completed: false, text: "Task 1".to_string() },
    Task { completed: true, text: "Task 2".to_string() },
];

let formatted = format_tasks(&tasks);
// Returns: "- [ ] Task 1\n- [x] Task 2"
```

### `add_task()`

Add a task to a section.

```rust
pub fn add_task(content: &str, section: &str, task: &str) -> Result<String, AutomationError>
```

**Parameters:**

- `content` - Original markdown content
- `section` - Section to add task to
- `task` - Task text to add

**Returns:**

- `Result<String, AutomationError>` - Updated content

**Example:**

```rust
use automation_workspace::add_task;

let content = "# TODO\n\n- [ ] Task 1\n";
let updated = add_task(content, "TODO", "New task")?;
// Returns: "# TODO\n\n- [ ] Task 1\n- [ ] New task\n"
```

### `complete_task()`

Mark a task as completed.

```rust
pub fn complete_task(content: &str, section: &str, task_text: &str) -> Result<String, AutomationError>
```

**Parameters:**

- `content` - Original markdown content
- `section` - Section containing the task
- `task_text` - Task text to complete

**Returns:**

- `Result<String, AutomationError>` - Updated content

**Example:**

```rust
use automation_workspace::complete_task;

let content = "# TODO\n\n- [ ] Task 1\n- [ ] Task 2\n";
let updated = complete_task(content, "TODO", "Task 1")?;
// Returns: "# TODO\n\n- [x] Task 1\n- [ ] Task 2\n"
```

### `move_task()`

Move a task between sections.

```rust
pub fn move_task(
    content: &str,
    from_section: &str,
    to_section: &str,
    task_text: &str
) -> Result<String, AutomationError>
```

**Parameters:**

- `content` - Original markdown content
- `from_section` - Section to move task from
- `to_section` - Section to move task to
- `task_text` - Task text to move

**Returns:**

- `Result<String, AutomationError>` - Updated content

**Example:**

```rust
use automation_workspace::move_task;

let content = "# TODO\n\n- [ ] Task 1\n\n# COMPLETED\n\n";
let updated = move_task(content, "TODO", "COMPLETED", "Task 1")?;
// Task moved to COMPLETED section
```

## Template Processing

### `process_template()`

Process a template with variable substitution.

```rust
pub fn process_template(template: &str, variables: &HashMap<String, String>) -> String
```

**Parameters:**

- `template` - Template string with `{{variable}}` placeholders
- `variables` - Map of variable names to values

**Returns:**

- `String` - Processed template

**Example:**

```rust
use automation_workspace::process_template;
use std::collections::HashMap;

let template = "Hello, {{name}}! Today is {{date}}.";
let mut variables = HashMap::new();
variables.insert("name".to_string(), "Alice".to_string());
variables.insert("date".to_string(), "2024-02-09".to_string());

let result = process_template(template, &variables);
// Returns: "Hello, Alice! Today is 2024-02-09."
```

### `load_template()`

Load a template file from the workspace.

```rust
impl WorkspaceManager {
    pub async fn load_template(
        &self,
        template_name: impl AsRef<Path>
    ) -> Result<String, AutomationError>
}
```

**Parameters:**

- `template_name` - Name of the template file

**Returns:**

- `Result<String, AutomationError>` - Template content

**Example:**

```rust
use automation_workspace::WorkspaceManager;

let workspace = WorkspaceManager::new("/workspace");
let template = workspace.load_template("prompt-template.md").await?;
```

### `render_template()`

Load and process a template with variables.

```rust
impl WorkspaceManager {
    pub async fn render_template(
        &self,
        template_name: impl AsRef<Path>,
        variables: &HashMap<String, String>
    ) -> Result<String, AutomationError>
}
```

**Parameters:**

- `template_name` - Name of the template file
- `variables` - Variables to substitute

**Returns:**

- `Result<String, AutomationError>` - Rendered template

**Example:**

```rust
use automation_workspace::WorkspaceManager;
use std::collections::HashMap;

let workspace = WorkspaceManager::new("/workspace");
let mut variables = HashMap::new();
variables.insert("task".to_string(), "Build feature".to_string());

let rendered = workspace.render_template("prompt-template.md", &variables).await?;
```

## Usage Examples

### Basic Workspace Operations

```rust
use automation_workspace::WorkspaceManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace");
    
    // Ensure workspace exists
    workspace.ensure_exists().await?;
    
    // Write a file
    let content = "# TODO\n\n- [ ] First task\n";
    workspace.write_file("TODO.md", content).await?;
    
    // Read the file back
    let read_content = workspace.read_file("TODO.md").await?;
    println!("{}", read_content);
    
    Ok(())
}
```

### Working with Markdown Sections

```rust
use automation_workspace::{extract_section, add_task, complete_task};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace");
    
    // Read TODO.md
    let content = workspace.read_file("TODO.md").await?;
    
    // Extract TODO section
    let todo_section = extract_section(&content, "TODO")?;
    println!("TODO section:\n{}", todo_section);
    
    // Add a task
    let updated = add_task(&content, "TODO", "New important task")?;
    workspace.write_file("TODO.md", updated).await?;
    
    // Complete a task
    let content = workspace.read_file("TODO.md").await?;
    let updated = complete_task(&content, "TODO", "New important task")?;
    workspace.write_file("TODO.md", updated).await?;
    
    Ok(())
}
```

### Parsing and Formatting Tasks

```rust
use automation_workspace::{parse_tasks, Task, format_tasks};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let todo_section = "- [ ] Task 1\n- [x] Task 2\n- [ ] Task 3\n";
    
    // Parse tasks
    let tasks = parse_tasks(todo_section)?;
    
    // Process tasks
    let mut incomplete_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|t| !t.completed)
        .collect();
    
    println!("Incomplete tasks: {}", incomplete_tasks.len());
    
    // Add new task
    incomplete_tasks.push(Task {
        completed: false,
        text: "New task".to_string(),
    });
    
    // Format back to markdown
    let formatted = format_tasks(&incomplete_tasks);
    println!("{}", formatted);
    
    Ok(())
}
```

### Template Processing

```rust
use automation_workspace::{WorkspaceManager, process_template};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace");
    
    // Create a template
    let template = "Generate a plan for: {{task}}\n\nContext: {{context}}";
    workspace.write_file("prompt-template.md", template).await?;
    
    // Prepare variables
    let mut variables = HashMap::new();
    variables.insert("task".to_string(), "Build new feature".to_string());
    variables.insert("context".to_string(), "Project phase 2".to_string());
    
    // Render template
    let rendered = workspace.render_template("prompt-template.md", &variables).await?;
    println!("{}", rendered);
    
    Ok(())
}
```

### Moving Tasks Between Sections

```rust
use automation_workspace::{WorkspaceManager, move_task};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace");
    
    // Read workspace files
    let mut todo_content = workspace.read_file("TODO.md").await?;
    let mut completed_content = workspace.read_file("COMPLETED.md").await?;
    
    // Move completed task from TODO to COMPLETED
    let updated_todo = move_task(&todo_content, "TODO", "COMPLETED", "Task 1")?;
    let updated_completed = move_task(&completed_content, "TODO", "COMPLETED", "Task 1")?;
    
    // Write updated files
    workspace.write_file("TODO.md", updated_todo).await?;
    workspace.write_file("COMPLETED.md", updated_completed).await?;
    
    Ok(())
}
```

### Atomic Write Example

The `write_file()` method uses atomic writes to prevent data corruption:

```rust
use automation_workspace::WorkspaceManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace");
    
    // Write is atomic: writes to temp file, then renames
    // This prevents partial writes if the process crashes
    let content = "# Important Data\n\nDo not lose this!";
    workspace.write_file("important.md", content).await?;
    
    // Even if this crashes, important.md is never in a partial state
    // Either the old file exists, or the new file exists completely
    
    Ok(())
}
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AutomationError::Workspace("Not found")` | File doesn't exist | Create file first or use `ensure_exists()` |
| `AutomationError::Io` | Permission issues | Check file permissions |
| `AutomationError::Workspace("Invalid section")` | Section not found | Verify section name in markdown |
| `AutomationError::Workspace("Parse error")` | Invalid task format | Check markdown format |

### Error Handling Example

```rust
use automation_workspace::WorkspaceManager;
use automation_common::AutomationError;

async fn safe_read(workspace: &WorkspaceManager, file: &str) -> Option<String> {
    match workspace.read_file(file).await {
        Ok(content) => Some(content),
        Err(AutomationError::Workspace(msg)) if msg.contains("Not found") => {
            // File doesn't exist, that's OK
            None
        }
        Err(e) => {
            eprintln!("Error reading {}: {}", file, e);
            None
        }
    }
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [Common API](common.md) - Common types and utilities
- [State API](state.md) - State management module
- [Executor API](executor.md) - CLI executor module
