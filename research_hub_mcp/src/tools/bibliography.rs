use crate::{Config, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

/// Input parameters for the bibliography tool
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BibliographyInput {
    /// List of DOIs or paper identifiers
    #[schemars(
        description = "Array of DOIs/identifiers. No limit on quantity. Fetches metadata for 30 papers concurrently."
    )]
    pub identifiers: Vec<String>,

    /// Citation format (bibtex, apa, mla, chicago, ieee, harvard)
    #[schemars(
        description = "Citation format: 'bibtex' (default), 'apa', 'mla', 'chicago', 'ieee', or 'harvard'"
    )]
    #[serde(default = "default_format")]
    pub format: CitationFormat,

    /// Include abstract in citation
    #[schemars(description = "Include paper abstract in the citation (default: false)")]
    #[serde(default)]
    pub include_abstract: bool,

    /// Include keywords in citation
    #[schemars(description = "Include paper keywords in the citation (default: false)")]
    #[serde(default)]
    pub include_keywords: bool,
}

const fn default_format() -> CitationFormat {
    CitationFormat::BibTeX
}

/// Citation format types
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CitationFormat {
    #[default]
    BibTeX,
    APA,
    MLA,
    Chicago,
    IEEE,
    Harvard,
}

/// Result of bibliography generation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BibliographyResult {
    /// Generated citations
    pub citations: Vec<Citation>,

    /// Combined bibliography text
    pub bibliography: String,

    /// Format used
    pub format: CitationFormat,

    /// Errors encountered for specific papers
    pub errors: Vec<CitationError>,
}

/// Individual citation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Citation {
    /// Paper identifier (DOI or URL)
    pub identifier: String,

    /// Citation text
    pub text: String,

    /// Citation key (for BibTeX)
    pub key: Option<String>,

    /// Paper metadata
    pub metadata: PaperMetadata,
}

/// Citation error
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CitationError {
    /// Paper identifier that failed
    pub identifier: String,

    /// Error message
    pub message: String,
}

/// Paper metadata for citations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaperMetadata {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub publication_date: Option<String>,
}

/// Bibliography generation tool
#[derive(Debug, Clone)]
pub struct BibliographyTool {
    _config: Arc<Config>,
    http_client: reqwest::Client,
}

