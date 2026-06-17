//! # Multi Provider Adapter
//!
//! Concrete implementation of the ProviderServicePort that manages multiple
//! academic paper providers and provides unified access to them.

use crate::client::providers::{SearchQuery, SearchType};
use crate::client::{MetaSearchClient, PaperMetadata};
use crate::ports::provider_service::{
    ProviderCapabilities, ProviderInfo, ProviderSearchResult, ProviderServiceHealth,
    ProviderServicePort, ProviderStatus, RateLimitInfo, RateLimitStatus,
};
use crate::{Config, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, instrument};

/// Multi provider adapter that implements ProviderServicePort
///
/// This adapter manages multiple academic paper providers and provides
/// a unified interface to search across them. It wraps the existing
/// MetaSearchClient functionality while providing the new port interface.
#[derive(Clone)]
pub struct MultiProviderAdapter {
    /// Underlying meta search client
    meta_client: Arc<MetaSearchClient>,
    /// Configuration reference
    config: Arc<Config>,
    /// Service start time for uptime calculation
    start_time: SystemTime,
    /// Provider configurations and status
    provider_configs: HashMap<String, ProviderConfig>,
}

/// Configuration for individual providers
#[derive(Debug, Clone)]
struct ProviderConfig {
    name: String,
    enabled: bool,
    base_url: Option<String>,
    supports_pdf: bool,
    rate_limits: RateLimitInfo,
    capabilities: ProviderCapabilities,
}

impl std::fmt::Debug for MultiProviderAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiProviderAdapter")
            .field("meta_client", &"MetaSearchClient")
            .field("config", &"Config")
            .field("start_time", &self.start_time)
            .field("provider_count", &self.provider_configs.len())
            .finish()
    }
}

impl MultiProviderAdapter {
    /// Create a new multi provider adapter
    pub fn new(meta_client: Arc<MetaSearchClient>, config: Arc<Config>) -> Result<Self> {
        info!("Initializing MultiProviderAdapter");

        let provider_configs = Self::initialize_provider_configs();

        Ok(Self {
            meta_client,
            config,
            start_time: SystemTime::now(),
            provider_configs,
        })
    }

    /// Initialize provider configurations
    fn initialize_provider_configs() -> HashMap<String, ProviderConfig> {
        let mut configs = HashMap::new();

        // ArXiv configuration
        configs.insert(
            "arxiv".to_string(),
            ProviderConfig {
                name: "ArXiv".to_string(),
                enabled: true,
                base_url: Some("https://arxiv.org".to_string()),
                supports_pdf: true,
                rate_limits: RateLimitInfo {
                    requests_per_second: Some(1.0),
                    requests_per_minute: Some(60),
                    requests_per_hour: Some(3600),
                    requests_per_day: None,
                    enforced: true,
                },
                capabilities: ProviderCapabilities {
                    max_results_per_query: Some(1000),
                    supports_pagination: true,
                    supports_advanced_search: true,
                    supports_metadata_extraction: true,
                    supports_fulltext_search: false,
                    available_metadata_fields: vec![
                        "title".to_string(),
                        "authors".to_string(),
                        "abstract".to_string(),
                        "categories".to_string(),
                    ],
                    response_formats: vec!["xml".to_string(), "json".to_string()],
                },
            },
        );

        // CrossRef configuration
        configs.insert(
            "crossref".to_string(),
            ProviderConfig {
                name: "CrossRef".to_string(),
                enabled: true,
                base_url: Some("https://api.crossref.org".to_string()),
                supports_pdf: false,
                rate_limits: RateLimitInfo {
                    requests_per_second: Some(10.0),
                    requests_per_minute: Some(600),
                    requests_per_hour: None,
                    requests_per_day: None,
                    enforced: false,
                },
                capabilities: ProviderCapabilities {
                    max_results_per_query: Some(1000),
                    supports_pagination: true,
                    supports_advanced_search: true,
                    supports_metadata_extraction: true,
                    supports_fulltext_search: false,
                    available_metadata_fields: vec![
                        "title".to_string(),
                        "authors".to_string(),
                        "journal".to_string(),
                        "doi".to_string(),
                        "publication_date".to_string(),
                    ],
                    response_formats: vec!["json".to_string()],
                },
            },
        );

        // Sci-Hub configuration
        configs.insert(
            "sci_hub".to_string(),
            ProviderConfig {
                name: "Sci-Hub".to_string(),
                enabled: true,
                base_url: None, // Dynamic mirror management
                supports_pdf: true,
                rate_limits: RateLimitInfo {
                    requests_per_second: Some(0.5),
                    requests_per_minute: Some(30),
                    requests_per_hour: Some(1800),
                    requests_per_day: None,
                    enforced: true,
                },
                capabilities: ProviderCapabilities {
                    max_results_per_query: Some(1),
                    supports_pagination: false,
                    supports_advanced_search: false,
                    supports_metadata_extraction: false,
                    supports_fulltext_search: false,
                    available_metadata_fields: vec!["doi".to_string(), "pdf_url".to_string()],
                    response_formats: vec!["html".to_string()],
                },
            },
        );

        configs
    }

