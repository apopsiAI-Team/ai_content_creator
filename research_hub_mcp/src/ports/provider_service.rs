//! # Provider Service Port
//!
//! Defines the port interface for academic paper provider services.
//! This interface abstracts the provider management functionality, allowing different
//! provider implementations to be used interchangeably.

use crate::client::providers::{SearchQuery, SearchType};
use crate::client::PaperMetadata;
use crate::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Port interface for academic paper provider services
///
/// This trait defines the contract for managing and interacting with
/// academic paper providers (ArXiv, CrossRef, Sci-Hub, etc.).
/// Implementations should handle:
/// - Provider discovery and health monitoring
/// - Search query routing and execution
/// - Provider-specific data normalization
/// - Circuit breaking and fault tolerance
/// - Rate limiting and quota management
///
/// # Design Principles
///
/// - **Provider Agnostic**: Uniform interface regardless of provider
/// - **Resilient**: Handle provider failures and rate limits gracefully
/// - **Observable**: Provide metrics and health monitoring
/// - **Extensible**: Easy to add new providers
/// - **Efficient**: Support parallel queries and caching
///
/// # Example Implementation Structure
///
/// ```rust
/// use async_trait::async_trait;
/// use crate::ports::ProviderServicePort;
/// use crate::client::providers::SearchQuery;
/// use crate::Result;
///
/// pub struct MultiProviderAdapter {
///     // Implementation details...
/// }
///
/// #[async_trait]
/// impl ProviderServicePort for MultiProviderAdapter {
///     async fn search_across_providers(&self, query: &SearchQuery) -> Result<ProviderSearchResult> {
///         // 1. Select appropriate providers based on query
///         // 2. Execute searches in parallel with circuit breaking
///         // 3. Aggregate and normalize results
///         // 4. Apply rate limiting and caching
///         // 5. Return consolidated results
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait ProviderServicePort: Send + Sync + Debug {
    /// Search for papers across multiple providers
    ///
    /// # Arguments
    ///
    /// * `query` - Search query with type, parameters, and options
    ///
    /// # Returns
    ///
    /// A `ProviderSearchResult` containing:
    /// - Papers found across all providers
    /// - Results grouped by provider
    /// - Search timing and statistics
    /// - Provider status information
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - All providers are unavailable
    /// - Query validation fails
    /// - Critical system errors occur
    ///
    /// Individual provider failures should be handled gracefully
    /// and reported in the result statistics.
    async fn search_across_providers(&self, query: &SearchQuery) -> Result<ProviderSearchResult>;

    /// Search using a specific provider
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the specific provider to use
    /// * `query` - Search query parameters
    ///
    /// # Returns
    ///
    /// Papers found by the specified provider.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Provider is not available or configured
    /// - Provider-specific errors occur
    /// - Query is not supported by the provider
    async fn search_provider(
        &self,
        provider_name: &str,
        query: &SearchQuery,
    ) -> Result<Vec<PaperMetadata>>;

    /// Get a PDF URL for a paper using cascade approach
    ///
    /// # Arguments
    ///
    /// * `doi` - DOI of the paper to get PDF for
    ///
    /// # Returns
    ///
    /// URL to download the PDF if found, None if not available.
    ///
    /// # Errors
    ///
    /// Returns an error if all providers fail or are unavailable.
    async fn get_pdf_url_cascade(&self, doi: &str) -> Result<Option<String>>;

    /// Get the status of all configured providers
    ///
    /// # Returns
    ///
    /// A map of provider names to their current status.
    async fn get_provider_status(
        &self,
    ) -> Result<std::collections::HashMap<String, ProviderStatus>>;

    /// Get detailed information about a specific provider
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the provider to get info for
    ///
    /// # Returns
    ///
    /// Detailed provider information including capabilities and status.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not found.
    async fn get_provider_info(&self, provider_name: &str) -> Result<ProviderInfo>;

    /// Enable or disable a specific provider
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the provider to modify
    /// * `enabled` - Whether to enable or disable the provider
    ///
    /// # Returns
    ///
    /// Success if the provider status was changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not found.
    async fn set_provider_enabled(&self, provider_name: &str, enabled: bool) -> Result<()>;

    /// Get list of all available providers
    ///
    /// # Returns
    ///
    /// Vector of provider names that are configured.
    async fn list_providers(&self) -> Result<Vec<String>>;

    /// Get provider service health and metrics
    ///
    /// # Returns
    ///
    /// Overall health status including individual provider health.
    async fn health_check(&self) -> Result<ProviderServiceHealth>;

    /// Get provider service metrics
    ///
    /// # Returns
    ///
    /// A map of metric names to values, including:
    /// - Total queries per provider
    /// - Success/failure rates per provider
    /// - Average response times
    /// - Rate limit status
    /// - Circuit breaker states
    async fn get_metrics(&self) -> Result<std::collections::HashMap<String, serde_json::Value>>;

    /// Refresh provider configurations and health status
    ///
    /// # Returns
    ///
    /// Success if refresh completed, with count of providers updated.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration refresh fails.
    async fn refresh_providers(&self) -> Result<usize>;
}

