//! # Paper Repository
//!
//! This module provides data access abstraction for paper metadata and content.
//! It handles storage, retrieval, and querying of academic papers.

use super::{Repository, RepositoryError, RepositoryResult, RepositoryStats};
use crate::client::PaperMetadata;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Query parameters for searching papers
#[derive(Debug, Clone, Default)]
pub struct PaperQuery {
    /// Search by DOI
    pub doi: Option<String>,
    /// Search by title (partial match)
    pub title: Option<String>,
    /// Search by author names (partial match)
    pub authors: Vec<String>,
    /// Search by journal name (partial match)
    pub journal: Option<String>,
    /// Filter by publication year range
    pub year_range: Option<(u32, u32)>,
    /// Search in abstract text
    pub abstract_keywords: Vec<String>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

impl PaperQuery {
    /// Create a new empty query
    pub fn new() -> Self {
        Self::default()
    }

    /// Search by DOI
    pub fn with_doi(mut self, doi: impl Into<String>) -> Self {
        self.doi = Some(doi.into());
        self
    }

    /// Search by title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add author to search
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.authors.push(author.into());
        self
    }

    /// Search by journal
    pub fn with_journal(mut self, journal: impl Into<String>) -> Self {
        self.journal = Some(journal.into());
        self
    }

    /// Filter by year range (inclusive)
    pub fn with_year_range(mut self, start: u32, end: u32) -> Self {
        self.year_range = Some((start, end));
        self
    }

    /// Add abstract keyword
    pub fn with_abstract_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.abstract_keywords.push(keyword.into());
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset for pagination
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Filter options for paper search results
#[derive(Debug, Clone, Default)]
pub struct PaperFilter {
    /// Include only papers with PDF URLs
    pub has_pdf: Option<bool>,
    /// Include only papers with abstracts
    pub has_abstract: Option<bool>,
    /// Minimum file size in bytes
    pub min_file_size: Option<u64>,
    /// Maximum file size in bytes
    pub max_file_size: Option<u64>,
    /// Filter by specific sources
    pub sources: Vec<String>,
}

/// Repository trait for paper metadata and content management
#[async_trait]
pub trait PaperRepository: Repository {
    /// Store a paper in the repository
    async fn store(&self, paper: &PaperMetadata) -> RepositoryResult<()>;

    /// Store multiple papers in a batch operation
    async fn store_batch(&self, papers: &[PaperMetadata]) -> RepositoryResult<usize>;

    /// Find a paper by its DOI
    async fn find_by_doi(&self, doi: &str) -> RepositoryResult<Option<PaperMetadata>>;

    /// Search papers using query parameters
    async fn search(&self, query: &PaperQuery) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Search papers with additional filtering
    async fn search_with_filter(
        &self,
        query: &PaperQuery,
        filter: &PaperFilter,
    ) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Get papers by a list of DOIs
    async fn find_by_dois(&self, dois: &[String]) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Update an existing paper
    async fn update(&self, paper: &PaperMetadata) -> RepositoryResult<()>;

    /// Delete a paper by DOI
    async fn delete(&self, doi: &str) -> RepositoryResult<bool>;

    /// Count total papers in repository
    async fn count(&self) -> RepositoryResult<u64>;

    /// Count papers matching a query
    async fn count_matching(&self, query: &PaperQuery) -> RepositoryResult<u64>;

    /// Get papers by author
    async fn find_by_author(&self, author: &str) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Get papers by journal
    async fn find_by_journal(&self, journal: &str) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Get papers by year
    async fn find_by_year(&self, year: u32) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Get papers published in a year range
    async fn find_by_year_range(
        &self,
        start: u32,
        end: u32,
    ) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Get recently added papers
    async fn get_recent(&self, limit: usize) -> RepositoryResult<Vec<PaperMetadata>>;

