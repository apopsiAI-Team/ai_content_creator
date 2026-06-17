use crate::{Error, Result};
use std::future::Future;
use std::time::Duration;
use tokio::time::{timeout, Instant};
use tracing::{debug, warn};

/// Timeout configuration for operations
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout for operations
    pub default_timeout: Duration,
    /// Timeout for network operations
    pub network_timeout: Duration,
    /// Timeout for file operations
    pub file_timeout: Duration,
    /// Timeout for health checks
    pub health_check_timeout: Duration,
    /// Timeout for circuit breaker operations
    pub circuit_breaker_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            network_timeout: Duration::from_secs(10),
            file_timeout: Duration::from_secs(5),
            health_check_timeout: Duration::from_secs(5),
            circuit_breaker_timeout: Duration::from_secs(1),
        }
    }
}

impl TimeoutConfig {
    /// Create a fast timeout configuration for development
    #[must_use]
    pub const fn fast() -> Self {
        Self {
            default_timeout: Duration::from_secs(5),
            network_timeout: Duration::from_secs(3),
            file_timeout: Duration::from_secs(2),
            health_check_timeout: Duration::from_secs(2),
            circuit_breaker_timeout: Duration::from_millis(500),
        }
    }

    /// Create a slow timeout configuration for production
    #[must_use]
    pub const fn slow() -> Self {
        Self {
            default_timeout: Duration::from_secs(60),
            network_timeout: Duration::from_secs(30),
            file_timeout: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(10),
            circuit_breaker_timeout: Duration::from_secs(2),
        }
    }

    /// Get timeout for a specific operation type
    #[must_use]
    pub const fn get_timeout(&self, operation_type: TimeoutType) -> Duration {
        match operation_type {
            TimeoutType::Default => self.default_timeout,
            TimeoutType::Network => self.network_timeout,
            TimeoutType::File => self.file_timeout,
            TimeoutType::HealthCheck => self.health_check_timeout,
            TimeoutType::CircuitBreaker => self.circuit_breaker_timeout,
            TimeoutType::Custom(duration) => duration,
        }
    }
}

/// Types of operations for timeout configuration
#[derive(Debug, Clone)]
pub enum TimeoutType {
    Default,
    Network,
    File,
    HealthCheck,
    CircuitBreaker,
    Custom(Duration),
}

/// Extension trait to add timeout functionality to futures
#[allow(async_fn_in_trait)]
pub trait TimeoutExt<T> {
    /// Add timeout to a future with default timeout
    async fn with_timeout(self) -> Result<T>;

    /// Add timeout to a future with custom duration
    async fn with_timeout_duration(self, duration: Duration) -> Result<T>;

    /// Add timeout to a future with operation type
    async fn with_timeout_type(
        self,
        timeout_type: TimeoutType,
        config: &TimeoutConfig,
    ) -> Result<T>;
}

impl<F, T> TimeoutExt<T> for F
where
    F: Future<Output = T>,
{
    async fn with_timeout(self) -> Result<T> {
        self.with_timeout_duration(TimeoutConfig::default().default_timeout)
            .await
    }

    async fn with_timeout_duration(self, duration: Duration) -> Result<T> {
        match timeout(duration, self).await {
            Ok(result) => Ok(result),
            Err(_) => Err(Error::Timeout { timeout: duration }),
        }
    }

    async fn with_timeout_type(
        self,
        timeout_type: TimeoutType,
        config: &TimeoutConfig,
    ) -> Result<T> {
        let duration = config.get_timeout(timeout_type);
        self.with_timeout_duration(duration).await
    }
}

/// Timeout wrapper for operations with logging and metrics
pub struct TimeoutWrapper {
    config: TimeoutConfig,
    operation_name: String,
}

impl TimeoutWrapper {
    /// Create a new timeout wrapper
    pub fn new(operation_name: impl Into<String>, config: TimeoutConfig) -> Self {
        Self {
            config,
            operation_name: operation_name.into(),
        }
    }

