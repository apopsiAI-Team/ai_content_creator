use super::traits::{
    ProviderError, ProviderResult, SearchContext, SearchQuery, SearchType, SourceProvider,
};
use crate::client::PaperMetadata;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// bioRxiv API response for paper details
#[derive(Debug, Deserialize)]
struct BiorxivResponse {
    messages: Vec<BiorxivMessage>,
    collection: Vec<BiorxivPaper>,
}

/// Status message from bioRxiv API
#[derive(Debug, Deserialize)]
struct BiorxivMessage {
    status: String,
    text: String,
}

/// Individual paper from bioRxiv API
#[derive(Debug, Deserialize)]
struct BiorxivPaper {
    doi: String,
    title: String,
    authors: String,
    #[allow(dead_code)]
    author_corresponding: Option<String>,
    #[allow(dead_code)]
    author_corresponding_institution: Option<String>,
    date: String, // Format: YYYY-MM-DD
    #[allow(dead_code)]
    version: Option<u32>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    paper_type: Option<String>,
    #[allow(dead_code)]
    category: Option<String>,
    #[allow(dead_code)]
    jatsxml: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    #[allow(dead_code)]
    published: Option<String>,
    server: String, // "biorxiv" or "medrxiv"
}

/// bioRxiv provider for biology preprints
pub struct BiorxivProvider {
    client: Client,
    base_url: String,
}

impl BiorxivProvider {
    /// Create a new bioRxiv provider
    pub fn new() -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("knowledge_accumulator_mcp/0.2.1 (Academic Research Tool)")
            .build()
            .map_err(|e| ProviderError::Network(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: "https://api.biorxiv.org".to_string(),
        })
    }

    /// Build DOI lookup URL for bioRxiv API
    fn build_doi_url(&self, doi: &str) -> String {
        format!("{}/details/biorxiv/{}", self.base_url, doi)
    }

    /// Build search URL for bioRxiv API (by date range)
    fn build_date_search_url(&self, start_date: &str, end_date: &str) -> String {
        format!(
            "{}/details/biorxiv/{}/{}",
            self.base_url, start_date, end_date
        )
    }

    /// Extract DOI from various bioRxiv formats
    fn extract_biorxiv_doi(doi_or_url: &str) -> Option<String> {
        // Handle various bioRxiv DOI formats:
        // - 10.1101/2023.01.01.000001
        // - https://doi.org/10.1101/2023.01.01.000001
        // - https://www.biorxiv.org/content/10.1101/2023.01.01.000001v1

        if doi_or_url.contains("10.1101/") {
            // Extract the DOI part
            if let Some(doi_start) = doi_or_url.find("10.1101/") {
                let doi_part = &doi_or_url[doi_start..];
                // Remove version suffix if present (e.g., "v1", "v2")
                if let Some(version_pos) = doi_part.find('v') {
                    if version_pos > 8 {
                        // Ensure it's not part of the date
                        return Some(doi_part[..version_pos].to_string());
                    }
                }
                return Some(doi_part.to_string());
            }
        }
        None
    }

    /// Convert bioRxiv paper to `PaperMetadata`
    fn convert_paper(&self, paper: BiorxivPaper) -> PaperMetadata {
        // Parse authors from the comma-separated string
        let authors: Vec<String> = paper
            .authors
            .split(',')
            .map(|author| author.trim().to_string())
            .filter(|author| !author.is_empty())
            .collect();

        // Extract year from date (YYYY-MM-DD format)
        let year = paper
            .date
            .split('-')
            .next()
            .and_then(|year_str| year_str.parse::<u32>().ok());

        // Generate PDF URL based on DOI
        let pdf_url = Some(format!(
            "https://www.biorxiv.org/content/biorxiv/early/{}/{}.full.pdf",
            paper.date.replace('-', "/"),
            paper.doi
        ));

        PaperMetadata {
            doi: paper.doi,
            title: Some(paper.title),
            authors,
            journal: Some(format!("{} preprint", paper.server)), // "biorxiv preprint" or "medrxiv preprint"
            year,
            abstract_text: paper.abstract_text,
            pdf_url,
            file_size: None,
        }
    }

    /// Get paper by DOI from bioRxiv
    async fn get_paper_by_doi(&self, doi: &str) -> Result<Option<PaperMetadata>, ProviderError> {
        let url = self.build_doi_url(doi);
        debug!("Getting paper by DOI from bioRxiv: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Network(format!("Request failed: {e}")))?;

        if response.status().as_u16() == 404 {
            debug!("Paper not found in bioRxiv for DOI: {}", doi);
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(ProviderError::Network(format!(
                "API request failed with status: {}",
                response.status()
            )));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| ProviderError::Network(format!("Failed to read response: {e}")))?;

        debug!("bioRxiv response: {}", response_text);

        let biorxiv_response: BiorxivResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                warn!("Failed to parse bioRxiv response: {}", response_text);
                ProviderError::Parse(format!("Failed to parse JSON: {e}"))
            })?;

        // Check for error messages
        for message in &biorxiv_response.messages {
            if message.status != "ok" {
                warn!("bioRxiv API message: {}", message.text);
            }
        }

        // Return the first paper if found
        if let Some(paper) = biorxiv_response.collection.into_iter().next() {
            Ok(Some(self.convert_paper(paper)))
        } else {
            Ok(None)
        }
    }

    /// Search for papers by date range (bioRxiv doesn't support general text search)
    async fn search_recent_papers(
        &self,
        days_back: u32,
        limit: u32,
    ) -> Result<Vec<PaperMetadata>, ProviderError> {
        use chrono::{Duration as ChronoDuration, Utc};

        // Calculate date range
        let end_date = Utc::now();
        let start_date = end_date - ChronoDuration::days(i64::from(days_back));

        let start_date_str = start_date.format("%Y-%m-%d").to_string();
        let end_date_str = end_date.format("%Y-%m-%d").to_string();

        let url = self.build_date_search_url(&start_date_str, &end_date_str);
        debug!("Searching bioRxiv by date range: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Network(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(ProviderError::Network(format!(
                "API request failed with status: {}",
                response.status()
            )));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| ProviderError::Network(format!("Failed to read response: {e}")))?;

        debug!("bioRxiv search response: {}", response_text);

        let biorxiv_response: BiorxivResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                warn!("Failed to parse bioRxiv search response: {}", response_text);
                ProviderError::Parse(format!("Failed to parse JSON: {e}"))
            })?;

        // Check for error messages
        for message in &biorxiv_response.messages {
            if message.status != "ok" {
                warn!("bioRxiv API message: {}", message.text);
            }
        }

        // Convert papers and limit results
        let papers: Vec<PaperMetadata> = biorxiv_response
            .collection
            .into_iter()
            .take(limit as usize)
            .map(|paper| self.convert_paper(paper))
            .collect();

        Ok(papers)
    }
}