    /// Check if a paper exists by DOI
    async fn exists(&self, doi: &str) -> RepositoryResult<bool>;
}

/// In-memory implementation of PaperRepository for testing and caching
#[derive(Debug)]
pub struct InMemoryPaperRepository {
    /// Papers indexed by DOI
    papers: Arc<RwLock<HashMap<String, PaperMetadata>>>,
    /// Repository statistics
    stats: Arc<RwLock<RepositoryStats>>,
    /// Creation timestamp for ordering
    creation_timestamps: Arc<RwLock<HashMap<String, Instant>>>,
}

impl InMemoryPaperRepository {
    /// Create a new in-memory paper repository
    pub fn new() -> Self {
        Self {
            papers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RepositoryStats::new())),
            creation_timestamps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Normalize DOI for consistent storage and retrieval
    fn normalize_doi(&self, doi: &str) -> String {
        doi.trim().to_lowercase()
    }

    /// Check if a paper matches the given query
    fn matches_query(&self, paper: &PaperMetadata, query: &PaperQuery) -> bool {
        // DOI match (exact)
        if let Some(ref query_doi) = query.doi {
            if self.normalize_doi(&paper.doi) != self.normalize_doi(query_doi) {
                return false;
            }
        }

        // Title match (case-insensitive partial match)
        if let Some(ref query_title) = query.title {
            if let Some(ref paper_title) = paper.title {
                if !paper_title
                    .to_lowercase()
                    .contains(&query_title.to_lowercase())
                {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Author match (case-insensitive partial match)
        if !query.authors.is_empty() {
            let found_author = query.authors.iter().any(|query_author| {
                paper.authors.iter().any(|paper_author| {
                    paper_author
                        .to_lowercase()
                        .contains(&query_author.to_lowercase())
                })
            });
            if !found_author {
                return false;
            }
        }

        // Journal match (case-insensitive partial match)
        if let Some(ref query_journal) = query.journal {
            if let Some(ref paper_journal) = paper.journal {
                if !paper_journal
                    .to_lowercase()
                    .contains(&query_journal.to_lowercase())
                {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Year range match
        if let Some((start_year, end_year)) = query.year_range {
            if let Some(paper_year) = paper.year {
                if paper_year < start_year || paper_year > end_year {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Abstract keyword match
        if !query.abstract_keywords.is_empty() {
            if let Some(ref abstract_text) = paper.abstract_text {
                let abstract_lower = abstract_text.to_lowercase();
                let found_keyword = query
                    .abstract_keywords
                    .iter()
                    .any(|keyword| abstract_lower.contains(&keyword.to_lowercase()));
                if !found_keyword {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Check if a paper matches the given filter
    fn matches_filter(&self, paper: &PaperMetadata, filter: &PaperFilter) -> bool {
        // PDF availability filter
        if let Some(has_pdf) = filter.has_pdf {
            let paper_has_pdf = paper.pdf_url.is_some();
            if paper_has_pdf != has_pdf {
                return false;
            }
        }

        // Abstract availability filter
        if let Some(has_abstract) = filter.has_abstract {
            let paper_has_abstract = paper.abstract_text.is_some();
            if paper_has_abstract != has_abstract {
                return false;
            }
        }

        // File size filters
        if let Some(file_size) = paper.file_size {
            if let Some(min_size) = filter.min_file_size {
                if file_size < min_size {
                    return false;
                }
            }
            if let Some(max_size) = filter.max_file_size {
                if file_size > max_size {
                    return false;
                }
            }
        } else if filter.min_file_size.is_some() {
            // Paper has no file size but filter requires minimum size
            return false;
        }

        // Source filter (would need to be added to PaperMetadata)
        // For now, we'll skip this filter

        true
    }

    /// Apply pagination to results
    fn apply_pagination<T>(&self, mut results: Vec<T>, query: &PaperQuery) -> Vec<T> {
        // Apply offset
        if let Some(offset) = query.offset {
            if offset < results.len() {
                results.drain(0..offset);
            } else {
                return Vec::new();
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    /// Record operation timing and update stats
    async fn record_operation(&self, start_time: Instant, success: bool) {
        let duration_ms = start_time.elapsed().as_millis() as f64;
        let mut stats = self.stats.write().await;
        if success {
            stats.record_success(duration_ms);
        } else {
            stats.record_failure(duration_ms);
        }
    }
}

impl Default for InMemoryPaperRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Repository for InMemoryPaperRepository {
    fn name(&self) -> &'static str {
        "InMemoryPaperRepository"
    }

    async fn health_check(&self) -> RepositoryResult<bool> {
        // For in-memory repository, health check is always successful
        // unless there's a critical memory issue
        Ok(true)
    }

    async fn clear(&self) -> RepositoryResult<()> {
        let start_time = Instant::now();

        {
            let mut papers = self.papers.write().await;
            let mut timestamps = self.creation_timestamps.write().await;
            papers.clear();
            timestamps.clear();
        }

        // Reset stats but preserve them for debugging
        {
            let mut stats = self.stats.write().await;
            *stats = RepositoryStats::new();
        }

        self.record_operation(start_time, true).await;
        info!("Cleared all papers from in-memory repository");
        Ok(())
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let papers = self.papers.read().await;
        let mut stats = self.stats.read().await.clone();

        stats.total_entities = papers.len() as u64;

        // Estimate memory usage (rough calculation)
        let estimated_memory = papers
            .iter()
            .map(|(doi, paper)| {
                doi.len()
                    + paper.title.as_ref().map_or(0, |t| t.len())
                    + paper.authors.iter().map(|a| a.len()).sum::<usize>()
                    + paper.journal.as_ref().map_or(0, |j| j.len())
                    + paper.abstract_text.as_ref().map_or(0, |a| a.len())
                    + paper.pdf_url.as_ref().map_or(0, |p| p.len())
                    + 32 // Approximate overhead per paper
            })
            .sum::<usize>();

        stats.memory_usage_bytes = Some(estimated_memory as u64);

        Ok(stats)
    }
}

#[async_trait]
impl PaperRepository for InMemoryPaperRepository {
    async fn store(&self, paper: &PaperMetadata) -> RepositoryResult<()> {
        let start_time = Instant::now();

        if paper.doi.trim().is_empty() {
            self.record_operation(start_time, false).await;
            return Err(RepositoryError::Validation {
                field: "doi".to_string(),
                message: "DOI cannot be empty".to_string(),
            });
        }

        let normalized_doi = self.normalize_doi(&paper.doi);

        {
            let mut papers = self.papers.write().await;
            let mut timestamps = self.creation_timestamps.write().await;

            papers.insert(normalized_doi.clone(), paper.clone());
            timestamps.insert(normalized_doi.clone(), Instant::now());
        }

        self.record_operation(start_time, true).await;
        debug!("Stored paper with DOI: {}", normalized_doi);
        Ok(())
    }

    async fn store_batch(&self, papers: &[PaperMetadata]) -> RepositoryResult<usize> {
        let start_time = Instant::now();
        let mut stored_count = 0;

        for paper in papers {
            if let Err(e) = self.store(paper).await {
                warn!("Failed to store paper {}: {}", paper.doi, e);
                continue;
            }
            stored_count += 1;
        }

        self.record_operation(start_time, stored_count > 0).await;
        info!(
            "Stored {} out of {} papers in batch",
            stored_count,
            papers.len()
        );
        Ok(stored_count)
    }

    async fn find_by_doi(&self, doi: &str) -> RepositoryResult<Option<PaperMetadata>> {
        let start_time = Instant::now();
        let normalized_doi = self.normalize_doi(doi);

        let papers = self.papers.read().await;
        let result = papers.get(&normalized_doi).cloned();

        self.record_operation(start_time, true).await;

        if result.is_some() {
            debug!("Found paper with DOI: {}", normalized_doi);
        } else {
            debug!("Paper not found with DOI: {}", normalized_doi);
        }

        Ok(result)
    }

    async fn search(&self, query: &PaperQuery) -> RepositoryResult<Vec<PaperMetadata>> {
        self.search_with_filter(query, &PaperFilter::default())
            .await
    }

    async fn search_with_filter(
        &self,
        query: &PaperQuery,
        filter: &PaperFilter,
    ) -> RepositoryResult<Vec<PaperMetadata>> {
        let start_time = Instant::now();

        let papers = self.papers.read().await;

        let mut results: Vec<PaperMetadata> = papers
            .values()
            .filter(|paper| self.matches_query(paper, query))
            .filter(|paper| self.matches_filter(paper, filter))
            .cloned()
            .collect();

        // Sort by publication year (newest first) for consistent ordering
        results.sort_by(|a, b| match (b.year, a.year) {
            (Some(year_b), Some(year_a)) => year_b.cmp(&year_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        // Apply pagination
        results = self.apply_pagination(results, query);

        self.record_operation(start_time, true).await;
        debug!("Search returned {} results", results.len());
        Ok(results)
    }

    async fn find_by_dois(&self, dois: &[String]) -> RepositoryResult<Vec<PaperMetadata>> {
        let start_time = Instant::now();
        let mut results = Vec::new();

        for doi in dois {
            if let Some(paper) = self.find_by_doi(doi).await? {
                results.push(paper);
            }
        }

        self.record_operation(start_time, true).await;
        debug!(
            "Found {} papers out of {} DOIs requested",
            results.len(),
            dois.len()
        );
        Ok(results)
    }

    async fn update(&self, paper: &PaperMetadata) -> RepositoryResult<()> {
        let start_time = Instant::now();

        if paper.doi.trim().is_empty() {
            self.record_operation(start_time, false).await;
            return Err(RepositoryError::Validation {
                field: "doi".to_string(),
                message: "DOI cannot be empty".to_string(),
            });
        }

        let normalized_doi = self.normalize_doi(&paper.doi);

        {
            let mut papers = self.papers.write().await;

            if !papers.contains_key(&normalized_doi) {
                self.record_operation(start_time, false).await;
                return Err(RepositoryError::NotFound {
                    entity_type: "Paper".to_string(),
                    id: normalized_doi,
                });
            }

            papers.insert(normalized_doi.clone(), paper.clone());
        }

        self.record_operation(start_time, true).await;
        debug!("Updated paper with DOI: {}", normalized_doi);
        Ok(())
    }

    async fn delete(&self, doi: &str) -> RepositoryResult<bool> {
        let start_time = Instant::now();
        let normalized_doi = self.normalize_doi(doi);

        let removed = {
            let mut papers = self.papers.write().await;
            let mut timestamps = self.creation_timestamps.write().await;

            let removed = papers.remove(&normalized_doi).is_some();
            timestamps.remove(&normalized_doi);
            removed
        };

        self.record_operation(start_time, true).await;

        if removed {
            debug!("Deleted paper with DOI: {}", normalized_doi);
        } else {
            debug!("Paper not found for deletion with DOI: {}", normalized_doi);
        }

        Ok(removed)
    }

    async fn count(&self) -> RepositoryResult<u64> {
        let papers = self.papers.read().await;
        Ok(papers.len() as u64)
    }

    async fn count_matching(&self, query: &PaperQuery) -> RepositoryResult<u64> {
        let papers = self.papers.read().await;
        let count = papers
            .values()
            .filter(|paper| self.matches_query(paper, query))
            .count() as u64;
        Ok(count)
    }

    async fn find_by_author(&self, author: &str) -> RepositoryResult<Vec<PaperMetadata>> {
        let query = PaperQuery::new().with_author(author);
        self.search(&query).await
    }

    async fn find_by_journal(&self, journal: &str) -> RepositoryResult<Vec<PaperMetadata>> {
        let query = PaperQuery::new().with_journal(journal);
        self.search(&query).await
    }

    async fn find_by_year(&self, year: u32) -> RepositoryResult<Vec<PaperMetadata>> {
        let query = PaperQuery::new().with_year_range(year, year);
        self.search(&query).await
    }

    async fn find_by_year_range(
        &self,
        start: u32,
        end: u32,
    ) -> RepositoryResult<Vec<PaperMetadata>> {
        let query = PaperQuery::new().with_year_range(start, end);
        self.search(&query).await
    }

    async fn get_recent(&self, limit: usize) -> RepositoryResult<Vec<PaperMetadata>> {
        let start_time = Instant::now();

        let papers = self.papers.read().await;
        let timestamps = self.creation_timestamps.read().await;

        let mut paper_times: Vec<(String, Instant)> = timestamps
            .iter()
            .map(|(doi, time)| (doi.clone(), *time))
            .collect();

        // Sort by creation time (newest first)
        paper_times.sort_by(|a, b| b.1.cmp(&a.1));
        paper_times.truncate(limit);

        let results: Vec<PaperMetadata> = paper_times
            .into_iter()
            .filter_map(|(doi, _)| papers.get(&doi).cloned())
            .collect();

        self.record_operation(start_time, true).await;
        debug!("Retrieved {} recent papers", results.len());
        Ok(results)
    }

    async fn exists(&self, doi: &str) -> RepositoryResult<bool> {
        let normalized_doi = self.normalize_doi(doi);
        let papers = self.papers.read().await;
        Ok(papers.contains_key(&normalized_doi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_paper(doi: &str) -> PaperMetadata {
        PaperMetadata {
            doi: doi.to_string(),
            title: Some(format!("Test Paper {}", doi)),
            authors: vec!["Test Author".to_string()],
            journal: Some("Test Journal".to_string()),
            year: Some(2023),
            abstract_text: Some("This is a test abstract".to_string()),
            pdf_url: Some("https://example.com/paper.pdf".to_string()),
            file_size: Some(1024),
        }
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let repo = InMemoryPaperRepository::new();
        let paper = create_test_paper("10.1000/test1");

        // Store paper
        assert!(repo.store(&paper).await.is_ok());

        // Retrieve paper
        let retrieved = repo.find_by_doi("10.1000/test1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().doi, "10.1000/test1");
    }

    #[tokio::test]
    async fn test_search_by_title() {
        let repo = InMemoryPaperRepository::new();
        let paper = create_test_paper("10.1000/test2");
        repo.store(&paper).await.unwrap();

        let query = PaperQuery::new().with_title("Test Paper");
        let results = repo.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_by_author() {
        let repo = InMemoryPaperRepository::new();
        let paper = create_test_paper("10.1000/test3");
        repo.store(&paper).await.unwrap();

        let query = PaperQuery::new().with_author("Test Author");
        let results = repo.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_pagination() {
        let repo = InMemoryPaperRepository::new();

        // Store multiple papers
        for i in 1..=5 {
            let paper = create_test_paper(&format!("10.1000/test{}", i));
            repo.store(&paper).await.unwrap();
        }

        // Test limit
        let query = PaperQuery::new().limit(3);
        let results = repo.search(&query).await.unwrap();
        assert_eq!(results.len(), 3);

        // Test offset and limit
        let query = PaperQuery::new().offset(2).limit(2);
        let results = repo.search(&query).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_by_pdf() {
        let repo = InMemoryPaperRepository::new();

        let mut paper_with_pdf = create_test_paper("10.1000/with_pdf");
        let mut paper_without_pdf = create_test_paper("10.1000/without_pdf");
        paper_without_pdf.pdf_url = None;

        repo.store(&paper_with_pdf).await.unwrap();
        repo.store(&paper_without_pdf).await.unwrap();

        let query = PaperQuery::new();
        let filter = PaperFilter {
            has_pdf: Some(true),
            ..Default::default()
        };

        let results = repo.search_with_filter(&query, &filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doi, "10.1000/with_pdf");
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let repo = InMemoryPaperRepository::new();

        let papers = (1..=3)
            .map(|i| create_test_paper(&format!("10.1000/batch{}", i)))
            .collect::<Vec<_>>();

        let stored_count = repo.store_batch(&papers).await.unwrap();
        assert_eq!(stored_count, 3);

        let count = repo.count().await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_repository_stats() {
        let repo = InMemoryPaperRepository::new();
        let paper = create_test_paper("10.1000/stats_test");

        repo.store(&paper).await.unwrap();

        let stats = repo.stats().await.unwrap();
        assert_eq!(stats.total_entities, 1);
        assert!(stats.successful_operations > 0);
        assert!(stats.memory_usage_bytes.is_some());
    }
}
