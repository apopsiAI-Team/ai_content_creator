//! # Configuration Repository
//!
//! This module provides data access abstraction for configuration management.
//! It handles storage, retrieval, and validation of application configuration.

use super::{Repository, RepositoryError, RepositoryResult, RepositoryStats};
use crate::config::{Config, ConfigOverrides};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// The configuration value as JSON
    pub value: serde_json::Value,
    /// Schema version for this configuration
    pub schema_version: String,
    /// When this configuration was last updated
    pub updated_at: u64,
    /// Who/what updated this configuration
    pub updated_by: String,
    /// Validation errors (if any)
    pub validation_errors: Vec<String>,
    /// Whether this configuration is currently active
    pub is_active: bool,
}

impl ConfigEntry {
    /// Create a new configuration entry
    pub fn new<T: Serialize>(
        value: &T,
        schema_version: String,
        updated_by: String,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            value: serde_json::to_value(value)?,
            schema_version,
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            updated_by,
            validation_errors: Vec::new(),
            is_active: true,
        })
    }

    /// Get the value as a specific type
    pub fn get_value<T: for<'a> Deserialize<'a>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.value.clone())
    }

    /// Add a validation error
    pub fn add_validation_error(&mut self, error: String) {
        self.validation_errors.push(error);
        self.is_active = false;
    }

    /// Check if configuration is valid
    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_empty() && self.is_active
    }

    /// Get age of this configuration entry
    pub fn age(&self) -> std::time::Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        std::time::Duration::from_secs(now - self.updated_at)
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Configuration key that changed
    pub key: String,
    /// Old value (if any)
    pub old_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
    /// Who/what made the change
    pub changed_by: String,
    /// When the change occurred
    pub timestamp: u64,
}

/// Repository trait for configuration management
#[async_trait]
pub trait ConfigRepository: Repository {
    /// Store a configuration value
    async fn store_config<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        updated_by: &str,
    ) -> RepositoryResult<()>;

    /// Get a configuration value
    async fn get_config<T: for<'a> Deserialize<'a> + Send>(
        &self,
        key: &str,
    ) -> RepositoryResult<Option<T>>;

    /// Get a configuration entry with metadata
    async fn get_config_entry(&self, key: &str) -> RepositoryResult<Option<ConfigEntry>>;

    /// Get all configuration keys
    async fn get_config_keys(&self) -> RepositoryResult<Vec<String>>;

    /// Remove a configuration key
    async fn remove_config(&self, key: &str) -> RepositoryResult<bool>;

    /// Check if a configuration key exists
    async fn config_exists(&self, key: &str) -> RepositoryResult<bool>;

    /// Load configuration with overrides
    async fn load_config(&self) -> RepositoryResult<Config>;

    /// Load configuration from file path
    async fn load_config_from_file(&self, path: &PathBuf) -> RepositoryResult<Config>;

    /// Store the main application configuration
    async fn store_app_config(&self, config: &Config, updated_by: &str) -> RepositoryResult<()>;

    /// Get the main application configuration
    async fn get_app_config(&self) -> RepositoryResult<Option<Config>>;

    /// Apply configuration overrides
    async fn apply_overrides(&self, overrides: &ConfigOverrides) -> RepositoryResult<Config>;

    /// Validate a configuration value
    async fn validate_config<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
    ) -> RepositoryResult<Vec<String>>;

    /// Get configuration history for a key
    async fn get_config_history(&self, key: &str) -> RepositoryResult<Vec<ConfigEntry>>;

    /// Get recent configuration changes
    async fn get_recent_changes(&self, limit: usize) -> RepositoryResult<Vec<ConfigChangeEvent>>;

    /// Backup configuration to a file
    async fn backup_config(&self, path: &PathBuf) -> RepositoryResult<()>;

    /// Restore configuration from a backup file
    async fn restore_config(&self, path: &PathBuf, restored_by: &str) -> RepositoryResult<()>;

    /// Hot reload configuration (non-critical settings only)
    async fn hot_reload(&self) -> RepositoryResult<bool>;
}

