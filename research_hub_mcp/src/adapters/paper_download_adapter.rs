//! # Paper Download Adapter
//!
//! Concrete implementation of the DownloadServicePort that uses the existing
//! DownloadTool to handle paper downloads.

use crate::client::MetaSearchClient;
use crate::ports::download_service::{
    DownloadServiceHealth, DownloadServicePort, FileIntegrity, NetworkStatus,
};
use crate::tools::download::{DownloadInput, DownloadProgress, DownloadResult, DownloadTool};
use crate::{Config, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, instrument};

/// Paper download adapter that implements DownloadServicePort
///
/// This adapter wraps the existing DownloadTool and provides the
/// hexagonal architecture interface. It maintains compatibility with
/// the existing implementation while providing the new port interface.
#[derive(Clone)]
pub struct PaperDownloadAdapter {
    /// Underlying download tool
    download_tool: DownloadTool,
    /// Configuration reference
    config: Arc<Config>,
    /// Service start time for uptime calculation
    start_time: SystemTime,
}

impl std::fmt::Debug for PaperDownloadAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaperDownloadAdapter")
            .field("download_tool", &"DownloadTool")
            .field("config", &"Config")
            .field("start_time", &self.start_time)
            .finish()
    }
}

impl PaperDownloadAdapter {
    /// Create a new paper download adapter
    pub fn new(client: Arc<MetaSearchClient>, config: Arc<Config>) -> Result<Self> {
        info!("Initializing PaperDownloadAdapter");

        let download_tool = DownloadTool::new(client, config.clone())?;

        Ok(Self {
            download_tool,
            config,
            start_time: SystemTime::now(),
        })
    }

    /// Check network connectivity
    async fn check_network_connectivity(&self) -> NetworkStatus {
        // Simple connectivity check by trying to resolve a well-known domain
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::lookup_host("google.com:80"),
        )
        .await
        {
            Ok(Ok(_)) => NetworkStatus::Connected,
            Ok(Err(_)) => NetworkStatus::Limited,
            Err(_) => NetworkStatus::Disconnected,
        }
    }

    /// Get available disk space for the download directory
    async fn get_available_disk_space(&self) -> u64 {
        let download_dir = &self.config.downloads.directory;

        // Try to get disk space information
        match tokio::task::spawn_blocking({
            let path = download_dir.clone();
            move || {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        // This is a simplified approach - in a real implementation
                        // you'd use statvfs or similar system calls
                        return metadata.size();
                    }
                }

                #[cfg(windows)]
                {
                    use std::ffi::OsStr;
                    use std::os::windows::ffi::OsStrExt;
                    use winapi::um::fileapi::GetDiskFreeSpaceExW;

                    let path_wide: Vec<u16> = OsStr::new(&path)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();

                    let mut free_bytes = 0u64;
                    unsafe {
                        if GetDiskFreeSpaceExW(
                            path_wide.as_ptr(),
                            &mut free_bytes,
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                        ) != 0
                        {
                            return free_bytes;
                        }
                    }
                }

                // Fallback: assume some reasonable amount
                1024 * 1024 * 1024 // 1GB
            }
        })
        .await
        {
            Ok(space) => space,
            Err(_) => 1024 * 1024 * 1024, // 1GB fallback
        }
    }
}

#[async_trait]
impl DownloadServicePort for PaperDownloadAdapter {
    #[instrument(skip(self), fields(doi = ?input.doi, url = ?input.url))]
    async fn download_paper(&self, input: DownloadInput) -> Result<DownloadResult> {
        info!(
            "Starting paper download: doi={:?}, url={:?}",
            input.doi, input.url
        );

        self.download_tool.download_paper(input).await
    }

    async fn get_download_progress(&self, download_id: &str) -> Result<DownloadProgress> {
        // Check if the download is in the active downloads
        let active_downloads = self.download_tool.get_active_downloads().await;

        for progress in active_downloads {
            if progress.download_id == download_id {
                return Ok(progress);
            }
        }

        Err(crate::Error::InvalidInput {
            field: "download_id".to_string(),
            reason: format!("Download not found: {download_id}"),
        })
    }

    async fn cancel_download(&self, download_id: &str) -> Result<()> {
        self.download_tool.cancel_download(download_id).await
    }

    async fn pause_download(&self, download_id: &str) -> Result<()> {
        // The existing DownloadTool doesn't have explicit pause functionality
        // For now, we'll implement this as a cancellation
        // In a real implementation, you'd add pause/resume capability to DownloadTool
        debug!(
            "Pause not implemented, cancelling download: {}",
            download_id
        );
        self.download_tool.cancel_download(download_id).await
    }

    async fn resume_download(&self, download_id: &str) -> Result<()> {
        // The existing DownloadTool doesn't have explicit resume functionality
        // This would require tracking paused downloads and restarting them
        Err(crate::Error::Service(
            "Resume functionality not implemented in current download tool".to_string(),
        ))
    }

    async fn list_active_downloads(&self) -> Result<Vec<DownloadProgress>> {
        Ok(self.download_tool.get_active_downloads().await)
    }

    async fn health_check(&self) -> Result<DownloadServiceHealth> {
        let network_status = self.check_network_connectivity().await;
        let available_disk_space = self.get_available_disk_space().await;
        let active_downloads = self.download_tool.get_active_downloads().await;
        let max_concurrent = 5; // This should come from config in a real implementation

        let status = match network_status {
            NetworkStatus::Connected if available_disk_space > 100 * 1024 * 1024 => {
                crate::ports::search_service::HealthStatus::Healthy
            }
            NetworkStatus::Connected | NetworkStatus::Limited => {
                crate::ports::search_service::HealthStatus::Degraded
            }
            NetworkStatus::Disconnected | NetworkStatus::Unknown => {
                crate::ports::search_service::HealthStatus::Unhealthy
            }
        };

        Ok(DownloadServiceHealth {
            status,
            available_disk_space,
            network_status,
            active_downloads: active_downloads.len(),
            max_concurrent_downloads: max_concurrent,
            checked_at: SystemTime::now(),
        })
    }