/// Result of searching across multiple providers
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderSearchResult {
    /// All papers found across providers (deduplicated)
    pub papers: Vec<PaperMetadata>,
    /// Results grouped by provider
    pub by_provider: std::collections::HashMap<String, Vec<PaperMetadata>>,
    /// Total search time across all providers
    pub total_search_time: std::time::Duration,
    /// Number of providers that returned results successfully
    pub successful_providers: usize,
    /// Number of providers that failed
    pub failed_providers: usize,
    /// Errors encountered per provider
    pub provider_errors: std::collections::HashMap<String, String>,
    /// Provider-specific metadata (timing, counts, etc.)
    pub provider_metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Status of an individual provider
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderStatus {
    /// Whether the provider is enabled
    pub enabled: bool,
    /// Current health status
    pub health: super::search_service::HealthStatus,
    /// Last successful query timestamp
    pub last_success: Option<std::time::SystemTime>,
    /// Last failure timestamp
    pub last_failure: Option<std::time::SystemTime>,
    /// Current response time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Rate limit status
    pub rate_limit_status: RateLimitStatus,
    /// Circuit breaker state
    pub circuit_breaker_state: super::search_service::CircuitBreakerState,
    /// Error count in the last hour
    pub recent_error_count: u32,
    /// Success rate percentage over last 100 requests
    pub success_rate_percent: f64,
}

/// Detailed information about a provider
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider description
    pub description: String,
    /// Supported search types
    pub supported_search_types: Vec<SearchType>,
    /// Whether the provider supports PDF downloads
    pub supports_pdf_download: bool,
    /// Whether the provider requires authentication
    pub requires_authentication: bool,
    /// Rate limit information
    pub rate_limits: RateLimitInfo,
    /// Provider-specific capabilities
    pub capabilities: ProviderCapabilities,
    /// Base URL or endpoint
    pub base_url: Option<String>,
    /// Provider version or API version
    pub version: Option<String>,
}

/// Rate limit status for a provider
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RateLimitStatus {
    /// Requests remaining in current period
    pub requests_remaining: Option<u32>,
    /// Total requests allowed per period
    pub requests_limit: Option<u32>,
    /// When the rate limit resets
    pub reset_time: Option<std::time::SystemTime>,
    /// Whether currently rate limited
    pub is_rate_limited: bool,
    /// Rate limit period in seconds
    pub period_seconds: u32,
}

/// Rate limit configuration for a provider
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RateLimitInfo {
    /// Requests per second limit
    pub requests_per_second: Option<f64>,
    /// Requests per minute limit
    pub requests_per_minute: Option<u32>,
    /// Requests per hour limit
    pub requests_per_hour: Option<u32>,
    /// Requests per day limit
    pub requests_per_day: Option<u32>,
    /// Whether rate limiting is enforced
    pub enforced: bool,
}

