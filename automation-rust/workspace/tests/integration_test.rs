//! Integration tests for the workspace module.
//!
//! This module tests the integration of workspace components including:
//! - TemplateEngine rendering with variables
//! - Markdown section extraction (TODO, BACKLOG, COMPLETED, BLOCKERS, PRD)
//! - Parallel file reading with tokio::task::spawn_blocking

use automation_workspace::{
    TemplateEngine, TemplateContext,
    parse_sections, extract_section, has_section, extract_tasks,
    read_file, write_file, list_files, list_files_recursive,
    ensure_dir, file_exists, validate_path,
    WorkspaceManager,
    Section
};
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::task::spawn_blocking;

#[test]
fn test_template_engine_render_single_variable() {
    let template = "Hello, {{name}}!";
    let mut context = TemplateContext::new();
    context.workspace = String::new();
    
    let mut variables = HashMap::new();
    variables.insert("name".to_string(), "World".to_string());
    
    let rendered = TemplateEngine::render_custom(template, &variables);
    
    assert_eq!(rendered, "Hello, World!");
}

#[test]
fn test_template_engine_render_multiple_variables() {
    let template = "{{greeting}} {{name}}, working in {{workspace}}";
    let _context = TemplateContext {
        todo: None,
        backlog: None,
        completed: None,
        blockers: None,
        prd: None,
        workspace: "/workspace".to_string(),
        timestamp: None,
        agent_type: None,
    };

    let mut variables = HashMap::new();
    variables.insert("greeting".to_string(), "Hello".to_string());
    variables.insert("name".to_string(), "User".to_string());
    variables.insert("workspace".to_string(), "/workspace".to_string());

    let rendered = TemplateEngine::render_custom(template, &variables);

    assert_eq!(rendered, "Hello User, working in /workspace");
}

#[test]
fn test_template_engine_render_with_context() {
    let template = "TODO items:\n{{todo}}\nWorkspace: {{workspace}}";
    let context = TemplateContext {
        todo: Some("- Task 1\n- Task 2".to_string()),
        backlog: Some("- Backlog item".to_string()),
        completed: Some("- Done item".to_string()),
        blockers: Some("- Blocking issue".to_string()),
        prd: Some("PRD content".to_string()),
        workspace: "/workspace".to_string(),
        timestamp: Some("2024-01-01T00:00:00Z".to_string()),
        agent_type: Some("architect".to_string()),
    };
    
    let rendered = TemplateEngine::render(template, &context);
    
    assert!(rendered.contains("Task 1"));
    assert!(rendered.contains("Task 2"));
    assert!(rendered.contains("/workspace"));
}

#[test]
fn test_template_engine_render_with_timestamp() {
    let template = "Generated at: {{timestamp}}";
    let context = TemplateContext::with_workspace("/workspace")
        .with_timestamp();

    let rendered = TemplateEngine::render(template, &context);

    assert!(rendered.contains("Generated at:"));
    assert!(rendered.contains("T"), "Should contain ISO 8601 timestamp separator");
}

#[test]
fn test_template_engine_missing_variable() {
    let template = "Value: {{missing}}";
    let mut context = TemplateContext::new();
    context.workspace = String::new();

    let rendered = TemplateEngine::render(template, &context);

    // Missing variables should remain as-is
    assert_eq!(rendered, "Value: {{missing}}");
}

#[test]
fn test_template_engine_empty_template() {
    let template = "";
    let variables = HashMap::new();
    
    let rendered = TemplateEngine::render_custom(template, &variables);
    
    assert_eq!(rendered, "");
}

#[test]
fn test_template_engine_workspace_variable() {
    let template = "Workspace: {{workspace}}";
    let context = TemplateContext::with_workspace("/my/workspace/path");
    
    let rendered = TemplateEngine::render(template, &context);
    
    assert_eq!(rendered, "Workspace: /my/workspace/path");
}

#[test]
fn test_template_engine_agent_type_variable() {
    let template = "Agent type: {{agent_type}}";
    let context = TemplateContext {
        workspace: String::new(),
        agent_type: Some("janitor".to_string()),
        ..Default::default()
    };
    
    let rendered = TemplateEngine::render(template, &context);
    
    assert_eq!(rendered, "Agent type: janitor");
}

