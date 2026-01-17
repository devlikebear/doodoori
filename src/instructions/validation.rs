//! Spec file validation

use super::spec::SpecFile;

/// Validation error type
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub severity: Severity,
}

/// Error severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl ValidationError {
    pub fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: Severity::Error,
        }
    }

    pub fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: Severity::Warning,
        }
    }
}

/// Result of validating a spec file
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ValidationError::error(field, message));
    }

    pub fn add_warning(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(ValidationError::warning(field, message));
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// Validate a spec file
pub fn validate(spec: &SpecFile) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Required fields
    if spec.title.is_empty() {
        result.add_error("title", "Title is required (use # Task: or # Spec:)");
    }

    if spec.objective.is_empty() {
        result.add_error("objective", "Objective section is required");
    }

    // Warnings for missing optional fields
    if spec.requirements.is_empty() && spec.tasks.is_empty() {
        result.add_warning(
            "requirements",
            "No requirements specified. Consider adding requirements for clarity.",
        );
    }

    if spec.completion_promise.is_none() {
        result.add_warning(
            "completion_promise",
            "No completion promise specified. Using default: <promise>COMPLETE</promise>",
        );
    }

    // Validate max_iterations
    if let Some(max) = spec.max_iterations {
        if max == 0 {
            result.add_error("max_iterations", "Max iterations must be greater than 0");
        } else if max > 200 {
            result.add_warning(
                "max_iterations",
                "Max iterations is very high (>200). This may be costly.",
            );
        }
    }

    // Validate budget
    if let Some(budget) = spec.budget {
        if budget <= 0.0 {
            result.add_error("budget", "Budget must be greater than 0");
        }
    }

    // Validate tasks for multi-task specs
    if !spec.tasks.is_empty() {
        result.merge(validate_tasks(spec));
    }

    result
}

/// Validate tasks in a multi-task spec
fn validate_tasks(spec: &SpecFile) -> ValidationResult {
    let mut result = ValidationResult::new();
    let task_ids: Vec<&str> = spec.tasks.iter().map(|t| t.id.as_str()).collect();

    for task in &spec.tasks {
        // Task ID is required
        if task.id.is_empty() {
            result.add_error("task.id", "Task ID is required");
        }

        // Validate dependencies exist
        for dep in &task.depends_on {
            if !task_ids.contains(&dep.as_str()) {
                result.add_error(
                    format!("task.{}.depends_on", task.id),
                    format!("Unknown dependency: {}", dep),
                );
            }
        }

        // Check for circular dependencies (simple check)
        if task.depends_on.contains(&task.id) {
            result.add_error(
                format!("task.{}.depends_on", task.id),
                "Task cannot depend on itself",
            );
        }

        // Task should have description
        if task.description.is_empty() && task.requirements.is_empty() {
            result.add_warning(
                format!("task.{}", task.id),
                "Task has no description or requirements",
            );
        }
    }

    // Check for dependency cycles (more thorough)
    if let Some(cycle) = detect_cycle(&spec.tasks) {
        result.add_error(
            "tasks",
            format!("Circular dependency detected: {}", cycle.join(" -> ")),
        );
    }

    result
}

/// Detect circular dependencies in tasks
fn detect_cycle(tasks: &[super::spec::TaskSpec]) -> Option<Vec<String>> {
    use std::collections::{HashMap, HashSet};

    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();

    for task in tasks {
        graph.insert(&task.id, task.depends_on.iter().map(|s| s.as_str()).collect());
    }

    fn dfs<'a>(
        node: &'a str,
        graph: &HashMap<&str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        if path.contains(&node) {
            // Found cycle
            let cycle_start = path.iter().position(|&n| n == node).unwrap();
            let mut cycle: Vec<String> = path[cycle_start..].iter().map(|s| s.to_string()).collect();
            cycle.push(node.to_string());
            return Some(cycle);
        }

        if visited.contains(node) {
            return None;
        }

        visited.insert(node);
        path.push(node);

        if let Some(deps) = graph.get(node) {
            for &dep in deps {
                if let Some(cycle) = dfs(dep, graph, visited, path) {
                    return Some(cycle);
                }
            }
        }

        path.pop();
        None
    }

    let mut visited = HashSet::new();
    let mut path = Vec::new();

    for task in tasks {
        if let Some(cycle) = dfs(&task.id, &graph, &mut visited, &mut path) {
            return Some(cycle);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::spec::{Requirement, TaskSpec};

    #[test]
    fn test_valid_spec() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            requirements: vec![Requirement::new("Req 1")],
            completion_promise: Some("<promise>DONE</promise>".to_string()),
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(result.is_valid());
    }

    #[test]
    fn test_missing_title() {
        let spec = SpecFile {
            objective: "Do something".to_string(),
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "title"));
    }

    #[test]
    fn test_missing_objective() {
        let spec = SpecFile {
            title: "Test".to_string(),
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "objective"));
    }

    #[test]
    fn test_warning_no_requirements() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(result.is_valid());
        assert!(result.has_warnings());
        assert!(result.warnings.iter().any(|e| e.field == "requirements"));
    }

    #[test]
    fn test_invalid_max_iterations() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            max_iterations: Some(0),
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_unknown_dependency() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            tasks: vec![TaskSpec {
                id: "task1".to_string(),
                depends_on: vec!["nonexistent".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_self_dependency() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            tasks: vec![TaskSpec {
                id: "task1".to_string(),
                depends_on: vec!["task1".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_circular_dependency() {
        let spec = SpecFile {
            title: "Test".to_string(),
            objective: "Do something".to_string(),
            tasks: vec![
                TaskSpec {
                    id: "task1".to_string(),
                    depends_on: vec!["task2".to_string()],
                    ..Default::default()
                },
                TaskSpec {
                    id: "task2".to_string(),
                    depends_on: vec!["task1".to_string()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let result = validate(&spec);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("Circular")));
    }
}