    /// Execute an operation with timeout and logging
    pub async fn execute<F, Fut, T>(&self, operation: F, timeout_type: TimeoutType) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let timeout_duration = self.config.get_timeout(timeout_type.clone());
        let start_time = Instant::now();

        debug!(
            "Starting operation '{}' with timeout {:?}",
            self.operation_name, timeout_duration
        );

        let result = match timeout(timeout_duration, operation()).await {
            Ok(Ok(value)) => {
                let duration = start_time.elapsed();
                debug!(
                    "Operation '{}' completed successfully in {:?}",
                    self.operation_name, duration
                );
                Ok(value)
            }
            Ok(Err(error)) => {
                let duration = start_time.elapsed();
                debug!(
                    "Operation '{}' failed after {:?}: {}",
                    self.operation_name, duration, error
                );
                Err(error)
            }
            Err(_) => {
                warn!(
                    "Operation '{}' timed out after {:?}",
                    self.operation_name, timeout_duration
                );
                Err(Error::Timeout {
                    timeout: timeout_duration,
                })
            }
        };

        result
    }

    /// Execute an operation with custom timeout
    pub async fn execute_with_timeout<F, Fut, T>(
        &self,
        operation: F,
        custom_timeout: Duration,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.execute(operation, TimeoutType::Custom(custom_timeout))
            .await
    }

    /// Execute a network operation with appropriate timeout
    pub async fn execute_network<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.execute(operation, TimeoutType::Network).await
    }

    /// Execute a file operation with appropriate timeout
    pub async fn execute_file<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.execute(operation, TimeoutType::File).await
    }

    /// Execute a health check with appropriate timeout
    pub async fn execute_health_check<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.execute(operation, TimeoutType::HealthCheck).await
    }
}

/// Timeout manager for coordinating timeouts across the application
pub struct TimeoutManager {
    config: TimeoutConfig,
}

impl TimeoutManager {
    /// Create a new timeout manager
    #[must_use]
    pub const fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }

    /// Create a timeout wrapper for an operation
    pub fn wrapper(&self, operation_name: impl Into<String>) -> TimeoutWrapper {
        TimeoutWrapper::new(operation_name, self.config.clone())
    }

    /// Execute an operation with timeout
    pub async fn execute<F, Fut, T>(
        &self,
        operation_name: &str,
        operation: F,
        timeout_type: TimeoutType,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.wrapper(operation_name)
            .execute(operation, timeout_type)
            .await
    }

    /// Get the current timeout configuration
    #[must_use]
    pub const fn config(&self) -> &TimeoutConfig {
        &self.config
    }

    /// Update the timeout configuration
    pub fn update_config(&mut self, config: TimeoutConfig) {
        self.config = config;
    }
}

impl Default for TimeoutManager {
    fn default() -> Self {
        Self::new(TimeoutConfig::default())
    }
}

/// Convenience functions for common timeout operations
pub mod convenience {
    use super::{Result, TimeoutConfig, TimeoutExt, TimeoutWrapper};
    use std::future::Future;
    use tokio::time::Duration;

