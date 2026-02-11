//! # Template Engine
//!
//! This module provides a simple template engine for variable substitution
//! in prompt templates used by the automation system.
//!
//! # Supported Variables
//!
//! The template engine supports the following variables:
//! - `{{todo}}` - TODO section content
//! - `{{backlog}}` - BACKLOG section content
//! - `{{completed}}` - COMPLETED section content
//! - `{{blockers}}` - BLOCKERS section content
//! - `{{prd}}` - PRD section content
//! - `{{workspace}}` - Workspace directory path
//! - `{{timestamp}}` - Current timestamp in ISO 8601 format
//! - `{{agent_type}}` - Type of agent (architect, janitor, prompt)
//!
//! # Example
//!
//! ```no_run
//! use automation_workspace::template::{TemplateEngine, TemplateContext};
//!
//! let template = "TODO items:\n{{todo}}\n\nWorking in: {{workspace}}";
//! let context = TemplateContext {
//!     todo: Some("- Task 1\n- Task 2".to_string()),
//!     workspace: "/workspace".to_string(),
//!     ..Default::default()
//! };
//!
//! let rendered = TemplateEngine::render(&template, &context);
//! assert!(rendered.contains("Task 1"));
//! ```

use chrono::Utc;
use std::collections::HashMap;

/// Context data for template rendering.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    /// TODO section content
    pub todo: Option<String>,

    /// BACKLOG section content
    pub backlog: Option<String>,

    /// COMPLETED section content
    pub completed: Option<String>,

    /// BLOCKERS section content
    pub blockers: Option<String>,

    /// PRD section content
    pub prd: Option<String>,

    /// Workspace directory path
    pub workspace: String,

    /// Current timestamp
    pub timestamp: Option<String>,

    /// Type of agent running the template
    pub agent_type: Option<String>,
}

impl TemplateContext {
    /// Creates a new empty template context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a template context with default workspace.
    pub fn with_workspace(workspace: impl Into<String>) -> Self {
        Self {
            workspace: workspace.into(),
            ..Default::default()
        }
    }

    /// Adds timestamp to context if not present.
    pub fn with_timestamp(mut self) -> Self {
        if self.timestamp.is_none() {
            self.timestamp = Some(Utc::now().to_rfc3339());
        }
        self
    }
}

/// Simple template engine for variable substitution.
///
/// Uses `{{variable}}` syntax for placeholders and replaces them
/// with values from the provided context.
///
/// # Example
///
/// ```no_run
/// use automation_workspace::template::TemplateEngine;
///
/// let template = "Hello {{name}}, working in {{workspace}}";
/// let mut context = HashMap::new();
/// context.insert("name".to_string(), "World".to_string());
/// context.insert("workspace".to_string(), "/app".to_string());
///
/// let rendered = TemplateEngine::render_custom(&template, &context);
/// assert_eq!(rendered, "Hello World, working in /app");
/// ```
pub struct TemplateEngine;

impl TemplateEngine {
    /// Renders a template with the given context.
    ///
    /// This method uses direct string substitution for known template variables,
    /// avoiding the overhead of HashMap allocation per render call.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string to render
    /// * `context` - The template context containing variable values
    ///
    /// # Returns
    ///
    /// The rendered template with all placeholders replaced by their values
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::template::{TemplateEngine, TemplateContext};
    ///
    /// let template = "TODO: {{todo}}\nWorkspace: {{workspace}}";
    /// let context = TemplateContext {
    ///     todo: Some("- Task 1".to_string()),
    ///     workspace: "/app".to_string(),
    ///     ..Default::default()
    /// };
    ///
    /// let rendered = TemplateEngine::render(&template, &context);
    /// assert!(rendered.contains("Task 1"));
    /// ```
    pub fn render(template: &str, context: &TemplateContext) -> String {
        let mut result = template.to_string();

        // Direct substitution for each known field (no intermediate HashMap)
        // This avoids allocations per render call compared to HashMap-based approach
        if let Some(todo) = &context.todo {
            result = result.replace("{{todo}}", todo);
        }
        if let Some(backlog) = &context.backlog {
            result = result.replace("{{backlog}}", backlog);
        }
        if let Some(completed) = &context.completed {
            result = result.replace("{{completed}}", completed);
        }
        if let Some(blockers) = &context.blockers {
            result = result.replace("{{blockers}}", blockers);
        }
        if let Some(prd) = &context.prd {
            result = result.replace("{{prd}}", prd);
        }
        result = result.replace("{{workspace}}", &context.workspace);
        if let Some(timestamp) = &context.timestamp {
            result = result.replace("{{timestamp}}", timestamp);
        }
        if let Some(agent_type) = &context.agent_type {
            result = result.replace("{{agent_type}}", agent_type);
        }

        result
    }

