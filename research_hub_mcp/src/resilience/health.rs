use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Health status for a component
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy and operational
    Healthy,
    /// Component is degraded but still operational
    Degraded { reason: String },
    /// Component is unhealthy and not operational
    Unhealthy { reason: String },
    /// Component status is unknown or check failed
    Unknown { reason: String },
}

impl HealthStatus {
    /// Check if status indicates the component is operational
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded { .. })
    }

    /// Check if status indicates the component is fully healthy
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Get the reason for non-healthy status
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Healthy => None,
            Self::Degraded { reason } | Self::Unhealthy { reason } | Self::Unknown { reason } => {
                Some(reason)
            }
        }
    }
}

/// Health check result with timing information
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub checked_at: Instant,
    pub check_duration: Duration,
    pub details: HashMap<String, String>,
}

impl HealthCheckResult {
    /// Create a new health check result
    #[must_use]
    pub fn new(status: HealthStatus, check_duration: Duration) -> Self {
        Self {
            status,
            checked_at: Instant::now(),
            check_duration,
            details: HashMap::new(),
        }
    }

    /// Add detail information to the health check result
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Check if the result is stale based on max age
    #[must_use]
    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.checked_at.elapsed() > max_age
    }
}

/// Trait for components that can be health checked
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform a health check
    async fn check_health(&self) -> HealthCheckResult;

    /// Get the name of this health check
    fn name(&self) -> &str;

    /// Get the timeout for this health check
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    /// Get the maximum age before a cached result is considered stale
    fn max_cache_age(&self) -> Duration {
        Duration::from_secs(30)
    }
}

/// Component health information
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub name: String,
    pub result: HealthCheckResult,
}

/// Health check manager for multiple components
pub struct HealthCheckManager {
    checks: Arc<RwLock<HashMap<String, Arc<dyn HealthCheck>>>>,
    cache: Arc<RwLock<HashMap<String, HealthCheckResult>>>,
}

impl HealthCheckManager {
    /// Create a new health check manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a health check
    pub async fn register(&self, check: Arc<dyn HealthCheck>) {
        let name = check.name().to_string();
        self.checks.write().await.insert(name, check);
    }

    /// Unregister a health check
    pub async fn unregister(&self, name: &str) {
        self.checks.write().await.remove(name);
        self.cache.write().await.remove(name);
    }

    /// Check health of a specific component
    pub async fn check_component(&self, name: &str) -> Result<HealthCheckResult> {
        let checks = self.checks.read().await;
        let check = checks
            .get(name)
            .ok_or_else(|| Error::Service(format!("Health check '{name}' not found")))?;

        let _start_time = Instant::now();
        let timeout = check.timeout();

        debug!("Running health check for '{}'", name);

        // Run health check with timeout
        let result = if let Ok(result) = tokio::time::timeout(timeout, check.check_health()).await {
            result
        } else {
            warn!("Health check '{}' timed out after {:?}", name, timeout);
            HealthCheckResult::new(
                HealthStatus::Unknown {
                    reason: format!("Health check timed out after {timeout:?}"),
                },
                timeout,
            )
        };

        // Cache the result
        self.cache
            .write()
            .await
            .insert(name.to_string(), result.clone());

        debug!(
            "Health check '{}' completed: {:?} (took {:?})",
            name, result.status, result.check_duration
        );

        Ok(result)
    }

    /// Get cached health result if available and not stale
    pub async fn get_cached_health(&self, name: &str) -> Option<HealthCheckResult> {
        let cache = self.cache.read().await;
        let checks = self.checks.read().await;

        if let (Some(result), Some(check)) = (cache.get(name), checks.get(name)) {
            if !result.is_stale(check.max_cache_age()) {
                return Some(result.clone());
            }
        }

        None
    }

    /// Check health of all registered components
    pub async fn check_all(&self) -> Vec<ComponentHealth> {
        let checks = self.checks.read().await;
        let mut results = Vec::new();

        for name in checks.keys() {
            match self.check_component(name).await {
                Ok(result) => {
                    results.push(ComponentHealth {
                        name: name.clone(),
                        result,
                    });
                }
                Err(e) => {
                    error!("Failed to check health of '{}': {}", name, e);
                    results.push(ComponentHealth {
                        name: name.clone(),
                        result: HealthCheckResult::new(
                            HealthStatus::Unknown {
                                reason: format!("Health check failed: {e}"),
                            },
                            Duration::from_millis(0),
                        ),
                    });
                }
            }
        }

        results
    }