#[allow(dead_code)]
impl BibliographyTool {
    /// Create a new bibliography tool
    pub fn new(config: Arc<Config>) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("knowledge_accumulator_mcp/0.1.0 (mailto:research@example.com)")
            .build()
            .map_err(|e| crate::Error::Service(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            _config: config,
            http_client,
        })
    }

    /// Generate bibliography from paper identifiers with parallel metadata fetching
    #[instrument(skip(self))]
    pub async fn generate(&self, input: BibliographyInput) -> Result<BibliographyResult> {
        info!(
            "Generating bibliography for {} papers in {:?} format with parallel metadata fetching",
            input.identifiers.len(),
            input.format
        );

        // Cap identifiers to prevent unbounded task spawn (R-6)
        let max_identifiers = 200;
        if input.identifiers.len() > max_identifiers {
            return Err(crate::Error::Service(format!(
                "Too many identifiers: {}. Maximum is {max_identifiers}.",
                input.identifiers.len()
            )));
        }

        // Use semaphore to limit concurrent metadata fetches
        let semaphore = Arc::new(tokio::sync::Semaphore::new(30));

        // Create tasks for parallel metadata fetching
        let mut tasks = Vec::new();

        for identifier in input.identifiers.clone() {
            let semaphore = semaphore.clone();
            let format = input.format.clone();
            let include_abstract = input.include_abstract;
            let include_keywords = input.include_keywords;
            let client = self.http_client.clone();
            let identifier_for_task = identifier.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.map_err(|e| {
                    crate::Error::Service(format!("Failed to acquire bibliography semaphore: {e}"))
                })?;

                debug!("Fetching metadata for: {}", identifier_for_task);

                let metadata =
                    Self::fetch_metadata_from_crossref(&client, &identifier_for_task).await?;

                let citation = Self::format_citation_static(
                    &metadata,
                    &identifier_for_task,
                    &format,
                    include_abstract,
                    include_keywords,
                );

                Ok::<Citation, crate::Error>(citation)
            });

            tasks.push((identifier, task));
        }

        // Wait for all tasks to complete and collect results
        let mut citations = Vec::new();
        let mut errors = Vec::new();

        for (identifier, task) in tasks {
            match task.await {
                Ok(Ok(citation)) => {
                    citations.push(citation);
                }
                Ok(Err(e)) => {
                    warn!("Failed to generate citation for {}: {}", identifier, e);
                    errors.push(CitationError {
                        identifier,
                        message: e.to_string(),
                    });
                }
                Err(e) => {
                    error!("Task failed for {}: {}", identifier, e);
                    errors.push(CitationError {
                        identifier,
                        message: format!("Task execution failed: {e}"),
                    });
                }
            }
        }

        // Sort citations to maintain order (optional - by identifier)
        citations.sort_by(|a, b| a.identifier.cmp(&b.identifier));

        // Generate combined bibliography
        let bibliography = self.combine_citations(&citations, &input.format);

        info!(
            "Bibliography generation completed: {} citations, {} errors",
            citations.len(),
            errors.len()
        );

        Ok(BibliographyResult {
            citations,
            bibliography,
            format: input.format,
            errors,
        })
    }

    /// Normalize a DOI: strip URL prefixes, whitespace, return bare DOI like "10.1234/xyz"
    fn normalize_doi(identifier: &str) -> Option<String> {
        let trimmed = identifier.trim();

        // Strip known URL prefixes
        let doi = if let Some(rest) = trimmed.strip_prefix("https://doi.org/") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("http://doi.org/") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("https://dx.doi.org/") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("http://dx.doi.org/") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("doi:") {
            rest
        } else {
            trimmed
        };

        let doi = doi.trim();

        // A valid DOI starts with "10."
        if doi.starts_with("10.") && doi.len() > 4 {
            Some(doi.to_string())
        } else {
            None
        }
    }

    /// Fetch metadata from CrossRef API for a given identifier (DOI)
    async fn fetch_metadata_from_crossref(
        client: &reqwest::Client,
        identifier: &str,
    ) -> Result<PaperMetadata> {
        let doi = Self::normalize_doi(identifier).ok_or_else(|| {
            crate::Error::Service(format!(
                "Invalid DOI format: '{identifier}'. Expected format: 10.xxxx/yyyy"
            ))
        })?;

        let url = format!("https://api.crossref.org/works/{doi}");
        debug!("Fetching CrossRef metadata from: {}", url);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::Error::Service(format!("CrossRef request failed for {doi}: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(crate::Error::Service(format!(
                "CrossRef returned {status} for DOI {doi}. The DOI may not exist."
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| crate::Error::Service(format!("Failed to parse CrossRef JSON: {e}")))?;

        let message = json
            .get("message")
            .ok_or_else(|| crate::Error::Service("CrossRef response missing 'message'".into()))?;

        // Parse authors → APA format: "Family, G." (family name + given initial)
        let authors = message
            .get("author")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|author| {
                        let family = author.get("family")?.as_str()?;
                        let given = author.get("given").and_then(|g| g.as_str());
                        match given {
                            Some(g) => {
                                // Convert "John Michael" → "J. M."
                                let initials: String = g
                                    .split_whitespace()
                                    .filter_map(|part| part.chars().next())
                                    .map(|c| format!("{c}."))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                Some(format!("{family}, {initials}"))
                            }
                            None => Some(family.to_string()),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Parse title
        let title = message
            .get("title")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|t| t.as_str())
            .unwrap_or("Untitled")
            .to_string();

        // Parse year from published-print or published-online or issued
        let year = ["published-print", "published-online", "issued"]
            .iter()
            .find_map(|field| {
                message
                    .get(*field)?
                    .get("date-parts")?
                    .as_array()?
                    .first()?
                    .as_array()?
                    .first()?
                    .as_i64()
                    .map(|y| y as i32)
            });

        // Parse publication date string (YYYY-MM-DD)
        let publication_date = ["published-print", "published-online", "issued"]
            .iter()
            .find_map(|field| {
                let parts = message
                    .get(*field)?
                    .get("date-parts")?
                    .as_array()?
                    .first()?
                    .as_array()?;
                let y = parts.first()?.as_i64()?;
                let m = parts.get(1).and_then(|v| v.as_i64());
                let d = parts.get(2).and_then(|v| v.as_i64());
                match (m, d) {
                    (Some(m), Some(d)) => Some(format!("{y}-{m:02}-{d:02}")),
                    (Some(m), None) => Some(format!("{y}-{m:02}")),
                    _ => Some(format!("{y}")),
                }
            });

        // Journal name
        let journal = message
            .get("container-title")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|j| j.as_str())
            .map(String::from);

        // Volume, issue, pages
        let volume = message
            .get("volume")
            .and_then(|v| v.as_str())
            .map(String::from);
        let issue = message
            .get("issue")
            .and_then(|v| v.as_str())
            .map(String::from);
        let pages = message
            .get("page")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Abstract
        let abstract_text = message
            .get("abstract")
            .and_then(|a| a.as_str())
            .map(|s| {
                // CrossRef abstracts often contain JATS XML tags — strip them
                strip_xml_tags(s).trim().to_string()
            });

        // Keywords / subjects
        let keywords = message
            .get("subject")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|k| k.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // URL
        let url = message
            .get("URL")
            .and_then(|u| u.as_str())
            .map(String::from);

        info!(
            "CrossRef metadata fetched: \"{}\" by {} ({})",
            title,
            authors.first().unwrap_or(&"Unknown".to_string()),
            year.map_or("n.d.".to_string(), |y| y.to_string())
        );

        Ok(PaperMetadata {
            title,
            authors,
            year,
            journal,
            volume,
            issue,
            pages,
            doi: Some(doi),
            url,
            abstract_text,
            keywords,
            publication_date,
        })
    }

    /// Format a citation based on the selected style (static version for async tasks)
    fn format_citation_static(
        metadata: &PaperMetadata,
        identifier: &str,
        format: &CitationFormat,
        include_abstract: bool,
        include_keywords: bool,
    ) -> Citation {
        let text = match format {
            CitationFormat::BibTeX => {
                Self::format_bibtex_static(metadata, include_abstract, include_keywords)
            }
            CitationFormat::APA => Self::format_apa_static(metadata),
            CitationFormat::MLA => Self::format_mla_static(metadata),
            CitationFormat::Chicago => Self::format_chicago_static(metadata),
            CitationFormat::IEEE => Self::format_ieee_static(metadata),
            CitationFormat::Harvard => Self::format_harvard_static(metadata),
        };

        let key = match format {
            CitationFormat::BibTeX => Some(Self::generate_bibtex_key_static(metadata)),
            _ => None,
        };

        Citation {
            identifier: identifier.to_string(),
            text,
            key,
            metadata: metadata.clone(),
        }
    }

    /// Format a citation based on the selected style (instance method - delegates to static)
    fn format_citation(
        &self,
        metadata: &PaperMetadata,
        identifier: &str,
        format: &CitationFormat,
        include_abstract: bool,
        include_keywords: bool,
    ) -> Citation {
        Self::format_citation_static(
            metadata,
            identifier,
            format,
            include_abstract,
            include_keywords,
        )
    }

    /// Format as BibTeX
    fn format_bibtex(
        &self,
        metadata: &PaperMetadata,
        include_abstract: bool,
        include_keywords: bool,
    ) -> String {
        let key = self.generate_bibtex_key(metadata);
        let mut parts = vec![
            format!("@article{{{},", key),
            format!("  title = {{{}}},", metadata.title),
            format!("  author = {{{}}},", metadata.authors.join(" and ")),
        ];

        if let Some(year) = metadata.year {
            parts.push(format!("  year = {{{year}}},"));
        }

        if let Some(ref journal) = metadata.journal {
            parts.push(format!("  journal = {{{journal}}},"));
        }

        if let Some(ref volume) = metadata.volume {
            parts.push(format!("  volume = {{{volume}}},"));
        }

        if let Some(ref issue) = metadata.issue {
            parts.push(format!("  number = {{{issue}}},"));
        }

        if let Some(ref pages) = metadata.pages {
            parts.push(format!("  pages = {{{pages}}},"));
        }

        if let Some(ref doi) = metadata.doi {
            parts.push(format!("  doi = {{{doi}}},"));
        }

        if let Some(ref url) = metadata.url {
            parts.push(format!("  url = {{{url}}},"));
        }

        if include_abstract {
            if let Some(ref abstract_text) = metadata.abstract_text {
                parts.push(format!("  abstract = {{{abstract_text}}},"));
            }
        }

        if include_keywords && !metadata.keywords.is_empty() {
            parts.push(format!(
                "  keywords = {{{}}},",
                metadata.keywords.join(", ")
            ));
        }

        // Remove trailing comma from last entry
        if let Some(last) = parts.last_mut() {
            if last.ends_with(',') {
                last.pop();
            }
        }

        parts.push("}".to_string());
        parts.join("\n")
    }

    /// Generate BibTeX key
    fn generate_bibtex_key(&self, metadata: &PaperMetadata) -> String {
        let first_author = metadata
            .authors
            .first()
            .and_then(|a| a.split(',').next())
            .unwrap_or("Unknown");

        let year = metadata
            .year
            .map_or_else(|| "0000".to_string(), |y| y.to_string());

        let title_word = metadata.title.split_whitespace().next().unwrap_or("Paper");

        format!(
            "{}{}{}",
            first_author.replace(' ', ""),
            year,
            title_word.chars().take(4).collect::<String>()
        )
    }

    /// Format as APA
    fn format_apa(&self, metadata: &PaperMetadata) -> String {
        let authors = self.format_authors_apa(&metadata.authors);
        let year = metadata
            .year
            .map_or_else(|| "(n.d.)".to_string(), |y| format!("({y})"));

        let mut citation = format!("{}. {}. {}.", authors, year, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(" {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(", {volume}"));

                if let Some(ref issue) = metadata.issue {
                    citation.push_str(&format!("({issue})"));
                }
            }

            if let Some(ref pages) = metadata.pages {
                citation.push_str(&format!(", {pages}"));
            }
        }

        if let Some(ref doi) = metadata.doi {
            citation.push_str(&format!(". https://doi.org/{doi}"));
        }

        citation
    }

    /// Format authors for APA style
    fn format_authors_apa(&self, authors: &[String]) -> String {
        match authors.len() {
            0 => "Unknown".to_string(),
            1 => authors[0].clone(),
            2 => format!("{}, & {}", authors[0], authors[1]),
            _ => {
                let first_authors = &authors[..authors.len() - 1];
                let last_author = &authors[authors.len() - 1];
                format!("{}, & {}", first_authors.join(", "), last_author)
            }
        }
    }

    /// Format as MLA
    fn format_mla(&self, metadata: &PaperMetadata) -> String {
        let authors = if metadata.authors.len() > 1 {
            format!("{}, et al", metadata.authors[0])
        } else {
            metadata
                .authors
                .first()
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string())
        };

        let mut citation = format!("{}. \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(". {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(", vol. {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!(", no. {issue}"));
            }
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(", {year}"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(", pp. {pages}"));
        }

        citation.push('.');
        citation
    }

    /// Format as Chicago
    fn format_chicago(&self, metadata: &PaperMetadata) -> String {
        let authors = metadata.authors.join(", ");
        let mut citation = format!("{}. \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(". {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(" {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!(", no. {issue}"));
            }
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(" ({year})"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(": {pages}"));
        }

        citation.push('.');
        citation
    }

    /// Format as IEEE
    fn format_ieee(&self, metadata: &PaperMetadata) -> String {
        let authors = metadata
            .authors
            .iter()
            .map(|a| {
                let parts: Vec<&str> = a.split(',').collect();
                if parts.len() >= 2 {
                    format!(
                        "{}. {}",
                        parts[1].trim().chars().next().unwrap_or('?'),
                        parts[0].trim()
                    )
                } else {
                    a.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let mut citation = format!("{}, \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(", {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(", vol. {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!(", no. {issue}"));
            }

            if let Some(ref pages) = metadata.pages {
                citation.push_str(&format!(", pp. {pages}"));
            }
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(", {year}"));
        }

        citation.push('.');
        citation
    }

    /// Format as Harvard
    fn format_harvard(&self, metadata: &PaperMetadata) -> String {
        let authors = metadata.authors.join(", ");
        let year = metadata
            .year
            .map_or_else(|| "n.d.".to_string(), |y| y.to_string());

        let mut citation = format!("{} {}, '{}'", authors, year, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(", {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(", vol. {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!(", no. {issue}"));
            }

            if let Some(ref pages) = metadata.pages {
                citation.push_str(&format!(", pp. {pages}"));
            }
        }

        citation.push('.');
        citation
    }

    /// Combine citations into a bibliography
    fn combine_citations(&self, citations: &[Citation], format: &CitationFormat) -> String {
        if matches!(format, CitationFormat::BibTeX) {
            citations
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            // For other formats, sort alphabetically and number
            let mut sorted_citations = citations.to_vec();
            sorted_citations.sort_by(|a, b| {
                a.metadata
                    .authors
                    .first()
                    .unwrap_or(&String::new())
                    .cmp(b.metadata.authors.first().unwrap_or(&String::new()))
            });

            sorted_citations
                .iter()
                .enumerate()
                .map(|(i, c)| format!("[{}] {}", i + 1, c.text))
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }

    // ================================
    // Static versions for async tasks
    // ================================

    /// Format as BibTeX (static version)
    fn format_bibtex_static(
        metadata: &PaperMetadata,
        include_abstract: bool,
        include_keywords: bool,
    ) -> String {
        let key = Self::generate_bibtex_key_static(metadata);
        let mut parts = vec![
            format!("@article{{{},", key),
            format!("  title = {{{}}},", metadata.title),
            format!("  author = {{{}}},", metadata.authors.join(" and ")),
        ];

        if let Some(year) = metadata.year {
            parts.push(format!("  year = {{{year}}},"));
        }

        if let Some(ref journal) = metadata.journal {
            parts.push(format!("  journal = {{{journal}}},"));
        }

        if let Some(ref volume) = metadata.volume {
            parts.push(format!("  volume = {{{volume}}},"));
        }

        if let Some(ref issue) = metadata.issue {
            parts.push(format!("  number = {{{issue}}},"));
        }

        if let Some(ref pages) = metadata.pages {
            parts.push(format!("  pages = {{{pages}}},"));
        }

        if let Some(ref doi) = metadata.doi {
            parts.push(format!("  doi = {{{doi}}},"));
        }

        if let Some(ref url) = metadata.url {
            parts.push(format!("  url = {{{url}}},"));
        }

        if include_abstract {
            if let Some(ref abstract_text) = metadata.abstract_text {
                parts.push(format!("  abstract = {{{abstract_text}}},"));
            }
        }

        if include_keywords && !metadata.keywords.is_empty() {
            parts.push(format!(
                "  keywords = {{{}}},",
                metadata.keywords.join(", ")
            ));
        }

        // Remove trailing comma from last entry
        if let Some(last) = parts.last_mut() {
            if last.ends_with(',') {
                last.pop();
            }
        }

        parts.push("}".to_string());
        parts.join("\n")
    }

    /// Generate BibTeX key (static version)
    fn generate_bibtex_key_static(metadata: &PaperMetadata) -> String {
        let first_author = metadata
            .authors
            .first()
            .and_then(|a| a.split(',').next())
            .unwrap_or("Unknown");

        let year = metadata
            .year
            .map_or_else(|| "0000".to_string(), |y| y.to_string());

        let title_word = metadata.title.split_whitespace().next().unwrap_or("Paper");

        format!(
            "{}{}{}",
            first_author.replace(' ', ""),
            year,
            title_word.chars().take(4).collect::<String>()
        )
    }

    /// Format as APA (static version)
    fn format_apa_static(metadata: &PaperMetadata) -> String {
        let authors = Self::format_authors_apa_static(&metadata.authors);
        let year = metadata
            .year
            .map_or_else(|| "(n.d.)".to_string(), |y| format!("({y})"));

        let mut citation = format!("{}. {}. {}.", authors, year, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(" {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(", {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!("({issue})"));
            }

            if let Some(ref pages) = metadata.pages {
                citation.push_str(&format!(", {pages}"));
            }
        }

        if let Some(ref doi) = metadata.doi {
            citation.push_str(&format!(". https://doi.org/{doi}"));
        }

        citation
    }

    /// Format authors for APA style (static version)
    fn format_authors_apa_static(authors: &[String]) -> String {
        match authors.len() {
            0 => "Unknown Author".to_string(),
            1 => authors[0].clone(),
            2 => format!("{}, & {}", authors[0], authors[1]),
            _ => {
                let first_authors = &authors[..authors.len() - 1];
                let last_author = &authors[authors.len() - 1];
                format!("{}, & {}", first_authors.join(", "), last_author)
            }
        }
    }

    /// Format as MLA (static version)
    fn format_mla_static(metadata: &PaperMetadata) -> String {
        let authors = metadata.authors.join(", ");
        let mut citation = format!("{}. \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(". {journal}"));
        }

        if let Some(ref volume) = metadata.volume {
            citation.push_str(&format!(", vol. {volume}"));
        }

        if let Some(ref issue) = metadata.issue {
            citation.push_str(&format!(", no. {issue}"));
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(", {year}"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(", pp. {pages}"));
        }

        citation.push('.');
        citation
    }

    /// Format as Chicago (static version)
    fn format_chicago_static(metadata: &PaperMetadata) -> String {
        let authors = metadata.authors.join(", ");
        let mut citation = format!("{}. \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(". {journal}"));

            if let Some(ref volume) = metadata.volume {
                citation.push_str(&format!(" {volume}"));
            }

            if let Some(ref issue) = metadata.issue {
                citation.push_str(&format!(", no. {issue}"));
            }
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(" ({year})"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(": {pages}"));
        }

        citation.push('.');
        citation
    }

    /// Format as IEEE (static version)
    fn format_ieee_static(metadata: &PaperMetadata) -> String {
        let authors = metadata
            .authors
            .iter()
            .map(|a| {
                let parts: Vec<&str> = a.split(',').collect();
                if parts.len() >= 2 {
                    format!(
                        "{}. {}",
                        parts[1].trim().chars().next().unwrap_or('?'),
                        parts[0].trim()
                    )
                } else {
                    a.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let mut citation = format!("{}, \"{}\"", authors, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(", {journal}"));
        }

        if let Some(ref volume) = metadata.volume {
            citation.push_str(&format!(", vol. {volume}"));
        }

        if let Some(ref issue) = metadata.issue {
            citation.push_str(&format!(", no. {issue}"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(", pp. {pages}"));
        }

        if let Some(year) = metadata.year {
            citation.push_str(&format!(", {year}"));
        }

        citation.push('.');
        citation
    }

    /// Format as Harvard (static version)
    fn format_harvard_static(metadata: &PaperMetadata) -> String {
        let authors = metadata.authors.join(", ");
        let year = metadata
            .year
            .map_or_else(|| "n.d.".to_string(), |y| y.to_string());

        let mut citation = format!("{} {}, '{}',", authors, year, metadata.title);

        if let Some(ref journal) = metadata.journal {
            citation.push_str(&format!(" {journal},"));
        }

        if let Some(ref volume) = metadata.volume {
            citation.push_str(&format!(" vol. {volume},"));
        }

        if let Some(ref issue) = metadata.issue {
            citation.push_str(&format!(" no. {issue},"));
        }

        if let Some(ref pages) = metadata.pages {
            citation.push_str(&format!(" pp. {pages}"));
        }

        citation.push('.');
        citation
    }
}

/// Strip XML/JATS tags from text (e.g. CrossRef abstracts)
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metadata() -> PaperMetadata {
        PaperMetadata {
            title: "Test Paper".to_string(),
            authors: vec!["Smith, J.".to_string()],
            year: Some(2024),
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            doi: None,
            url: None,
            abstract_text: None,
            keywords: vec![],
            publication_date: None,
        }
    }

    #[test]
    fn test_bibtex_key_generation() {
        let metadata = test_metadata();
        let key = BibliographyTool::generate_bibtex_key_static(&metadata);
        assert_eq!(key, "Smith2024Test");
    }

    #[test]
    fn test_normalize_doi_bare() {
        assert_eq!(
            BibliographyTool::normalize_doi("10.1038/s41586-021-03819-2"),
            Some("10.1038/s41586-021-03819-2".to_string())
        );
    }

    #[test]
    fn test_normalize_doi_https_prefix() {
        assert_eq!(
            BibliographyTool::normalize_doi("https://doi.org/10.1038/s41586-021-03819-2"),
            Some("10.1038/s41586-021-03819-2".to_string())
        );
    }

    #[test]
    fn test_normalize_doi_dx_prefix() {
        assert_eq!(
            BibliographyTool::normalize_doi("http://dx.doi.org/10.1234/abc"),
            Some("10.1234/abc".to_string())
        );
    }

    #[test]
    fn test_normalize_doi_invalid() {
        assert_eq!(BibliographyTool::normalize_doi("not-a-doi"), None);
        assert_eq!(BibliographyTool::normalize_doi(""), None);
        assert_eq!(BibliographyTool::normalize_doi("isbn:978-3-16-148410-0"), None);
    }

    #[test]
    fn test_apa_format_journal_article() {
        let metadata = PaperMetadata {
            title: "Machine learning approaches".to_string(),
            authors: vec!["Smith, J.".to_string(), "Doe, A. B.".to_string()],
            year: Some(2023),
            journal: Some("Nature".to_string()),
            volume: Some("615".to_string()),
            issue: Some("7953".to_string()),
            pages: Some("620-630".to_string()),
            doi: Some("10.1038/s41586-023-05880-3".to_string()),
            url: None,
            abstract_text: None,
            keywords: vec![],
            publication_date: None,
        };

        let citation = BibliographyTool::format_apa_static(&metadata);
        assert_eq!(
            citation,
            "Smith, J., & Doe, A. B.. (2023). Machine learning approaches. Nature, 615(7953), 620-630. https://doi.org/10.1038/s41586-023-05880-3"
        );
    }

    #[test]
    fn test_apa_format_no_doi() {
        let metadata = PaperMetadata {
            title: "A study".to_string(),
            authors: vec!["Author, A.".to_string()],
            year: Some(2020),
            journal: Some("Journal of Science".to_string()),
            volume: Some("10".to_string()),
            issue: None,
            pages: Some("1-10".to_string()),
            doi: None,
            url: None,
            abstract_text: None,
            keywords: vec![],
            publication_date: None,
        };

        let citation = BibliographyTool::format_apa_static(&metadata);
        assert!(!citation.contains("doi.org"));
        assert!(citation.contains("(2020)"));
    }

    #[test]
    fn test_apa_format_no_year() {
        let metadata = PaperMetadata {
            title: "Unknown date paper".to_string(),
            authors: vec!["Author, A.".to_string()],
            year: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            doi: None,
            url: None,
            abstract_text: None,
            keywords: vec![],
            publication_date: None,
        };

        let citation = BibliographyTool::format_apa_static(&metadata);
        assert!(citation.contains("(n.d.)"));
    }

    #[test]
    fn test_identifier_limit_enforced() {
        let config = Arc::new(Config::default());
        let tool = BibliographyTool::new(config).unwrap();
        let input = BibliographyInput {
            identifiers: (0..201).map(|i| format!("10.1234/{i}")).collect(),
            format: CitationFormat::APA,
            include_abstract: false,
            include_keywords: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.generate(input));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Too many identifiers"));
    }
}
