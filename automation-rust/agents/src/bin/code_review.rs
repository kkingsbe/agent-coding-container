//! # Code Review Agent
//!
//! The code review agent performs codebase analysis:
//! - Identifies the 5 longest files by line count
//! - Finds code duplication patterns
//! - Identifies system design issues (SRP violations, tight coupling, missing abstractions)
//! - Generates a comprehensive markdown report
//!
//! ## Execution Modes
//!
//! The agent can run in two modes:
//! - **Manual mode (default)**: Runs once and exits (for backward compatibility)
//! - **Scheduled mode (--daemon)**: Runs periodically on a configured interval
//!
//! ## Configuration
//!
//! The agent reads configuration from `.automation-rust.toml`:
//!
//! ```toml
//! [agents.code_review]
//! interval_minutes = 30      # How often to run (default: 30 minutes)
//! timeout_minutes = 25       # Maximum execution time (default: 25 minutes)
//! immediate = false          # Run immediately on startup (default: false)
//! prompt_template = ""        # Not used (required for config compatibility)
//!
//! [agents.code_review.lock]
//! max_retries = 3            # Max retries for lock acquisition
//! timeout_seconds = 30        # Timeout for each lock attempt
//! ```
//!
//! ## Usage
//!
//! ### Manual Mode (single run)
//! ```bash
//! cargo run --bin code-review
//! # or
//! cargo run --bin code-review -- --analyze-dir ./my-project --output ./my-report.md
//! ```
//!
//! ### Daemon Mode (periodic execution)
//! ```bash
//! cargo run --bin code-review -- --daemon
//! ```
//!
//! ## Changes Made
//!
//! - Added `--daemon` flag to run as a scheduled long-running process
//! - Integrated with `automation-scheduler` for periodic execution
//! - Added configuration loading from TOML file for schedule settings
//! - Maintains backward compatibility with manual mode (no flags)
//! - Reports are still generated to `./reports/` directory with timestamps

use automation_common::{AutomationError, Result};
use automation_scheduler::{ScheduledTask, TaskConfig};
use clap::Parser;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use walkdir::WalkDir;

/// Code Review Agent - Codebase analysis and review
#[derive(Parser, Debug)]
#[command(name = "code-review")]
#[command(about = "Code review agent for automation-rust", long_about = None)]
struct Args {
    /// Path to the TOML configuration file
    /// Defaults to .automation-rust.toml in the current directory if not provided
    #[arg(long)]
    config: Option<String>,

    /// Directory to analyze (defaults to workspace from config)
    #[arg(long)]
    analyze_dir: Option<String>,

    /// Output report file (defaults to ./reports/code-review-<timestamp>.md)
    #[arg(long)]
    output: Option<String>,

    /// Exclude directories from analysis (comma-separated)
    #[arg(long, default_value = "target,.git")]
    exclude: String,

    /// Run as a daemon with periodic execution
    #[arg(long)]
    daemon: bool,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: String,
    line_count: usize,
    content: String,
}

#[derive(Debug, Clone)]
struct DuplicationFinding {
    pattern: String,
    files: Vec<(String, usize, usize)>, // (file_path, start_line, end_line)
    line_count: usize,
}

#[derive(Debug, Clone)]
struct DesignIssue {
    file_path: String,
    issue_type: String,
    description: String,
    line_number: Option<usize>,
    recommendation: String,
}

/// Configuration loaded from TOML file
#[derive(Debug, Clone)]
struct CodeReviewConfig {
    workspace_path: String,
    state_dir: String,
    interval_minutes: u64,
    timeout_minutes: u64,
    immediate: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config_path = args.config.clone().unwrap_or_else(|| ".automation-rust.toml".to_string());
    let config_path_buf = PathBuf::from(&config_path);
    let code_review_config = load_config(&config_path_buf)?;

    if args.daemon {
        // Run as a scheduled daemon
        run_daemon(args, code_review_config).await
    } else {
        // Run once (manual mode for backward compatibility)
        run_once(args, code_review_config)
    }
}

