use crate::error::ErrorCategory;
use crate::{Error, Result};
// Removed unused backoff imports - we implement our own calculation
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Maximum jitter as percentage of delay
    pub jitter: f64,
    /// Timeout for individual attempts
    pub attempt_timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: 0.1, // 10% jitter
            attempt_timeout: Duration::from_secs(30),
        }
    }
}

impl RetryConfig {
    /// Create config for fast retries (for transient network issues)
    #[must_use]
    pub const fn fast() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            multiplier: 1.5,
            jitter: 0.1,
            attempt_timeout: Duration::from_secs(10),
        }
    }

    /// Create config for slow retries (for service unavailable)
    #[must_use]
    pub const fn slow() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
            jitter: 0.2,
            attempt_timeout: Duration::from_secs(60),
        }
    }

    /// Create config for rate limited retries
    #[must_use]
    pub const fn rate_limited() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300), // 5 minutes
            multiplier: 1.5,
            jitter: 0.3,
            attempt_timeout: Duration::from_secs(30),
        }
    }
}

/// Retry policy that determines retry behavior based on error
pub struct RetryPolicy {
    default_config: RetryConfig,
    fast_config: RetryConfig,
    slow_config: RetryConfig,
    rate_limited_config: RetryConfig,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            default_config: RetryConfig::default(),
            fast_config: RetryConfig::fast(),
            slow_config: RetryConfig::slow(),
            rate_limited_config: RetryConfig::rate_limited(),
        }
    }
}

impl RetryPolicy {
    /// Get retry config based on error
    #[must_use]
    pub const fn config_for_error(&self, error: &Error) -> Option<&RetryConfig> {
        match error.category() {
            ErrorCategory::Permanent => None, // Don't retry permanent errors
            ErrorCategory::CircuitBreaker => None, // Don't retry circuit breaker errors
            ErrorCategory::RateLimited => Some(&self.rate_limited_config),
            ErrorCategory::Transient => {
                // Choose config based on error type
                match error {
                    Error::NetworkTimeout { .. }
                    | Error::ConnectionRefused { .. }
                    | Error::DnsFailure { .. } => Some(&self.fast_config),

                    Error::ServiceUnavailable { .. }
                    | Error::ServiceOverloaded { .. }
                    | Error::InternalServerError(_) => Some(&self.slow_config),

                    _ => Some(&self.default_config),
                }
            }
        }
    }
}

/// Trait for operations that can be retried
pub trait RetryableOperation<T> {
    type Future: Future<Output = Result<T>>;

    fn call(&self) -> Self::Future;
}

