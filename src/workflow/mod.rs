//! Workflow module for defining and executing complex multi-step workflows.
//!
//! This module provides YAML-based workflow definitions with dependency management
//! and DAG-based scheduling.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::claude::ModelAlias;

/// Global settings for a workflow
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowGlobalSettings {
    /// Default model for steps without explicit model
    #[serde(default = "default_model")]
    pub default_model: String,
    /// Maximum parallel workers
    #[serde(default = "default_max_workers")]
    pub max_parallel_workers: usize,
    /// Completion promise string
    #[serde(default = "default_completion_promise")]
    pub completion_promise: String,
    /// Total budget for the workflow in USD
    pub budget_usd: Option<f64>,
}

fn default_model() -> String {
    "sonnet".to_string()
}

fn default_max_workers() -> usize {
    4
}

fn default_completion_promise() -> String {
    "COMPLETE".to_string()
}

impl Default for WorkflowGlobalSettings {
    fn default() -> Self {
        Self {
            default_model: default_model(),
            max_parallel_workers: default_max_workers(),
            completion_promise: default_completion_promise(),
            budget_usd: None,
        }
    }
}

/// A step in a workflow
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowStep {
    /// Step name (unique identifier)
    pub name: String,
    /// Prompt or task description
    #[serde(default)]
    pub prompt: Option<String>,
    /// Path to spec file
    #[serde(default)]
    pub spec: Option<String>,
    /// Model to use for this step
    #[serde(default)]
    pub model: Option<String>,
    /// Parallel group (steps in same group run concurrently)
    #[serde(default)]
    pub parallel_group: u32,
    /// Dependencies (step names that must complete first)
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Maximum iterations for this step
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Budget limit for this step in USD
    #[serde(default)]
    pub budget_usd: Option<f64>,
}

fn default_max_iterations() -> u32 {
    50
}

/// Workflow definition loaded from YAML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowDefinition {
    /// Workflow name
    pub name: String,
    /// Global settings
    #[serde(default)]
    pub global: WorkflowGlobalSettings,
    /// Steps in the workflow
    pub steps: Vec<WorkflowStep>,
}

impl WorkflowDefinition {
    /// Load workflow from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read workflow file: {}", path.display()))?;
        Self::parse(&content)
    }

    /// Parse workflow from YAML string
    pub fn parse(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml).context("Failed to parse workflow YAML")
    }

    /// Validate the workflow definition
    pub fn validate(&self) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for duplicate step names
        let mut seen_names: HashSet<String> = HashSet::new();
        for step in &self.steps {
            if !seen_names.insert(step.name.clone()) {
                anyhow::bail!("Duplicate step name: {}", step.name);
            }
        }

        // Check that all dependencies exist
        for step in &self.steps {
            for dep in &step.depends_on {
                if !seen_names.contains(dep) {
                    anyhow::bail!(
                        "Step '{}' depends on unknown step '{}'",
                        step.name,
                        dep
                    );
                }
            }
        }

        // Check for circular dependencies
        self.check_circular_dependencies()?;

        // Warnings
        for step in &self.steps {
            if step.prompt.is_none() && step.spec.is_none() {
                warnings.push(format!(
                    "Step '{}' has neither prompt nor spec defined",
                    step.name
                ));
            }
        }

        Ok(warnings)
    }

    /// Check for circular dependencies using DFS
    fn check_circular_dependencies(&self) -> Result<()> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for step in &self.steps {
            if self.has_cycle(&step.name, &mut visited, &mut rec_stack)? {
                anyhow::bail!("Circular dependency detected involving step '{}'", step.name);
            }
        }

        Ok(())
    }

    fn has_cycle(
        &self,
        step_name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<bool> {
        if rec_stack.contains(step_name) {
            return Ok(true);
        }
        if visited.contains(step_name) {
            return Ok(false);
        }

        visited.insert(step_name.to_string());
        rec_stack.insert(step_name.to_string());

        let step = self
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or_else(|| anyhow::anyhow!("Step not found: {}", step_name))?;

        for dep in &step.depends_on {
            if self.has_cycle(dep, visited, rec_stack)? {
                return Ok(true);
            }
        }

        rec_stack.remove(step_name);
        Ok(false)
    }

    /// Get effective model for a step
    pub fn get_step_model(&self, step: &WorkflowStep) -> ModelAlias {
        let model_str = step
            .model
            .as_ref()
            .unwrap_or(&self.global.default_model);

        match model_str.to_lowercase().as_str() {
            "haiku" => ModelAlias::Haiku,
            "sonnet" => ModelAlias::Sonnet,
            "opus" => ModelAlias::Opus,
            _ => ModelAlias::Sonnet,
        }
    }
}

/// DAG-based scheduler for workflow execution
pub struct DagScheduler {
    workflow: WorkflowDefinition,
    completed: HashSet<String>,
    running: HashSet<String>,
}

