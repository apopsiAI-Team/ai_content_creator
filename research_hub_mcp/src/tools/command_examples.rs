//! Command pattern composition examples demonstrating the unified tool execution interface
//!
//! This module provides examples of how to use the command pattern for:
//! - Sequential command execution (pipelines)
//! - Parallel command execution
//! - Command composition and chaining
//! - Error handling and recovery
//! - Performance monitoring and instrumentation

use crate::tools::command::composition::{execute_parallel, execute_pipeline, PipelineStage};
use crate::tools::command::{Command, CommandExecutor, CommandResult, ExecutionContext};
use crate::tools::{DownloadTool, SearchTool};
use crate::{Config, Result};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

/// Comprehensive example of command pattern usage
pub struct CommandPatternDemo {
    executor: CommandExecutor,
    config: Arc<Config>,
}

impl CommandPatternDemo {
    /// Create new command pattern demo with registered tools
    pub fn new(config: Arc<Config>) -> Result<Self> {
        let mut executor = CommandExecutor::new()
            .with_default_timeout(Duration::from_secs(120))
            .with_max_concurrent(5);

        // Create and register search tool
        let search_tool = SearchTool::new(config.clone())?;
        executor.register_instrumented(search_tool);

        // Create and register download tool
        let meta_config = crate::client::MetaSearchConfig::from_config(&config);
        let meta_client = Arc::new(crate::client::MetaSearchClient::new(
            (*config).clone(),
            meta_config,
        )?);
        let download_tool = DownloadTool::new(meta_client, config.clone())?;
        executor.register_instrumented(download_tool);

        Ok(Self { executor, config })
    }

    /// Example 1: Basic single command execution
    pub async fn example_basic_execution(&self) -> Result<()> {
        info!("🔍 Example 1: Basic command execution");

        let context = ExecutionContext::new()
            .with_metadata("example".to_string(), "basic_execution".to_string())
            .with_verbose(true);

        let search_input = json!({
            "query": "machine learning neural networks",
            "search_type": "auto",
            "limit": 5
        });

        let result = self
            .executor
            .execute_command("search_papers", search_input, Some(context))
            .await?;

        info!(
            "✅ Search completed: {} papers found in {}ms",
            result
                .extract_data::<crate::tools::search::SearchResult>()?
                .returned_count,
            result.duration_ms
        );

        Ok(())
    }

    /// Example 2: Pipeline execution - search then download
    pub async fn example_search_and_download_pipeline(&self) -> Result<()> {
        info!("🔗 Example 2: Search and download pipeline");

        // Create a pipeline that searches and then downloads the first result
        let pipeline = vec![
            PipelineStage::with_static_input(
                "search_papers".to_string(),
                json!({
                    "query": "10.1038/nature25778", // Specific DOI for consistent results
                    "search_type": "doi",
                    "limit": 1
                }),
            ),
            PipelineStage::with_static_input(
                "download_paper".to_string(),
                json!({
                    "doi": "10.1038/nature25778",
                    "verify_integrity": true,
                    "overwrite": false
                }),
            ),
        ];

        let context = ExecutionContext::new()
            .with_metadata("example".to_string(), "pipeline".to_string())
            .with_timeout(Duration::from_secs(180)); // Longer timeout for downloads

        let results = execute_pipeline(&self.executor, pipeline, Some(context)).await?;

        info!("✅ Pipeline completed with {} stages", results.len());
        for (i, result) in results.iter().enumerate() {
            info!(
                "  Stage {}: {} ({}ms)",
                i + 1,
                if result.success {
                    "✅ Success"
                } else {
                    "❌ Failed"
                },
                result.duration_ms
            );
        }

        Ok(())
    }

    /// Example 3: Parallel execution - multiple searches
    pub async fn example_parallel_searches(&self) -> Result<()> {
        info!("🚀 Example 3: Parallel command execution");

        let commands = vec![
            (
                "search_papers",
                json!({
                    "query": "quantum computing",
                    "search_type": "title",
                    "limit": 3
                }),
            ),
            (
                "search_papers",
                json!({
                    "query": "machine learning",
                    "search_type": "title",
                    "limit": 3
                }),
            ),
            (
                "search_papers",
                json!({
                    "query": "climate change",
                    "search_type": "title",
                    "limit": 3
                }),
            ),
        ];

        let context = ExecutionContext::new()
            .with_metadata("example".to_string(), "parallel_execution".to_string());

        let start_time = SystemTime::now();
        let results = execute_parallel(&self.executor, commands, Some(context)).await?;
        let total_duration = start_time.elapsed().unwrap_or(Duration::ZERO);

        info!("✅ Parallel execution completed in {:?}", total_duration);

        let mut total_papers = 0;
        for (i, result) in results.iter().enumerate() {
            if result.success {
                let search_result: crate::tools::search::SearchResult = result.extract_data()?;
                total_papers += search_result.returned_count;
                info!(
                    "  Search {}: Found {} papers in {}ms",
                    i + 1,
                    search_result.returned_count,
                    result.duration_ms
                );
            } else {
                warn!(
                    "  Search {}: Failed - {}",
                    i + 1,
                    result.error.as_deref().unwrap_or("Unknown error")
                );
            }
        }

        info!(
            "📊 Total papers found across all searches: {}",
            total_papers
        );
        Ok(())
    }