/// Load configuration from TOML file
fn load_config(config_path: &Path) -> Result<CodeReviewConfig> {
    // Check if config file exists
    if !config_path.exists() {
        return Err(AutomationError::Config(format!(
            "Config file not found: {}. Please create a .automation-rust.toml file or specify a custom path with --config.",
            config_path.display()
        )));
    }

    // Load the TOML configuration
    let toml_config = automation_common::workspace_config::parse_config_file(config_path)
        .map_err(|e| AutomationError::Config(format!("Failed to load config from {}: {}", config_path.display(), e)))?;

    // Extract workspace and state paths
    let workspace_path = PathBuf::from(&toml_config.workspace.path);
    let workspace = if workspace_path.is_absolute() {
        workspace_path.to_string_lossy().to_string()
    } else {
        let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        base_dir.join(&workspace_path)
            .canonicalize()
            .unwrap_or_else(|_| base_dir.join(&workspace_path))
            .to_string_lossy()
            .to_string()
    };

    let state_dir = if toml_config.workspace.state_path == ".state" {
        PathBuf::from(&workspace).join(".state").to_string_lossy().to_string()
    } else {
        let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        if PathBuf::from(&toml_config.workspace.state_path).is_absolute() {
            toml_config.workspace.state_path.clone()
        } else {
            base_dir.join(&toml_config.workspace.state_path)
                .to_string_lossy()
                .to_string()
        }
    };

    // Extract code review settings from TOML config
    let code_review_config = &toml_config.agents.code_review;
    let interval_minutes = code_review_config.interval_minutes;
    let timeout_minutes = code_review_config.timeout_minutes;
    let immediate = code_review_config.immediate;

    Ok(CodeReviewConfig {
        workspace_path: workspace,
        state_dir,
        interval_minutes,
        timeout_minutes,
        immediate,
    })
}

/// Run the code review agent as a scheduled daemon
async fn run_daemon(args: Args, config: CodeReviewConfig) -> Result<()> {
    let analyze_dir = determine_analyze_dir(&args, &config.workspace_path);
    let exclude_dirs: Vec<String> = args.exclude.split(',').map(|s| s.trim().to_string()).collect();

    println!("Code Review Agent (Daemon Mode)");
    println!("================================");
    println!("Analyzing directory: {}", analyze_dir.display());
    println!("Interval: {} minutes", config.interval_minutes);
    println!("Timeout: {} minutes", config.timeout_minutes);
    println!("Immediate: {}", config.immediate);
    println!("Excluded directories: {:?}", exclude_dirs);
    println!();

    // Create scheduled task
    let interval = Duration::from_secs(config.interval_minutes * 60);
    let shutdown_notify = Arc::new(Notify::new());

    let task_config = TaskConfig {
        task_name: "code-review".to_string(),
        interval,
        immediate: config.immediate,
        max_retries: 3,
        retry_delay_base: Duration::from_millis(100),
    };

    let mut task = ScheduledTask::new(
        task_config.task_name.clone(),
        task_config.interval,
        task_config.immediate,
        task_config.max_retries,
        task_config.retry_delay_base,
    );

    // Create handler that performs code review analysis
    let analyze_dir_clone = analyze_dir.clone();
    let exclude_dirs_clone = exclude_dirs.clone();
    let shutdown_notify_clone = Arc::clone(&shutdown_notify);

    let handler = move || {
        let analyze_dir = analyze_dir_clone.clone();
        let exclude_dirs = exclude_dirs_clone.clone();
        let shutdown_notify = Arc::clone(&shutdown_notify_clone);

        async move {
            tokio::select! {
                result = async {
                    run_code_review_analysis(&analyze_dir, &exclude_dirs, None).await
                        .map_err(|e| anyhow::anyhow!("Code review analysis failed: {}", e))
                } => result,
                _ = shutdown_notify.notified() => {
                    Err(anyhow::anyhow!("Shutdown signal received"))
                }
            }
        }
    };

    // Start the scheduled task
    task.start(handler).await.map_err(|e| AutomationError::ScheduleConfigInvalid {
        reason: format!("Failed to start scheduled task: {}", e),
    })?;

    println!("Code review daemon started. Press Ctrl+C to stop.");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await
        .map_err(|e| AutomationError::Process(format!("Failed to wait for Ctrl+C: {}", e)))?;

    println!("\nShutting down code review daemon...");

    // Stop the task
    task.stop().await.map_err(|e| AutomationError::ScheduleConfigInvalid {
        reason: format!("Failed to stop scheduled task: {}", e),
    })?;

    println!("Code review daemon stopped successfully.");
    Ok(())
}

