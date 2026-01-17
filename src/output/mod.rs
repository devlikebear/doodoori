#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;

/// Output format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text (default)
    #[default]
    Text,
    /// JSON format
    Json,
    /// Pretty JSON format
    JsonPretty,
    /// YAML format
    Yaml,
    /// Markdown format
    Markdown,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" | "plain" | "txt" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "json-pretty" | "jsonpretty" => Ok(OutputFormat::JsonPretty),
            "yaml" | "yml" => Ok(OutputFormat::Yaml),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            _ => Err(format!(
                "Unknown format '{}'. Use: text, json, json-pretty, yaml, markdown",
                s
            )),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonPretty => write!(f, "json-pretty"),
            OutputFormat::Yaml => write!(f, "yaml"),
            OutputFormat::Markdown => write!(f, "markdown"),
        }
    }
}

/// Task execution result for output formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    /// Task ID
    pub task_id: String,
    /// Task prompt or description
    pub prompt: String,
    /// Model used
    pub model: String,
    /// Final status
    pub status: String,
    /// Number of iterations
    pub iterations: u32,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Input tokens used
    pub input_tokens: u64,
    /// Output tokens used
    pub output_tokens: u64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Final output/response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TaskOutput {
    pub fn new(task_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            prompt: prompt.into(),
            model: "sonnet".to_string(),
            status: "unknown".to_string(),
            iterations: 0,
            cost_usd: 0.0,
            duration_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            error: None,
            output: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = cost_usd;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_tokens(mut self, input: u64, output: u64) -> Self {
        self.input_tokens = input;
        self.output_tokens = output;
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Parallel execution result for output formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelOutput {
    /// Total number of tasks
    pub total_tasks: usize,
    /// Number of successful tasks
    pub successful: usize,
    /// Number of failed tasks
    pub failed: usize,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Individual task results
    pub tasks: Vec<TaskOutput>,
    /// Timestamp
    pub timestamp: String,
}

impl ParallelOutput {
    pub fn new() -> Self {
        Self {
            total_tasks: 0,
            successful: 0,
            failed: 0,
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            tasks: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn add_task(&mut self, task: TaskOutput) {
        self.total_tasks += 1;
        if task.error.is_some() {
            self.failed += 1;
        } else {
            self.successful += 1;
        }
        self.total_cost_usd += task.cost_usd;
        self.tasks.push(task);
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.total_duration_ms = duration_ms;
        self
    }
}

impl Default for ParallelOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Workflow execution result for output formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowOutput {
    /// Workflow name
    pub name: String,
    /// Workflow ID
    pub workflow_id: String,
    /// Final status
    pub status: String,
    /// Total steps
    pub total_steps: usize,
    /// Completed steps
    pub completed_steps: usize,
    /// Failed steps
    pub failed_steps: usize,
    /// Skipped steps
    pub skipped_steps: usize,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Step results
    pub steps: Vec<StepOutput>,
    /// Timestamp
    pub timestamp: String,
}

impl WorkflowOutput {
    pub fn new(name: impl Into<String>, workflow_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            workflow_id: workflow_id.into(),
            status: "unknown".to_string(),
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            skipped_steps: 0,
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            steps: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn add_step(&mut self, step: StepOutput) {
        self.total_steps += 1;
        match step.status.as_str() {
            "completed" => self.completed_steps += 1,
            "failed" => self.failed_steps += 1,
            "skipped" => self.skipped_steps += 1,
            _ => {}
        }
        self.total_cost_usd += step.cost_usd;
        self.steps.push(step);
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.total_duration_ms = duration_ms;
        self
    }
}

/// Workflow step result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    /// Step name
    pub name: String,
    /// Step status
    pub status: String,
    /// Cost in USD
    pub cost_usd: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl StepOutput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "pending".to_string(),
            cost_usd: 0.0,
            duration_ms: 0,
            error: None,
        }
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = cost_usd;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

/// Cost summary for output formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostOutput {
    /// Today's cost
    pub today_usd: f64,
    /// This month's cost
    pub month_usd: f64,
    /// All-time cost
    pub total_usd: f64,
    /// Total tasks
    pub total_tasks: u64,
    /// Cost by model
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub by_model: HashMap<String, f64>,
    /// Daily breakdown
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub daily: Vec<DailyCostOutput>,
}

/// Daily cost breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCostOutput {
    pub date: String,
    pub cost_usd: f64,
    pub tasks: u64,
}

impl CostOutput {
    pub fn new() -> Self {
        Self {
            today_usd: 0.0,
            month_usd: 0.0,
            total_usd: 0.0,
            total_tasks: 0,
            by_model: HashMap::new(),
            daily: Vec::new(),
        }
    }

    pub fn with_today(mut self, cost: f64) -> Self {
        self.today_usd = cost;
        self
    }

    pub fn with_month(mut self, cost: f64) -> Self {
        self.month_usd = cost;
        self
    }

    pub fn with_total(mut self, cost: f64) -> Self {
        self.total_usd = cost;
        self
    }

    pub fn with_task_count(mut self, count: u64) -> Self {
        self.total_tasks = count;
        self
    }

    pub fn with_tokens(mut self, _input: u64, _output: u64) -> Self {
        // Note: CostOutput doesn't currently track tokens directly
        // This method is a placeholder for future extension
        self
    }
}

impl Default for CostOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Output formatter trait
pub trait Formatter {
    fn format_task(&self, output: &TaskOutput) -> Result<String>;
    fn format_parallel(&self, output: &ParallelOutput) -> Result<String>;
    fn format_workflow(&self, output: &WorkflowOutput) -> Result<String>;
    fn format_cost(&self, output: &CostOutput) -> Result<String>;
}

/// JSON formatter
pub struct JsonFormatter {
    pretty: bool,
}

impl JsonFormatter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl Formatter for JsonFormatter {
    fn format_task(&self, output: &TaskOutput) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(output)?)
        } else {
            Ok(serde_json::to_string(output)?)
        }
    }

    fn format_parallel(&self, output: &ParallelOutput) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(output)?)
        } else {
            Ok(serde_json::to_string(output)?)
        }
    }

    fn format_workflow(&self, output: &WorkflowOutput) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(output)?)
        } else {
            Ok(serde_json::to_string(output)?)
        }
    }

    fn format_cost(&self, output: &CostOutput) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(output)?)
        } else {
            Ok(serde_json::to_string(output)?)
        }
    }
}

