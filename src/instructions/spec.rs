//! Spec file data structures

use serde::{Deserialize, Serialize};

use crate::claude::ModelAlias;

/// A parsed spec file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFile {
    /// Title of the spec (from # Task: or # Spec:)
    pub title: String,

    /// Objective description
    pub objective: String,

    /// Model to use (optional, defaults to sonnet)
    pub model: Option<ModelAlias>,

    /// List of requirements
    pub requirements: Vec<Requirement>,

    /// Constraints for the task
    pub constraints: Vec<String>,

    /// Completion criteria description
    pub completion_criteria: Option<String>,

    /// Maximum iterations for the loop engine
    pub max_iterations: Option<u32>,

    /// Completion promise string to look for
    pub completion_promise: Option<String>,

    /// Global settings (for multi-task specs)
    pub global_settings: Option<GlobalSettings>,

    /// Individual tasks (for parallel execution)
    pub tasks: Vec<TaskSpec>,

    /// Budget limit in USD
    pub budget: Option<f64>,

    /// Raw markdown content
    pub raw_content: String,
}

impl Default for SpecFile {
    fn default() -> Self {
        Self {
            title: String::new(),
            objective: String::new(),
            model: None,
            requirements: Vec::new(),
            constraints: Vec::new(),
            completion_criteria: None,
            max_iterations: Some(50),
            completion_promise: Some("<promise>COMPLETE</promise>".to_string()),
            global_settings: None,
            tasks: Vec::new(),
            budget: None,
            raw_content: String::new(),
        }
    }
}

impl SpecFile {
    /// Check if this spec has multiple tasks for parallel execution
    pub fn is_multi_task(&self) -> bool {
        !self.tasks.is_empty()
    }

    /// Get the effective model for this spec
    pub fn effective_model(&self) -> ModelAlias {
        self.model.clone().unwrap_or_else(|| {
            self.global_settings
                .as_ref()
                .and_then(|g| g.default_model.clone())
                .unwrap_or(ModelAlias::Sonnet)
        })
    }

    /// Get the effective max iterations
    pub fn effective_max_iterations(&self) -> u32 {
        self.max_iterations.unwrap_or(50)
    }

    /// Get the effective completion promise
    pub fn effective_completion_promise(&self) -> String {
        self.completion_promise
            .clone()
            .or_else(|| {
                self.global_settings
                    .as_ref()
                    .map(|g| g.completion_promise.clone())
            })
            .unwrap_or_else(|| "<promise>COMPLETE</promise>".to_string())
    }

    /// Build the prompt for Claude from this spec
    pub fn to_prompt(&self) -> String {
        let mut prompt = String::new();

        // Title and objective
        prompt.push_str(&format!("# Task: {}\n\n", self.title));
        prompt.push_str(&format!("## Objective\n{}\n\n", self.objective));

        // Requirements
        if !self.requirements.is_empty() {
            prompt.push_str("## Requirements\n");
            for req in &self.requirements {
                let checkbox = if req.completed { "[x]" } else { "[ ]" };
                prompt.push_str(&format!("- {} {}\n", checkbox, req.description));
            }
            prompt.push('\n');
        }

        // Constraints
        if !self.constraints.is_empty() {
            prompt.push_str("## Constraints\n");
            for constraint in &self.constraints {
                prompt.push_str(&format!("- {}\n", constraint));
            }
            prompt.push('\n');
        }

        // Completion criteria
        if let Some(ref criteria) = self.completion_criteria {
            prompt.push_str(&format!("## Completion Criteria\n{}\n\n", criteria));
        }

        // Completion promise instruction
        prompt.push_str("---\n\n");
        prompt.push_str(&format!(
            "When you have completed all requirements, output the completion marker: {}\n",
            self.effective_completion_promise()
        ));

        prompt
    }
}

/// A single requirement item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    /// Description of the requirement
    pub description: String,

    /// Whether this requirement is completed
    pub completed: bool,
}

impl Requirement {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            completed: false,
        }
    }

    pub fn completed(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            completed: true,
        }
    }
}