#[test]
fn test_markdown_parse_todo_section() {
    let content = r#"
## TODO
- Task 1
- Task 2
- Task 3
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    assert!(sections.contains_key(&Section::Todo));
    let todo_content = sections.get(&Section::Todo).unwrap();
    assert!(todo_content.contains("Task 1"));
    assert!(todo_content.contains("Task 2"));
    assert!(todo_content.contains("Task 3"));
}

#[test]
fn test_markdown_parse_all_sections() {
    let content = r#"
## TODO
- Active task

## BACKLOG
- Backlog item

## COMPLETED
- Completed task

## BLOCKERS
- Blocking issue

## PRD
Product requirements here
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    assert_eq!(sections.len(), 5, "Should parse all 5 sections");
    assert!(sections.contains_key(&Section::Todo));
    assert!(sections.contains_key(&Section::Backlog));
    assert!(sections.contains_key(&Section::Completed));
    assert!(sections.contains_key(&Section::Blockers));
    assert!(sections.contains_key(&Section::Prd));
}

#[test]
fn test_markdown_extract_specific_section() {
    let content = r#"
## TODO
- Task 1

## BACKLOG
- Backlog item
"#;

    let todo_section = extract_section(content, Section::Todo);
    
    assert!(todo_section.is_some(), "TODO section should exist");
    assert!(todo_section.unwrap().contains("Task 1"));
    
    let prd_section = extract_section(content, Section::Prd);
    assert!(prd_section.is_none(), "PRD section should not exist");
}

#[test]
fn test_markdown_has_section() {
    let content = r#"
## TODO
- Task 1

## BACKLOG
- Backlog item
"#;

    assert!(has_section(content, Section::Todo), "Should have TODO section");
    assert!(has_section(content, Section::Backlog), "Should have BACKLOG section");
    assert!(!has_section(content, Section::Completed), "Should not have COMPLETED section");
}

#[test]
fn test_markdown_extract_tasks() {
    let content = r#"
- Task 1
- Task 2
  - Nested task
- Task 3
* Bullet point task
"#;

    let tasks = extract_tasks(content);
    
    assert_eq!(tasks.len(), 4);
    assert_eq!(tasks[0], "Task 1");
    assert_eq!(tasks[1], "Task 2");
    assert_eq!(tasks[2], "Nested task");
    assert_eq!(tasks[3], "Task 3");
}

#[test]
fn test_markdown_empty_sections() {
    let content = "No sections here, just plain text";
    
    let sections = parse_sections(content).expect("Should parse sections");
    
    assert_eq!(sections.len(), 0, "Should have no sections");
}

#[test]
fn test_markdown_section_with_multiline_content() {
    let content = r#"
## TODO
- Task 1 with some description
  And more details
- Task 2

## BACKLOG
Item 1
Item 2
Item 3
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    let todo = sections.get(&Section::Todo).unwrap();
    assert!(todo.contains("Task 1"));
    assert!(todo.contains("And more details"));
    assert!(todo.contains("Task 2"));
    
    let backlog = sections.get(&Section::Backlog).unwrap();
    assert!(backlog.contains("Item 1"));
    assert!(backlog.contains("Item 2"));
    assert!(backlog.contains("Item 3"));
}

#[test]
fn test_markdown_section_header_variants() {
    // Test that only ## is recognized, not # or ###
    let content = r#"
# Wrong header
## TODO
- Correct task
### Wrong header 2
## BACKLOG
- Another task
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    assert_eq!(sections.len(), 2, "Should only parse ## sections");
    assert!(sections.contains_key(&Section::Todo));
    assert!(sections.contains_key(&Section::Backlog));
}

#[test]
fn test_markdown_empty_section_content() {
    let content = r#"
## TODO

## BACKLOG
- Some item
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    // Empty section should not be included
    assert!(!sections.contains_key(&Section::Todo), "Empty TODO should not be included");
    assert!(sections.contains_key(&Section::Backlog), "BACKLOG should be included");
}

#[test]
fn test_file_operations_read_write() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    
    // Write file
    write_file(&file_path, "Hello, world!")
        .expect("Should write file");
    
    assert!(file_exists(&file_path), "File should exist");
    
    // Read file
    let content = read_file(&file_path)
        .expect("Should read file");
    
    assert_eq!(content, "Hello, world!");
}

