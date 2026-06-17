//! # Download Service Port
//!
//! Defines the port interface for paper download services.
//! This interface abstracts the paper download functionality, allowing different
//! download implementations to be used interchangeably.

use crate::tools::download::{DownloadInput, DownloadProgress, DownloadResult};
use crate::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Port interface for paper download services
///
/// This trait defines the contract for downloading academic papers.
/// Implementations should handle:
/// - Input validation and sanitization
/// - DOI resolution to download URLs
/// - Progress tracking and reporting
/// - File integrity verification
/// - Resume capability for interrupted downloads
/// - Categorization and organization
///
/// # Design Principles
///
/// - **Source Agnostic**: Works with any paper source (DOI, direct URL)
/// - **Resumable**: Supports resuming interrupted downloads
/// - **Observable**: Provides progress tracking and metrics
/// - **Secure**: Validates inputs and file integrity
/// - **Organized**: Supports categorization and file organization
///
/// # Example Implementation Structure
///
/// ```rust
/// use async_trait::async_trait;
/// use crate::ports::DownloadServicePort;
/// use crate::tools::download::{DownloadInput, DownloadResult};
/// use crate::Result;
///
/// pub struct PaperDownloadAdapter {
///     // Implementation details...
/// }
///
/// #[async_trait]
/// impl DownloadServicePort for PaperDownloadAdapter {
///     async fn download_paper(&self, input: DownloadInput) -> Result<DownloadResult> {
///         // 1. Validate and sanitize input
///         // 2. Resolve DOI to download URL if needed
///         // 3. Execute download with progress tracking
///         // 4. Verify file integrity
///         // 5. Organize file in categorized structure
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait DownloadServicePort: Send + Sync + Debug {
    /// Download a paper based on the provided input
    ///
    /// # Arguments
    ///
    /// * `input` - Download parameters including DOI/URL, target location, etc.
    ///
    /// # Returns
    ///
    /// A `DownloadResult` containing:
    /// - Download status and file information
    /// - File path and integrity hash
    /// - Download statistics (speed, duration)
    /// - Paper metadata if available
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input validation fails
    /// - DOI cannot be resolved
    /// - Download source is unavailable
    /// - File system errors occur
    /// - Integrity verification fails
    async fn download_paper(&self, input: DownloadInput) -> Result<DownloadResult>;

    /// Get the current progress of an active download
    ///
    /// # Arguments
    ///
    /// * `download_id` - Unique identifier for the download
    ///
    /// # Returns
    ///
    /// Current progress information including:
    /// - Bytes downloaded and total size
    /// - Download speed and ETA
    /// - Current status
    ///
    /// # Errors
    ///
    /// Returns an error if the download ID is not found.
    async fn get_download_progress(&self, download_id: &str) -> Result<DownloadProgress>;

    /// Cancel an active download
    ///
    /// # Arguments
    ///
    /// * `download_id` - Unique identifier for the download to cancel
    ///
    /// # Returns
    ///
    /// Success if the download was cancelled or already completed.
    ///
    /// # Errors
    ///
    /// Returns an error if the download ID is not found.
    async fn cancel_download(&self, download_id: &str) -> Result<()>;

    /// Pause an active download
    ///
    /// # Arguments
    ///
    /// * `download_id` - Unique identifier for the download to pause
    ///
    /// # Returns
    ///
    /// Success if the download was paused.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Download ID is not found
    /// - Download cannot be paused (already completed/failed)
    async fn pause_download(&self, download_id: &str) -> Result<()>;

    /// Resume a paused download
    ///
    /// # Arguments
    ///
    /// * `download_id` - Unique identifier for the download to resume
    ///
    /// # Returns
    ///
    /// Success if the download was resumed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Download ID is not found
    /// - Download is not in a paused state
    /// - Resume capability is not available
    async fn resume_download(&self, download_id: &str) -> Result<()>;

    /// List all active downloads
    ///
    /// # Returns
    ///
    /// A vector of progress information for all active downloads.
    async fn list_active_downloads(&self) -> Result<Vec<DownloadProgress>>;

    /// Get download service health and status
    ///
    /// # Returns
    ///
    /// Health information including:
    /// - Service operational status
    /// - Available disk space
    /// - Network connectivity status
    /// - Active download count
    async fn health_check(&self) -> Result<DownloadServiceHealth>;

    /// Get download service metrics
    ///
    /// # Returns
    ///
    /// A map of metric names to values, including:
    /// - Total downloads completed
    /// - Success/failure rates
    /// - Average download speeds
    /// - Total bytes downloaded
    /// - Active download count
    async fn get_metrics(&self) -> Result<std::collections::HashMap<String, serde_json::Value>>;

    /// Verify the integrity of a downloaded file
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to verify
    ///
    /// # Returns
    ///
    /// File integrity information including hash and validation status.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File does not exist
    /// - File cannot be read
    /// - Integrity verification fails
    async fn verify_file_integrity(&self, file_path: &std::path::Path) -> Result<FileIntegrity>;
}

/// Health status of the download service
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DownloadServiceHealth {
    /// Overall service status
    pub status: super::search_service::HealthStatus,
    /// Available disk space in bytes
    pub available_disk_space: u64,
    /// Network connectivity status
    pub network_status: NetworkStatus,
    /// Number of active downloads
    pub active_downloads: usize,
    /// Maximum concurrent downloads allowed
    pub max_concurrent_downloads: usize,
    /// Last health check timestamp
    pub checked_at: std::time::SystemTime,
}

/// Network connectivity status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NetworkStatus {
    /// Network is available and responsive
    Connected,
    /// Network has limited connectivity
    Limited,
    /// Network is not available
    Disconnected,
    /// Network status cannot be determined
    Unknown,
}

/// File integrity verification result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct FileIntegrity {
    /// Whether the file passed integrity checks
    pub is_valid: bool,
    /// SHA256 hash of the file
    pub sha256_hash: String,
    /// File size in bytes
    pub file_size: u64,
    /// Verification timestamp
    pub verified_at: std::time::SystemTime,
    /// Error message if verification failed
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_status_serialization() {
        let status = NetworkStatus::Connected;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"connected\"");

        let status = NetworkStatus::Limited;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"limited\"");
    }

    #[test]
    fn test_download_service_health_creation() {
        let health = DownloadServiceHealth {
            status: super::search_service::HealthStatus::Healthy,
            available_disk_space: 1024 * 1024 * 1024, // 1GB
            network_status: NetworkStatus::Connected,
            active_downloads: 2,
            max_concurrent_downloads: 5,
            checked_at: std::time::SystemTime::now(),
        };

        assert!(matches!(
            health.status,
            super::search_service::HealthStatus::Healthy
        ));
        assert_eq!(health.active_downloads, 2);
        assert_eq!(health.max_concurrent_downloads, 5);
    }

    #[test]
    fn test_file_integrity_creation() {
        let integrity = FileIntegrity {
            is_valid: true,
            sha256_hash: "abcd1234".to_string(),
            file_size: 1024,
            verified_at: std::time::SystemTime::now(),
            error_message: None,
        };

        assert!(integrity.is_valid);
        assert_eq!(integrity.sha256_hash, "abcd1234");
        assert_eq!(integrity.file_size, 1024);
        assert!(integrity.error_message.is_none());
    }
}