    /// Renders a template with custom variable mappings.
    ///
    /// This allows for more flexible variable substitution beyond the
    /// predefined TemplateContext fields.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string to render
    /// * `variables` - A map of variable names to their values
    ///
    /// # Returns
    ///
    /// The rendered template with all placeholders replaced
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_workspace::template::TemplateEngine;
    /// use std::collections::HashMap;
    ///
    /// let template = "Hello {{name}}!";
    /// let mut variables = HashMap::new();
    /// variables.insert("name".to_string(), "World".to_string());
    ///
    /// let rendered = TemplateEngine::render_custom(&template, &variables);
    /// assert_eq!(rendered, "Hello World!");
    /// ```
    pub fn render_custom(template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        // Replace each variable in the template
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_context_default() {
        let context = TemplateContext::default();
        assert_eq!(context.todo, None);
        assert_eq!(context.workspace, String::new());
    }

    #[test]
    fn test_template_context_with_workspace() {
        let context = TemplateContext::with_workspace("/app");
        assert_eq!(context.workspace, "/app");
    }

    #[test]
    fn test_template_context_with_timestamp() {
        let context = TemplateContext::default().with_timestamp();
        assert!(context.timestamp.is_some());
    }

    #[test]
    fn test_render_single_variable() {
        let template = "Hello {{name}}!";
        let mut context = TemplateContext::new();
        context.workspace = String::new();

        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "World".to_string());

        let rendered = TemplateEngine::render_custom(template, &variables);
        assert_eq!(rendered, "Hello World!");
    }

    #[test]
    fn test_render_multiple_variables() {
        let template = "{{greeting}} {{name}}, working in {{workspace}}";
        let mut variables = HashMap::new();
        variables.insert("greeting".to_string(), "Hello".to_string());
        variables.insert("name".to_string(), "World".to_string());
        variables.insert("workspace".to_string(), "/app".to_string());

        let rendered = TemplateEngine::render_custom(template, &variables);
        assert_eq!(rendered, "Hello World, working in /app");
    }

    #[test]
    fn test_render_template_context() {
        let template = "TODO:\n{{todo}}\nWorkspace: {{workspace}}";
        let context = TemplateContext {
            todo: Some("- Task 1\n- Task 2".to_string()),
            workspace: "/workspace".to_string(),
            ..Default::default()
        };

        let rendered = TemplateEngine::render(template, &context);
        assert!(rendered.contains("Task 1"));
        assert!(rendered.contains("/workspace"));
    }

    #[test]
    fn test_render_with_timestamp() {
        let template = "Generated at: {{timestamp}}";
        let context = TemplateContext {
            workspace: String::new(),
            ..Default::default()
        }.with_timestamp();

        let rendered = TemplateEngine::render(template, &context);
        assert!(rendered.contains("Generated at:"));
    }

    #[test]
    fn test_render_missing_variable() {
        let template = "Value: {{missing}}";
        let variables = HashMap::new();

        let rendered = TemplateEngine::render_custom(template, &variables);
        // Missing variables should remain as-is
        assert_eq!(rendered, "Value: {{missing}}");
    }

    #[test]
    fn test_render_empty_template() {
        let template = "";
        let variables = HashMap::new();

        let rendered = TemplateEngine::render_custom(template, &variables);
        assert_eq!(rendered, "");
    }
}