/// Run the code review agent once (manual mode for backward compatibility)
fn run_once(args: Args, config: CodeReviewConfig) -> Result<()> {
    let analyze_dir = determine_analyze_dir(&args, &config.workspace_path);
    let exclude_dirs: Vec<String> = args.exclude.split(',').map(|s| s.trim().to_string()).collect();

    println!("Code Review Agent (Manual Mode)");
    println!("================================");
    println!("Analyzing directory: {}", analyze_dir.display());
    println!("Excluded directories: {:?}", exclude_dirs);
    println!();

    // Create a runtime for async execution
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| AutomationError::Process(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(run_code_review_analysis(&analyze_dir, &exclude_dirs, args.output.clone()))?;

    Ok(())
}

/// Determine the analyze directory from args or config
fn determine_analyze_dir(args: &Args, workspace: &str) -> PathBuf {
    if let Some(ref dir) = args.analyze_dir {
        PathBuf::from(dir)
    } else {
        PathBuf::from(workspace)
    }
}

/// Run the code review analysis
async fn run_code_review_analysis(
    analyze_dir: &Path,
    exclude_dirs: &[String],
    output_file: Option<String>,
) -> Result<()> {
    println!("Step 1: Scanning source files...");
    let files = scan_files(analyze_dir, exclude_dirs)?;
    println!("  Found {} files to analyze", files.len());

    // Step 2: Find the 5 longest files
    println!("\nStep 2: Finding longest files...");
    let mut sorted_files = files.clone();
    sorted_files.sort_by(|a, b| b.line_count.cmp(&a.line_count));
    let longest_files: Vec<_> = sorted_files.iter().take(5).cloned().collect();

    for (i, file) in longest_files.iter().enumerate() {
        println!("  {}. {} ({} lines)", i + 1, file.path, file.line_count);
    }

    // Step 3: Find code duplication
    println!("\nStep 3: Analyzing code duplication...");
    let duplications = find_duplications(&files);
    println!("  Found {} duplication patterns", duplications.len());

    // Step 4: Find design issues
    println!("\nStep 4: Identifying design issues...");
    let design_issues = find_design_issues(&files);
    println!("  Found {} design issues", design_issues.len());

    // Step 5: Generate report
    println!("\nStep 5: Generating report...");
    let output_dir = PathBuf::from("./reports");
    fs::create_dir_all(&output_dir)?;

    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M");
    let output_path: String = output_file.unwrap_or_else(|| {
        format!("./reports/code-review-{}.md", timestamp)
    });

    generate_report(&output_path, analyze_dir, &longest_files, &duplications, &design_issues)?;

    println!("\n✓ Code review complete!");
    println!("  Report saved to: {}", output_path);

    // Print summary
    println!("\n=== Summary ===");
    println!("Total files analyzed: {}", files.len());
    if !longest_files.is_empty() {
        println!("Longest file: {} ({} lines)", longest_files[0].path, longest_files[0].line_count);
    }
    println!("Duplication patterns found: {}", duplications.len());
    println!("Design issues found: {}", design_issues.len());
    println!("Analysis timestamp: {}", Utc::now().to_rfc3339());

    Ok(())
}

fn scan_files(root_dir: &Path, exclude_dirs: &[String]) -> Result<Vec<FileInfo>> {
    let mut files = Vec::new();

    // Supported source file extensions
    let supported_extensions: HashSet<&str> = [
        "rs", // Rust
        "toml", // Configuration
        "md", // Documentation
        "json", "yaml", "yml", // Config files
        "sh", "ps1", // Scripts
    ].iter().cloned().collect();

    let walker = WalkDir::new(root_dir)
        .follow_links(true)
        .into_iter();

    for entry in walker.filter_entry(|e| {
        let path = e.path();
        let file_name = e.file_name().to_string_lossy();

        // Skip excluded directories
        if path.is_dir() {
            if exclude_dirs.iter().any(|excluded| file_name.contains(excluded)) {
                return false;
            }
        }

        // Skip hidden files/directories (except .config files)
        if file_name.starts_with('.') && !matches!(file_name.as_ref(), ".automation-rust.toml" | ".env.example" | ".gitignore") {
            return false;
        }

        true
    }) {
        let entry = entry.map_err(|e| AutomationError::Process(format!("Failed to read directory entry: {}", e)))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if supported_extensions.contains(ext.to_str().unwrap_or("")) {
                    let content = fs::read_to_string(path)?;

                    let line_count = content.lines().count();
                    let relative_path = path.strip_prefix(root_dir)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();

                    files.push(FileInfo {
                        path: relative_path,
                        line_count,
                        content,
                    });
                }
            }
        }
    }

    Ok(files)
}