    /// Example 4: Error handling and recovery
    pub async fn example_error_handling(&self) -> Result<()> {
        info!("⚠️  Example 4: Error handling and recovery");

        // Try to download with an invalid DOI to demonstrate error handling
        let invalid_input = json!({
            "doi": "invalid-doi-format",
            "verify_integrity": true
        });

        let context = ExecutionContext::new()
            .with_metadata("example".to_string(), "error_handling".to_string());

        let result = self
            .executor
            .execute_command("download_paper", invalid_input, Some(context))
            .await?;

        if result.success {
            info!("🤔 Unexpected success with invalid input");
        } else {
            info!(
                "✅ Error handled gracefully: {}",
                result.error.as_deref().unwrap_or("Unknown error")
            );
        }

        // Now try with valid input to show recovery
        let valid_input = json!({
            "query": "machine learning",
            "search_type": "auto",
            "limit": 1
        });

        let recovery_context = ExecutionContext::new()
            .with_metadata("example".to_string(), "error_recovery".to_string());

        let recovery_result = self
            .executor
            .execute_command("search_papers", valid_input, Some(recovery_context))
            .await?;

        info!(
            "✅ Recovery successful: {} papers found",
            recovery_result
                .extract_data::<crate::tools::search::SearchResult>()?
                .returned_count
        );

        Ok(())
    }

    /// Example 5: Performance monitoring and instrumentation
    pub async fn example_performance_monitoring(&self) -> Result<()> {
        info!("📈 Example 5: Performance monitoring");

        let context = ExecutionContext::new()
            .with_metadata("example".to_string(), "performance_monitoring".to_string())
            .with_metadata("benchmark".to_string(), "true".to_string())
            .with_verbose(true);

        // Execute multiple searches with timing
        let search_queries = vec![
            "neural networks",
            "quantum mechanics",
            "climate modeling",
            "bioinformatics",
            "robotics",
        ];

        let mut performance_stats = Vec::new();

        for query in search_queries {
            let start = SystemTime::now();

            let result = self
                .executor
                .execute_command(
                    "search_papers",
                    json!({
                        "query": query,
                        "search_type": "auto",
                        "limit": 2
                    }),
                    Some(context.clone()),
                )
                .await?;

            let query_duration = start.elapsed().unwrap_or(Duration::ZERO);

            if result.success {
                let search_result: crate::tools::search::SearchResult = result.extract_data()?;
                performance_stats.push((
                    query.to_string(),
                    query_duration,
                    result.duration_ms,
                    search_result.returned_count,
                ));

                info!(
                    "  📊 Query '{}': {}ms (found {} papers)",
                    query, result.duration_ms, search_result.returned_count
                );
            } else {
                warn!(
                    "  ❌ Query '{}' failed: {}",
                    query,
                    result.error.as_deref().unwrap_or("Unknown")
                );
            }
        }

        // Calculate performance statistics
        if !performance_stats.is_empty() {
            let avg_duration: f64 = performance_stats
                .iter()
                .map(|(_, _, duration_ms, _)| *duration_ms as f64)
                .sum::<f64>()
                / performance_stats.len() as f64;

            let total_papers: u32 = performance_stats
                .iter()
                .map(|(_, _, _, count)| *count)
                .sum();

            info!("📈 Performance Summary:");
            info!("  • Average query time: {:.1}ms", avg_duration);
            info!("  • Total papers found: {}", total_papers);
            info!(
                "  • Successful queries: {}/{}",
                performance_stats.len(),
                search_queries.len()
            );
        }

        Ok(())
    }

    /// Example 6: Command introspection and metadata
    pub async fn example_command_introspection(&self) -> Result<()> {
        info!("🔍 Example 6: Command introspection");

        // List all available commands
        let commands = self.executor.list_commands();
        info!("📋 Available commands:");

        for cmd in &commands {
            info!("  • {} - {}", cmd.name, cmd.description);
            info!("    - Estimated duration: {:?}", cmd.estimated_duration);
            info!("    - Concurrent safe: {}", cmd.is_concurrent_safe);
            info!("    - Features: {}", cmd.supported_features.join(", "));
        }

        // Demonstrate feature checking
        for cmd in &commands {
            if cmd.supported_features.contains(&"validation".to_string()) {
                info!("✅ Command '{}' supports input validation", cmd.name);
            }
            if cmd
                .supported_features
                .contains(&"progress_tracking".to_string())
            {
                info!("📊 Command '{}' supports progress tracking", cmd.name);
            }
        }

        Ok(())
    }

