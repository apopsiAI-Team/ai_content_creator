//! # Adapters Module
//!
//! This module contains the concrete implementations of the port interfaces defined
//! in the ports module. Adapters handle the actual interaction with external services
//! and systems while conforming to the port contracts.
//!
//! ## Architecture Overview
//!
//! In hexagonal architecture:
//! - **Ports** define the interfaces (what the application needs)
//! - **Adapters** provide the implementations (how external systems are accessed)
//! - **Application Core** (tools) depends only on ports, not adapters
//!
//! This allows us to:
//! - Swap implementations without changing business logic
//! - Test with mock adapters
//! - Add new external services easily
//! - Maintain clear separation of concerns
//!
//! ## Adapter Types
//!
//! ### Primary Adapters (Inbound)
//! - Accept requests from external systems
//! - Convert external formats to internal formats
//! - Example: HTTP API adapters, CLI adapters
//!
//! ### Secondary Adapters (Outbound)
//! - Call external systems on behalf of the application
//! - Convert internal formats to external formats
//! - Example: Database adapters, HTTP client adapters
//!
//! ## Example Usage
//!
//! ```rust
//! use crate::adapters::MetaSearchAdapter;
//! use crate::ports::SearchServicePort;
//! use std::sync::Arc;
//!
//! // Create an adapter instance
//! let config = Config::default();
//! let adapter = MetaSearchAdapter::new(config)?;
//!
//! // Use it through the port interface
//! let search_service: Arc<dyn SearchServicePort> = Arc::new(adapter);
//! let results = search_service.search_papers(input).await?;
//! ```

pub mod meta_search_adapter;
pub mod multi_provider_adapter;
pub mod paper_download_adapter;
pub mod pdf_metadata_adapter;

// Re-export adapters for convenience
pub use meta_search_adapter::MetaSearchAdapter;
pub use multi_provider_adapter::MultiProviderAdapter;
pub use paper_download_adapter::PaperDownloadAdapter;
pub use pdf_metadata_adapter::PdfMetadataAdapter;