fn find_duplications(files: &[FileInfo]) -> Vec<DuplicationFinding> {
    let mut duplications = Vec::new();

    // Look for repeated patterns across files
    // We'll use a simple approach: find identical or near-identical code blocks

    let min_duplication_lines = 5;
    let mut pattern_map: HashMap<String, Vec<(String, usize, usize)>> = HashMap::new();

    for file in files {
        let lines: Vec<&str> = file.content.lines().collect();

        // Look for blocks of min_duplication_lines lines
        for start in 0..=(lines.len().saturating_sub(min_duplication_lines)) {
            let end = (start + min_duplication_lines).min(lines.len());
            let block: String = lines[start..end].join("\n");

            // Skip very short blocks and common patterns
            if block.chars().count() < 50 {
                continue;
            }

            // Skip common boilerplate patterns
            if is_common_pattern(&block) {
                continue;
            }

            pattern_map.entry(block.clone())
                .or_insert_with(Vec::new)
                .push((file.path.clone(), start + 1, end));
        }
    }

    // Find patterns that appear multiple times
    for (pattern, occurrences) in pattern_map {
        if occurrences.len() >= 2 {
            // Check if the files are different
            let unique_files: HashSet<_> = occurrences.iter().map(|(p, _, _)| p).collect();
            if unique_files.len() >= 2 {
                let line_count = pattern.lines().count();
                duplications.push(DuplicationFinding {
                    pattern,
                    files: occurrences,
                    line_count,
                });
            }
        }
    }

    // Sort by line count (descending) and occurrence count
    duplications.sort_by(|a, b| {
        b.line_count.cmp(&a.line_count)
            .then(b.files.len().cmp(&a.files.len()))
    });

    // Limit to top findings
    duplications.truncate(20);

    duplications
}

fn is_common_pattern(block: &str) -> bool {
    let common_patterns = [
        "use std::",
        "use chrono::",
        "use serde::",
        "#[derive(Debug",
        "#[cfg(test)]",
        "pub fn ",
        "pub async fn ",
        "pub struct ",
        "impl ",
        "async fn ",
        "#[instrument(",
        "/// # ",
    ];

    block.lines().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || common_patterns.iter().any(|p| trimmed.starts_with(p))
    })
}

