//! HTTP REST API server for Research Hub
//!
//! Provides REST endpoints for paper search across multiple academic sources.

use crate::tools::search::{SearchInput, SearchResult, SearchType};
use crate::{Config, Result, SearchTool};
use axum::{
    extract::{Query, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub search_tool: Arc<SearchTool>,
    pub config: Arc<Config>,
}

/// Query parameters for search endpoint
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub search_type: Option<String>,
}

fn default_limit() -> u32 {
    10
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Paper result for API response (simplified)
#[derive(Debug, Serialize)]
pub struct PaperResponse {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<u32>,
    pub doi: String,
    pub journal: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub pdf_url: Option<String>,
    pub source: String,
    pub relevance_score: f64,
}

/// Search response for API
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub papers: Vec<PaperResponse>,
    pub total_count: u32,
    pub returned_count: u32,
    pub search_time_ms: u64,
    pub providers_used: Vec<String>,
    pub providers_failed: Vec<String>,
}

impl From<SearchResult> for SearchResponse {
    fn from(result: SearchResult) -> Self {
        let papers = result
            .papers
            .into_iter()
            .map(|p| PaperResponse {
                title: p.metadata.title,
                authors: p.metadata.authors,
                year: p.metadata.year,
                doi: p.metadata.doi,
                journal: p.metadata.journal,
                abstract_text: p.metadata.abstract_text,
                pdf_url: p.metadata.pdf_url,
                source: p.source,
                relevance_score: p.relevance_score,
            })
            .collect();

        Self {
            query: result.query,
            papers,
            total_count: result.total_count,
            returned_count: result.returned_count,
            search_time_ms: result.search_time_ms,
            providers_used: result.successful_providers,
            providers_failed: result.failed_providers,
        }
    }
}

/// POST /api/search request body
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub search_type: Option<String>,
}

/// Create the HTTP router
pub fn create_router(state: AppState) -> Router {
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT, header::AUTHORIZATION]);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/search", get(search_get).post(search_post))
        .route("/api/providers", get(list_providers))
        .with_state(state)
        .layer(cors)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "research-hub",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /api/search - Search papers via query parameters
async fn search_get(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    info!("GET /api/search - query: {}", params.query);

    let search_type = parse_search_type(params.search_type.as_deref());

    let input = SearchInput {
        query: params.query,
        search_type,
        limit: params.limit.min(100),
        offset: params.offset,
    };

    execute_search(&state, input).await
}

/// POST /api/search - Search papers via JSON body
async fn search_post(
    State(state): State<AppState>,
    Json(body): Json<SearchRequest>,
) -> impl IntoResponse {
    info!("POST /api/search - query: {}", body.query);

    let search_type = parse_search_type(body.search_type.as_deref());

    let input = SearchInput {
        query: body.query,
        search_type,
        limit: body.limit.min(100),
        offset: body.offset,
    };

    execute_search(&state, input).await
}

/// Execute the search and return response
async fn execute_search(state: &AppState, input: SearchInput) -> impl IntoResponse {
    match state.search_tool.search_papers(input).await {
        Ok(result) => {
            let response: SearchResponse = result.into();
            (StatusCode::OK, Json(ApiResponse::success(response))).into_response()
        }
        Err(e) => {
            error!("Search failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(format!("Search failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Parse search type from string
fn parse_search_type(s: Option<&str>) -> SearchType {
    match s {
        Some("doi") => SearchType::Doi,
        Some("title") => SearchType::Title,
        Some("author") => SearchType::Author,
        Some("author_year") => SearchType::AuthorYear,
        _ => SearchType::Auto,
    }
}

/// GET /api/providers - List available search providers
async fn list_providers(State(_state): State<AppState>) -> impl IntoResponse {
    let providers = vec![
        ProviderInfo {
            name: "crossref".to_string(),
            description: "CrossRef DOI and metadata".to_string(),
            priority: "high".to_string(),
        },
        ProviderInfo {
            name: "semantic_scholar".to_string(),
            description: "Semantic Scholar AI papers".to_string(),
            priority: "high".to_string(),
        },
        ProviderInfo {
            name: "unpaywall".to_string(),
            description: "Open Access PDF finder".to_string(),
            priority: "high".to_string(),
        },
        ProviderInfo {
            name: "arxiv".to_string(),
            description: "arXiv preprints (CS, Physics, Math)".to_string(),
            priority: "medium".to_string(),
        },
        ProviderInfo {
            name: "pubmed".to_string(),
            description: "PubMed Central biomedical papers".to_string(),
            priority: "medium".to_string(),
        },
        ProviderInfo {
            name: "core".to_string(),
            description: "CORE Open Access aggregator".to_string(),
            priority: "medium".to_string(),
        },
        ProviderInfo {
            name: "biorxiv".to_string(),
            description: "bioRxiv biology preprints".to_string(),
            priority: "medium".to_string(),
        },
        ProviderInfo {
            name: "openreview".to_string(),
            description: "OpenReview ML/AI conferences".to_string(),
            priority: "medium".to_string(),
        },
        ProviderInfo {
            name: "openalex".to_string(),
            description: "OpenAlex comprehensive dataset".to_string(),
            priority: "medium".to_string(),
        },
    ];

    Json(ApiResponse::success(providers))
}

#[derive(Debug, Serialize)]
struct ProviderInfo {
    name: String,
    description: String,
    priority: String,
}

/// HTTP server for Research Hub
pub struct HttpServer {
    config: Arc<Config>,
    port: u16,
    host: String,
}

impl HttpServer {
    pub fn new(config: Config, port: u16, host: String) -> Self {
        Self {
            config: Arc::new(config),
            port,
            host,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting Research Hub HTTP server on {}:{}", self.host, self.port);

        // Initialize search tool
        let search_tool = SearchTool::new(self.config.clone())?;

        // Create application state
        let state = AppState {
            search_tool: Arc::new(search_tool),
            config: self.config.clone(),
        };

        // Create router
        let app = create_router(state);

        // Bind to address
        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| crate::Error::Service(format!("Failed to bind to {addr}: {e}")))?;

        info!("Research Hub HTTP server listening on http://{}", addr);
        info!("Available endpoints:");
        info!("  GET  /health - Health check");
        info!("  GET  /api/search?query=<query>&limit=<n> - Search papers");
        info!("  POST /api/search - Search papers (JSON body)");
        info!("  GET  /api/providers - List available providers");

        // Run server with graceful shutdown
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| crate::Error::Service(format!("Server error: {e}")))?;

        info!("HTTP server shutdown complete");
        Ok(())
    }
}

/// Shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Received Ctrl+C, initiating shutdown");
        }
        () = terminate => {
            info!("Received SIGTERM, initiating shutdown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_type() {
        assert!(matches!(parse_search_type(Some("doi")), SearchType::Doi));
        assert!(matches!(parse_search_type(Some("title")), SearchType::Title));
        assert!(matches!(parse_search_type(Some("author")), SearchType::Author));
        assert!(matches!(parse_search_type(None), SearchType::Auto));
        assert!(matches!(parse_search_type(Some("unknown")), SearchType::Auto));
    }

    #[test]
    fn test_api_response() {
        let response = ApiResponse::success("test");
        assert!(response.success);
        assert_eq!(response.data, Some("test"));

        let error = ApiResponse::<()>::error("error message".to_string());
        assert!(!error.success);
        assert_eq!(error.error, Some("error message".to_string()));
    }
}