#[async_trait]
impl SourceProvider for BiorxivProvider {
    fn name(&self) -> &'static str {
        "biorxiv"
    }

    fn description(&self) -> &'static str {
        "bioRxiv - Biology preprint server"
    }

    fn supported_search_types(&self) -> Vec<SearchType> {
        vec![SearchType::Doi, SearchType::Keywords] // Limited search capabilities
    }

    fn supports_full_text(&self) -> bool {
        true // bioRxiv provides PDF access for all preprints
    }

    fn priority(&self) -> u8 {
        75 // Lower priority - more specialized for biology preprints
    }

    fn base_delay(&self) -> Duration {
        Duration::from_millis(500) // Be respectful to the free API
    }

    async fn search(
        &self,
        query: &SearchQuery,
        _context: &SearchContext,
    ) -> Result<ProviderResult, ProviderError> {
        let start_time = Instant::now();

        info!(
            "Searching bioRxiv for: {} (type: {:?})",
            query.query, query.search_type
        );

        let papers = match query.search_type {
            SearchType::Doi => {
                // Check if this is a bioRxiv DOI
                if let Some(biorxiv_doi) = Self::extract_biorxiv_doi(&query.query) {
                    if let Some(paper) = self.get_paper_by_doi(&biorxiv_doi).await? {
                        vec![paper]
                    } else {
                        Vec::new()
                    }
                } else {
                    // Not a bioRxiv DOI, return empty
                    Vec::new()
                }
            }
            SearchType::Keywords => {
                // bioRxiv doesn't support text search, so we search recent papers
                // This is a limitation of the bioRxiv API
                warn!("bioRxiv doesn't support keyword search, returning recent papers");
                self.search_recent_papers(30, query.max_results).await?
            }
            _ => {
                // bioRxiv doesn't support other search types
                warn!(
                    "bioRxiv only supports DOI and limited keyword searches, ignoring query: {}",
                    query.query
                );
                Vec::new()
            }
        };

        let search_time = start_time.elapsed();
        let papers_count = papers.len();

        let result = ProviderResult {
            papers,
            source: "bioRxiv".to_string(),
            total_available: Some(u32::try_from(papers_count).unwrap_or(u32::MAX)),
            search_time,
            has_more: false, // bioRxiv API doesn't support pagination in our simple implementation
            metadata: HashMap::new(),
        };

        info!(
            "bioRxiv search completed: {} papers found in {:?}",
            result.papers.len(),
            search_time
        );

        Ok(result)
    }

    async fn get_by_doi(
        &self,
        doi: &str,
        _context: &SearchContext,
    ) -> Result<Option<PaperMetadata>, ProviderError> {
        info!("Getting paper by DOI from bioRxiv: {}", doi);

        // Check if this is a bioRxiv DOI first
        if let Some(biorxiv_doi) = Self::extract_biorxiv_doi(doi) {
            self.get_paper_by_doi(&biorxiv_doi).await
        } else {
            // Not a bioRxiv DOI
            Ok(None)
        }
    }

    async fn health_check(&self, _context: &SearchContext) -> Result<bool, ProviderError> {
        debug!("Performing bioRxiv health check");

        // Use a known bioRxiv DOI for health check
        let test_url = self.build_doi_url("10.1101/2020.01.01.000001");

        match self.client.get(&test_url).send().await {
            Ok(response) if response.status().is_success() || response.status().as_u16() == 404 => {
                info!("bioRxiv health check: OK");
                Ok(true)
            }
            Ok(response) => {
                warn!(
                    "bioRxiv health check failed with status: {}",
                    response.status()
                );
                Ok(false)
            }
            Err(e) => {
                warn!("bioRxiv health check failed: {}", e);
                Ok(false)
            }
        }
    }

    async fn get_pdf_url(
        &self,
        doi: &str,
        context: &SearchContext,
    ) -> Result<Option<String>, ProviderError> {
        // For bioRxiv, if we can get the paper, we can construct the PDF URL
        if let Some(paper) = self.get_by_doi(doi, context).await? {
            Ok(paper.pdf_url)
        } else {
            Ok(None)
        }
    }
}

