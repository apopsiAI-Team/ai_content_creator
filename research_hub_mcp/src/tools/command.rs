//! Command pattern implementation for unified tool execution interface
//!
//! This module provides a unified Command trait that all tools implement,
//! enabling consistent execution patterns, composition, and testability.

use crate::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, instrument, warn};

/// Execution context passed to commands with request metadata and configuration
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Unique request ID for tracing
    pub request_id: String,
    /// Request start time for performance tracking
    pub start_time: SystemTime,
    /// Optional user ID for auditing
    pub user_id: Option<String>,
    /// Command execution metadata
    pub metadata: HashMap<String, String>,
    /// Maximum execution time allowed
    pub timeout: Duration,
    /// Whether to enable verbose logging
    pub verbose: bool,
}

impl ExecutionContext {
    /// Create new execution context with generated request ID
    pub fn new() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            start_time: SystemTime::now(),
            user_id: None,
            metadata: HashMap::new(),
            timeout: Duration::from_secs(300), // 5 minute default timeout
            verbose: false,
        }
    }

    /// Create execution context with specific request ID
    pub fn with_request_id(request_id: String) -> Self {
        Self {
            request_id,
            start_time: SystemTime::now(),
            user_id: None,
            metadata: HashMap::new(),
            timeout: Duration::from_secs(300),
            verbose: false,
        }
    }

    /// Add metadata to the execution context
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set timeout for command execution
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Get elapsed time since context creation
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed().unwrap_or(Duration::ZERO)
    }

    /// Check if execution has timed out
    pub fn is_timed_out(&self) -> bool {
        self.elapsed() > self.timeout
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Command execution result with detailed outcome information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommandResult {
    /// Unique command execution ID
    pub execution_id: String,
    /// Command name that was executed
    pub command_name: String,
    /// Whether the command succeeded
    pub success: bool,
    /// Primary result data (tool-specific)
    pub data: serde_json::Value,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Optional error message if command failed
    pub error: Option<String>,
    /// Additional metadata about the execution
    pub metadata: HashMap<String, String>,
    /// Warning messages (non-fatal issues)
    pub warnings: Vec<String>,
}

impl CommandResult {
    /// Create successful command result
    pub fn success<T: Serialize>(
        execution_id: String,
        command_name: String,
        data: T,
        duration: Duration,
    ) -> Result<Self> {
        Ok(Self {
            execution_id,
            command_name,
            success: true,
            data: serde_json::to_value(data)
                .map_err(|e| crate::Error::Serialization(e.to_string()))?,
            duration_ms: duration.as_millis() as u64,
            error: None,
            metadata: HashMap::new(),
            warnings: Vec::new(),
        })
    }

    /// Create failed command result
    pub fn failure(
        execution_id: String,
        command_name: String,
        error: String,
        duration: Duration,
    ) -> Self {
        Self {
            execution_id,
            command_name,
            success: false,
            data: serde_json::Value::Null,
            duration_ms: duration.as_millis() as u64,
            error: Some(error),
            metadata: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// Add metadata to the result
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Add warning to the result
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Extract typed data from the result
    pub fn extract_data<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_value(self.data.clone())
            .map_err(|e| crate::Error::Serialization(e.to_string()))
    }
}

/// Unified command trait that all tools must implement
#[async_trait]
pub trait Command: Send + Sync + fmt::Debug {
    /// Get the command name (used for identification and logging)
    fn name(&self) -> &'static str;

    /// Get command description for help and documentation
    fn description(&self) -> &'static str;

    /// Get input schema for validation and documentation
    fn input_schema(&self) -> serde_json::Value;

    /// Get output schema for documentation
    fn output_schema(&self) -> serde_json::Value;

    /// Execute the command with typed input and context
    async fn execute(
        &self,
        input: serde_json::Value,
        context: ExecutionContext,
    ) -> Result<CommandResult>;

    /// Validate input before execution (default implementation checks against schema)
    async fn validate_input(&self, input: &serde_json::Value) -> Result<()> {
        // Basic validation - can be overridden by implementations
        if input.is_null() {
            return Err(crate::Error::InvalidInput {
                field: "input".to_string(),
                reason: "Input cannot be null".to_string(),
            });
        }
        Ok(())
    }

    /// Check if the command supports a specific feature
    fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "validation" => true,
            "timeout" => true,
            "metadata" => true,
            _ => false,
        }
    }

    /// Get estimated execution time for planning and timeouts
    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(30) // Default 30 second estimate
    }

    /// Check if command can run concurrently with others
    fn is_concurrent_safe(&self) -> bool {
        true // Most commands are safe to run concurrently
    }

    /// Get command as Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Command wrapper that adds instrumentation and error handling