fn find_design_issues(files: &[FileInfo]) -> Vec<DesignIssue> {
    let mut issues = Vec::new();

    for file in files {
        let lines: Vec<(usize, &str)> = file.content.lines().enumerate()
            .map(|(i, l)| (i + 1, l)).collect();

        for (line_num, line) in lines {
            let trimmed = line.trim();

            // SRP Violations: Functions that might be doing too much
            if trimmed.starts_with("pub async fn ") || trimmed.starts_with("pub fn ") {
                // Look for complex function signatures
                if line.matches(" & ").count() > 4 || line.matches(": ").count() > 5 {
                    issues.push(DesignIssue {
                        file_path: file.path.clone(),
                        issue_type: "Potential SRP Violation".to_string(),
                        description: "Function has many parameters which may indicate it's doing too much".to_string(),
                        line_number: Some(line_num),
                        recommendation: "Consider refactoring into smaller functions or using a parameter object/struct".to_string(),
                    });
                }
            }

            // Nested closures with move
            if trimmed.contains("move ||") && trimmed.len() > 100 {
                issues.push(DesignIssue {
                    file_path: file.path.clone(),
                    issue_type: "Complex Closure".to_string(),
                    description: "Complex closure with 'move' keyword may indicate tight coupling".to_string(),
                    line_number: Some(line_num),
                    recommendation: "Consider extracting to a separate named function or method".to_string(),
                });
            }

            // Multiple await in sequence without error handling
            if trimmed.contains(".await?") && line.chars().filter(|c| *c == '.').count() > 3 {
                issues.push(DesignIssue {
                    file_path: file.path.clone(),
                    issue_type: "Complex Async Chain".to_string(),
                    description: "Multiple async operations chained together may be hard to test and maintain".to_string(),
                    line_number: Some(line_num),
                    recommendation: "Consider breaking into smaller async functions with clear responsibilities".to_string(),
                });
            }

            // Large struct definitions
            if trimmed.starts_with("pub struct ") && trimmed.len() > 150 {
                issues.push(DesignIssue {
                    file_path: file.path.clone(),
                    issue_type: "Large Struct Definition".to_string(),
                    description: "Struct definition spans multiple lines and has many fields".to_string(),
                    line_number: Some(line_num),
                    recommendation: "Consider using composition or builder pattern if the struct has many related fields".to_string(),
                });
            }
        }

        // Check for long files (> 400 lines for this project size)
        if file.line_count > 400 {
            issues.push(DesignIssue {
                file_path: file.path.clone(),
                issue_type: "Long File".to_string(),
                description: format!("File has {} lines, which may indicate multiple responsibilities", file.line_count),
                line_number: None,
                recommendation: "Consider splitting into multiple modules based on functionality".to_string(),
            });
        }

        // Check for TODO/FIXME comments
        for (line_num, line) in file.content.lines().enumerate() {
            if line.contains("TODO:") || line.contains("FIXME:") {
                issues.push(DesignIssue {
                    file_path: file.path.clone(),
                    issue_type: "Unfinished Work".to_string(),
                    description: line.trim().to_string(),
                    line_number: Some(line_num + 1),
                    recommendation: "Address or update the TODO/FIXME comment".to_string(),
                });
            }
        }

        // Check for unwrap() usage (potential panics)
        if file.content.contains(".unwrap()") {
            let mut count = 0;
            for (line_num, line) in file.content.lines().enumerate() {
                if line.contains(".unwrap()") {
                    count += 1;
                    issues.push(DesignIssue {
                        file_path: file.path.clone(),
                        issue_type: "Potential Panic Risk".to_string(),
                        description: "Using .unwrap() which will panic if the Result/Option is None/Err".to_string(),
                        line_number: Some(line_num + 1),
                        recommendation: "Use proper error handling with ? operator or match statement".to_string(),
                    });
                    // Limit to 3 per file
                    if count >= 3 {
                        break;
                    }
                }
            }
        }

        // Check for unused or minimal comments on public functions
        if file.path.ends_with(".rs") {
            let lines: Vec<&str> = file.content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ") {
                    // Check if there's a doc comment before this
                    let has_doc_comment = if i > 0 {
                        let prev_lines: Vec<_> = lines[..i].iter().rev().take(5).collect();
                        prev_lines.iter().any(|l| l.trim().starts_with("///"))
                    } else {
                        false
                    };

                    if !has_doc_comment {
                        issues.push(DesignIssue {
                            file_path: file.path.clone(),
                            issue_type: "Missing Documentation".to_string(),
                            description: "Public function lacks documentation".to_string(),
                            line_number: Some(i + 1),
                            recommendation: "Add doc comments (///) explaining the function's purpose, parameters, and return value".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Limit issues per category
    issues.sort_by(|a, b| {
        // Prioritize by issue type severity
        let severity_order = |s: &str| -> usize {
            match s {
                "Potential Panic Risk" => 0,
                "Potential SRP Violation" => 1,
                "Long File" => 2,
                "Complex Closure" => 3,
                "Complex Async Chain" => 4,
                _ => 5,
            }
        };
        severity_order(&a.issue_type)
            .cmp(&severity_order(&b.issue_type))
    });

    issues.truncate(50);

    issues
}

fn generate_report(
    output_path: &str,
    analyze_dir: &Path,
    longest_files: &[FileInfo],
    duplications: &[DuplicationFinding],
    design_issues: &[DesignIssue],
) -> Result<()> {
    let timestamp = chrono::Local::now().to_rfc3339();

    let mut report = format!(
        "# Code Review Report\n\n\
        **Generated:** {}\n\n\
        **Analyzed Directory:** `{}`\n\n\
        ---\n\n",
        timestamp,
        analyze_dir.display()
    );

    // Executive Summary
    report.push_str("## Executive Summary\n\n");
    report.push_str("| Metric | Value |\n");
    report.push_str("|--------|-------|\n");
    report.push_str(&format!("| Longest File | {} lines ({}) |\n",
        longest_files.first().map(|f| f.line_count).unwrap_or(0),
        longest_files.first().map(|f| &f.path).unwrap_or(&"N/A".to_string())
    ));
    report.push_str(&format!("| Duplication Patterns | {} |\n", duplications.len()));
    report.push_str(&format!("| Design Issues | {} |\n", design_issues.len()));
    report.push_str("\n");

    // Section 1: Longest Files
    report.push_str("## 1. Longest Files\n\n");
    report.push_str("The following files have the highest line count and may benefit from refactoring:\n\n");
    report.push_str("| Rank | File | Lines |\n");
    report.push_str("|------|------|-------|\n");
    for (i, file) in longest_files.iter().enumerate() {
        report.push_str(&format!("| {} | [`{}`]({}) | {} |\n",
            i + 1,
            file.path,
            file.path.replace("\\", "/"), // Use forward slashes for links
            file.line_count
        ));
    }
    report.push_str("\n");

    // Section 2: Code Duplication
    report.push_str("## 2. Code Duplication\n\n");
    report.push_str(&format!("Found {} duplication patterns. Review the following areas for potential extraction:\n\n", duplications.len()));

    for (i, dup) in duplications.iter().enumerate() {
        report.push_str(&format!("### Pattern {}\n\n", i + 1));
        report.push_str(&format!("- **Line Count:** {} lines\n", dup.line_count));
        report.push_str(&format!("- **Occurrences:** {}\n", dup.files.len()));
        report.push_str("- **Files:**\n");
        for (file_path, start, end) in &dup.files {
            report.push_str(&format!("  - [`{}`]({}) (lines {}-{})\n",
                file_path,
                file_path.replace("\\", "/"),
                start, end
            ));
        }

        // Show a preview of the pattern
        let preview_lines: Vec<_> = dup.pattern.lines().take(3).collect();
        report.push_str("- **Preview:**\n");
        report.push_str("```rust\n");
        for line in &preview_lines {
            report.push_str(line);
            report.push_str("\n");
        }
        report.push_str("```\n\n");

        report.push_str("**Recommendation:** Extract this repeated pattern into a shared function or module.\n\n");
    }

    // Section 3: Design Issues
    report.push_str("## 3. Design Issues\n\n");

    // Group issues by type
    let mut issues_by_type: HashMap<&str, Vec<&DesignIssue>> = HashMap::new();
    for issue in design_issues {
        issues_by_type.entry(&issue.issue_type)
            .or_insert_with(Vec::new)
            .push(issue);
    }

    // Severity ordering
    let severity_order = [
        "Potential Panic Risk",
        "Potential SRP Violation",
        "Long File",
        "Complex Closure",
        "Complex Async Chain",
        "Large Struct Definition",
        "Missing Documentation",
        "Unfinished Work",
    ];

    for issue_type in &severity_order {
        if let Some(issues) = issues_by_type.get(issue_type) {
            report.push_str(&format!("### {} ({})\n\n", issue_type, issues.len()));

            for issue in issues {
                report.push_str(&format!("#### [{}]:[{}]\n\n", issue.file_path,
                    issue.line_number.map(|n| n.to_string()).unwrap_or_else(|| "multiple".to_string())
                ));
                report.push_str(&format!("- **Issue:** {}\n", issue.description));
                report.push_str(&format!("- **Recommendation:** {}\n", issue.recommendation));
                report.push_str("\n");
            }
        }
    }

    // Section 4: Recommendations
    report.push_str("## 4. Recommendations\n\n");

    let has_long_files = longest_files.first().map(|f| f.line_count > 300).unwrap_or(false);
    let has_duplication = !duplications.is_empty();
    let has_panic_risks = design_issues.iter().any(|i| i.issue_type == "Potential Panic Risk");
    let has_missing_docs = design_issues.iter().any(|i| i.issue_type == "Missing Documentation");

    if has_long_files {
        report.push_str("- **Long Files:** Consider breaking down files with >300 lines into smaller, focused modules. This improves maintainability and makes the codebase easier to navigate.\n");
    }

    if has_duplication {
        report.push_str("- **Code Duplication:** Extract repeated code blocks into shared utilities. This reduces maintenance burden and ensures consistency across the codebase.\n");
    }

    if has_panic_risks {
        report.push_str("- **Panic Risks:** Replace `.unwrap()` calls with proper error handling using the `?` operator or `match` statements. This makes the codebase more robust.\n");
    }

    if has_missing_docs {
        report.push_str("- **Documentation:** Add doc comments to public functions and structs. This improves API discoverability and helps other developers understand the intended usage.\n");
    }

    if has_long_files || has_duplication || has_panic_risks || has_missing_docs {
        report.push_str("\n");
    }

    report.push_str("---\n\n");
    report.push_str("*This report was generated by the Code Review Agent for the automation-rust project.*\n");

    fs::write(output_path, report)?;

    Ok(())
}