impl Default for BiorxivProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create BiorxivProvider")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biorxiv_provider_creation() {
        let provider = BiorxivProvider::new();
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_interface() {
        let provider = BiorxivProvider::new().unwrap();

        assert_eq!(provider.name(), "biorxiv");
        assert!(provider.supports_full_text());
        assert_eq!(provider.priority(), 75);
        assert!(provider.supported_search_types().contains(&SearchType::Doi));
    }

    #[test]
    fn test_biorxiv_doi_extraction() {
        let _provider = BiorxivProvider::new().unwrap();

        let test_cases = vec![
            (
                "10.1101/2023.01.01.000001",
                Some("10.1101/2023.01.01.000001"),
            ),
            (
                "https://doi.org/10.1101/2023.01.01.000001",
                Some("10.1101/2023.01.01.000001"),
            ),
            (
                "https://www.biorxiv.org/content/10.1101/2023.01.01.000001v1",
                Some("10.1101/2023.01.01.000001"),
            ),
            ("10.1038/nature12373", None), // Not a bioRxiv DOI
        ];

        for (input, expected) in test_cases {
            let result = BiorxivProvider::extract_biorxiv_doi(input);
            assert_eq!(result.as_deref(), expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_url_building() {
        let provider = BiorxivProvider::new().unwrap();

        let doi_url = provider.build_doi_url("10.1101/2023.01.01.000001");
        assert!(doi_url.contains("api.biorxiv.org"));
        assert!(doi_url.contains("details/biorxiv"));
        assert!(doi_url.contains("10.1101/2023.01.01.000001"));

        let search_url = provider.build_date_search_url("2023-01-01", "2023-01-31");
        assert!(search_url.contains("2023-01-01"));
        assert!(search_url.contains("2023-01-31"));
    }
}
