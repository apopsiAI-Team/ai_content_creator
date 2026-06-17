//! # Search Service Port
//!
//! Defines the port interface for paper search services.
//! This interface abstracts the paper search functionality, allowing different
//! search implementations (Sci-Hub, ArXiv, CrossRef, etc.) to be used interchangeably.

use crate::tools::search::{SearchInput, SearchResult};
use crate::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Port interface for paper search services
///
/// This trait defines the contract for searching academic papers across
/// different providers. Implementations should handle:
/// - Input validation
/// - Search execution across configured providers
/// - Result aggregation and normalization
/// - Error handling and resilience
/// - Caching for performance
///
/// # Design Principles
///
/// - **Provider Agnostic**: The interface doesn't expose provider-specific details
/// - **Resilient**: Implementations should handle provider failures gracefully
/// - **Cacheable**: Results should be cached when appropriate
/// - **Observable**: Include metrics and logging for monitoring
///
/// # Example Implementation Structure
///
/// ```rust
/// use async_trait::async_trait;
/// use crate::ports::SearchServicePort;
/// use crate::tools::search::{SearchInput, SearchResult};
/// use crate::Result;
///
/// pub struct MetaSearchAdapter {
///     // Implementation details...
/// }
///
/// #[async_trait]
/// impl SearchServicePort for MetaSearchAdapter {
///     async fn search_papers(&self, input: SearchInput) -> Result<SearchResult> {
///         // 1. Validate input
///         // 2. Execute search across providers
///         // 3. Aggregate and normalize results
///         // 4. Cache results
///         // 5. Return formatted response
///         todo!()
///     }
///
///     async fn health_check(&self) -> Result<ServiceHealth> {
///         // Check provider availability
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait SearchServicePort: Send + Sync + Debug {
    /// Search for academic papers based on the provided input
    ///
    /// # Arguments
    ///
    /// * `input` - Search parameters including query, type, limits, etc.
    ///
    /// # Returns
    ///
    /// A `SearchResult` containing:
    /// - Found papers with metadata
    /// - Search statistics (timing, provider counts)
    /// - Pagination information
    /// - Suggested categorization
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input validation fails
    /// - All providers are unavailable
    /// - Critical system errors occur
    ///
    /// Partial failures (some providers failing) should be handled gracefully
    /// and reported in the result metadata.
    async fn search_papers(&self, input: SearchInput) -> Result<SearchResult>;

    /// Check the health of the search service and its providers
    ///
    /// # Returns
    ///
    /// A `ServiceHealth` struct containing:
    /// - Overall service status
    /// - Individual provider health statuses
    /// - Performance metrics
    /// - Error counts and rates
    ///
    /// This is used for monitoring and determining if the service
    /// is ready to handle requests.
    async fn health_check(&self) -> Result<ServiceHealth>;

    /// Get search service statistics and metrics
    ///
    /// # Returns
    ///
    /// A map of metric names to values, including:
    /// - Total searches performed
    /// - Success/failure rates
    /// - Average response times
    /// - Cache hit rates
    /// - Provider-specific metrics
    async fn get_metrics(&self) -> Result<std::collections::HashMap<String, serde_json::Value>>;

    /// Clear any cached search results
    ///
    /// This is useful for:
    /// - Testing with fresh data
    /// - Forcing re-validation of cached results
    /// - Clearing stale cache entries
    async fn clear_cache(&self) -> Result<()>;
}

/// Health status of the search service
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ServiceHealth {
    /// Overall service status
    pub status: HealthStatus,
    /// Health of individual providers
    pub providers: std::collections::HashMap<String, ProviderHealth>,
    /// Last health check timestamp
    pub checked_at: std::time::SystemTime,
    /// Service uptime in seconds
    pub uptime_seconds: u64,
    /// Error rate percentage (0-100)
    pub error_rate_percent: f64,
}

/// Health status enumeration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Service is fully operational
    Healthy,
    /// Service is operational but with some degradation
    Degraded,
    /// Service is not operational
    Unhealthy,
    /// Service status cannot be determined
    Unknown,
}

/// Health information for individual providers
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderHealth {
    /// Provider health status
    pub status: HealthStatus,
    /// Last successful request timestamp
    pub last_success: Option<std::time::SystemTime>,
    /// Last failure timestamp
    pub last_failure: Option<std::time::SystemTime>,
    /// Response time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Error message if unhealthy
    pub error_message: Option<String>,
    /// Circuit breaker state
    pub circuit_breaker_state: CircuitBreakerState,
}

/// Circuit breaker state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests are flowing normally
    Closed,
    /// Circuit is open, requests are being rejected
    Open,
    /// Circuit is half-open, testing if service has recovered
    HalfOpen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"healthy\"");

        let status = HealthStatus::Degraded;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"degraded\"");
    }

    #[test]
    fn test_circuit_breaker_state_serialization() {
        let state = CircuitBreakerState::Closed;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"closed\"");

        let state = CircuitBreakerState::Open;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"open\"");
    }

    #[test]
    fn test_service_health_creation() {
        let health = ServiceHealth {
            status: HealthStatus::Healthy,
            providers: std::collections::HashMap::new(),
            checked_at: std::time::SystemTime::now(),
            uptime_seconds: 3600,
            error_rate_percent: 0.5,
        };

        assert!(matches!(health.status, HealthStatus::Healthy));
        assert_eq!(health.uptime_seconds, 3600);
        assert!((health.error_rate_percent - 0.5).abs() < f64::EPSILON);
    }
}