    /// Get overall system health status
    pub async fn get_system_health(&self) -> HealthStatus {
        let results = self.check_all().await;

        if results.is_empty() {
            return HealthStatus::Unknown {
                reason: "No health checks registered".to_string(),
            };
        }

        let mut _healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;
        let mut unknown_count = 0;

        for component in &results {
            match component.result.status {
                HealthStatus::Healthy => _healthy_count += 1,
                HealthStatus::Degraded { .. } => degraded_count += 1,
                HealthStatus::Unhealthy { .. } => unhealthy_count += 1,
                HealthStatus::Unknown { .. } => unknown_count += 1,
            }
        }

        // System is unhealthy if any component is unhealthy
        if unhealthy_count > 0 {
            return HealthStatus::Unhealthy {
                reason: format!(
                    "{} unhealthy components out of {}",
                    unhealthy_count,
                    results.len()
                ),
            };
        }

        // System is degraded if any component is degraded or unknown
        if degraded_count > 0 || unknown_count > 0 {
            return HealthStatus::Degraded {
                reason: format!(
                    "{} degraded, {} unknown out of {} components",
                    degraded_count,
                    unknown_count,
                    results.len()
                ),
            };
        }

        // All components are healthy
        HealthStatus::Healthy
    }

    /// Get health summary as a map
    pub async fn get_health_summary(&self) -> HashMap<String, HealthStatus> {
        let results = self.check_all().await;
        results
            .into_iter()
            .map(|component| (component.name, component.result.status))
            .collect()
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Basic HTTP health check implementation
pub struct HttpHealthCheck {
    name: String,
    url: String,
    client: reqwest::Client,
    timeout: Duration,
    expected_status: Option<reqwest::StatusCode>,
}

impl HttpHealthCheck {
    /// Create a new HTTP health check
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            client: reqwest::Client::new(),
            timeout: Duration::from_secs(5),
            expected_status: Some(reqwest::StatusCode::OK),
        }
    }

    /// Set the timeout for HTTP requests
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the expected HTTP status code
    #[must_use]
    pub const fn with_expected_status(mut self, status: reqwest::StatusCode) -> Self {
        self.expected_status = Some(status);
        self
    }

    /// Don't check HTTP status code
    #[must_use]
    pub const fn ignore_status(mut self) -> Self {
        self.expected_status = None;
        self
    }
}

#[async_trait::async_trait]
impl HealthCheck for HttpHealthCheck {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = Instant::now();

        let response = match self
            .client
            .get(&self.url)
            .timeout(self.timeout)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return HealthCheckResult::new(
                    HealthStatus::Unhealthy {
                        reason: format!("HTTP request failed: {e}"),
                    },
                    start_time.elapsed(),
                )
                .with_detail("url", &self.url)
                .with_detail("error", e.to_string());
            }
        };

        let duration = start_time.elapsed();
        let status_code = response.status();

        // Check if status code matches expected
        if let Some(expected) = self.expected_status {
            if status_code != expected {
                return HealthCheckResult::new(
                    HealthStatus::Unhealthy {
                        reason: format!("Unexpected status code: {status_code}"),
                    },
                    duration,
                )
                .with_detail("url", &self.url)
                .with_detail("status_code", status_code.to_string())
                .with_detail("expected_status", expected.to_string());
            }
        }

        // Check response time
        let status = if duration > Duration::from_secs(2) {
            HealthStatus::Degraded {
                reason: format!("Slow response time: {duration:?}"),
            }
        } else {
            HealthStatus::Healthy
        };

        HealthCheckResult::new(status, duration)
            .with_detail("url", &self.url)
            .with_detail("status_code", status_code.to_string())
            .with_detail("response_time_ms", duration.as_millis().to_string())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn timeout(&self) -> Duration {
        self.timeout + Duration::from_millis(100) // Add small buffer
    }
}

/// Simple ping health check that always returns healthy
pub struct PingHealthCheck {
    name: String,
}

impl PingHealthCheck {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait::async_trait]
impl HealthCheck for PingHealthCheck {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = Instant::now();