/// YAML formatter
pub struct YamlFormatter;

impl Formatter for YamlFormatter {
    fn format_task(&self, output: &TaskOutput) -> Result<String> {
        Ok(serde_yaml::to_string(output)?)
    }

    fn format_parallel(&self, output: &ParallelOutput) -> Result<String> {
        Ok(serde_yaml::to_string(output)?)
    }

    fn format_workflow(&self, output: &WorkflowOutput) -> Result<String> {
        Ok(serde_yaml::to_string(output)?)
    }

    fn format_cost(&self, output: &CostOutput) -> Result<String> {
        Ok(serde_yaml::to_string(output)?)
    }
}

/// Markdown formatter
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    fn format_duration(ms: u64) -> String {
        if ms < 1000 {
            format!("{}ms", ms)
        } else if ms < 60_000 {
            format!("{:.1}s", ms as f64 / 1000.0)
        } else {
            let mins = ms / 60_000;
            let secs = (ms % 60_000) / 1000;
            format!("{}m {}s", mins, secs)
        }
    }

    fn format_cost(cost: f64) -> String {
        if cost < 0.01 {
            format!("${:.4}", cost)
        } else {
            format!("${:.2}", cost)
        }
    }

    fn status_emoji(status: &str) -> &'static str {
        match status.to_lowercase().as_str() {
            "completed" | "success" => "✅",
            "failed" | "error" => "❌",
            "running" | "in_progress" => "🔄",
            "skipped" => "⏭️",
            "pending" => "⏳",
            "max_iterations" | "max_iterations_reached" => "⚠️",
            "budget_exceeded" => "💸",
            _ => "❓",
        }
    }
}

