use crate::resilience::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Service for managing circuit breakers for external HTTP calls
pub struct CircuitBreakerService {
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerService {
    /// Create a new circuit breaker service
    #[must_use]
    pub fn new() -> Self {
        let default_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            failure_timeout: Duration::from_secs(60),
            recovery_timeout: Duration::from_secs(30),
            half_open_max_calls: 3,
        };

        Self {
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }

    /// Create a new circuit breaker service with custom config
    #[must_use]
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
        }
    }

    /// Get or create a circuit breaker for a specific service
    pub async fn get_circuit_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.circuit_breakers.write().await;

        breakers.get(service_name).cloned().unwrap_or_else(|| {
            info!("Creating new circuit breaker for service: {}", service_name);
            let breaker = Arc::new(CircuitBreaker::new(
                service_name.to_string(),
                self.default_config.clone(),
            ));
            breakers.insert(service_name.to_string(), breaker.clone());
            breaker
        })
    }

    /// Execute an operation with circuit breaker protection
    pub async fn call<T, F, Fut>(&self, service_name: &str, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let circuit_breaker = self.get_circuit_breaker(service_name).await;

        debug!("Executing operation with circuit breaker: {}", service_name);
        circuit_breaker.call(operation).await
    }

    /// Execute a HTTP request with circuit breaker protection
    pub async fn call_http<T, F, Fut>(&self, service_name: &str, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    {
        let circuit_breaker = self.get_circuit_breaker(service_name).await;

        circuit_breaker
            .call(|| async {
                operation().await.map_err(|e| {
                    if e.is_timeout() {
                        Error::NetworkTimeout {
                            timeout: Duration::from_secs(30),
                            message: format!("HTTP request to {service_name} timed out: {e}"),
                        }
                    } else if e.is_connect() {
                        Error::ConnectionRefused {
                            endpoint: service_name.to_string(),
                        }
                    } else {
                        Error::Http(e)
                    }
                })
            })
            .await
    }

    /// Get health status of all circuit breakers
    pub async fn get_health_status(&self) -> HashMap<String, bool> {
        let mut status = HashMap::new();

        {
            let breakers = self.circuit_breakers.read().await;
            for (service_name, breaker) in breakers.iter() {
                let metrics = breaker.get_metrics().await;
                status.insert(service_name.clone(), metrics.is_healthy());
            }
        }

        status
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        let breakers = self.circuit_breakers.read().await;

        for (service_name, breaker) in breakers.iter() {
            breaker.reset().await;
            info!("Reset circuit breaker for service: {}", service_name);
        }
    }

    /// Reset a specific circuit breaker
    pub async fn reset(&self, service_name: &str) -> Result<()> {
        let breakers = self.circuit_breakers.read().await;

        if let Some(breaker) = breakers.get(service_name) {
            breaker.reset().await;
            info!("Reset circuit breaker for service: {}", service_name);
            Ok(())
        } else {
            warn!("Circuit breaker not found for service: {}", service_name);
            Err(Error::Service(format!(
                "Circuit breaker not found for service: {service_name}"
            )))
        }
    }

    /// Force a circuit breaker to open (for testing or maintenance)
    pub async fn force_open(&self, service_name: &str) -> Result<()> {
        let breakers = self.circuit_breakers.read().await;

        if let Some(breaker) = breakers.get(service_name) {
            breaker.force_open().await;
            warn!("Forced circuit breaker open for service: {}", service_name);
            Ok(())
        } else {
            warn!("Circuit breaker not found for service: {}", service_name);
            Err(Error::Service(format!(
                "Circuit breaker not found for service: {service_name}"
            )))
        }
    }

    /// Get detailed metrics for all circuit breakers
    pub async fn get_all_metrics(
        &self,
    ) -> HashMap<String, crate::resilience::circuit_breaker::CircuitBreakerMetrics> {
        let mut metrics = HashMap::new();

        {
            let breakers = self.circuit_breakers.read().await;
            for (service_name, breaker) in breakers.iter() {
                let cb_metrics = breaker.get_metrics().await;
                metrics.insert(service_name.clone(), cb_metrics);
            }
        }

        metrics
    }
}

impl Default for CircuitBreakerService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_circuit_breaker_service_creation() {
        let service = CircuitBreakerService::new();
        let status = service.get_health_status().await;
        assert!(status.is_empty()); // No circuit breakers created yet
    }

    #[tokio::test]
    async fn test_successful_operation() {
        let service = CircuitBreakerService::new();

        let result = service
            .call("test_service", || async { Ok::<i32, Error>(42) })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let status = service.get_health_status().await;
        assert_eq!(status.get("test_service"), Some(&true));
    }

    #[tokio::test]
    async fn test_failed_operation_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            failure_timeout: Duration::from_millis(100),
            recovery_timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
        };

        let service = CircuitBreakerService::with_config(config);
        let call_count = Arc::new(AtomicUsize::new(0));

        // First failure
        let call_count_clone = call_count.clone();
        let result = service
            .call("test_service", move || {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                async {
                    Err::<i32, Error>(Error::ServiceUnavailable {
                        service: "test".to_string(),
                        reason: "test failure".to_string(),
                    })
                }
            })
            .await;
        assert!(result.is_err());

        // Second failure - should open circuit
        let call_count_clone = call_count.clone();
        let result = service
            .call("test_service", move || {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                async {
                    Err::<i32, Error>(Error::ServiceUnavailable {
                        service: "test".to_string(),
                        reason: "test failure".to_string(),
                    })
                }
            })
            .await;
        assert!(result.is_err());

        // Third call should be rejected by circuit breaker
        let call_count_clone = call_count.clone();
        let result = service
            .call("test_service", move || {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                async {
                    Ok::<i32, Error>(42) // This shouldn't be called
                }
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::CircuitBreakerOpen { .. }
        ));

        // Should have only called the operation twice (circuit breaker prevented third call)
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        let status = service.get_health_status().await;
        assert_eq!(status.get("test_service"), Some(&false));
    }

    #[tokio::test]
    async fn test_http_error_conversion() {
        let service = CircuitBreakerService::new();

        // Simulate a timeout error by creating a proper reqwest error
        let result = service
            .call_http("test_service", || async {
                // Create a mock timeout error using reqwest's timeout functionality
                let client = reqwest::Client::new();
                let timeout_result = tokio::time::timeout(
                    Duration::from_millis(1),
                    client.get("http://httpbin.org/delay/5").send(),
                )
                .await;

                match timeout_result {
                    Ok(resp) => resp,
                    Err(_) => {
                        // Create a proper timeout error
                        let client = reqwest::Client::builder()
                            .timeout(Duration::from_millis(1))
                            .build()
                            .unwrap();
                        client.get("http://httpbin.org/delay/5").send().await
                    }
                }
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NetworkTimeout { .. }));
    }

    #[tokio::test]
    async fn test_reset_functionality() {
        let service = CircuitBreakerService::new();

        // Create a circuit breaker by making a call
        let _ = service
            .call("test_service", || async { Ok::<i32, Error>(42) })
            .await;

        // Reset the specific circuit breaker
        let result = service.reset("test_service").await;
        assert!(result.is_ok());

        // Try to reset a non-existent circuit breaker
        let result = service.reset("non_existent").await;
        assert!(result.is_err());
    }
}