impl DagScheduler {
    /// Create a new scheduler for a workflow
    pub fn new(workflow: WorkflowDefinition) -> Self {
        Self {
            workflow,
            completed: HashSet::new(),
            running: HashSet::new(),
        }
    }

    /// Get the next batch of steps that can be executed
    pub fn get_ready_steps(&self) -> Vec<&WorkflowStep> {
        self.workflow
            .steps
            .iter()
            .filter(|step| {
                // Not already completed or running
                !self.completed.contains(&step.name) && !self.running.contains(&step.name)
            })
            .filter(|step| {
                // All dependencies are completed
                step.depends_on
                    .iter()
                    .all(|dep| self.completed.contains(dep))
            })
            .collect()
    }

    /// Get steps organized by parallel groups
    pub fn get_execution_groups(&self) -> Vec<Vec<&WorkflowStep>> {
        let mut groups: HashMap<u32, Vec<&WorkflowStep>> = HashMap::new();

        for step in &self.workflow.steps {
            groups.entry(step.parallel_group).or_default().push(step);
        }

        let mut sorted_groups: Vec<(u32, Vec<&WorkflowStep>)> = groups.into_iter().collect();
        sorted_groups.sort_by_key(|(group, _)| *group);

        sorted_groups.into_iter().map(|(_, steps)| steps).collect()
    }

    /// Mark a step as started
    pub fn mark_started(&mut self, step_name: &str) {
        self.running.insert(step_name.to_string());
    }

    /// Mark a step as completed
    pub fn mark_completed(&mut self, step_name: &str) {
        self.running.remove(step_name);
        self.completed.insert(step_name.to_string());
    }

    /// Mark a step as failed
    pub fn mark_failed(&mut self, step_name: &str) {
        self.running.remove(step_name);
    }

    /// Check if all steps are completed
    pub fn is_complete(&self) -> bool {
        self.completed.len() == self.workflow.steps.len()
    }

    /// Get topological order of steps
    pub fn topological_order(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize
        for step in &self.workflow.steps {
            in_degree.insert(step.name.clone(), step.depends_on.len());
            adjacency.entry(step.name.clone()).or_default();
            for dep in &step.depends_on {
                adjacency.entry(dep.clone()).or_default().push(step.name.clone());
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(step) = queue.pop_front() {
            result.push(step.clone());

            if let Some(dependents) = adjacency.get(&step) {
                for dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.workflow.steps.len() {
            anyhow::bail!("Circular dependency detected in workflow");
        }

        Ok(result)
    }

    /// Get workflow reference
    pub fn workflow(&self) -> &WorkflowDefinition {
        &self.workflow
    }
}

/// Execution state of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Workflow ID
    pub workflow_id: String,
    /// Workflow name
    pub name: String,
    /// Current status
    pub status: WorkflowStatus,
    /// Current parallel group being executed
    pub current_group: u32,
    /// Step states
    pub steps: HashMap<String, StepState>,
    /// Total cost so far
    pub total_cost_usd: f64,
    /// Started at
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Updated at
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Status of a workflow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// State of a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    /// Step status
    pub status: StepStatus,
    /// Model used
    pub model: String,
    /// Cost for this step
    pub cost_usd: f64,
    /// Error message if failed
    pub error: Option<String>,
    /// Started at
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Completed at
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Status of a step
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl WorkflowState {
    /// Create a new workflow state
    pub fn new(workflow_id: String, name: String, steps: &[WorkflowStep]) -> Self {
        let now = chrono::Utc::now();
        let step_states: HashMap<String, StepState> = steps
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    StepState {
                        status: StepStatus::Pending,
                        model: s.model.clone().unwrap_or_else(|| "sonnet".to_string()),
                        cost_usd: 0.0,
                        error: None,
                        started_at: None,
                        completed_at: None,
                    },
                )
            })
            .collect();

        Self {
            workflow_id,
            name,
            status: WorkflowStatus::Pending,
            current_group: 0,
            steps: step_states,
            total_cost_usd: 0.0,
            started_at: now,
            updated_at: now,
        }
    }

    /// Update a step's state
    pub fn update_step(&mut self, step_name: &str, status: StepStatus, cost: f64, error: Option<String>) {
        if let Some(step) = self.steps.get_mut(step_name) {
            step.status = status.clone();
            step.cost_usd = cost;
            step.error = error;
            if status == StepStatus::Running {
                step.started_at = Some(chrono::Utc::now());
            } else if matches!(status, StepStatus::Completed | StepStatus::Failed) {
                step.completed_at = Some(chrono::Utc::now());
            }
            self.total_cost_usd += cost;
            self.updated_at = chrono::Utc::now();
        }
    }

    /// Check if the workflow can be resumed
    pub fn can_resume(&self) -> bool {
        matches!(self.status, WorkflowStatus::Failed | WorkflowStatus::Cancelled)
    }

    /// Get steps that need to be re-executed for resume
    pub fn get_resumable_steps(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter(|(_, state)| {
                matches!(state.status, StepStatus::Pending | StepStatus::Failed)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WORKFLOW: &str = r#"
name: "Full Stack Development"

global:
  default_model: sonnet
  max_parallel_workers: 4
  budget_usd: 20.00
  completion_promise: "COMPLETE"

steps:
  - name: "Project Setup"
    prompt: "Initialize project with TypeScript"
    model: haiku
    parallel_group: 0
    max_iterations: 10
    budget_usd: 1.00

  - name: "Backend API"
    prompt: "Implement REST API"
    model: sonnet
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Frontend UI"
    prompt: "Create React frontend"
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Integration"
    prompt: "Connect frontend to backend"
    model: haiku
    parallel_group: 2
    depends_on: ["Backend API", "Frontend UI"]
    budget_usd: 2.00
"#;

    #[test]
    fn test_parse_workflow() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();

        assert_eq!(workflow.name, "Full Stack Development");
        assert_eq!(workflow.global.default_model, "sonnet");
        assert_eq!(workflow.global.max_parallel_workers, 4);
        assert_eq!(workflow.global.budget_usd, Some(20.00));
        assert_eq!(workflow.steps.len(), 4);
    }

    #[test]
    fn test_validate_workflow() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let warnings = workflow.validate().unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_circular_dependency_detection() {
        let yaml = r#"
name: "Circular"
steps:
  - name: "A"
    prompt: "A"
    depends_on: ["C"]
  - name: "B"
    prompt: "B"
    depends_on: ["A"]
  - name: "C"
    prompt: "C"
    depends_on: ["B"]
"#;
        let workflow = WorkflowDefinition::parse(yaml).unwrap();
        assert!(workflow.validate().is_err());
    }

    #[test]
    fn test_unknown_dependency() {
        let yaml = r#"
name: "Unknown Dep"
steps:
  - name: "A"
    prompt: "A"
    depends_on: ["NonExistent"]
"#;
        let workflow = WorkflowDefinition::parse(yaml).unwrap();
        assert!(workflow.validate().is_err());
    }

    #[test]
    fn test_dag_scheduler_ready_steps() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let scheduler = DagScheduler::new(workflow);

        let ready = scheduler.get_ready_steps();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].name, "Project Setup");
    }