impl Formatter for MarkdownFormatter {
    fn format_task(&self, output: &TaskOutput) -> Result<String> {
        let mut md = String::new();

        md.push_str(&format!("# Task: {}\n\n", output.task_id));
        md.push_str(&format!(
            "**Status:** {} {}\n\n",
            Self::status_emoji(&output.status),
            output.status
        ));

        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Model | {} |\n", output.model));
        md.push_str(&format!("| Iterations | {} |\n", output.iterations));
        md.push_str(&format!("| Cost | {} |\n", Self::format_cost(output.cost_usd)));
        md.push_str(&format!(
            "| Duration | {} |\n",
            Self::format_duration(output.duration_ms)
        ));
        md.push_str(&format!("| Input Tokens | {} |\n", output.input_tokens));
        md.push_str(&format!("| Output Tokens | {} |\n", output.output_tokens));
        md.push_str(&format!("| Timestamp | {} |\n", output.timestamp));

        md.push_str("\n## Prompt\n\n");
        md.push_str("```\n");
        md.push_str(&output.prompt);
        md.push_str("\n```\n");

        if let Some(ref error) = output.error {
            md.push_str("\n## Error\n\n");
            md.push_str("```\n");
            md.push_str(error);
            md.push_str("\n```\n");
        }

        if let Some(ref out) = output.output {
            md.push_str("\n## Output\n\n");
            md.push_str(out);
            md.push_str("\n");
        }

        Ok(md)
    }

    fn format_parallel(&self, output: &ParallelOutput) -> Result<String> {
        let mut md = String::new();

        md.push_str("# Parallel Execution Results\n\n");

        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Total Tasks | {} |\n", output.total_tasks));
        md.push_str(&format!("| Successful | {} ✅ |\n", output.successful));
        md.push_str(&format!("| Failed | {} ❌ |\n", output.failed));
        md.push_str(&format!(
            "| Total Cost | {} |\n",
            Self::format_cost(output.total_cost_usd)
        ));
        md.push_str(&format!(
            "| Total Duration | {} |\n",
            Self::format_duration(output.total_duration_ms)
        ));

        md.push_str("\n## Tasks\n\n");
        md.push_str("| Task | Status | Iterations | Cost | Duration |\n");
        md.push_str("|------|--------|------------|------|----------|\n");

        for task in &output.tasks {
            md.push_str(&format!(
                "| {} | {} {} | {} | {} | {} |\n",
                task.task_id,
                Self::status_emoji(&task.status),
                task.status,
                task.iterations,
                Self::format_cost(task.cost_usd),
                Self::format_duration(task.duration_ms)
            ));
        }

        Ok(md)
    }

    fn format_workflow(&self, output: &WorkflowOutput) -> Result<String> {
        let mut md = String::new();

        md.push_str(&format!("# Workflow: {}\n\n", output.name));
        md.push_str(&format!(
            "**Status:** {} {}\n\n",
            Self::status_emoji(&output.status),
            output.status
        ));

        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Workflow ID | {} |\n", output.workflow_id));
        md.push_str(&format!("| Total Steps | {} |\n", output.total_steps));
        md.push_str(&format!("| Completed | {} ✅ |\n", output.completed_steps));
        md.push_str(&format!("| Failed | {} ❌ |\n", output.failed_steps));
        md.push_str(&format!("| Skipped | {} ⏭️ |\n", output.skipped_steps));
        md.push_str(&format!(
            "| Total Cost | {} |\n",
            Self::format_cost(output.total_cost_usd)
        ));
        md.push_str(&format!(
            "| Total Duration | {} |\n",
            Self::format_duration(output.total_duration_ms)
        ));

        md.push_str("\n## Steps\n\n");
        md.push_str("| Step | Status | Cost | Duration |\n");
        md.push_str("|------|--------|------|----------|\n");

        for step in &output.steps {
            let error_note = if step.error.is_some() { " ⚠️" } else { "" };
            md.push_str(&format!(
                "| {} | {} {}{} | {} | {} |\n",
                step.name,
                Self::status_emoji(&step.status),
                step.status,
                error_note,
                Self::format_cost(step.cost_usd),
                Self::format_duration(step.duration_ms)
            ));
        }

        Ok(md)
    }