/// Global settings for multi-task specs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Default model for all tasks
    pub default_model: Option<ModelAlias>,

    /// Maximum parallel workers
    pub max_parallel_workers: Option<usize>,

    /// Completion promise string
    pub completion_promise: String,

    /// Total budget limit
    pub max_total_usd: Option<f64>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_model: Some(ModelAlias::Sonnet),
            max_parallel_workers: Some(3),
            completion_promise: "COMPLETE".to_string(),
            max_total_usd: None,
        }
    }
}

/// A single task within a multi-task spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Task identifier (from ### Task: name)
    pub id: String,

    /// Model to use for this task
    pub model: Option<ModelAlias>,

    /// Priority (lower = higher priority)
    pub priority: u32,

    /// Dependencies (task IDs that must complete first)
    pub depends_on: Vec<String>,

    /// Task description
    pub description: String,

    /// Requirements for this task
    pub requirements: Vec<Requirement>,

    /// Completion criteria
    pub completion_criteria: Option<String>,

    /// Maximum iterations for this task
    pub max_iterations: Option<u32>,
}

impl Default for TaskSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            model: None,
            priority: 1,
            depends_on: Vec::new(),
            description: String::new(),
            requirements: Vec::new(),
            completion_criteria: None,
            max_iterations: Some(30),
        }
    }
}

impl TaskSpec {
    /// Check if this task has no dependencies
    pub fn is_independent(&self) -> bool {
        self.depends_on.is_empty()
    }

    /// Get effective model, falling back to default
    pub fn effective_model(&self, default: &ModelAlias) -> ModelAlias {
        self.model.clone().unwrap_or_else(|| default.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_file_default() {
        let spec = SpecFile::default();
        assert!(spec.title.is_empty());
        assert_eq!(spec.effective_max_iterations(), 50);
        assert!(spec.effective_completion_promise().contains("COMPLETE"));
    }

    #[test]
    fn test_spec_effective_model() {
        let spec = SpecFile::default();
        assert_eq!(spec.effective_model(), ModelAlias::Sonnet);

        let spec_with_model = SpecFile {
            model: Some(ModelAlias::Opus),
            ..Default::default()
        };
        assert_eq!(spec_with_model.effective_model(), ModelAlias::Opus);
    }

    #[test]
    fn test_spec_is_multi_task() {
        let spec = SpecFile::default();
        assert!(!spec.is_multi_task());

        let multi_spec = SpecFile {
            tasks: vec![TaskSpec::default()],
            ..Default::default()
        };
        assert!(multi_spec.is_multi_task());
    }

    #[test]
    fn test_requirement_new() {
        let req = Requirement::new("Implement feature X");
        assert_eq!(req.description, "Implement feature X");
        assert!(!req.completed);

        let done = Requirement::completed("Done feature");
        assert!(done.completed);
    }

    #[test]
    fn test_task_spec_independent() {
        let task = TaskSpec::default();
        assert!(task.is_independent());

        let dependent = TaskSpec {
            depends_on: vec!["other-task".to_string()],
            ..Default::default()
        };
        assert!(!dependent.is_independent());
    }

    #[test]
    fn test_spec_to_prompt() {
        let spec = SpecFile {
            title: "Test Task".to_string(),
            objective: "Do something useful".to_string(),
            requirements: vec![
                Requirement::new("First requirement"),
                Requirement::completed("Second requirement"),
            ],
            constraints: vec!["Use Rust".to_string()],
            completion_criteria: Some("All tests pass".to_string()),
            ..Default::default()
        };

        let prompt = spec.to_prompt();
        assert!(prompt.contains("# Task: Test Task"));
        assert!(prompt.contains("Do something useful"));
        assert!(prompt.contains("[ ] First requirement"));
        assert!(prompt.contains("[x] Second requirement"));
        assert!(prompt.contains("Use Rust"));
        assert!(prompt.contains("All tests pass"));
        assert!(prompt.contains("COMPLETE"));
    }
}
