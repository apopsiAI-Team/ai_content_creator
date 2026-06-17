use crate::{Error, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed - requests flow normally
    Closed,
    /// Circuit is open - requests are rejected immediately
    Open { opened_at: Instant },
    /// Circuit is half-open - limited requests allowed to test recovery
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open the circuit
    pub failure_threshold: u32,
    /// Success threshold to close the circuit from half-open
    pub success_threshold: u32,
    /// Time window for counting failures
    pub failure_timeout: Duration,
    /// Time to wait before transitioning from open to half-open
    pub recovery_timeout: Duration,
    /// Maximum number of requests allowed in half-open state
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            failure_timeout: Duration::from_secs(60),
            recovery_timeout: Duration::from_secs(30),
            half_open_max_calls: 3,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone, Default)]
struct CircuitMetrics {
    failure_count: u32,
    success_count: u32,
    total_requests: u64,
    last_failure_time: Option<Instant>,
    half_open_calls: u32,
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    metrics: Arc<RwLock<CircuitMetrics>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            metrics: Arc::new(RwLock::new(CircuitMetrics::default())),
        }
    }

    /// Execute an operation with circuit breaker protection
    pub async fn call<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit allows the call
        if !self.can_execute().await? {
            return Err(Error::CircuitBreakerOpen {
                service: self.name.clone(),
            });
        }

        // Execute the operation
        let start_time = Instant::now();
        let result = operation().await;
        let duration = start_time.elapsed();

        // Update circuit breaker based on result
        match &result {
            Ok(_) => {
                self.on_success().await;
                debug!(
                    "Circuit breaker '{}': Success after {:?}",
                    self.name, duration
                );
            }
            Err(error) => {
                if error.should_trigger_circuit_breaker() {
                    self.on_failure().await;
                    debug!(
                        "Circuit breaker '{}': Failure after {:?} - {}",
                        self.name, duration, error
                    );
                }
            }
        }

        result
    }

    /// Check if the circuit breaker allows execution
    async fn can_execute(&self) -> Result<bool> {
        let mut state = self.state.write().await;
        let mut metrics = self.metrics.write().await;

        match &*state {
            CircuitState::Closed => Ok(true),
            CircuitState::Open { opened_at } => {
                // Check if recovery timeout has passed
                if opened_at.elapsed() >= self.config.recovery_timeout {
                    *state = CircuitState::HalfOpen;
                    metrics.half_open_calls = 1; // Count this call
                    info!(
                        "Circuit breaker '{}': Transitioning from Open to Half-Open",
                        self.name
                    );
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            CircuitState::HalfOpen => {
                if metrics.half_open_calls < self.config.half_open_max_calls {
                    metrics.half_open_calls += 1;
                    Ok(true)
                } else {
                    // Circuit should remain half-open but reject this call
                    Ok(false)
                }
            }
        }
    }

    /// Handle successful operation
    async fn on_success(&self) {
        let mut state = self.state.write().await;
        let mut metrics = self.metrics.write().await;

        metrics.total_requests += 1;

        match &*state {
            CircuitState::Closed => {
                // Reset failure count on success
                metrics.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                metrics.success_count += 1;
                if metrics.success_count >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    metrics.failure_count = 0;
                    metrics.success_count = 0;
                    metrics.half_open_calls = 0;
                    info!(
                        "Circuit breaker '{}': Transitioning from Half-Open to Closed",
                        self.name
                    );
                }
            }
            CircuitState::Open { .. } => {
                // Should not happen, but reset to closed if it does
                *state = CircuitState::Closed;
                metrics.failure_count = 0;
                metrics.success_count = 0;
            }
        }
    }

    /// Handle failed operation
    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        let mut metrics = self.metrics.write().await;

        metrics.total_requests += 1;
        metrics.failure_count += 1;
        metrics.last_failure_time = Some(Instant::now());

        match &*state {
            CircuitState::Closed => {
                if metrics.failure_count >= self.config.failure_threshold {
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                    warn!(
                        "Circuit breaker '{}': Opening due to {} failures",
                        self.name, metrics.failure_count
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Return to open state on any failure during half-open
                *state = CircuitState::Open {
                    opened_at: Instant::now(),
                };
                metrics.success_count = 0;
                metrics.half_open_calls = 0;
                warn!(
                    "Circuit breaker '{}': Returning to Open from Half-Open due to failure",
                    self.name
                );
            }
            CircuitState::Open { .. } => {
                // Already open, just update metrics
            }
        }
    }

    /// Get current circuit breaker state
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Get circuit breaker metrics
    pub async fn get_metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.read().await;
        let metrics = self.metrics.read().await;

        CircuitBreakerMetrics {
            name: self.name.clone(),
            state: state.clone(),
            failure_count: metrics.failure_count,
            success_count: metrics.success_count,
            total_requests: metrics.total_requests,
            last_failure_time: metrics.last_failure_time,
        }
    }

    /// Reset circuit breaker to closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        let mut metrics = self.metrics.write().await;

        *state = CircuitState::Closed;
        metrics.failure_count = 0;
        metrics.success_count = 0;
        metrics.half_open_calls = 0;

        info!("Circuit breaker '{}': Reset to Closed state", self.name);
    }

    /// Force circuit breaker to open state
    pub async fn force_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open {
            opened_at: Instant::now(),
        };

        warn!("Circuit breaker '{}': Forced to Open state", self.name);
    }
}

/// Public metrics for circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub name: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_requests: u64,
    pub last_failure_time: Option<Instant>,
}

impl CircuitBreakerMetrics {
    /// Check if circuit breaker is healthy
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self.state, CircuitState::Closed)
    }

    /// Get failure rate as percentage
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (f64::from(self.failure_count) / self.total_requests as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        // Should allow calls when closed
        let result = cb.call(|| async { Ok::<(), Error>(()) }).await;
        assert!(result.is_ok());

        let state = cb.get_state().await;
        assert_eq!(state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        // First failure
        let _ = cb
            .call(|| async {
                Err::<(), Error>(Error::ServiceUnavailable {
                    service: "test".to_string(),
                    reason: "test failure".to_string(),
                })
            })
            .await;

        // Second failure - should open circuit
        let _ = cb
            .call(|| async {
                Err::<(), Error>(Error::ServiceUnavailable {
                    service: "test".to_string(),
                    reason: "test failure".to_string(),
                })
            })
            .await;

        let state = cb.get_state().await;
        assert!(matches!(state, CircuitState::Open { .. }));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(10),
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        // Trigger failure to open circuit
        let _ = cb
            .call(|| async {
                Err::<(), Error>(Error::ServiceUnavailable {
                    service: "test".to_string(),
                    reason: "test failure".to_string(),
                })
            })
            .await;

        let state = cb.get_state().await;
        assert!(matches!(state, CircuitState::Open { .. }));

        // Wait for recovery timeout
        sleep(Duration::from_millis(20)).await;

        // Next call should transition to half-open
        let result = cb.call(|| async { Ok::<(), Error>(()) }).await;
        assert!(result.is_ok());

        // Should eventually transition to closed on success
        let metrics = cb.get_metrics().await;
        // After success threshold is met, it should be closed
        assert!(metrics.is_healthy() || matches!(metrics.state, CircuitState::HalfOpen));
    }
}
