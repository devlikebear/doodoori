//! Event Broadcasting System for Live TUI
//!
//! Provides real-time event streaming for the dashboard using broadcast channels.

use tokio::sync::broadcast;

use crate::claude::ExecutionUsage;
use super::{LoopEvent, LoopStatus};

/// Fine-grained live events for real-time TUI updates
#[derive(Debug, Clone)]
pub enum LiveEvent {
    /// Wrapper for existing loop events
    Loop(LoopEvent),

    /// Token usage delta (incremental update)
    TokenDelta {
        input: u64,
        output: u64,
        cache_read: u64,
        cache_creation: u64,
    },

    /// Cost update with running total and delta
    CostUpdate {
        total_usd: f64,
        delta_usd: f64,
    },

    /// Text streaming from Claude response
    TextStream {
        text: String,
        is_complete: bool,
    },

    /// Tool execution started
    ToolStart {
        tool_name: String,
        tool_id: String,
    },

    /// Tool execution completed
    ToolEnd {
        tool_name: String,
        tool_id: String,
        success: bool,
        duration_ms: u64,
    },

    /// Iteration progress (percentage within current iteration)
    IterationProgress {
        iteration: u32,
        phase: IterationPhase,
    },

    /// Status change notification
    StatusChange {
        status: LiveStatus,
        message: Option<String>,
    },
}

/// Phases within a single iteration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IterationPhase {
    /// Starting iteration, building prompt
    Starting,
    /// Sending request to Claude
    Sending,
    /// Receiving response stream
    Receiving,
    /// Executing tools
    ExecutingTools,
    /// Processing results
    Processing,
    /// Iteration complete
    Complete,
}

impl std::fmt::Display for IterationPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Sending => write!(f, "Sending"),
            Self::Receiving => write!(f, "Receiving"),
            Self::ExecutingTools => write!(f, "Executing"),
            Self::Processing => write!(f, "Processing"),
            Self::Complete => write!(f, "Complete"),
        }
    }
}

/// Live status for real-time display
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveStatus {
    /// Initializing the loop
    Initializing,
    /// Running normally
    Running,
    /// Paused by user
    Paused,
    /// Waiting for user input/permission
    WaitingForInput,
    /// Completing successfully
    Completing,
    /// Encountered an error
    Error,
    /// Finished
    Finished(LoopStatus),
}

impl std::fmt::Display for LiveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "Initializing"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::WaitingForInput => write!(f, "Waiting"),
            Self::Completing => write!(f, "Completing"),
            Self::Error => write!(f, "Error"),
            Self::Finished(status) => write!(f, "Finished: {:?}", status),
        }
    }
}

/// Configuration for the event bus
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Channel capacity (number of events to buffer)
    pub capacity: usize,
    /// Whether to emit token delta events
    pub emit_token_deltas: bool,
    /// Whether to emit text stream events
    pub emit_text_stream: bool,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            capacity: 256,
            emit_token_deltas: true,
            emit_text_stream: true,
        }
    }
}

/// Event bus for broadcasting live events to multiple subscribers
pub struct EventBus {
    sender: broadcast::Sender<LiveEvent>,
    config: EventBusConfig,
    // Track statistics for summary
    stats: EventStats,
}

/// Statistics tracked by the event bus
#[derive(Debug, Clone, Default)]
pub struct EventStats {
    pub events_sent: u64,
    pub events_dropped: u64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub total_cost_usd: f64,
    pub tools_executed: u64,
    pub tools_succeeded: u64,
    pub tools_failed: u64,
}

impl EventBus {
    /// Create a new event bus with default configuration
    pub fn new() -> Self {
        Self::with_config(EventBusConfig::default())
    }

    /// Create a new event bus with custom configuration
    pub fn with_config(config: EventBusConfig) -> Self {
        let (sender, _) = broadcast::channel(config.capacity);
        Self {
            sender,
            config,
            stats: EventStats::default(),
        }
    }