/// In-memory implementation of ConfigRepository for testing
#[derive(Debug)]
pub struct InMemoryConfigRepository {
    /// Current configuration entries
    entries: Arc<RwLock<HashMap<String, ConfigEntry>>>,
    /// Configuration history
    history: Arc<RwLock<HashMap<String, Vec<ConfigEntry>>>>,
    /// Recent changes
    changes: Arc<RwLock<Vec<ConfigChangeEvent>>>,
    /// Repository statistics
    stats: Arc<RwLock<RepositoryStats>>,
    /// Current application configuration
    app_config: Arc<RwLock<Option<Config>>>,
    /// Maximum history entries per key
    max_history_entries: usize,
    /// Maximum recent changes to keep
    max_recent_changes: usize,
}

impl InMemoryConfigRepository {
    /// Create a new in-memory configuration repository
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            changes: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(RepositoryStats::new())),
            app_config: Arc::new(RwLock::new(None)),
            max_history_entries: 100,
            max_recent_changes: 1000,
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_history_entries: usize, max_recent_changes: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            changes: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(RepositoryStats::new())),
            app_config: Arc::new(RwLock::new(None)),
            max_history_entries,
            max_recent_changes,
        }
    }

    /// Record a configuration change
    async fn record_change(
        &self,
        key: &str,
        old_value: Option<serde_json::Value>,
        new_value: serde_json::Value,
        changed_by: &str,
    ) {
        let change = ConfigChangeEvent {
            key: key.to_string(),
            old_value,
            new_value,
            changed_by: changed_by.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut changes = self.changes.write().await;
        changes.push(change);

        // Limit the number of recent changes
        if changes.len() > self.max_recent_changes {
            changes.remove(0);
        }
    }

    /// Add entry to history
    async fn add_to_history(&self, key: &str, entry: ConfigEntry) {
        let mut history = self.history.write().await;
        let entries = history.entry(key.to_string()).or_insert_with(Vec::new);

        entries.push(entry);

        // Limit history size
        if entries.len() > self.max_history_entries {
            entries.remove(0);
        }
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

impl Default for InMemoryConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Repository for InMemoryConfigRepository {
    fn name(&self) -> &'static str {
        "InMemoryConfigRepository"
    }

    async fn health_check(&self) -> RepositoryResult<bool> {
        // For in-memory repository, health check is always successful
        Ok(true)
    }

    async fn clear(&self) -> RepositoryResult<()> {
        let start_time = Instant::now();

        {
            let mut entries = self.entries.write().await;
            let mut history = self.history.write().await;
            let mut changes = self.changes.write().await;
            let mut app_config = self.app_config.write().await;

            entries.clear();
            history.clear();
            changes.clear();
            *app_config = None;
        }

        {
            let mut stats = self.stats.write().await;
            *stats = RepositoryStats::new();
        }

        self.record_operation(start_time, true).await;
        info!("Cleared all configuration entries");
        Ok(())
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let entries = self.entries.read().await;
        let mut stats = self.stats.read().await.clone();

        stats.total_entities = entries.len() as u64;

        // Estimate memory usage
        let estimated_memory = entries
            .iter()
            .map(|(key, entry)| {
                key.len()
                    + entry.value.to_string().len()
                    + entry.schema_version.len()
                    + entry.updated_by.len()
                    + entry
                        .validation_errors
                        .iter()
                        .map(|e| e.len())
                        .sum::<usize>()
                    + 64 // Approximate overhead
            })
            .sum::<usize>();

        stats.memory_usage_bytes = Some(estimated_memory as u64);

        Ok(stats)
    }
}

#[async_trait]
impl ConfigRepository for InMemoryConfigRepository {
    async fn store_config<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        updated_by: &str,
    ) -> RepositoryResult<()> {
        let start_time = Instant::now();

        if key.trim().is_empty() {
            self.record_operation(start_time, false).await;
            return Err(RepositoryError::Validation {
                field: "key".to_string(),
                message: "Configuration key cannot be empty".to_string(),
            });
        }

        // Create new config entry
        let mut entry = ConfigEntry::new(value, "1.0".to_string(), updated_by.to_string())
            .map_err(|e| RepositoryError::Serialization {
                message: format!("Failed to serialize config value: {}", e),
            })?;

        // Validate the configuration
        let validation_errors = self.validate_config(key, value).await?;
        for error in validation_errors {
            entry.add_validation_error(error);
        }

        let old_value = {
            let entries = self.entries.read().await;
            entries.get(key).map(|e| e.value.clone())
        };

        // Store the entry
        {
            let mut entries = self.entries.write().await;
            entries.insert(key.to_string(), entry.clone());
        }

        // Record change and add to history
        self.record_change(key, old_value, entry.value.clone(), updated_by)
            .await;
        self.add_to_history(key, entry).await;

        self.record_operation(start_time, true).await;
        debug!("Stored configuration for key: {}", key);
        Ok(())
    }

    async fn get_config<T: for<'a> Deserialize<'a> + Send>(
        &self,
        key: &str,
    ) -> RepositoryResult<Option<T>> {
        let start_time = Instant::now();

        let entries = self.entries.read().await;
        let result = if let Some(entry) = entries.get(key) {
            if entry.is_valid() {
                match entry.get_value::<T>() {
                    Ok(value) => Some(value),
                    Err(e) => {
                        warn!("Failed to deserialize config value for key {}: {}", key, e);
                        self.record_operation(start_time, false).await;
                        return Err(RepositoryError::Serialization {
                            message: format!("Failed to deserialize config value: {}", e),
                        });
                    }
                }
            } else {
                warn!(
                    "Configuration for key {} is invalid: {:?}",
                    key, entry.validation_errors
                );
                None
            }
        } else {
            None
        };

        self.record_operation(start_time, true).await;
        Ok(result)
    }

    async fn get_config_entry(&self, key: &str) -> RepositoryResult<Option<ConfigEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(key).cloned())
    }

    async fn get_config_keys(&self) -> RepositoryResult<Vec<String>> {
        let entries = self.entries.read().await;
        Ok(entries.keys().cloned().collect())
    }

    async fn remove_config(&self, key: &str) -> RepositoryResult<bool> {
        let start_time = Instant::now();

        let removed = {
            let mut entries = self.entries.write().await;
            entries.remove(key).is_some()
        };

        if removed {
            self.record_change(key, None, serde_json::Value::Null, "system")
                .await;
            debug!("Removed configuration for key: {}", key);
        }

        self.record_operation(start_time, true).await;
        Ok(removed)
    }

    async fn config_exists(&self, key: &str) -> RepositoryResult<bool> {
        let entries = self.entries.read().await;
        Ok(entries.contains_key(key))
    }

    async fn load_config(&self) -> RepositoryResult<Config> {
        let start_time = Instant::now();

        // Try to get stored app config first
        if let Some(config) = self.get_app_config().await? {
            self.record_operation(start_time, true).await;
            return Ok(config);
        }

        // Fall back to loading default config
        let config = Config::default();
        self.record_operation(start_time, true).await;
        Ok(config)
    }

    async fn load_config_from_file(&self, path: &PathBuf) -> RepositoryResult<Config> {
        let start_time = Instant::now();

        let config = Config::load_from_file(path).map_err(|e| RepositoryError::Storage {
            message: format!("Failed to load config from file: {}", e),
        })?;

        self.record_operation(start_time, true).await;
        Ok(config)
    }

    async fn store_app_config(&self, config: &Config, updated_by: &str) -> RepositoryResult<()> {
        let start_time = Instant::now();

        // Validate config before storing
        config.validate().map_err(|e| RepositoryError::Validation {
            field: "config".to_string(),
            message: format!("Configuration validation failed: {}", e),
        })?;

        {
            let mut app_config = self.app_config.write().await;
            *app_config = Some(config.clone());
        }

        // Also store as a regular config entry
        self.store_config("app_config", config, updated_by).await?;

        self.record_operation(start_time, true).await;
        info!("Stored application configuration");
        Ok(())
    }

    async fn get_app_config(&self) -> RepositoryResult<Option<Config>> {
        let app_config = self.app_config.read().await;
        Ok(app_config.clone())
    }

    async fn apply_overrides(&self, overrides: &ConfigOverrides) -> RepositoryResult<Config> {
        let start_time = Instant::now();

        let mut config = self.load_config().await?;

        // Apply overrides (simplified version)
        if let Some(port) = overrides.server_port {
            config.server.port = port;
        }
        if let Some(ref host) = overrides.server_host {
            config.server.host = host.clone();
        }
        if let Some(ref level) = overrides.log_level {
            config.logging.level = level.clone();
        }
        if let Some(ref profile) = overrides.profile {
            config.profile = profile.clone();
        }
        if let Some(ref dir) = overrides.download_directory {
            config.downloads.directory = dir.clone();
        }

        self.record_operation(start_time, true).await;
        Ok(config)
    }

    async fn validate_config<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
    ) -> RepositoryResult<Vec<String>> {
        let mut errors = Vec::new();

        // Basic validation
        if key.trim().is_empty() {
            errors.push("Key cannot be empty".to_string());
        }

        // Try to serialize to check if value is valid
        if serde_json::to_value(value).is_err() {
            errors.push("Value cannot be serialized to JSON".to_string());
        }

        // Key-specific validation
        match key {
            "server.port" => {
                if let Ok(port_value) = serde_json::to_value(value) {
                    if let Some(port) = port_value.as_u64() {
                        if port == 0 || port > 65535 {
                            errors.push("Port must be between 1 and 65535".to_string());
                        }
                    }
                }
            }
            "logging.level" => {
                if let Ok(level_value) = serde_json::to_value(value) {
                    if let Some(level) = level_value.as_str() {
                        let valid_levels = ["trace", "debug", "info", "warn", "error"];
                        if !valid_levels.contains(&level) {
                            errors.push(format!(
                                "Invalid log level. Valid levels: {:?}",
                                valid_levels
                            ));
                        }
                    }
                }
            }
            _ => {} // No specific validation for other keys
        }

        Ok(errors)
    }

    async fn get_config_history(&self, key: &str) -> RepositoryResult<Vec<ConfigEntry>> {
        let history = self.history.read().await;
        Ok(history.get(key).cloned().unwrap_or_default())
    }

    async fn get_recent_changes(&self, limit: usize) -> RepositoryResult<Vec<ConfigChangeEvent>> {
        let changes = self.changes.read().await;
        let mut recent_changes = changes.clone();
        recent_changes.reverse(); // Most recent first
        recent_changes.truncate(limit);
        Ok(recent_changes)
    }

    async fn backup_config(&self, path: &PathBuf) -> RepositoryResult<()> {
        let start_time = Instant::now();

        let entries = self.entries.read().await;
        let backup_data = serde_json::to_string_pretty(&*entries).map_err(|e| {
            RepositoryError::Serialization {
                message: format!("Failed to serialize backup data: {}", e),
            }
        })?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RepositoryError::Storage {
                message: format!("Failed to create backup directory: {}", e),
            })?;
        }

        std::fs::write(path, backup_data).map_err(|e| RepositoryError::Storage {
            message: format!("Failed to write backup file: {}", e),
        })?;

        self.record_operation(start_time, true).await;
        info!("Backed up configuration to: {:?}", path);
        Ok(())
    }

    async fn restore_config(&self, path: &PathBuf, restored_by: &str) -> RepositoryResult<()> {
        let start_time = Instant::now();

        let backup_data = std::fs::read_to_string(path).map_err(|e| RepositoryError::Storage {
            message: format!("Failed to read backup file: {}", e),
        })?;

        let backup_entries: HashMap<String, ConfigEntry> = serde_json::from_str(&backup_data)
            .map_err(|e| RepositoryError::Serialization {
                message: format!("Failed to deserialize backup data: {}", e),
            })?;

        {
            let mut entries = self.entries.write().await;
            *entries = backup_entries;
        }

        // Record the restore operation
        self.record_change(
            "system.restore",
            None,
            serde_json::json!({"restored_from": path, "restored_by": restored_by}),
            restored_by,
        )
        .await;

        self.record_operation(start_time, true).await;
        info!("Restored configuration from: {:?}", path);
        Ok(())
    }

    async fn hot_reload(&self) -> RepositoryResult<bool> {
        let start_time = Instant::now();

        // For in-memory repository, hot reload doesn't do much
        // In a real implementation, this would reload from external sources
        warn!("Hot reload not fully implemented for in-memory repository");

        self.record_operation(start_time, true).await;
        Ok(false) // No changes made
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_store_and_get_config() {
        let repo = InMemoryConfigRepository::new();

        // Store a config value
        let value = json!({"port": 8080, "host": "localhost"});
        repo.store_config("server", &value, "test").await.unwrap();

        // Get the config value
        let retrieved: Option<serde_json::Value> = repo.get_config("server").await.unwrap();
        assert_eq!(retrieved, Some(value));
    }

    #[tokio::test]
    async fn test_config_validation() {
        let repo = InMemoryConfigRepository::new();

        // Test invalid port
        let errors = repo
            .validate_config("server.port", &json!(0))
            .await
            .unwrap();
        assert!(!errors.is_empty());

        // Test valid port
        let errors = repo
            .validate_config("server.port", &json!(8080))
            .await
            .unwrap();
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_config_history() {
        let repo = InMemoryConfigRepository::new();

        // Store multiple versions
        repo.store_config("test_key", &json!(1), "user1")
            .await
            .unwrap();
        repo.store_config("test_key", &json!(2), "user2")
            .await
            .unwrap();
        repo.store_config("test_key", &json!(3), "user3")
            .await
            .unwrap();

        let history = repo.get_config_history("test_key").await.unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].get_value::<i32>().unwrap(), 1);
        assert_eq!(history[1].get_value::<i32>().unwrap(), 2);
        assert_eq!(history[2].get_value::<i32>().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_recent_changes() {
        let repo = InMemoryConfigRepository::new();

        // Store some config values
        repo.store_config("key1", &json!("value1"), "user1")
            .await
            .unwrap();
        repo.store_config("key2", &json!("value2"), "user2")
            .await
            .unwrap();

        let changes = repo.get_recent_changes(10).await.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].key, "key2"); // Most recent first
        assert_eq!(changes[1].key, "key1");
    }

    #[tokio::test]
    async fn test_app_config() {
        let repo = InMemoryConfigRepository::new();

        let config = Config::default();
        repo.store_app_config(&config, "test").await.unwrap();

        let retrieved = repo.get_app_config().await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_backup_restore() {
        let repo = InMemoryConfigRepository::new();
        let temp_path = std::env::temp_dir().join("test_backup.json");

        // Store some config
        repo.store_config("test", &json!({"key": "value"}), "test")
            .await
            .unwrap();

        // Backup
        repo.backup_config(&temp_path).await.unwrap();
        assert!(temp_path.exists());

        // Clear and restore
        repo.clear().await.unwrap();
        assert!(repo
            .get_config::<serde_json::Value>("test")
            .await
            .unwrap()
            .is_none());

        repo.restore_config(&temp_path, "test").await.unwrap();
        let restored: Option<serde_json::Value> = repo.get_config("test").await.unwrap();
        assert!(restored.is_some());

        // Cleanup
        std::fs::remove_file(&temp_path).ok();
    }
}