#[test]
fn test_file_operations_overwrite() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    
    // Write initial content
    write_file(&file_path, "Initial content")
        .expect("Should write file");
    
    // Overwrite
    write_file(&file_path, "New content")
        .expect("Should overwrite file");
    
    let content = read_file(&file_path)
        .expect("Should read file");
    
    assert_eq!(content, "New content");
}

#[test]
fn test_file_operations_nested_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nested_path = temp_dir.path().join("nested").join("deep").join("file.txt");
    
    // Write to nested path (should create directories)
    write_file(&nested_path, "Nested content")
        .expect("Should write to nested path");
    
    assert!(file_exists(&nested_path), "Nested file should exist");
    
    let content = read_file(&nested_path)
        .expect("Should read nested file");
    
    assert_eq!(content, "Nested content");
}

#[test]
fn test_file_operations_list_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create some files
    write_file(&temp_dir.path().join("file1.txt"), "content1")
        .expect("Should write file");
    write_file(&temp_dir.path().join("file2.md"), "content2")
        .expect("Should write file");
    write_file(&temp_dir.path().join("file3.txt"), "content3")
        .expect("Should write file");
    
    // List all files
    let all_files = list_files(temp_dir.path(), None)
        .expect("Should list files");
    
    assert_eq!(all_files.len(), 3);
    
    // List only .txt files
    let txt_files = list_files(temp_dir.path(), Some("*.txt"))
        .expect("Should list txt files");
    
    assert_eq!(txt_files.len(), 2);
}

#[test]
fn test_file_operations_list_files_recursive() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create nested directory structure
    let subdir = temp_dir.path().join("subdir");
    ensure_dir(&subdir).expect("Should create directory");
    
    write_file(&temp_dir.path().join("file1.txt"), "content1")
        .expect("Should write file");
    write_file(&subdir.join("file2.txt"), "content2")
        .expect("Should write file");
    write_file(&subdir.join("file3.md"), "content3")
        .expect("Should write file");
    
    // List all files recursively
    let all_files = list_files_recursive(temp_dir.path(), None)
        .expect("Should list files recursively");
    
    assert_eq!(all_files.len(), 3);
    
    // List only .txt files recursively
    let txt_files = list_files_recursive(temp_dir.path(), Some("*.txt"))
        .expect("Should list txt files recursively");
    
    assert_eq!(txt_files.len(), 2);
}

#[test]
fn test_file_operations_ensure_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nested_path = temp_dir.path().join("nested").join("deep").join("dir");
    
    assert!(!nested_path.exists(), "Directory should not exist yet");
    
    ensure_dir(&nested_path).expect("Should create directory");
    
    assert!(nested_path.exists(), "Directory should exist");
    assert!(nested_path.is_dir(), "Should be a directory");
}

#[test]
fn test_file_operations_validate_path_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let valid_path = temp_dir.path().join("nested").join("file.txt");
    
    // Create nested structure
    ensure_dir(valid_path.parent().unwrap()).expect("Should create directory");
    
    // Should validate successfully
    validate_path(&valid_path, temp_dir.path())
        .expect("Should validate path within workspace");
}

#[test]
fn test_file_operations_validate_path_invalid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let other_dir = TempDir::new().expect("Failed to create temp dir");
    
    let invalid_path = other_dir.path().join("outside.txt");
    
    // Should fail validation
    let result = validate_path(&invalid_path, temp_dir.path());
    
    assert!(result.is_err(), "Should reject path outside workspace");
}

#[tokio::test]
async fn test_parallel_file_reading() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create multiple files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    let file3 = temp_dir.path().join("file3.txt");

    write_file(&file1, "Content 1").expect("Should write file");
    write_file(&file2, "Content 2").expect("Should write file");
    write_file(&file3, "Content 3").expect("Should write file");

    // Read files in parallel using spawn_blocking
    let handle1 = spawn_blocking(move || read_file(&file1));
    let handle2 = spawn_blocking(move || read_file(&file2));
    let handle3 = spawn_blocking(move || read_file(&file3));

    let (content1, content2, content3) = tokio::try_join!(
        handle1,
        handle2,
        handle3
    ).expect("All reads should succeed");

    assert_eq!(content1.unwrap(), "Content 1");
    assert_eq!(content2.unwrap(), "Content 2");
    assert_eq!(content3.unwrap(), "Content 3");
}