    #[test]
    fn test_dag_scheduler_after_completion() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let mut scheduler = DagScheduler::new(workflow);

        scheduler.mark_completed("Project Setup");

        let ready = scheduler.get_ready_steps();
        assert_eq!(ready.len(), 2);
        let names: HashSet<_> = ready.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains("Backend API"));
        assert!(names.contains("Frontend UI"));
    }

    #[test]
    fn test_topological_order() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let scheduler = DagScheduler::new(workflow);

        let order = scheduler.topological_order().unwrap();

        // Project Setup must come before Backend API and Frontend UI
        let setup_idx = order.iter().position(|s| s == "Project Setup").unwrap();
        let backend_idx = order.iter().position(|s| s == "Backend API").unwrap();
        let frontend_idx = order.iter().position(|s| s == "Frontend UI").unwrap();
        let integration_idx = order.iter().position(|s| s == "Integration").unwrap();

        assert!(setup_idx < backend_idx);
        assert!(setup_idx < frontend_idx);
        assert!(backend_idx < integration_idx);
        assert!(frontend_idx < integration_idx);
    }

    #[test]
    fn test_execution_groups() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let scheduler = DagScheduler::new(workflow);

        let groups = scheduler.get_execution_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 1); // Group 0: Project Setup
        assert_eq!(groups[1].len(), 2); // Group 1: Backend API, Frontend UI
        assert_eq!(groups[2].len(), 1); // Group 2: Integration
    }

    #[test]
    fn test_workflow_state() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();
        let mut state = WorkflowState::new(
            "wf-123".to_string(),
            workflow.name.clone(),
            &workflow.steps,
        );

        assert_eq!(state.status, WorkflowStatus::Pending);
        assert_eq!(state.steps.len(), 4);

        state.update_step("Project Setup", StepStatus::Completed, 0.5, None);
        assert_eq!(state.total_cost_usd, 0.5);
        assert_eq!(state.steps["Project Setup"].status, StepStatus::Completed);
    }

    #[test]
    fn test_get_step_model() {
        let workflow = WorkflowDefinition::parse(SAMPLE_WORKFLOW).unwrap();

        // Step with explicit model
        let setup_step = workflow.steps.iter().find(|s| s.name == "Project Setup").unwrap();
        assert_eq!(workflow.get_step_model(setup_step), ModelAlias::Haiku);

        // Step with no model (uses default)
        let frontend_step = workflow.steps.iter().find(|s| s.name == "Frontend UI").unwrap();
        assert_eq!(workflow.get_step_model(frontend_step), ModelAlias::Sonnet);
    }
}