        // Simulate a very fast check
        tokio::time::sleep(Duration::from_millis(1)).await;

        HealthCheckResult::new(HealthStatus::Healthy, start_time.elapsed())
            .with_detail("type", "ping")
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn max_cache_age(&self) -> Duration {
        Duration::from_secs(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestHealthCheck {
        name: String,
        result: HealthStatus,
        delay: Duration,
        call_count: Arc<AtomicU32>,
    }

    impl TestHealthCheck {
        fn new(name: &str, result: HealthStatus) -> Self {
            Self {
                name: name.to_string(),
                result,
                delay: Duration::from_millis(10),
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl HealthCheck for TestHealthCheck {
        async fn check_health(&self) -> HealthCheckResult {
            let start_time = Instant::now();
            self.call_count.fetch_add(1, Ordering::SeqCst);

            tokio::time::sleep(self.delay).await;

            HealthCheckResult::new(self.result.clone(), start_time.elapsed())
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn timeout(&self) -> Duration {
            Duration::from_millis(100)
        }

        fn max_cache_age(&self) -> Duration {
            Duration::from_millis(50)
        }
    }

    #[tokio::test]
    async fn test_health_check_manager() {
        let manager = HealthCheckManager::new();

        // Register some health checks
        let healthy_check = Arc::new(TestHealthCheck::new("healthy", HealthStatus::Healthy));
        let degraded_check = Arc::new(TestHealthCheck::new(
            "degraded",
            HealthStatus::Degraded {
                reason: "slow response".to_string(),
            },
        ));

        manager.register(healthy_check.clone()).await;
        manager.register(degraded_check.clone()).await;

        // Check individual components
        let result = manager.check_component("healthy").await.unwrap();
        assert!(result.status.is_healthy());

        let result = manager.check_component("degraded").await.unwrap();
        assert!(result.status.is_operational());
        assert!(!result.status.is_healthy());

        // Check all components
        let results = manager.check_all().await;
        assert_eq!(results.len(), 2);

        // Get system health
        let system_health = manager.get_system_health().await;
        assert!(matches!(system_health, HealthStatus::Degraded { .. }));
    }

    #[tokio::test]
    async fn test_health_check_caching() {
        let manager = HealthCheckManager::new();
        let check = Arc::new(TestHealthCheck::new("test", HealthStatus::Healthy));

        manager.register(check.clone()).await;

        // First call should execute the check
        let _result1 = manager.check_component("test").await.unwrap();
        assert_eq!(check.call_count(), 1);

        // Second call should use cache
        let cached = manager.get_cached_health("test").await;
        assert!(cached.is_some());

        // Wait for cache to expire
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Should not use stale cache
        let cached = manager.get_cached_health("test").await;
        assert!(cached.is_none());

        // New call should execute the check again
        let _result2 = manager.check_component("test").await.unwrap();
        assert_eq!(check.call_count(), 2);
    }

    #[tokio::test]
    async fn test_health_check_timeout() {
        let manager = HealthCheckManager::new();
        let slow_check = Arc::new(
            TestHealthCheck::new("slow", HealthStatus::Healthy)
                .with_delay(Duration::from_millis(200)),
        );

        manager.register(slow_check.clone()).await;

        // Check should timeout and return Unknown status
        let result = manager.check_component("slow").await.unwrap();
        assert!(matches!(result.status, HealthStatus::Unknown { .. }));
    }

    #[tokio::test]
    async fn test_ping_health_check() {
        let ping = PingHealthCheck::new("ping");
        let result = ping.check_health().await;

        assert!(result.status.is_healthy());
        assert!(result.check_duration < Duration::from_millis(10));
    }

    #[test]
    fn test_health_status_methods() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Healthy.reason().is_none());

        let degraded = HealthStatus::Degraded {
            reason: "slow".to_string(),
        };
        assert!(!degraded.is_healthy());
        assert!(degraded.is_operational());
        assert_eq!(degraded.reason(), Some("slow"));

        let unhealthy = HealthStatus::Unhealthy {
            reason: "down".to_string(),
        };
        assert!(!unhealthy.is_healthy());
        assert!(!unhealthy.is_operational());
        assert_eq!(unhealthy.reason(), Some("down"));
    }
}