#[test]
fn test_markdown_integration_with_file_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a markdown file with all sections
    let todo_file = temp_dir.path().join("TODO.md");
    let content = r#"
## TODO
- Implement feature X
- Write tests

## BACKLOG
- Research Y
- Document Z

## COMPLETED
- Initial setup
- Config management

## BLOCKERS
- Waiting for API key

## PRD
The system should be able to...
"#;
    
    write_file(&todo_file, content)
        .expect("Should write markdown file");
    
    // Read and parse
    let file_content = read_file(&todo_file)
        .expect("Should read file");
    
    let sections = parse_sections(&file_content)
        .expect("Should parse sections");
    
    assert_eq!(sections.len(), 5);
    
    // Extract tasks from TODO
    let todo_section = sections.get(&Section::Todo).unwrap();
    let todo_tasks = extract_tasks(todo_section);
    assert_eq!(todo_tasks.len(), 2);
    assert!(todo_tasks.iter().any(|t| t.contains("feature X")));
    assert!(todo_tasks.iter().any(|t| t.contains("tests")));
}

#[test]
fn test_template_engine_integration_with_markdown() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create markdown file
    let todo_file = temp_dir.path().join("TODO.md");
    let markdown_content = r#"
## TODO
- Build frontend
- Write API
- Deploy to staging
"#;
    write_file(&todo_file, markdown_content)
        .expect("Should write file");
    
    // Read and parse
    let file_content = read_file(&todo_file)
        .expect("Should read file");
    
    let sections = parse_sections(&file_content)
        .expect("Should parse sections");
    
    let todo_content = sections.get(&Section::Todo)
        .expect("Should have TODO section")
        .clone();
    
    // Create template
    let template = "Current TODO:\n{{todo}}\n\nWorking in: {{workspace}}";
    let context = TemplateContext {
        todo: Some(todo_content),
        workspace: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    
    let rendered = TemplateEngine::render(template, &context);
    
    assert!(rendered.contains("Build frontend"));
    assert!(rendered.contains("Write API"));
    assert!(rendered.contains("Deploy to staging"));
    assert!(rendered.contains("Working in:"));
}

#[test]
fn test_markdown_section_headers_case_sensitive() {
    let content = r#"
## TODO
- Lowercase

## todo
- Should not match (wrong case)

## BACKLOG
- Backlog item
"#;

    let sections = parse_sections(content).expect("Should parse sections");
    
    assert!(sections.contains_key(&Section::Todo), "Should match 'TODO'");
    assert!(sections.contains_key(&Section::Backlog), "Should match 'BACKLOG'");
    
    // Lowercase 'todo' should not create a Section::Todo
    let todo_content = sections.get(&Section::Todo).unwrap();
    assert!(todo_content.contains("Lowercase"));
    assert!(!todo_content.contains("Should not match"));
}

// ==================== WorkspaceManager Tests ====================

#[test]
fn test_workspace_manager_get_workspace_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    assert_eq!(manager.get_workspace_path(), temp_dir.path());
}

#[test]
fn test_workspace_manager_file_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    assert!(!manager.file_exists("TODO.md"));

    write_file(&temp_dir.path().join("TODO.md"), "content").expect("Should write file");
    assert!(manager.file_exists("TODO.md"));
}

#[test]
fn test_workspace_manager_ensure_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    let nested = temp_dir.path().join("nested").join("deep");
    assert!(!nested.exists());

    manager.ensure_directory("nested/deep").expect("Should create directory");
    assert!(nested.exists());
}

#[test]
fn test_workspace_manager_read_write_markdown_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    let content = "## Test Section\n- Item 1\n- Item 2";
    manager.write_markdown_file("test.md", content).expect("Should write file");

    let read = manager.read_markdown_file("test.md").expect("Should read file");
    assert_eq!(read, content);
}