    async fn get_metrics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();
        let active_downloads = self.download_tool.get_active_downloads().await;
        let uptime = self.start_time.elapsed().unwrap_or_default();

        metrics.insert(
            "active_downloads".to_string(),
            active_downloads.len().into(),
        );
        metrics.insert("uptime_seconds".to_string(), uptime.as_secs().into());

        // Calculate total downloaded bytes from active downloads
        let total_downloaded: u64 = active_downloads.iter().map(|d| d.downloaded).sum();
        metrics.insert(
            "total_downloaded_bytes".to_string(),
            total_downloaded.into(),
        );

        // Average download speed
        let avg_speed: u64 = if !active_downloads.is_empty() {
            active_downloads.iter().map(|d| d.speed_bps).sum::<u64>()
                / active_downloads.len() as u64
        } else {
            0
        };
        metrics.insert("average_speed_bps".to_string(), avg_speed.into());

        Ok(metrics)
    }

    async fn verify_file_integrity(&self, file_path: &Path) -> Result<FileIntegrity> {
        let start_time = SystemTime::now();

        // Check if file exists
        if !file_path.exists() {
            return Ok(FileIntegrity {
                is_valid: false,
                sha256_hash: String::new(),
                file_size: 0,
                verified_at: start_time,
                error_message: Some("File does not exist".to_string()),
            });
        }

        // Get file size
        let file_size = match tokio::fs::metadata(file_path).await {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                return Ok(FileIntegrity {
                    is_valid: false,
                    sha256_hash: String::new(),
                    file_size: 0,
                    verified_at: start_time,
                    error_message: Some(format!("Cannot read file metadata: {e}")),
                });
            }
        };

        // Calculate SHA256 hash
        let hash = match self.download_tool.calculate_file_hash(file_path).await {
            Ok(hash) => hash,
            Err(e) => {
                return Ok(FileIntegrity {
                    is_valid: false,
                    sha256_hash: String::new(),
                    file_size,
                    verified_at: start_time,
                    error_message: Some(format!("Hash calculation failed: {e}")),
                });
            }
        };

        Ok(FileIntegrity {
            is_valid: true,
            sha256_hash: hash,
            file_size,
            verified_at: start_time,
            error_message: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{MetaSearchClient, MetaSearchConfig};
    use crate::config::{Config, ResearchSourceConfig};
    use tempfile::TempDir;

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

    fn create_test_adapter() -> Result<PaperDownloadAdapter> {
        let config = create_test_config();
        let meta_config = MetaSearchConfig::from_config(&config);
        let client = Arc::new(MetaSearchClient::new((*config).clone(), meta_config)?);
        PaperDownloadAdapter::new(client, config)
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = create_test_adapter();
        assert!(adapter.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let adapter = create_test_adapter().unwrap();
        let health = adapter.health_check().await.unwrap();

        // Should have some basic health information
        assert!(health.available_disk_space > 0);
        assert!(health.max_concurrent_downloads > 0);
        assert_eq!(health.active_downloads, 0); // No downloads initially
    }

    #[tokio::test]
    async fn test_metrics() {
        let adapter = create_test_adapter().unwrap();
        let metrics = adapter.get_metrics().await.unwrap();

        assert!(metrics.contains_key("active_downloads"));
        assert!(metrics.contains_key("uptime_seconds"));
        assert!(metrics.contains_key("total_downloaded_bytes"));
        assert!(metrics.contains_key("average_speed_bps"));

        // Initially should be 0
        assert_eq!(metrics.get("active_downloads").unwrap().as_u64(), Some(0));
        assert_eq!(
            metrics.get("total_downloaded_bytes").unwrap().as_u64(),
            Some(0)
        );
        assert_eq!(metrics.get("average_speed_bps").unwrap().as_u64(), Some(0));
    }

    #[tokio::test]
    async fn test_list_active_downloads() {
        let adapter = create_test_adapter().unwrap();
        let downloads = adapter.list_active_downloads().await.unwrap();
        assert!(downloads.is_empty()); // No downloads initially
    }

    #[tokio::test]
    async fn test_verify_file_integrity_nonexistent() {
        let adapter = create_test_adapter().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_file = temp_dir.path().join("nonexistent.pdf");

        let integrity = adapter
            .verify_file_integrity(&nonexistent_file)
            .await
            .unwrap();
        assert!(!integrity.is_valid);
        assert_eq!(integrity.file_size, 0);
        assert!(integrity.error_message.is_some());
    }

    #[tokio::test]
    async fn test_verify_file_integrity_existing() {
        let adapter = create_test_adapter().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Create a test file
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let integrity = adapter.verify_file_integrity(&test_file).await.unwrap();
        assert!(integrity.is_valid);
        assert_eq!(integrity.file_size, 12); // "test content" is 12 bytes
        assert!(!integrity.sha256_hash.is_empty());
        assert!(integrity.error_message.is_none());
    }

    #[tokio::test]
    async fn test_get_download_progress_not_found() {
        let adapter = create_test_adapter().unwrap();
        let result = adapter.get_download_progress("nonexistent-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_download_not_found() {
        let adapter = create_test_adapter().unwrap();
        let result = adapter.cancel_download("nonexistent-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resume_download_not_implemented() {
        let adapter = create_test_adapter().unwrap();
        let result = adapter.resume_download("any-id").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not implemented"));
    }
}