#[derive(Debug)]
pub struct InstrumentedCommand<C: Command> {
    inner: C,
    enable_metrics: bool,
    enable_tracing: bool,
}

impl<C: Command> InstrumentedCommand<C> {
    /// Create new instrumented command wrapper
    pub fn new(command: C) -> Self {
        Self {
            inner: command,
            enable_metrics: true,
            enable_tracing: true,
        }
    }

    /// Disable metrics collection
    pub fn without_metrics(mut self) -> Self {
        self.enable_metrics = false;
        self
    }

    /// Disable tracing
    pub fn without_tracing(mut self) -> Self {
        self.enable_tracing = false;
        self
    }
}

#[async_trait]
impl<C: Command> Command for InstrumentedCommand<C> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn description(&self) -> &'static str {
        self.inner.description()
    }

    fn input_schema(&self) -> serde_json::Value {
        self.inner.input_schema()
    }

    fn output_schema(&self) -> serde_json::Value {
        self.inner.output_schema()
    }

    #[instrument(skip(self, input), fields(command = %self.name(), request_id = %context.request_id))]
    async fn execute(
        &self,
        input: serde_json::Value,
        context: ExecutionContext,
    ) -> Result<CommandResult> {
        let start_time = SystemTime::now();

        if self.enable_tracing {
            info!(
                "Executing command '{}' with request_id '{}'",
                self.name(),
                context.request_id
            );
        }

        // Validate input first
        if let Err(e) = self.validate_input(&input).await {
            let duration = start_time.elapsed().unwrap_or(Duration::ZERO);
            warn!("Command '{}' validation failed: {}", self.name(), e);
            return Ok(CommandResult::failure(
                context.request_id.clone(),
                self.name().to_string(),
                format!("Input validation failed: {e}"),
                duration,
            ));
        }

        // Check timeout before execution
        if context.is_timed_out() {
            let duration = start_time.elapsed().unwrap_or(Duration::ZERO);
            warn!("Command '{}' timed out before execution", self.name());
            return Ok(CommandResult::failure(
                context.request_id.clone(),
                self.name().to_string(),
                "Command timed out before execution".to_string(),
                duration,
            ));
        }

        // Execute the inner command
        let result = self.inner.execute(input, context.clone()).await;
        let duration = start_time.elapsed().unwrap_or(Duration::ZERO);

        match result {
            Ok(mut cmd_result) => {
                if self.enable_tracing {
                    info!(
                        "Command '{}' completed successfully in {:?}",
                        self.name(),
                        duration
                    );
                }

                // Add instrumentation metadata
                if self.enable_metrics {
                    cmd_result = cmd_result
                        .with_metadata("instrumented".to_string(), "true".to_string())
                        .with_metadata(
                            "total_duration_ms".to_string(),
                            duration.as_millis().to_string(),
                        );
                }

                Ok(cmd_result)
            }
            Err(e) => {
                warn!(
                    "Command '{}' failed after {:?}: {}",
                    self.name(),
                    duration,
                    e
                );
                Ok(CommandResult::failure(
                    context.request_id,
                    self.name().to_string(),
                    e.to_string(),
                    duration,
                ))
            }
        }
    }

    async fn validate_input(&self, input: &serde_json::Value) -> Result<()> {
        self.inner.validate_input(input).await
    }

    fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "instrumentation" => true,
            "metrics" => self.enable_metrics,
            "tracing" => self.enable_tracing,
            _ => self.inner.supports_feature(feature),
        }
    }

    fn estimated_duration(&self) -> Duration {
        self.inner.estimated_duration()
    }

    fn is_concurrent_safe(&self) -> bool {
        self.inner.is_concurrent_safe()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Command executor that can run multiple commands
#[derive(Debug, Clone)]
pub struct CommandExecutor {
    commands: HashMap<String, Arc<dyn Command>>,
    default_timeout: Duration,
    max_concurrent: usize,
}

impl CommandExecutor {
    /// Create new command executor
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            default_timeout: Duration::from_secs(300),
            max_concurrent: 10,
        }
    }

    /// Register a command with the executor
    pub fn register_command<C: Command + 'static>(&mut self, command: C) -> &mut Self {
        let name = command.name().to_string();
        self.commands.insert(name, Arc::new(command));
        self
    }

    /// Register an instrumented command
    pub fn register_instrumented<C: Command + 'static>(&mut self, command: C) -> &mut Self {
        let instrumented = InstrumentedCommand::new(command);
        self.register_command(instrumented)
    }

    /// Execute a command by name
    #[instrument(skip(self, input), fields(command = %command_name))]
    pub async fn execute_command(
        &self,
        command_name: &str,
        input: serde_json::Value,
        context: Option<ExecutionContext>,
    ) -> Result<CommandResult> {
        let context = context.unwrap_or_else(ExecutionContext::new);

        let command =
            self.commands
                .get(command_name)
                .ok_or_else(|| crate::Error::InvalidInput {
                    field: "command_name".to_string(),
                    reason: format!("Unknown command: {command_name}"),
                })?;

        // Apply default timeout if none specified
        let context = if context.timeout == Duration::from_secs(300) {
            context.with_timeout(self.default_timeout)
        } else {
            context
        };

        debug!(
            "Executing command '{}' with timeout {:?}",
            command_name, context.timeout
        );

        command.execute(input, context).await
    }

    /// Get list of registered commands
    pub fn list_commands(&self) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .map(|(name, command)| CommandInfo {
                name: name.clone(),
                description: command.description().to_string(),
                estimated_duration: command.estimated_duration().as_millis() as u64,
                is_concurrent_safe: command.is_concurrent_safe(),
                supported_features: self.get_supported_features(command.as_ref()),
            })
            .collect()
    }

    /// Get supported features for a command
    fn get_supported_features(&self, command: &dyn Command) -> Vec<String> {
        let features = [
            "validation",
            "timeout",
            "metadata",
            "instrumentation",
            "metrics",
            "tracing",
        ];
        features
            .iter()
            .filter(|&&feature| command.supports_feature(feature))
            .map(|&feature| feature.to_string())
            .collect()
    }

    /// Set default timeout for commands
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set maximum concurrent commands
    pub fn with_max_concurrent(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent = max_concurrent;
        self
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a registered command
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommandInfo {
    /// Command name
    pub name: String,
    /// Command description
    pub description: String,
    /// Estimated execution duration in milliseconds
    #[serde(rename = "estimated_duration_ms")]
    pub estimated_duration: u64,
    /// Whether command is safe to run concurrently
    pub is_concurrent_safe: bool,
    /// List of supported features
    pub supported_features: Vec<String>,
}

/// Serde support for Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

/// Command composition helpers for chaining commands
pub mod composition {
    use super::*;
    use futures::future::try_join_all;

    /// Execute multiple commands in parallel
    pub async fn execute_parallel(
        executor: &CommandExecutor,
        commands: Vec<(&str, serde_json::Value)>,
        context: Option<ExecutionContext>,
    ) -> Result<Vec<CommandResult>> {
        let base_context = context.unwrap_or_else(ExecutionContext::new);

        let futures: Vec<_> = commands
            .into_iter()
            .enumerate()
            .map(|(i, (name, input))| {
                let ctx = base_context
                    .clone()
                    .with_metadata("parallel_index".to_string(), i.to_string());
                executor.execute_command(name, input, Some(ctx))
            })
            .collect();

        try_join_all(futures).await
    }

    /// Execute commands in sequence, passing results between them
    pub async fn execute_pipeline(
        executor: &CommandExecutor,
        pipeline: Vec<PipelineStage>,
        initial_context: Option<ExecutionContext>,
    ) -> Result<Vec<CommandResult>> {
        let mut results = Vec::new();
        let mut context = initial_context.unwrap_or_else(ExecutionContext::new);

        for (index, stage) in pipeline.into_iter().enumerate() {
            // Update context for each stage
            context = context.with_metadata("pipeline_stage".to_string(), index.to_string());

            let input = match stage.input_source {
                InputSource::Static(value) => value,
                InputSource::FromPrevious(result_index) => {
                    if result_index >= results.len() {
                        return Err(crate::Error::InvalidInput {
                            field: "input_source".to_string(),
                            reason: format!("Invalid result index: {result_index}"),
                        });
                    }
                    results[result_index].data.clone()
                }
            };

            let result = executor
                .execute_command(&stage.command_name, input, Some(context.clone()))
                .await?;

            results.push(result);
        }

        Ok(results)
    }

    /// Pipeline stage definition
    #[derive(Debug, Clone)]
    pub struct PipelineStage {
        pub command_name: String,
        pub input_source: InputSource,
    }

    /// Source of input for a pipeline stage
    #[derive(Debug, Clone)]
    pub enum InputSource {
        /// Static input value
        Static(serde_json::Value),
        /// Use output from previous command at given index
        FromPrevious(usize),
    }

    impl PipelineStage {
        /// Create pipeline stage with static input
        pub fn with_static_input(command_name: String, input: serde_json::Value) -> Self {
            Self {
                command_name,
                input_source: InputSource::Static(input),
            }
        }

        /// Create pipeline stage that uses output from previous command
        pub fn with_previous_output(command_name: String, previous_index: usize) -> Self {
            Self {
                command_name,
                input_source: InputSource::FromPrevious(previous_index),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    #[derive(Debug)]
    struct TestCommand {
        name: &'static str,
        should_fail: bool,
        duration: Duration,
    }

    #[async_trait]
    impl Command for TestCommand {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            "Test command for unit tests"
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "value": {"type": "string"}
                },
                "required": ["value"]
            })
        }

        fn output_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "result": {"type": "string"}
                }
            })
        }

        async fn execute(
            &self,
            input: serde_json::Value,
            context: ExecutionContext,
        ) -> Result<CommandResult> {
            // Simulate work
            tokio::time::sleep(self.duration).await;

            if self.should_fail {
                return Err(crate::Error::Service("Test failure".to_string()));
            }

            let value = input
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("default");

            CommandResult::success(
                context.request_id,
                self.name.to_string(),
                json!({"result": format!("processed: {value}")}),
                context.elapsed(),
            )
        }

        fn estimated_duration(&self) -> Duration {
            self.duration
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[tokio::test]
    async fn test_command_execution() {
        let command = TestCommand {
            name: "test_cmd",
            should_fail: false,
            duration: Duration::from_millis(10),
        };

        let context = ExecutionContext::new();
        let input = json!({"value": "hello"});

        let result = command.execute(input, context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.command_name, "test_cmd");
        assert!(result.duration_ms > 0);

        let data: serde_json::Value = result.extract_data().unwrap();
        assert_eq!(data["result"], "processed: hello");
    }

    #[tokio::test]
    async fn test_command_failure() {
        let command = TestCommand {
            name: "failing_cmd",
            should_fail: true,
            duration: Duration::from_millis(10),
        };

        let context = ExecutionContext::new();
        let input = json!({"value": "hello"});

        let result = command.execute(input, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_instrumented_command() {
        let base_command = TestCommand {
            name: "base_cmd",
            should_fail: false,
            duration: Duration::from_millis(10),
        };

        let instrumented = InstrumentedCommand::new(base_command);
        let context = ExecutionContext::new();
        let input = json!({"value": "test"});

        let result = instrumented.execute(input, context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.command_name, "base_cmd");
        assert!(result.metadata.contains_key("instrumented"));
        assert!(result.metadata.contains_key("total_duration_ms"));
    }

    #[tokio::test]
    async fn test_command_executor() {
        let mut executor = CommandExecutor::new();

        let cmd1 = TestCommand {
            name: "cmd1",
            should_fail: false,
            duration: Duration::from_millis(10),
        };

        let cmd2 = TestCommand {
            name: "cmd2",
            should_fail: false,
            duration: Duration::from_millis(10),
        };

        executor.register_command(cmd1);
        executor.register_instrumented(cmd2);

        // Test command execution
        let result = executor
            .execute_command("cmd1", json!({"value": "test"}), None)
            .await
            .unwrap();

        assert!(result.success);

        // Test listing commands
        let commands = executor.list_commands();
        assert_eq!(commands.len(), 2);
        assert!(commands.iter().any(|c| c.name == "cmd1"));
        assert!(commands.iter().any(|c| c.name == "cmd2"));

        // Test unknown command
        let result = executor.execute_command("unknown", json!({}), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        use composition::*;

        let mut executor = CommandExecutor::new();

        executor.register_command(TestCommand {
            name: "parallel1",
            should_fail: false,
            duration: Duration::from_millis(50),
        });

        executor.register_command(TestCommand {
            name: "parallel2",
            should_fail: false,
            duration: Duration::from_millis(50),
        });

        let commands = vec![
            ("parallel1", json!({"value": "first"})),
            ("parallel2", json!({"value": "second"})),
        ];

        let start = SystemTime::now();
        let results = execute_parallel(&executor, commands, None).await.unwrap();
        let duration = start.elapsed().unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
        // Should complete in roughly the time of the longest command (not sum)
        assert!(duration < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        use composition::*;

        let mut executor = CommandExecutor::new();

        executor.register_command(TestCommand {
            name: "stage1",
            should_fail: false,
            duration: Duration::from_millis(10),
        });

        executor.register_command(TestCommand {
            name: "stage2",
            should_fail: false,
            duration: Duration::from_millis(10),
        });

        let pipeline = vec![
            PipelineStage::with_static_input("stage1".to_string(), json!({"value": "initial"})),
            PipelineStage::with_previous_output("stage2".to_string(), 0),
        ];

        let results = execute_pipeline(&executor, pipeline, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_execution_context() {
        let mut context = ExecutionContext::new()
            .with_metadata("test_key".to_string(), "test_value".to_string())
            .with_timeout(Duration::from_secs(1))
            .with_verbose(true);

        assert_eq!(
            context.metadata.get("test_key"),
            Some(&"test_value".to_string())
        );
        assert_eq!(context.timeout, Duration::from_secs(1));
        assert!(context.verbose);

        // Test timeout check
        context.timeout = Duration::from_millis(1);
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(context.is_timed_out());
    }
}