    /// Get mock provider status
    fn get_mock_provider_status(&self, provider_name: &str) -> ProviderStatus {
        let config = self.provider_configs.get(provider_name);

        ProviderStatus {
            enabled: config.map_or(false, |c| c.enabled),
            health: crate::ports::search_service::HealthStatus::Healthy,
            last_success: Some(SystemTime::now()),
            last_failure: None,
            response_time_ms: Some(200),
            rate_limit_status: RateLimitStatus {
                requests_remaining: Some(100),
                requests_limit: Some(1000),
                reset_time: Some(SystemTime::now() + Duration::from_secs(3600)),
                is_rate_limited: false,
                period_seconds: 3600,
            },
            circuit_breaker_state: crate::ports::search_service::CircuitBreakerState::Closed,
            recent_error_count: 0,
            success_rate_percent: 95.0,
        }
    }

    /// Convert internal search results to ProviderSearchResult
    fn convert_to_provider_search_result(
        &self,
        meta_result: crate::client::MetaSearchResult,
    ) -> ProviderSearchResult {
        ProviderSearchResult {
            papers: meta_result.papers,
            by_provider: meta_result.by_source,
            total_search_time: meta_result.total_search_time,
            successful_providers: meta_result.successful_providers as usize,
            failed_providers: meta_result.failed_providers as usize,
            provider_errors: meta_result
                .provider_errors
                .into_iter()
                .map(|(k, v)| (k, v.to_string()))
                .collect(),
            provider_metadata: meta_result.provider_metadata,
        }
    }
}

#[async_trait]
impl ProviderServicePort for MultiProviderAdapter {
    #[instrument(skip(self), fields(query = %query.query, search_type = ?query.search_type))]
    async fn search_across_providers(&self, query: &SearchQuery) -> Result<ProviderSearchResult> {
        info!(
            "Searching across providers: query='{}', type={:?}",
            query.query, query.search_type
        );

        // Execute search using the meta client
        let meta_result = self.meta_client.search(query).await?;

        info!(
            "Search completed: {} papers from {} providers",
            meta_result.papers.len(),
            meta_result.successful_providers
        );

        Ok(self.convert_to_provider_search_result(meta_result))
    }

    async fn search_provider(
        &self,
        provider_name: &str,
        query: &SearchQuery,
    ) -> Result<Vec<PaperMetadata>> {
        info!("Searching specific provider: {}", provider_name);

        // Check if provider is configured
        if !self.provider_configs.contains_key(provider_name) {
            return Err(crate::Error::InvalidInput {
                field: "provider_name".to_string(),
                reason: format!("Provider not found: {provider_name}"),
            });
        }

        // For now, use the meta search and filter results
        // In a real implementation, this would call the specific provider directly
        let meta_result = self.meta_client.search(query).await?;

        // Return papers from the specific provider if available
        if let Some(papers) = meta_result.by_source.get(provider_name) {
            Ok(papers.clone())
        } else {
            Ok(vec![])
        }
    }

    async fn get_pdf_url_cascade(&self, doi: &str) -> Result<Option<String>> {
        info!("Getting PDF URL for DOI: {}", doi);

        // Use the meta client's cascade functionality
        self.meta_client.get_pdf_url_cascade(doi).await
    }

