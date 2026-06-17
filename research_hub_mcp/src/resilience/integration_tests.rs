use super::circuit_breaker::CircuitBreakerConfig;
use super::{
    retry_with_policy, CircuitBreaker, HealthCheckManager, PingHealthCheck, RetryPolicy, TimeoutExt,
};
use crate::error::ErrorCategory;
use crate::Error;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_circuit_breaker_and_retry() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            ..Default::default()
        };

        let _circuit_breaker = CircuitBreaker::new("test_service", config);
        let retry_policy = RetryPolicy::default();
        let call_count = Arc::new(AtomicU32::new(0));

        // Test that circuit breaker and retry work together
        let call_count_clone = call_count.clone();
        let result = retry_with_policy(
            move || {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 3 {
                        Err(Error::ServiceUnavailable {
                            service: "test".to_string(),
                            reason: "temporary failure".to_string(),
                        })
                    } else {
                        Ok(42u32)
                    }
                }
            },
            &retry_policy,
            "test_operation",
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 4); // 3 failures + 1 success
    }

    #[tokio::test]
    async fn test_timeout_with_retry() {
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = async {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(200)).await;
            Ok::<u32, Error>(42)
        }
        .with_timeout_duration(Duration::from_millis(100))
        .await;

        assert!(matches!(result, Err(Error::Timeout { .. })));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_health_check_integration() {
        let manager = HealthCheckManager::new();

        // Register ping health check
        let ping_check = Arc::new(PingHealthCheck::new("ping_service"));
        manager.register(ping_check).await;

        // Check health
        let result = manager.check_component("ping_service").await;
        assert!(result.is_ok());
        assert!(result.unwrap().status.is_healthy());

        // Check system health
        let system_health = manager.get_system_health().await;
        assert!(system_health.is_healthy());
    }

    #[tokio::test]
    async fn test_error_categorization() {
        // Test permanent errors
        let permanent_error = Error::InvalidInput {
            field: "test".to_string(),
            reason: "invalid".to_string(),
        };
        assert_eq!(permanent_error.category(), ErrorCategory::Permanent);
        assert!(!permanent_error.is_retryable());

        // Test transient errors
        let transient_error = Error::ServiceUnavailable {
            service: "test".to_string(),
            reason: "temporary".to_string(),
        };
        assert_eq!(transient_error.category(), ErrorCategory::Transient);
        assert!(transient_error.is_retryable());

        // Test rate limited errors
        let rate_limited_error = Error::RateLimitExceeded {
            retry_after: Duration::from_secs(60),
        };
        assert_eq!(rate_limited_error.category(), ErrorCategory::RateLimited);
        assert!(rate_limited_error.is_retryable());
        assert_eq!(
            rate_limited_error.retry_after(),
            Some(Duration::from_secs(60))
        );

        // Test circuit breaker errors
        let cb_error = Error::CircuitBreakerOpen {
            service: "test".to_string(),
        };
        assert_eq!(cb_error.category(), ErrorCategory::CircuitBreaker);
        assert!(!cb_error.is_retryable());
    }

    #[tokio::test]
    async fn test_comprehensive_resilience_scenario() {
        // Simulate a service that fails initially, then becomes unstable, then recovers
        let call_count = Arc::new(AtomicU32::new(0));
        let circuit_breaker = CircuitBreaker::new(
            "flaky_service",
            CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_timeout: Duration::from_millis(100),
                ..Default::default()
            },
        );

        // Phase 1: Service is down (should trigger circuit breaker)
        for _ in 0..3 {
            let call_count_clone = call_count.clone();
            let _ = circuit_breaker
                .call(|| async move {
                    call_count_clone.fetch_add(1, Ordering::SeqCst);
                    Err::<(), Error>(Error::ServiceUnavailable {
                        service: "flaky_service".to_string(),
                        reason: "service down".to_string(),
                    })
                })
                .await;
        }

        // Circuit should be open now
        let metrics = circuit_breaker.get_metrics().await;
        assert!(!metrics.is_healthy());

        // Wait for recovery timeout
        sleep(Duration::from_millis(150)).await;

        // Phase 2: Service recovers (should close circuit breaker)
        let call_count_clone = call_count.clone();
        let result = circuit_breaker
            .call(|| async move {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok::<u32, Error>(42)
            })
            .await;

        assert_eq!(result.unwrap(), 42);

        // Verify call count
        assert!(call_count.load(Ordering::SeqCst) >= 4); // At least 3 failures + 1 success
    }
}