/// Execute an operation with retry logic
pub async fn retry_with_policy<T, F, Fut>(
    operation: F,
    policy: &RetryPolicy,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    #[allow(unused_assignments)]
    let mut last_error: Option<Error> = None;
    let mut attempt = 1;

    loop {
        debug!(
            "Executing operation '{}' (attempt {})",
            operation_name, attempt
        );

        let result = operation().await;

        match result {
            Ok(value) => {
                if attempt > 1 {
                    debug!(
                        "Operation '{}' succeeded after {} attempts",
                        operation_name, attempt
                    );
                }
                return Ok(value);
            }
            Err(error) => {
                last_error = Some(error);
                let error_ref = last_error.as_ref().unwrap();

                // Check if error is retryable
                let retry_config = if let Some(config) = policy.config_for_error(error_ref) {
                    config
                } else {
                    debug!(
                        "Operation '{}' failed with non-retryable error: {}",
                        operation_name, error_ref
                    );
                    return Err(last_error.unwrap());
                };

                // Check if we've exceeded max attempts
                if attempt >= retry_config.max_attempts {
                    warn!(
                        "Operation '{}' failed after {} attempts: {}",
                        operation_name, attempt, error_ref
                    );
                    return Err(last_error.unwrap());
                }

                // Calculate delay for next attempt
                let delay = calculate_delay(attempt - 1, retry_config, error_ref);

                debug!(
                    "Operation '{}' failed (attempt {}), retrying after {:?}: {}",
                    operation_name, attempt, delay, error_ref
                );

                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Calculate delay for retry attempt
fn calculate_delay(attempt: u32, config: &RetryConfig, error: &Error) -> Duration {
    // Use error-specific delay if available (e.g., Retry-After header)
    if let Some(retry_after) = error.retry_after() {
        return retry_after.min(config.max_delay);
    }

    // Calculate exponential backoff delay
    let base_delay_ms = config.initial_delay.as_millis() as f64;
    let exponential_delay_ms = base_delay_ms * config.multiplier.powi(attempt as i32);
    let capped_delay_ms = exponential_delay_ms.min(config.max_delay.as_millis() as f64);
    let delay = Duration::from_millis(capped_delay_ms as u64);

    // Add jitter to prevent thundering herd
    add_jitter(delay, config.jitter)
}

/// Add jitter to delay
fn add_jitter(delay: Duration, jitter_factor: f64) -> Duration {
    if jitter_factor <= 0.0 {
        return delay;
    }

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let jitter_ms = (delay.as_millis() as f64 * jitter_factor) as u64;
    let jitter = rng.gen_range(0..=jitter_ms);

    delay + Duration::from_millis(jitter)
}

/// Convenience function for simple retry with default policy
pub async fn retry<T, F, Fut>(operation: F, operation_name: &str) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    retry_with_policy(operation, &RetryPolicy::default(), operation_name).await
}

/// Convenience function for retry with custom config
pub async fn retry_with_config<T, F, Fut>(
    operation: F,
    config: RetryConfig,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let policy = RetryPolicy {
        default_config: config.clone(),
        fast_config: config.clone(),
        slow_config: config.clone(),
        rate_limited_config: config,
    };

    retry_with_policy(operation, &policy, operation_name).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let result = retry(|| async { Ok::<u32, Error>(42) }, "test_operation").await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry(
            move || {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 2 {
                        Err(Error::ServiceUnavailable {
                            service: "test".to_string(),
                            reason: "temporary failure".to_string(),
                        })
                    } else {
                        Ok(42u32)
                    }
                }
            },
            "test_operation",
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_permanent_error_no_retry() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry(
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    Err::<u32, Error>(Error::InvalidInput {
                        field: "test".to_string(),
                        reason: "invalid".to_string(),
                    })
                }
            },
            "test_operation",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Should not retry
    }

    #[tokio::test]
    async fn test_retry_max_attempts() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 2,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };

        let result = retry_with_config(
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    Err::<u32, Error>(Error::ServiceUnavailable {
                        service: "test".to_string(),
                        reason: "always fails".to_string(),
                    })
                }
            },
            config,
            "test_operation",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2); // Should try exactly max_attempts times
    }

    #[test]
    fn test_jitter_calculation() {
        let delay = Duration::from_millis(1000);
        let jittered = add_jitter(delay, 0.1);

        // Jittered delay should be between 1000ms and 1100ms
        assert!(jittered >= delay);
        assert!(jittered <= delay + Duration::from_millis(100));
    }

    #[test]
    fn test_error_categorization() {
        let policy = RetryPolicy::default();

        // Permanent error should not have retry config
        let permanent_error = Error::InvalidInput {
            field: "test".to_string(),
            reason: "invalid".to_string(),
        };
        assert!(policy.config_for_error(&permanent_error).is_none());

        // Transient error should have retry config
        let transient_error = Error::ServiceUnavailable {
            service: "test".to_string(),
            reason: "temporary".to_string(),
        };
        assert!(policy.config_for_error(&transient_error).is_some());

        // Rate limited error should have rate limited config
        let rate_limited_error = Error::RateLimitExceeded {
            retry_after: Duration::from_secs(60),
        };
        let config = policy.config_for_error(&rate_limited_error).unwrap();
        assert_eq!(config.max_delay, Duration::from_secs(300));
    }
}