    /// Run all examples in sequence
    pub async fn run_all_examples(&self) -> Result<()> {
        info!("🚀 Starting comprehensive command pattern demonstration");

        let examples = vec![
            ("Basic Execution", Self::example_basic_execution),
            (
                "Search and Download Pipeline",
                Self::example_search_and_download_pipeline,
            ),
            ("Parallel Searches", Self::example_parallel_searches),
            ("Error Handling", Self::example_error_handling),
            (
                "Performance Monitoring",
                Self::example_performance_monitoring,
            ),
            ("Command Introspection", Self::example_command_introspection),
        ];

        for (name, example_fn) in examples {
            info!("\n{}", "=".repeat(60));
            info!("Running example: {}", name);
            info!("{}", "=".repeat(60));

            match example_fn(self).await {
                Ok(()) => info!("✅ Example '{}' completed successfully\n", name),
                Err(e) => {
                    warn!("❌ Example '{}' failed: {}\n", name, e);
                    // Continue with other examples even if one fails
                }
            }
        }

        info!("🎉 All command pattern examples completed!");
        Ok(())
    }
}

/// Utility functions for command composition
pub mod utils {
    use super::*;

    /// Create a search-then-download workflow
    pub async fn search_and_download_workflow(
        executor: &CommandExecutor,
        query: &str,
        download_first_result: bool,
    ) -> Result<(CommandResult, Option<CommandResult>)> {
        // Execute search
        let search_result = executor
            .execute_command(
                "search_papers",
                json!({
                    "query": query,
                    "search_type": "auto",
                    "limit": if download_first_result { 1 } else { 5 }
                }),
                None,
            )
            .await?;

        if !search_result.success {
            return Ok((search_result, None));
        }

        // If requested, download the first result
        let download_result = if download_first_result {
            let search_data: crate::tools::search::SearchResult = search_result.extract_data()?;

            if let Some(first_paper) = search_data.papers.first() {
                let download_result = executor
                    .execute_command(
                        "download_paper",
                        json!({
                            "doi": first_paper.metadata.doi,
                            "verify_integrity": true,
                            "overwrite": false
                        }),
                        None,
                    )
                    .await?;
                Some(download_result)
            } else {
                None
            }
        } else {
            None
        };

        Ok((search_result, download_result))
    }

    /// Batch process multiple queries with rate limiting
    pub async fn batch_process_queries(
        executor: &CommandExecutor,
        queries: Vec<&str>,
        batch_size: usize,
        delay_between_batches: Duration,
    ) -> Result<Vec<CommandResult>> {
        let mut all_results = Vec::new();

        for batch in queries.chunks(batch_size) {
            info!("Processing batch of {} queries", batch.len());

            let batch_commands: Vec<_> = batch
                .iter()
                .map(|&query| {
                    (
                        "search_papers",
                        json!({
                            "query": query,
                            "search_type": "auto",
                            "limit": 3
                        }),
                    )
                })
                .collect();

            let batch_results = execute_parallel(executor, batch_commands, None).await?;
            all_results.extend(batch_results);

            if delay_between_batches > Duration::ZERO {
                tokio::time::sleep(delay_between_batches).await;
            }
        }

        Ok(all_results)
    }

    /// Monitor command execution with detailed metrics
    pub async fn monitor_command_execution<F, Fut>(
        command_name: &str,
        operation: F,
    ) -> Result<(CommandResult, Duration, usize)>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<CommandResult>>,
    {
        let start_time = SystemTime::now();
        let start_memory = get_memory_usage();

        info!("🚀 Starting monitored execution of '{}'", command_name);

        let result = operation().await?;

        let end_time = SystemTime::now();
        let end_memory = get_memory_usage();
        let duration = end_time
            .duration_since(start_time)
            .unwrap_or(Duration::ZERO);
        let memory_delta = end_memory.saturating_sub(start_memory);

        info!("📊 Execution metrics for '{}':", command_name);
        info!("  • Duration: {:?}", duration);
        info!("  • Memory delta: {} bytes", memory_delta);
        info!("  • Success: {}", result.success);

        Ok((result, duration, memory_delta))
    }

    /// Get current memory usage (simplified version)
    fn get_memory_usage() -> usize {
        // This is a simplified placeholder - in a real implementation,
        // you would use a proper memory monitoring library
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_command_pattern_demo_creation() {
        let config = Arc::new(Config::default());
        let demo = CommandPatternDemo::new(config);
        assert!(demo.is_ok());

        if let Ok(demo) = demo {
            let commands = demo.executor.list_commands();
            assert_eq!(commands.len(), 2); // search_papers and download_paper

            let command_names: Vec<_> = commands.iter().map(|c| c.name.as_str()).collect();
            assert!(command_names.contains(&"search_papers"));
            assert!(command_names.contains(&"download_paper"));
        }
    }

    #[tokio::test]
    async fn test_utility_functions() {
        use utils::*;

        let config = Arc::new(Config::default());
        let demo = CommandPatternDemo::new(config).unwrap();

        // Test batch processing
        let queries = vec!["test1", "test2"];
        let results =
            batch_process_queries(&demo.executor, queries, 1, Duration::from_millis(10)).await;

        // Should complete without error (even if individual searches fail due to test env)
        assert!(results.is_ok());
    }
}
