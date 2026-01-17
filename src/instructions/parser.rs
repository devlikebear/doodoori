//! Markdown spec file parser

use anyhow::{Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::path::Path;

use super::spec::{GlobalSettings, Requirement, SpecFile, TaskSpec};
use crate::claude::ModelAlias;

/// Parser for spec files
pub struct SpecParser;

impl SpecParser {
    /// Parse a spec file from a path
    pub fn parse_file(path: &Path) -> Result<SpecFile> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read spec file: {}", path.display()))?;
        Self::parse(&content)
    }

    /// Parse a spec file from a string
    pub fn parse(content: &str) -> Result<SpecFile> {
        let mut spec = SpecFile {
            raw_content: content.to_string(),
            ..Default::default()
        };

        let parser = Parser::new(content);
        let events: Vec<Event> = parser.collect();

        let mut current_section: Option<String> = None;
        let mut current_task: Option<TaskSpec> = None;
        let mut in_heading = false;
        let mut heading_level = HeadingLevel::H1;
        let mut text_buffer = String::new();
        let mut list_items: Vec<String> = Vec::new();
        let mut in_list_item = false;

        for event in events {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    // Save previous section content
                    Self::process_section(
                        &mut spec,
                        &current_section,
                        &text_buffer,
                        &list_items,
                        &mut current_task,
                    );
                    text_buffer.clear();
                    list_items.clear();

                    in_heading = true;
                    heading_level = level;
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;
                    let heading_text = text_buffer.trim().to_string();
                    text_buffer.clear();

                    match heading_level {
                        HeadingLevel::H1 => {
                            // Parse title: "Task: Name" or "Spec: Name"
                            if let Some(title) = heading_text.strip_prefix("Task:") {
                                spec.title = title.trim().to_string();
                            } else if let Some(title) = heading_text.strip_prefix("Spec:") {
                                spec.title = title.trim().to_string();
                            } else {
                                spec.title = heading_text;
                            }
                            current_section = None;
                        }
                        HeadingLevel::H2 => {
                            current_section = Some(heading_text.to_lowercase());
                        }
                        HeadingLevel::H3 => {
                            // Task definition: "Task: task-id"
                            if let Some(task_id) = heading_text.strip_prefix("Task:") {
                                // Save previous task if exists
                                if let Some(task) = current_task.take() {
                                    spec.tasks.push(task);
                                }
                                current_task = Some(TaskSpec {
                                    id: task_id.trim().to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Event::Start(Tag::Item) => {
                    in_list_item = true;
                    text_buffer.clear();
                }
                Event::End(TagEnd::Item) => {
                    in_list_item = false;
                    list_items.push(text_buffer.trim().to_string());
                    text_buffer.clear();
                }
                Event::Text(text) | Event::Code(text) => {
                    if in_heading || in_list_item {
                        text_buffer.push_str(&text);
                    } else if current_section.is_some() {
                        text_buffer.push_str(&text);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !in_heading {
                        text_buffer.push('\n');
                    }
                }
                _ => {}
            }
        }

        // Process final section
        Self::process_section(
            &mut spec,
            &current_section,
            &text_buffer,
            &list_items,
            &mut current_task,
        );

        // Save final task if exists
        if let Some(task) = current_task.take() {
            spec.tasks.push(task);
        }

        Ok(spec)
    }

    fn process_section(
        spec: &mut SpecFile,
        section: &Option<String>,
        text: &str,
        list_items: &[String],
        current_task: &mut Option<TaskSpec>,
    ) {
        let Some(section_name) = section else {
            return;
        };

        let text = text.trim();

        match section_name.as_str() {
            "objective" => {
                if let Some(task) = current_task.as_mut() {
                    task.description = text.to_string();
                } else {
                    spec.objective = text.to_string();
                }
            }
            "model" => {
                if let Ok(model) = text.parse::<ModelAlias>() {
                    if current_task.is_some() {
                        current_task.as_mut().unwrap().model = Some(model);
                    } else {
                        spec.model = Some(model);
                    }
                }
            }
            "requirements" => {
                let reqs: Vec<Requirement> = list_items
                    .iter()
                    .filter(|s| !s.is_empty())
                    .map(|s| Self::parse_requirement(s))
                    .collect();

                if let Some(task) = current_task.as_mut() {
                    task.requirements = reqs;
                } else {
                    spec.requirements = reqs;
                }
            }
            "constraints" => {
                spec.constraints = list_items
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect();
            }
            "completion criteria" => {
                if let Some(task) = current_task.as_mut() {
                    task.completion_criteria = Some(text.to_string());
                } else {
                    spec.completion_criteria = Some(text.to_string());
                }
            }
            "max iterations" => {
                if let Ok(n) = text.parse::<u32>() {
                    if let Some(task) = current_task.as_mut() {
                        task.max_iterations = Some(n);
                    } else {
                        spec.max_iterations = Some(n);
                    }
                }
            }
            "completion promise" => {
                spec.completion_promise = Some(text.to_string());
            }
            "budget" => {
                // Parse budget: "max_total_usd: 15.00" or just "15.00"
                let value = text
                    .split(':')
                    .last()
                    .unwrap_or(text)
                    .trim()
                    .trim_end_matches("USD")
                    .trim();
                if let Ok(budget) = value.parse::<f64>() {
                    spec.budget = Some(budget);
                }
            }
            "global settings" => {
                spec.global_settings = Some(Self::parse_global_settings(text, list_items));
            }
            "tasks" => {
                // Tasks section header, actual tasks are H3 headers
            }
            _ => {
                // Handle task-specific fields when in a task context
                if let Some(task) = current_task.as_mut() {
                    Self::parse_task_field(task, section_name, text, list_items);
                }
            }
        }
    }

    fn parse_requirement(text: &str) -> Requirement {
        let text = text.trim();

        // Check for checkbox: "[ ] text" or "[x] text"
        if let Some(rest) = text.strip_prefix("[ ]") {
            Requirement::new(rest.trim())
        } else if let Some(rest) = text.strip_prefix("[x]").or_else(|| text.strip_prefix("[X]")) {
            Requirement::completed(rest.trim())
        } else {
            Requirement::new(text)
        }
    }

    fn parse_global_settings(text: &str, _list_items: &[String]) -> GlobalSettings {
        let mut settings = GlobalSettings::default();

        for line in text.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_lowercase();
                let value = value.trim();

                match key.as_str() {
                    "default_model" => {
                        if let Ok(model) = value.parse::<ModelAlias>() {
                            settings.default_model = Some(model);
                        }
                    }
                    "max_parallel_workers" => {
                        if let Ok(n) = value.parse::<usize>() {
                            settings.max_parallel_workers = Some(n);
                        }
                    }
                    "completion_promise" => {
                        settings.completion_promise = value.trim_matches('"').to_string();
                    }
                    "max_total_usd" => {
                        if let Ok(n) = value.parse::<f64>() {
                            settings.max_total_usd = Some(n);
                        }
                    }
                    _ => {}
                }
            }
        }

        settings
    }

    fn parse_task_field(task: &mut TaskSpec, field: &str, text: &str, list_items: &[String]) {
        match field {
            "description" => {
                task.description = text.to_string();
            }
            "priority" => {
                if let Ok(n) = text.parse::<u32>() {
                    task.priority = n;
                }
            }
            "depends_on" | "dependencies" => {
                // Parse: "[task1, task2]" or list items
                if text.starts_with('[') && text.ends_with(']') {
                    let inner = &text[1..text.len() - 1];
                    task.depends_on = inner
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else if !list_items.is_empty() {
                    task.depends_on = list_items.to_vec();
                }
            }
            _ => {}
        }
    }

    /// Generate a spec file from a simple prompt
    pub fn generate_spec(description: &str, model: Option<ModelAlias>) -> SpecFile {
        // Extract a title from the first line or first sentence
        let title = description
            .lines()
            .next()
            .unwrap_or(description)
            .chars()
            .take(50)
            .collect::<String>();

        SpecFile {
            title,
            objective: description.to_string(),
            model,
            max_iterations: Some(50),
            completion_promise: Some("<promise>COMPLETE</promise>".to_string()),
            ..Default::default()
        }
    }

    /// Generate markdown content from a SpecFile
    pub fn to_markdown(spec: &SpecFile) -> String {
        let mut md = String::new();

        // Title
        md.push_str(&format!("# Task: {}\n\n", spec.title));

        // Objective
        md.push_str(&format!("## Objective\n{}\n\n", spec.objective));

        // Model
        if let Some(ref model) = spec.model {
            md.push_str(&format!("## Model\n{}\n\n", model));
        }

        // Requirements
        if !spec.requirements.is_empty() {
            md.push_str("## Requirements\n");
            for req in &spec.requirements {
                let checkbox = if req.completed { "[x]" } else { "[ ]" };
                md.push_str(&format!("- {} {}\n", checkbox, req.description));
            }
            md.push('\n');
        }

        // Constraints
        if !spec.constraints.is_empty() {
            md.push_str("## Constraints\n");
            for constraint in &spec.constraints {
                md.push_str(&format!("- {}\n", constraint));
            }
            md.push('\n');
        }

        // Completion Criteria
        if let Some(ref criteria) = spec.completion_criteria {
            md.push_str(&format!("## Completion Criteria\n{}\n\n", criteria));
        }

        // Max Iterations
        if let Some(max) = spec.max_iterations {
            md.push_str(&format!("## Max Iterations\n{}\n\n", max));
        }

        // Completion Promise
        if let Some(ref promise) = spec.completion_promise {
            md.push_str(&format!("## Completion Promise\n{}\n", promise));
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_spec() {
        let content = r#"
# Task: Build REST API

## Objective
Create a simple REST API for todos

## Model
sonnet

## Requirements
- [ ] GET /todos endpoint
- [x] POST /todos endpoint
- [ ] DELETE /todos endpoint

## Constraints
- Use Rust
- Use Axum framework

## Completion Criteria
All endpoints working

## Max Iterations
30

## Completion Promise
<promise>DONE</promise>
"#;

        let spec = SpecParser::parse(content).unwrap();

        assert_eq!(spec.title, "Build REST API");
        assert_eq!(spec.objective, "Create a simple REST API for todos");
        assert_eq!(spec.model, Some(ModelAlias::Sonnet));
        assert_eq!(spec.requirements.len(), 3);
        assert!(!spec.requirements[0].completed);
        assert!(spec.requirements[1].completed);
        assert_eq!(spec.constraints.len(), 2);
        assert_eq!(spec.completion_criteria, Some("All endpoints working".to_string()));
        assert_eq!(spec.max_iterations, Some(30));
        // Note: pulldown-cmark treats <promise> as HTML and may parse it differently
        assert!(spec.completion_promise.as_ref().unwrap().contains("DONE"));
    }

    #[test]
    fn test_parse_multi_task_spec() {
        let content = r#"
# Spec: Full Stack App

## Objective
Build a full stack application

## Global Settings
default_model: sonnet
max_parallel_workers: 3
completion_promise: "COMPLETE"

## Tasks

### Task: backend
Backend implementation

### Task: frontend
Frontend implementation
"#;

        let spec = SpecParser::parse(content).unwrap();

        assert_eq!(spec.title, "Full Stack App");
        assert!(spec.global_settings.is_some());
        assert_eq!(spec.tasks.len(), 2);
        assert_eq!(spec.tasks[0].id, "backend");
        assert_eq!(spec.tasks[1].id, "frontend");
    }

    #[test]
    fn test_parse_requirement() {
        let req1 = SpecParser::parse_requirement("[ ] Incomplete task");
        assert!(!req1.completed);
        assert_eq!(req1.description, "Incomplete task");

        let req2 = SpecParser::parse_requirement("[x] Complete task");
        assert!(req2.completed);
        assert_eq!(req2.description, "Complete task");

        let req3 = SpecParser::parse_requirement("Plain task");
        assert!(!req3.completed);
        assert_eq!(req3.description, "Plain task");
    }

    #[test]
    fn test_generate_spec() {
        let spec = SpecParser::generate_spec("Build a todo app", Some(ModelAlias::Haiku));

        assert_eq!(spec.title, "Build a todo app");
        assert_eq!(spec.objective, "Build a todo app");
        assert_eq!(spec.model, Some(ModelAlias::Haiku));
    }

    #[test]
    fn test_to_markdown() {
        let spec = SpecFile {
            title: "Test Task".to_string(),
            objective: "Do something".to_string(),
            model: Some(ModelAlias::Opus),
            requirements: vec![
                Requirement::new("First"),
                Requirement::completed("Second"),
            ],
            constraints: vec!["Constraint 1".to_string()],
            completion_criteria: Some("Tests pass".to_string()),
            max_iterations: Some(25),
            completion_promise: Some("<promise>DONE</promise>".to_string()),
            ..Default::default()
        };

        let md = SpecParser::to_markdown(&spec);

        assert!(md.contains("# Task: Test Task"));
        assert!(md.contains("## Objective\nDo something"));
        assert!(md.contains("## Model\nopus"));
        assert!(md.contains("- [ ] First"));
        assert!(md.contains("- [x] Second"));
        assert!(md.contains("- Constraint 1"));
        assert!(md.contains("## Max Iterations\n25"));
    }

    #[test]
    fn test_roundtrip() {
        let original = SpecFile {
            title: "Roundtrip Test".to_string(),
            objective: "Test parsing roundtrip".to_string(),
            model: Some(ModelAlias::Sonnet),
            requirements: vec![Requirement::new("Requirement 1")],
            max_iterations: Some(40),
            ..Default::default()
        };

        let md = SpecParser::to_markdown(&original);
        let parsed = SpecParser::parse(&md).unwrap();

        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.objective, original.objective);
        assert_eq!(parsed.model, original.model);
        assert_eq!(parsed.requirements.len(), original.requirements.len());
        assert_eq!(parsed.max_iterations, original.max_iterations);
    }
}
