//! Dependency Injection Container Usage Examples
//!
//! This module demonstrates how to use the ServiceContainer for managing
//! dependencies in the knowledge_accumulator_mcp application.

use crate::{
    di::{ServiceContainer, ServiceScope},
    Config, MetaSearchClient, MetaSearchConfig, Result, SearchTool,
};
use std::sync::Arc;
use tracing::info;

/// Example demonstrating basic DI container usage
pub async fn basic_container_example() -> Result<()> {
    info!("Starting basic DI container example");

    // Create configuration
    let config = Arc::new(Config::default());

    // Create a DI container
    let mut container = ServiceContainer::new();

    // Register configuration as singleton
    container.register_singleton(config.clone()).await?;

    // Create and register MetaSearch client
    let meta_config = MetaSearchConfig::from_config(&config);
    let meta_client = Arc::new(MetaSearchClient::new((*config).clone(), meta_config)?);
    container.register_singleton(meta_client).await?;

    // Create and register SearchTool
    let search_tool = Arc::new(SearchTool::new(config.clone())?);
    container.register_singleton(search_tool).await?;

    // Demonstrate service resolution
    let resolved_config = container.resolve::<Arc<Config>>().await?;
    let resolved_client = container.resolve::<Arc<MetaSearchClient>>().await?;
    let resolved_search_tool = container.resolve::<Arc<SearchTool>>().await?;

    info!("Successfully resolved {} services from DI container", 3);
    info!(
        "Config endpoints: {:?}",
        resolved_config.research_source.endpoints
    );

    // Verify that singletons are actually the same instances
    let config_again = container.resolve::<Arc<Config>>().await?;
    assert!(Arc::ptr_eq(&resolved_config, &config_again));

    info!("Singleton integrity verified - same instances returned");

    Ok(())
}

/// Example demonstrating DI container integration with server setup
pub async fn server_integration_example() -> Result<()> {
    info!("Starting server integration DI example");

    let config = Arc::new(Config::default());

    // Use the same pattern as ResearchServerHandler
    let container = setup_research_container(config).await?;

    // Verify all services are available
    let _config = container.resolve::<Arc<Config>>().await?;
    let _meta_client = container.resolve::<Arc<MetaSearchClient>>().await?;
    let _search_tool = container.resolve::<Arc<SearchTool>>().await?;

    info!("All research services successfully configured");
    info!(
        "Container has {} singleton services",
        container.singleton_count().await
    );

    Ok(())
}

/// Helper function that mirrors the ResearchServerHandler setup
async fn setup_research_container(config: Arc<Config>) -> Result<ServiceContainer> {
    info!("Setting up research service container");

    // Create services directly (simplified approach)
    let meta_config = MetaSearchConfig::from_config(&config);
    let meta_client = Arc::new(MetaSearchClient::new((*config).clone(), meta_config)?);
    let search_tool = Arc::new(SearchTool::new(config.clone())?);

    let mut container = ServiceContainer::new();

    // Register all services as singletons
    container.register_singleton(config).await?;
    container.register_singleton(meta_client).await?;
    container.register_singleton(search_tool).await?;

    info!(
        "Research container setup complete with {} services",
        container.singleton_count().await
    );

    Ok(container)
}

/// Example demonstrating container lifecycle management
pub async fn lifecycle_management_example() -> Result<()> {
    info!("Starting lifecycle management example");

    let config = Arc::new(Config::default());
    let mut container = ServiceContainer::new();

    // Initial state
    assert_eq!(container.singleton_count().await, 0);
    info!("Container initialized - empty state verified");

    // Register services
    container.register_singleton(config.clone()).await?;
    assert_eq!(container.singleton_count().await, 1);
    info!("Config registered - container has 1 service");

    // Check if service is registered
    let is_registered = container.is_registered::<Arc<Config>>().await;
    assert!(is_registered);
    info!("Service registration verified");

    // List registered services
    let singletons = container.list_singletons().await;
    info!("Registered singleton types: {:?}", singletons);

    // Clear all services
    container.clear_singletons().await;
    assert_eq!(container.singleton_count().await, 0);
    info!("Container cleared - all services removed");

    Ok(())
}

/// Demonstrates error handling in DI container
pub async fn error_handling_example() -> Result<()> {
    info!("Starting error handling example");

    let container = ServiceContainer::new();

    // Try to resolve a service that doesn't exist
    let result = container.resolve::<Arc<Config>>().await;
    assert!(result.is_err());

    if let Err(error) = result {
        info!("Expected error caught: {}", error);
    }

    info!("Error handling example completed successfully");
    Ok(())
}

/// Run all DI examples
pub async fn run_all_examples() -> Result<()> {
    info!("Running all DI container examples");

    basic_container_example().await?;
    server_integration_example().await?;
    lifecycle_management_example().await?;
    error_handling_example().await?;

    info!("All DI container examples completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_container_example() {
        basic_container_example().await.unwrap();
    }

    #[tokio::test]
    async fn test_server_integration_example() {
        server_integration_example().await.unwrap();
    }

    #[tokio::test]
    async fn test_lifecycle_management_example() {
        lifecycle_management_example().await.unwrap();
    }

    #[tokio::test]
    async fn test_error_handling_example() {
        error_handling_example().await.unwrap();
    }

    #[tokio::test]
    async fn test_run_all_examples() {
        run_all_examples().await.unwrap();
    }
}