    fn format_cost(&self, output: &CostOutput) -> Result<String> {
        let mut md = String::new();

        md.push_str("# Cost Summary\n\n");

        md.push_str("## Overview\n\n");
        md.push_str("| Period | Cost |\n");
        md.push_str("|--------|------|\n");
        md.push_str(&format!("| Today | {} |\n", Self::format_cost(output.today_usd)));
        md.push_str(&format!(
            "| This Month | {} |\n",
            Self::format_cost(output.month_usd)
        ));
        md.push_str(&format!(
            "| All Time | {} |\n",
            Self::format_cost(output.total_usd)
        ));
        md.push_str(&format!("| Total Tasks | {} |\n", output.total_tasks));

        if !output.by_model.is_empty() {
            md.push_str("\n## Cost by Model\n\n");
            md.push_str("| Model | Cost |\n");
            md.push_str("|-------|------|\n");
            for (model, cost) in &output.by_model {
                md.push_str(&format!("| {} | {} |\n", model, Self::format_cost(*cost)));
            }
        }

        if !output.daily.is_empty() {
            md.push_str("\n## Daily Breakdown\n\n");
            md.push_str("| Date | Cost | Tasks |\n");
            md.push_str("|------|------|-------|\n");
            for day in &output.daily {
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    day.date,
                    Self::format_cost(day.cost_usd),
                    day.tasks
                ));
            }
        }

        Ok(md)
    }
}

/// Text formatter (plain text)
pub struct TextFormatter;

impl Formatter for TextFormatter {
    fn format_task(&self, output: &TaskOutput) -> Result<String> {
        let mut text = String::new();

        text.push_str(&format!("Task: {}\n", output.task_id));
        text.push_str(&format!("Status: {}\n", output.status));
        text.push_str(&format!("Model: {}\n", output.model));
        text.push_str(&format!("Iterations: {}\n", output.iterations));
        text.push_str(&format!("Cost: ${:.4}\n", output.cost_usd));
        text.push_str(&format!("Duration: {}ms\n", output.duration_ms));
        text.push_str(&format!(
            "Tokens: {} in / {} out\n",
            output.input_tokens, output.output_tokens
        ));

        if let Some(ref error) = output.error {
            text.push_str(&format!("Error: {}\n", error));
        }

        Ok(text)
    }

    fn format_parallel(&self, output: &ParallelOutput) -> Result<String> {
        let mut text = String::new();

        text.push_str("Parallel Execution Results\n");
        text.push_str(&format!("Total: {} tasks\n", output.total_tasks));
        text.push_str(&format!("Successful: {}\n", output.successful));
        text.push_str(&format!("Failed: {}\n", output.failed));
        text.push_str(&format!("Total Cost: ${:.4}\n", output.total_cost_usd));
        text.push_str(&format!("Duration: {}ms\n", output.total_duration_ms));

        Ok(text)
    }

    fn format_workflow(&self, output: &WorkflowOutput) -> Result<String> {
        let mut text = String::new();

        text.push_str(&format!("Workflow: {}\n", output.name));
        text.push_str(&format!("Status: {}\n", output.status));
        text.push_str(&format!(
            "Steps: {}/{} completed\n",
            output.completed_steps, output.total_steps
        ));
        text.push_str(&format!("Total Cost: ${:.4}\n", output.total_cost_usd));
        text.push_str(&format!("Duration: {}ms\n", output.total_duration_ms));

        Ok(text)
    }

    fn format_cost(&self, output: &CostOutput) -> Result<String> {
        let mut text = String::new();

        text.push_str("Cost Summary\n");
        text.push_str(&format!("Today: ${:.4}\n", output.today_usd));
        text.push_str(&format!("This Month: ${:.4}\n", output.month_usd));
        text.push_str(&format!("All Time: ${:.4}\n", output.total_usd));
        text.push_str(&format!("Total Tasks: {}\n", output.total_tasks));

        Ok(text)
    }
}

/// Output writer that handles formatting and writing
pub struct OutputWriter {
    format: OutputFormat,
    output_file: Option<String>,
}