/// Provider-specific capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderCapabilities {
    /// Maximum results per query
    pub max_results_per_query: Option<u32>,
    /// Whether pagination is supported
    pub supports_pagination: bool,
    /// Whether advanced search syntax is supported
    pub supports_advanced_search: bool,
    /// Whether metadata extraction is supported
    pub supports_metadata_extraction: bool,
    /// Whether full-text search is available
    pub supports_fulltext_search: bool,
    /// Available metadata fields
    pub available_metadata_fields: Vec<String>,
    /// Response format options
    pub response_formats: Vec<String>,
}

/// Health status of the provider service
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProviderServiceHealth {
    /// Overall service status
    pub status: super::search_service::HealthStatus,
    /// Health status of individual providers
    pub providers: std::collections::HashMap<String, ProviderStatus>,
    /// Total number of configured providers
    pub total_providers: usize,
    /// Number of healthy providers
    pub healthy_providers: usize,
    /// Number of degraded providers
    pub degraded_providers: usize,
    /// Number of unhealthy providers
    pub unhealthy_providers: usize,
    /// Last health check timestamp
    pub checked_at: std::time::SystemTime,
    /// Service uptime in seconds
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_search_result_creation() {
        let result = ProviderSearchResult {
            papers: vec![],
            by_provider: std::collections::HashMap::new(),
            total_search_time: std::time::Duration::from_millis(500),
            successful_providers: 2,
            failed_providers: 1,
            provider_errors: std::collections::HashMap::new(),
            provider_metadata: std::collections::HashMap::new(),
        };

        assert_eq!(result.successful_providers, 2);
        assert_eq!(result.failed_providers, 1);
        assert_eq!(result.total_search_time.as_millis(), 500);
    }

    #[test]
    fn test_rate_limit_status_creation() {
        let status = RateLimitStatus {
            requests_remaining: Some(100),
            requests_limit: Some(1000),
            reset_time: Some(std::time::SystemTime::now()),
            is_rate_limited: false,
            period_seconds: 3600,
        };

        assert_eq!(status.requests_remaining, Some(100));
        assert_eq!(status.requests_limit, Some(1000));
        assert!(!status.is_rate_limited);
        assert_eq!(status.period_seconds, 3600);
    }

    #[test]
    fn test_provider_capabilities_creation() {
        let capabilities = ProviderCapabilities {
            max_results_per_query: Some(100),
            supports_pagination: true,
            supports_advanced_search: false,
            supports_metadata_extraction: true,
            supports_fulltext_search: false,
            available_metadata_fields: vec!["title".to_string(), "authors".to_string()],
            response_formats: vec!["json".to_string(), "xml".to_string()],
        };

        assert_eq!(capabilities.max_results_per_query, Some(100));
        assert!(capabilities.supports_pagination);
        assert!(!capabilities.supports_advanced_search);
        assert_eq!(capabilities.available_metadata_fields.len(), 2);
    }

    #[test]
    fn test_provider_service_health_creation() {
        let health = ProviderServiceHealth {
            status: super::super::search_service::HealthStatus::Healthy,
            providers: std::collections::HashMap::new(),
            total_providers: 5,
            healthy_providers: 4,
            degraded_providers: 1,
            unhealthy_providers: 0,
            checked_at: std::time::SystemTime::now(),
            uptime_seconds: 3600,
        };

        assert!(matches!(
            health.status,
            super::super::search_service::HealthStatus::Healthy
        ));
        assert_eq!(health.total_providers, 5);
        assert_eq!(health.healthy_providers, 4);
        assert_eq!(health.degraded_providers, 1);
        assert_eq!(health.unhealthy_providers, 0);
    }
}
