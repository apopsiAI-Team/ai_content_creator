use rust_research_mcp::{Config, Server};
use std::sync::Arc;

#[tokio::test]
async fn test_server_lifecycle() {
    let config = Config::default();
    let server = Server::new(config);

    // Server should be created successfully
    assert!(!server.is_shutdown_requested());

    // Test immediate shutdown
    server.shutdown().await;
    assert!(server.is_shutdown_requested());
}

#[tokio::test]
async fn test_server_graceful_shutdown() {
    let mut config = Config::default();
    config.server.graceful_shutdown_timeout_secs = 1; // Short timeout for testing
    config.server.health_check_interval_secs = 1; // Short interval for testing

    let server = Arc::new(Server::new(config));

    // Test shutdown mechanism without running the full MCP server
    // (Full MCP server requires stdio transport and client handshake)

    // Request shutdown
    server.shutdown().await;

    // Verify shutdown was requested
    assert!(
        server.is_shutdown_requested(),
        "Server should be marked as shutdown requested"
    );

    // Test that shutdown is idempotent
    server.shutdown().await;
    assert!(
        server.is_shutdown_requested(),
        "Server should remain shutdown after second call"
    );
}

#[tokio::test]
async fn test_server_with_custom_config() {
    let mut config = Config::default();
    config.server.health_check_interval_secs = 1; // Short interval for testing
    config.server.graceful_shutdown_timeout_secs = 1;

    let server = Server::new(config);

    // Test that custom config is applied
    assert_eq!(server.config().server.health_check_interval_secs, 1);
    assert_eq!(server.config().server.graceful_shutdown_timeout_secs, 1);
}

#[tokio::test]
async fn test_transport_validation() {
    // This test ensures transport validation doesn't block in development
    let result = rust_research_mcp::server::transport::validate_stdio_transport();
    assert!(
        result.is_ok(),
        "Transport validation should allow terminal in development"
    );
}

#[tokio::test]
async fn test_server_handler_integration() {
    let config = Config::default();
    let handler = rust_research_mcp::server::ResearchServerHandler::new(Arc::new(config)).unwrap();

    // Test ping
    let ping_result = handler.ping().await;
    assert!(ping_result.is_ok());
}

#[tokio::test]
async fn test_concurrent_operations() {
    let config = Config::default();
    let handler =
        Arc::new(rust_research_mcp::server::ResearchServerHandler::new(Arc::new(config)).unwrap());

    // Test multiple concurrent ping operations
    let mut tasks = Vec::new();

    for _ in 0..10 {
        let handler_clone = Arc::clone(&handler);
        tasks.push(tokio::spawn(async move { handler_clone.ping().await }));
    }

    // All operations should complete successfully
    for task in tasks {
        let result = task.await.unwrap();
        assert!(result.is_ok());
    }
}