impl OutputWriter {
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            output_file: None,
        }
    }

    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.output_file = Some(path.into());
        self
    }

    fn get_formatter(&self) -> Box<dyn Formatter> {
        match self.format {
            OutputFormat::Text => Box::new(TextFormatter),
            OutputFormat::Json => Box::new(JsonFormatter::new(false)),
            OutputFormat::JsonPretty => Box::new(JsonFormatter::new(true)),
            OutputFormat::Yaml => Box::new(YamlFormatter),
            OutputFormat::Markdown => Box::new(MarkdownFormatter),
        }
    }

    fn write_output(&self, content: &str) -> Result<()> {
        if let Some(ref path) = self.output_file {
            let mut file = std::fs::File::create(path)?;
            file.write_all(content.as_bytes())?;
            tracing::info!("Output written to: {}", path);
        } else {
            println!("{}", content);
        }
        Ok(())
    }

    pub fn write_task(&self, output: &TaskOutput) -> Result<()> {
        let formatter = self.get_formatter();
        let content = formatter.format_task(output)?;
        self.write_output(&content)
    }

    pub fn write_parallel(&self, output: &ParallelOutput) -> Result<()> {
        let formatter = self.get_formatter();
        let content = formatter.format_parallel(output)?;
        self.write_output(&content)
    }

    pub fn write_workflow(&self, output: &WorkflowOutput) -> Result<()> {
        let formatter = self.get_formatter();
        let content = formatter.format_workflow(output)?;
        self.write_output(&content)
    }

    pub fn write_cost(&self, output: &CostOutput) -> Result<()> {
        let formatter = self.get_formatter();
        let content = formatter.format_cost(output)?;
        self.write_output(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_str() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("yaml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!("yml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!(
            "markdown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!("md".parse::<OutputFormat>().unwrap(), OutputFormat::Markdown);
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!(
            "json-pretty".parse::<OutputFormat>().unwrap(),
            OutputFormat::JsonPretty
        );
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_task_output_builder() {
        let output = TaskOutput::new("task-123", "Test prompt")
            .with_model("opus")
            .with_status("completed")
            .with_iterations(5)
            .with_cost(0.05)
            .with_duration(10000)
            .with_tokens(1000, 500);

        assert_eq!(output.task_id, "task-123");
        assert_eq!(output.model, "opus");
        assert_eq!(output.status, "completed");
        assert_eq!(output.iterations, 5);
        assert_eq!(output.cost_usd, 0.05);
    }

    #[test]
    fn test_parallel_output() {
        let mut parallel = ParallelOutput::new();

        parallel.add_task(
            TaskOutput::new("task-1", "Prompt 1")
                .with_status("completed")
                .with_cost(0.01),
        );
        parallel.add_task(
            TaskOutput::new("task-2", "Prompt 2")
                .with_status("failed")
                .with_error("Some error")
                .with_cost(0.02),
        );

        assert_eq!(parallel.total_tasks, 2);
        assert_eq!(parallel.successful, 1);
        assert_eq!(parallel.failed, 1);
        assert_eq!(parallel.total_cost_usd, 0.03);
    }

    #[test]
    fn test_json_formatter() {
        let output = TaskOutput::new("task-123", "Test")
            .with_status("completed")
            .with_cost(0.05);

        let formatter = JsonFormatter::new(false);
        let json = formatter.format_task(&output).unwrap();

        assert!(json.contains("task-123"));
        assert!(json.contains("completed"));
    }

    #[test]
    fn test_yaml_formatter() {
        let output = TaskOutput::new("task-123", "Test")
            .with_status("completed");

        let formatter = YamlFormatter;
        let yaml = formatter.format_task(&output).unwrap();

        assert!(yaml.contains("task_id: task-123"));
        assert!(yaml.contains("status: completed"));
    }

    #[test]
    fn test_markdown_formatter() {
        let output = TaskOutput::new("task-123", "Test prompt")
            .with_status("completed")
            .with_iterations(5)
            .with_cost(0.05);

        let formatter = MarkdownFormatter;
        let md = formatter.format_task(&output).unwrap();

        assert!(md.contains("# Task: task-123"));
        assert!(md.contains("✅"));
        assert!(md.contains("| Iterations | 5 |"));
    }

    #[test]
    fn test_markdown_duration_format() {
        assert_eq!(MarkdownFormatter::format_duration(500), "500ms");
        assert_eq!(MarkdownFormatter::format_duration(1500), "1.5s");
        assert_eq!(MarkdownFormatter::format_duration(65000), "1m 5s");
    }

    #[test]
    fn test_output_writer() {
        let writer = OutputWriter::new(OutputFormat::Json);
        let output = TaskOutput::new("test", "prompt").with_status("completed");

        // Just test that it doesn't panic
        let formatter = writer.get_formatter();
        let result = formatter.format_task(&output);
        assert!(result.is_ok());
    }
}