    /// Execute a future with default timeout
    pub async fn with_default_timeout<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        future.with_timeout().await
    }

    /// Execute a network operation with appropriate timeout
    pub async fn with_network_timeout<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        let config = TimeoutConfig::default();
        future.with_timeout_duration(config.network_timeout).await
    }

    /// Execute a file operation with appropriate timeout
    pub async fn with_file_timeout<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        let config = TimeoutConfig::default();
        future.with_timeout_duration(config.file_timeout).await
    }

    /// Execute a health check with appropriate timeout
    pub async fn with_health_check_timeout<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        let config = TimeoutConfig::default();
        future
            .with_timeout_duration(config.health_check_timeout)
            .await
    }

    /// Execute an operation with custom timeout and operation name for logging
    pub async fn with_timeout_and_logging<F, T>(
        future: F,
        timeout_duration: Duration,
        operation_name: &str,
    ) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let wrapper = TimeoutWrapper::new(operation_name, TimeoutConfig::default());
        wrapper
            .execute_with_timeout(|| future, timeout_duration)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_timeout_ext_success() {
        let result = async { 42 }
            .with_timeout_duration(Duration::from_millis(100))
            .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_ext_timeout() {
        let result = async {
            sleep(Duration::from_millis(200)).await;
            42
        }
        .with_timeout_duration(Duration::from_millis(100))
        .await;

        assert!(matches!(result, Err(Error::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_timeout_wrapper() {
        let config = TimeoutConfig::fast();
        let wrapper = TimeoutWrapper::new("test_operation", config);

        // Test successful operation
        let result = wrapper
            .execute(|| async { Ok::<i32, Error>(42) }, TimeoutType::Default)
            .await;
        assert_eq!(result.unwrap(), 42);

        // Test timeout
        let result = wrapper
            .execute(
                || async {
                    sleep(Duration::from_secs(10)).await;
                    Ok::<i32, Error>(42)
                },
                TimeoutType::Default,
            )
            .await;
        assert!(matches!(result, Err(Error::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_timeout_manager() {
        let manager = TimeoutManager::new(TimeoutConfig::fast());

        let result = manager
            .execute(
                "test_op",
                || async { Ok::<i32, Error>(42) },
                TimeoutType::Network,
            )
            .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_types() {
        let config = TimeoutConfig::default();

        assert_eq!(
            config.get_timeout(TimeoutType::Default),
            config.default_timeout
        );
        assert_eq!(
            config.get_timeout(TimeoutType::Network),
            config.network_timeout
        );
        assert_eq!(config.get_timeout(TimeoutType::File), config.file_timeout);
        assert_eq!(
            config.get_timeout(TimeoutType::HealthCheck),
            config.health_check_timeout
        );
        assert_eq!(
            config.get_timeout(TimeoutType::CircuitBreaker),
            config.circuit_breaker_timeout
        );

        let custom_duration = Duration::from_secs(99);
        assert_eq!(
            config.get_timeout(TimeoutType::Custom(custom_duration)),
            custom_duration
        );
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        use super::convenience::*;

        // Test default timeout
        let result = with_default_timeout(async { 42 }).await;
        assert_eq!(result.unwrap(), 42);

        // Test network timeout
        let result = with_network_timeout(async { 42 }).await;
        assert_eq!(result.unwrap(), 42);

        // Test file timeout
        let result = with_file_timeout(async { 42 }).await;
        assert_eq!(result.unwrap(), 42);

        // Test health check timeout
        let result = with_health_check_timeout(async { 42 }).await;
        assert_eq!(result.unwrap(), 42);

        // Test timeout with logging
        let result = with_timeout_and_logging(
            async { Ok::<i32, Error>(42) },
            Duration::from_millis(100),
            "test_operation",
        )
        .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_config_presets() {
        let fast = TimeoutConfig::fast();
        assert!(fast.default_timeout < TimeoutConfig::default().default_timeout);

        let slow = TimeoutConfig::slow();
        assert!(slow.default_timeout > TimeoutConfig::default().default_timeout);
    }

    #[tokio::test]
    async fn test_timeout_wrapper_operations() {
        let config = TimeoutConfig::fast();
        let wrapper = TimeoutWrapper::new("test", config);

        // Test network operation
        let result = wrapper
            .execute_network(|| async { Ok::<i32, Error>(42) })
            .await;
        assert_eq!(result.unwrap(), 42);

        // Test file operation
        let result = wrapper
            .execute_file(|| async { Ok::<i32, Error>(42) })
            .await;
        assert_eq!(result.unwrap(), 42);

        // Test health check operation
        let result = wrapper
            .execute_health_check(|| async { Ok::<i32, Error>(42) })
            .await;
        assert_eq!(result.unwrap(), 42);
    }
}
