pub mod circuit_breaker;
pub mod health;
pub mod retry;
pub mod timeout;

// Integration tests - only compiled during testing
#[cfg(test)]
mod integration_tests;

pub use circuit_breaker::CircuitBreaker;
pub use health::{
    ComponentHealth, HealthCheck, HealthCheckManager, HealthStatus, HttpHealthCheck,
    PingHealthCheck,
};
pub use retry::{retry, retry_with_policy, RetryConfig, RetryPolicy, RetryableOperation};
pub use timeout::{TimeoutConfig, TimeoutExt};