#[test]
fn test_workspace_manager_read_write_all_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    // Write all standard files
    manager.write_todo_file("## TODO\n- Task 1").expect("Should write TODO.md");
    manager.write_backlog_file("## BACKLOG\n- Future task").expect("Should write BACKLOG.md");
    manager.write_completed_file("## COMPLETED\n- Done task").expect("Should write COMPLETED.md");
    manager.write_blockers_file("## BLOCKERS\n- Blocking issue").expect("Should write BLOCKERS.md");
    manager.write_prd_file("## PRD\nRequirements").expect("Should write PRD.md");

    // Read all files and verify
    assert_eq!(manager.read_todo_file().expect("Should read TODO.md"), "## TODO\n- Task 1");
    assert_eq!(manager.read_backlog_file().expect("Should read BACKLOG.md"), "## BACKLOG\n- Future task");
    assert_eq!(manager.read_completed_file().expect("Should read COMPLETED.md"), "## COMPLETED\n- Done task");
    assert_eq!(manager.read_blockers_file().expect("Should read BLOCKERS.md"), "## BLOCKERS\n- Blocking issue");
    assert_eq!(manager.read_prd_file().expect("Should read PRD.md"), "## PRD\nRequirements");
}

#[test]
fn test_workspace_manager_atomic_write_prevents_corruption() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());
    let file_path = temp_dir.path().join("TODO.md");

    // Write initial content
    manager.write_todo_file("## TODO\n- Initial task").expect("Should write initial content");
    let initial_content = read_file(&file_path).expect("Should read file");
    assert_eq!(initial_content, "## TODO\n- Initial task");

    // Overwrite with new content
    manager.write_todo_file("## TODO\n- Updated task").expect("Should overwrite");
    let updated_content = read_file(&file_path).expect("Should read file");
    assert_eq!(updated_content, "## TODO\n- Updated task");

    // Verify file is complete (atomic write should not leave partial content)
    assert!(updated_content.starts_with("## TODO"));
    assert!(updated_content.ends_with("- Updated task"));
}

#[test]
fn test_workspace_manager_absolute_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    let absolute_path = temp_dir.path().join("absolute.md");

    manager.write_markdown_file(&absolute_path, "Absolute content").expect("Should write to absolute path");
    assert!(manager.file_exists(&absolute_path));

    let content = manager.read_markdown_file(&absolute_path).expect("Should read from absolute path");
    assert_eq!(content, "Absolute content");
}

#[test]
fn test_workspace_manager_resolve_paths() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    // Relative path should resolve within workspace
    manager.write_markdown_file("relative.md", "Relative content").expect("Should write relative path");
    assert!(manager.file_exists("relative.md"));

    // Absolute path should work as-is
    let absolute_file = temp_dir.path().join("absolute.md");
    manager.write_markdown_file(&absolute_file, "Absolute content").expect("Should write absolute path");
    assert!(manager.file_exists(&absolute_file));
}

#[test]
fn test_workspace_manager_integration_with_markdown() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    // Write a markdown file with sections
    let content = r#"
## TODO
- Implement feature X
- Write tests

## BACKLOG
- Research Y
"#;
    manager.write_todo_file(content).expect("Should write TODO.md");

    // Read and parse
    let file_content = manager.read_todo_file().expect("Should read TODO.md");
    let sections = parse_sections(&file_content).expect("Should parse sections");

    assert_eq!(sections.len(), 2);
    assert!(sections.contains_key(&Section::Todo));
    assert!(sections.contains_key(&Section::Backlog));
}

#[test]
fn test_workspace_manager_integration_with_template() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = WorkspaceManager::new(temp_dir.path());

    // Create workspace files
    manager.write_todo_file("## TODO\n- Task 1").expect("Should write TODO.md");
    manager.write_backlog_file("## BACKLOG\n- Future task").expect("Should write BACKLOG.md");

    // Read and create template
    let todo_content = manager.read_todo_file().expect("Should read TODO.md");
    let backlog_content = manager.read_backlog_file().expect("Should read BACKLOG.md");

    let template = "TODO:\n{{todo}}\n\nBacklog:\n{{backlog}}";
    let context = TemplateContext {
        todo: Some(todo_content),
        backlog: Some(backlog_content),
        workspace: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };

    let rendered = TemplateEngine::render(template, &context);
    assert!(rendered.contains("Task 1"));
    assert!(rendered.contains("Future task"));
}