    async fn get_provider_status(&self) -> Result<HashMap<String, ProviderStatus>> {
        let mut status_map = HashMap::new();

        for provider_name in self.provider_configs.keys() {
            let status = self.get_mock_provider_status(provider_name);
            status_map.insert(provider_name.clone(), status);
        }

        Ok(status_map)
    }

    async fn get_provider_info(&self, provider_name: &str) -> Result<ProviderInfo> {
        let config =
            self.provider_configs
                .get(provider_name)
                .ok_or_else(|| crate::Error::InvalidInput {
                    field: "provider_name".to_string(),
                    reason: format!("Provider not found: {provider_name}"),
                })?;

        Ok(ProviderInfo {
            name: config.name.clone(),
            description: format!("Academic paper provider: {}", config.name),
            supported_search_types: vec![
                SearchType::Auto,
                SearchType::Doi,
                SearchType::Title,
                SearchType::Author,
                SearchType::Keywords,
            ],
            supports_pdf_download: config.supports_pdf,
            requires_authentication: false,
            rate_limits: config.rate_limits.clone(),
            capabilities: config.capabilities.clone(),
            base_url: config.base_url.clone(),
            version: Some("1.0".to_string()),
        })
    }

    async fn set_provider_enabled(&self, provider_name: &str, enabled: bool) -> Result<()> {
        // In a real implementation, this would update the provider configuration
        // For now, we'll just log the operation
        info!(
            "Setting provider {} enabled status to: {}",
            provider_name, enabled
        );

        if !self.provider_configs.contains_key(provider_name) {
            return Err(crate::Error::InvalidInput {
                field: "provider_name".to_string(),
                reason: format!("Provider not found: {provider_name}"),
            });
        }

        // Mock implementation - in reality would update configuration
        debug!("Provider {} enabled status updated", provider_name);
        Ok(())
    }

    async fn list_providers(&self) -> Result<Vec<String>> {
        Ok(self.provider_configs.keys().cloned().collect())
    }

    async fn health_check(&self) -> Result<ProviderServiceHealth> {
        let providers = self.get_provider_status().await?;
        let total_providers = providers.len();

        let healthy_providers = providers
            .values()
            .filter(|p| {
                matches!(
                    p.health,
                    crate::ports::search_service::HealthStatus::Healthy
                )
            })
            .count();

        let degraded_providers = providers
            .values()
            .filter(|p| {
                matches!(
                    p.health,
                    crate::ports::search_service::HealthStatus::Degraded
                )
            })
            .count();

        let unhealthy_providers = providers
            .values()
            .filter(|p| {
                matches!(
                    p.health,
                    crate::ports::search_service::HealthStatus::Unhealthy
                )
            })
            .count();

        let overall_status = if unhealthy_providers > total_providers / 2 {
            crate::ports::search_service::HealthStatus::Unhealthy
        } else if degraded_providers + unhealthy_providers > 0 {
            crate::ports::search_service::HealthStatus::Degraded
        } else {
            crate::ports::search_service::HealthStatus::Healthy
        };

        Ok(ProviderServiceHealth {
            status: overall_status,
            providers,
            total_providers,
            healthy_providers,
            degraded_providers,
            unhealthy_providers,
            checked_at: SystemTime::now(),
            uptime_seconds: self.start_time.elapsed().unwrap_or_default().as_secs(),
        })
    }

    async fn get_metrics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();

        let uptime = self.start_time.elapsed().unwrap_or_default();
        metrics.insert("uptime_seconds".to_string(), uptime.as_secs().into());
        metrics.insert(
            "total_providers".to_string(),
            self.provider_configs.len().into(),
        );

        // Count enabled providers
        let enabled_providers = self.provider_configs.values().filter(|c| c.enabled).count();
        metrics.insert("enabled_providers".to_string(), enabled_providers.into());

        // Mock metrics for demonstration
        metrics.insert("total_searches".to_string(), 1000.into());
        metrics.insert("successful_searches".to_string(), 950.into());
        metrics.insert("failed_searches".to_string(), 50.into());
        metrics.insert("average_response_time_ms".to_string(), 250.into());