    /// Subscribe to live events
    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.sender.subscribe()
    }

    /// Get the current number of subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Send an event to all subscribers
    pub fn send(&mut self, event: LiveEvent) {
        // Update statistics based on event type
        self.update_stats(&event);

        // Send to all subscribers
        match self.sender.send(event) {
            Ok(_) => {
                self.stats.events_sent += 1;
            }
            Err(_) => {
                // No subscribers, increment dropped count
                self.stats.events_dropped += 1;
            }
        }
    }

    /// Send a loop event (convenience method)
    pub fn send_loop_event(&mut self, event: LoopEvent) {
        self.send(LiveEvent::Loop(event));
    }

    /// Send token delta event
    pub fn send_token_delta(&mut self, input: u64, output: u64, cache_read: u64, cache_creation: u64) {
        if self.config.emit_token_deltas {
            self.send(LiveEvent::TokenDelta {
                input,
                output,
                cache_read,
                cache_creation,
            });
        }
    }

    /// Send cost update event
    pub fn send_cost_update(&mut self, total_usd: f64, delta_usd: f64) {
        self.send(LiveEvent::CostUpdate { total_usd, delta_usd });
    }

    /// Send text stream event
    pub fn send_text_stream(&mut self, text: String, is_complete: bool) {
        if self.config.emit_text_stream {
            self.send(LiveEvent::TextStream { text, is_complete });
        }
    }

    /// Send tool start event
    pub fn send_tool_start(&mut self, tool_name: String, tool_id: String) {
        self.send(LiveEvent::ToolStart { tool_name, tool_id });
    }

    /// Send tool end event
    pub fn send_tool_end(&mut self, tool_name: String, tool_id: String, success: bool, duration_ms: u64) {
        self.send(LiveEvent::ToolEnd {
            tool_name,
            tool_id,
            success,
            duration_ms,
        });
    }

    /// Send iteration progress event
    pub fn send_iteration_progress(&mut self, iteration: u32, phase: IterationPhase) {
        self.send(LiveEvent::IterationProgress { iteration, phase });
    }

    /// Send status change event
    pub fn send_status_change(&mut self, status: LiveStatus, message: Option<String>) {
        self.send(LiveEvent::StatusChange { status, message });
    }

    /// Get current statistics
    pub fn stats(&self) -> &EventStats {
        &self.stats
    }

    /// Update statistics based on event
    fn update_stats(&mut self, event: &LiveEvent) {
        match event {
            LiveEvent::TokenDelta { input, output, .. } => {
                self.stats.total_tokens_input += input;
                self.stats.total_tokens_output += output;
            }
            LiveEvent::CostUpdate { total_usd, .. } => {
                self.stats.total_cost_usd = *total_usd;
            }
            LiveEvent::ToolStart { .. } => {
                self.stats.tools_executed += 1;
            }
            LiveEvent::ToolEnd { success, .. } => {
                if *success {
                    self.stats.tools_succeeded += 1;
                } else {
                    self.stats.tools_failed += 1;
                }
            }
            _ => {}
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating an EventBus with custom configuration
pub struct EventBusBuilder {
    config: EventBusConfig,
}

impl EventBusBuilder {
    pub fn new() -> Self {
        Self {
            config: EventBusConfig::default(),
        }
    }

    pub fn capacity(mut self, capacity: usize) -> Self {
        self.config.capacity = capacity;
        self
    }

    pub fn emit_token_deltas(mut self, emit: bool) -> Self {
        self.config.emit_token_deltas = emit;
        self
    }

    pub fn emit_text_stream(mut self, emit: bool) -> Self {
        self.config.emit_text_stream = emit;
        self
    }

    pub fn build(self) -> EventBus {
        EventBus::with_config(self.config)
    }
}

impl Default for EventBusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of current execution state for UI
#[derive(Debug, Clone, Default)]
pub struct ExecutionSnapshot {
    /// Current iteration number
    pub iteration: u32,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Current phase within iteration
    pub phase: Option<IterationPhase>,
    /// Current status
    pub status: Option<LiveStatus>,
    /// Current usage totals
    pub usage: ExecutionUsage,
    /// Recent text from Claude (last N characters)
    pub recent_text: String,
    /// Currently executing tools
    pub active_tools: Vec<ActiveTool>,
    /// Recent tool executions
    pub tool_history: Vec<ToolExecution>,
    /// Token usage history (for sparkline)
    pub token_history: Vec<TokenSample>,
}

/// Information about a currently executing tool
#[derive(Debug, Clone)]
pub struct ActiveTool {
    pub name: String,
    pub id: String,
    pub start_time: std::time::Instant,
}

/// Record of a completed tool execution
#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Token usage sample for sparkline history
#[derive(Debug, Clone)]
pub struct TokenSample {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl ExecutionSnapshot {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            max_iterations,
            ..Default::default()
        }
    }

    /// Update snapshot from a live event
    pub fn update(&mut self, event: &LiveEvent) {
        match event {
            LiveEvent::Loop(loop_event) => {
                self.update_from_loop_event(loop_event);
            }
            LiveEvent::TokenDelta { input, output, cache_read, cache_creation } => {
                self.usage.input_tokens += input;
                self.usage.output_tokens += output;
                self.usage.cache_read_tokens += cache_read;
                self.usage.cache_creation_tokens += cache_creation;

                // Add to history
                self.token_history.push(TokenSample {
                    timestamp: chrono::Utc::now(),
                    input_tokens: *input,
                    output_tokens: *output,
                });

                // Keep only last 60 samples (for sparkline)
                if self.token_history.len() > 60 {
                    self.token_history.remove(0);
                }
            }
            LiveEvent::CostUpdate { total_usd, .. } => {
                self.usage.total_cost_usd = *total_usd;
            }
            LiveEvent::TextStream { text, is_complete } => {
                if *is_complete {
                    self.recent_text.clear();
                } else {
                    self.recent_text.push_str(text);
                    // Keep only last 500 characters
                    if self.recent_text.len() > 500 {
                        let start = self.recent_text.len() - 500;
                        self.recent_text = self.recent_text[start..].to_string();
                    }
                }
            }
            LiveEvent::ToolStart { tool_name, tool_id } => {
                self.active_tools.push(ActiveTool {
                    name: tool_name.clone(),
                    id: tool_id.clone(),
                    start_time: std::time::Instant::now(),
                });
            }
            LiveEvent::ToolEnd { tool_name, tool_id, success, duration_ms } => {
                // Remove from active tools
                self.active_tools.retain(|t| &t.id != tool_id);

                // Add to history
                self.tool_history.push(ToolExecution {
                    name: tool_name.clone(),
                    success: *success,
                    duration_ms: *duration_ms,
                    timestamp: chrono::Utc::now(),
                });

                // Keep only last 20 tool executions
                if self.tool_history.len() > 20 {
                    self.tool_history.remove(0);
                }
            }
            LiveEvent::IterationProgress { iteration, phase } => {
                self.iteration = *iteration;
                self.phase = Some(phase.clone());
            }
            LiveEvent::StatusChange { status, .. } => {
                self.status = Some(status.clone());
            }
        }
    }

    fn update_from_loop_event(&mut self, event: &LoopEvent) {
        match event {
            super::LoopEvent::IterationStarted { iteration } => {
                self.iteration = *iteration;
                self.phase = Some(IterationPhase::Starting);
            }
            super::LoopEvent::IterationCompleted { iteration, usage, .. } => {
                self.iteration = *iteration;
                self.phase = Some(IterationPhase::Complete);
                self.usage = usage.clone();
            }
            super::LoopEvent::LoopFinished { status, total_iterations, total_usage } => {
                self.iteration = *total_iterations;
                self.status = Some(LiveStatus::Finished(status.clone()));
                self.usage = total_usage.clone();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_creation() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_event_bus_subscribe() {
        let bus = EventBus::new();
        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_event_bus_send_no_subscribers() {
        let mut bus = EventBus::new();
        bus.send(LiveEvent::StatusChange {
            status: LiveStatus::Running,
            message: None,
        });
        // Should not panic, but event is dropped
        assert_eq!(bus.stats().events_dropped, 1);
    }

    #[test]
    fn test_event_bus_send_with_subscriber() {
        let mut bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.send(LiveEvent::StatusChange {
            status: LiveStatus::Running,
            message: Some("Test".to_string()),
        });

        let event = rx.try_recv().unwrap();
        match event {
            LiveEvent::StatusChange { status, message } => {
                assert_eq!(status, LiveStatus::Running);
                assert_eq!(message, Some("Test".to_string()));
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_event_bus_stats() {
        let mut bus = EventBus::new();
        let _rx = bus.subscribe();

        bus.send_token_delta(100, 50, 10, 5);
        bus.send_cost_update(0.05, 0.05);
        bus.send_tool_start("Read".to_string(), "tool-1".to_string());
        bus.send_tool_end("Read".to_string(), "tool-1".to_string(), true, 100);

        let stats = bus.stats();
        assert_eq!(stats.total_tokens_input, 100);
        assert_eq!(stats.total_tokens_output, 50);
        assert_eq!(stats.total_cost_usd, 0.05);
        assert_eq!(stats.tools_executed, 1);
        assert_eq!(stats.tools_succeeded, 1);
    }

    #[test]
    fn test_execution_snapshot_update() {
        let mut snapshot = ExecutionSnapshot::new(50);

        snapshot.update(&LiveEvent::TokenDelta {
            input: 100,
            output: 50,
            cache_read: 10,
            cache_creation: 5,
        });

        assert_eq!(snapshot.usage.input_tokens, 100);
        assert_eq!(snapshot.usage.output_tokens, 50);
        assert_eq!(snapshot.token_history.len(), 1);
    }

    #[test]
    fn test_execution_snapshot_text_truncation() {
        let mut snapshot = ExecutionSnapshot::new(50);

        // Add more than 500 characters
        let long_text = "a".repeat(600);
        snapshot.update(&LiveEvent::TextStream {
            text: long_text,
            is_complete: false,
        });

        assert_eq!(snapshot.recent_text.len(), 500);
    }

    #[test]
    fn test_iteration_phase_display() {
        assert_eq!(format!("{}", IterationPhase::Starting), "Starting");
        assert_eq!(format!("{}", IterationPhase::Receiving), "Receiving");
        assert_eq!(format!("{}", IterationPhase::ExecutingTools), "Executing");
    }

    #[test]
    fn test_live_status_display() {
        assert_eq!(format!("{}", LiveStatus::Running), "Running");
        assert_eq!(format!("{}", LiveStatus::Paused), "Paused");
    }

    #[test]
    fn test_event_bus_builder() {
        let bus = EventBusBuilder::new()
            .capacity(512)
            .emit_token_deltas(false)
            .emit_text_stream(true)
            .build();

        assert_eq!(bus.config.capacity, 512);
        assert!(!bus.config.emit_token_deltas);
        assert!(bus.config.emit_text_stream);
    }
}
