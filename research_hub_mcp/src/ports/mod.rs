//! # Ports Module
//!
//! This module defines the port interfaces for the hexagonal architecture pattern.
//! Ports represent the business logic interfaces that define what the application
//! needs from external services without specifying how they are implemented.
//!
//! ## Architecture Overview
//!
//! In hexagonal architecture (also known as ports and adapters):
//! - **Ports** are interfaces that define the contract between the application core and external services
//! - **Adapters** are concrete implementations of these ports that interact with external systems
//! - **Tools** (our application core) depend only on port interfaces, not concrete implementations
//!
//! This separation allows for:
//! - Easy testing through mock implementations
//! - Flexibility to swap implementations without changing business logic
//! - Clear boundaries between application core and infrastructure
//!
//! ## Example Usage
//!
//! ```rust
//! use crate::ports::{SearchServicePort, SearchInput, SearchResult};
//! use std::sync::Arc;
//!
//! async fn search_papers(
//!     search_service: Arc<dyn SearchServicePort>,
//!     input: SearchInput
//! ) -> Result<SearchResult, Box<dyn std::error::Error>> {
//!     // Business logic doesn't know or care about the concrete implementation
//!     search_service.search_papers(input).await
//! }
//! ```

pub mod download_service;
pub mod metadata_service;
pub mod provider_service;
pub mod search_service;

// Re-export all port traits for convenience
pub use download_service::DownloadServicePort;
pub use metadata_service::MetadataServicePort;
pub use provider_service::ProviderServicePort;
pub use search_service::SearchServicePort;