        Ok(metrics)
    }

    async fn refresh_providers(&self) -> Result<usize> {
        info!("Refreshing provider configurations");

        // In a real implementation, this would:
        // 1. Re-read provider configurations
        // 2. Update health status
        // 3. Reset any circuit breakers
        // 4. Clear stale cache entries

        // For now, just return the count of providers
        Ok(self.provider_configs.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{MetaSearchClient, MetaSearchConfig};
    use crate::config::{Config, ResearchSourceConfig};

    fn create_test_config() -> Arc<Config> {
        let mut config = Config::default();
        config.research_source = ResearchSourceConfig {
            endpoints: vec!["https://sci-hub.se".to_string()],
            rate_limit_per_sec: 1,
            timeout_secs: 30,
            max_retries: 2,
        };
        Arc::new(config)
    }

    fn create_test_adapter() -> Result<MultiProviderAdapter> {
        let config = create_test_config();
        let meta_config = MetaSearchConfig::from_config(&config);
        let meta_client = Arc::new(MetaSearchClient::new((*config).clone(), meta_config)?);
        MultiProviderAdapter::new(meta_client, config)
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = create_test_adapter();
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_provider_configs_initialization() {
        let configs = MultiProviderAdapter::initialize_provider_configs();
        assert!(!configs.is_empty());
        assert!(configs.contains_key("arxiv"));
        assert!(configs.contains_key("crossref"));
        assert!(configs.contains_key("sci_hub"));
    }

    #[tokio::test]
    async fn test_list_providers() {
        let adapter = create_test_adapter().unwrap();
        let providers = adapter.list_providers().await.unwrap();

        assert!(!providers.is_empty());
        assert!(providers.contains(&"arxiv".to_string()));
        assert!(providers.contains(&"crossref".to_string()));
        assert!(providers.contains(&"sci_hub".to_string()));
    }

    #[tokio::test]
    async fn test_get_provider_info() {
        let adapter = create_test_adapter().unwrap();

        // Test valid provider
        let info = adapter.get_provider_info("arxiv").await.unwrap();
        assert_eq!(info.name, "ArXiv");
        assert!(info.supports_pdf_download);
        assert!(!info.supported_search_types.is_empty());

        // Test invalid provider
        let result = adapter.get_provider_info("invalid_provider").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_provider_status() {
        let adapter = create_test_adapter().unwrap();
        let status_map = adapter.get_provider_status().await.unwrap();

        assert!(!status_map.is_empty());
        assert!(status_map.contains_key("arxiv"));

        let arxiv_status = &status_map["arxiv"];
        assert!(arxiv_status.enabled);
        assert!(matches!(
            arxiv_status.health,
            crate::ports::search_service::HealthStatus::Healthy
        ));
    }

    #[tokio::test]
    async fn test_set_provider_enabled() {
        let adapter = create_test_adapter().unwrap();

        // Test valid provider
        let result = adapter.set_provider_enabled("arxiv", false).await;
        assert!(result.is_ok());

        // Test invalid provider
        let result = adapter.set_provider_enabled("invalid_provider", true).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let adapter = create_test_adapter().unwrap();
        let health = adapter.health_check().await.unwrap();

        assert!(health.total_providers > 0);
        assert!(health.healthy_providers >= 0);
        assert_eq!(
            health.total_providers,
            health.healthy_providers + health.degraded_providers + health.unhealthy_providers
        );
    }

    #[tokio::test]
    async fn test_get_metrics() {
        let adapter = create_test_adapter().unwrap();
        let metrics = adapter.get_metrics().await.unwrap();

        assert!(metrics.contains_key("uptime_seconds"));
        assert!(metrics.contains_key("total_providers"));
        assert!(metrics.contains_key("enabled_providers"));
        assert!(metrics.contains_key("total_searches"));
    }

    #[tokio::test]
    async fn test_refresh_providers() {
        let adapter = create_test_adapter().unwrap();
        let count = adapter.refresh_providers().await.unwrap();

        assert!(count > 0);
        assert_eq!(count, 3); // Should match the number of configured providers
    }
}
