//! # Cache Repository
//!
//! This module provides caching functionality with TTL (Time To Live) support.
//! It's designed to cache frequently accessed data to improve performance.

use super::{Repository, RepositoryError, RepositoryResult, RepositoryStats};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A cache entry with expiration support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    /// The cached value
    pub value: T,
    /// When this entry expires (Unix timestamp)
    pub expires_at: u64,
    /// When this entry was created
    pub created_at: u64,
    /// Number of times this entry has been accessed
    pub access_count: u64,
    /// Last access time
    pub last_accessed: u64,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry with TTL
    pub fn new(value: T, ttl: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            value,
            expires_at: now + ttl.as_secs(),
            created_at: now,
            access_count: 0,
            last_accessed: now,
        }
    }

    /// Check if this entry has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }

    /// Get the value and update access statistics
    pub fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_accessed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        &self.value
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if self.expires_at > now {
            Duration::from_secs(self.expires_at - now)
        } else {
            Duration::from_secs(0)
        }
    }

    /// Get age of the entry
    pub fn age(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Duration::from_secs(now - self.created_at)
    }
}

/// Cache statistics for monitoring and optimization
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Total number of entries expired and removed
    pub expirations: u64,
    /// Total number of entries manually evicted
    pub evictions: u64,
    /// Current memory usage estimate in bytes
    pub memory_usage_bytes: u64,
    /// Maximum memory usage observed
    pub peak_memory_usage_bytes: u64,
}

impl CacheStats {
    /// Calculate hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    /// Record a cache hit
    pub fn record_hit(&mut self) {
        self.hits += 1;
    }

    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.misses += 1;
    }

    /// Record an expiration
    pub fn record_expiration(&mut self) {
        self.expirations += 1;
    }

    /// Record an eviction
    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }

    /// Update memory usage
    pub fn update_memory_usage(&mut self, bytes: u64) {
        self.memory_usage_bytes = bytes;
        if bytes > self.peak_memory_usage_bytes {
            self.peak_memory_usage_bytes = bytes;
        }
    }
}

/// Repository trait for caching operations
#[async_trait]
pub trait CacheRepository: Repository {
    /// Store a value in the cache with default TTL
    async fn set<T>(&self, key: &str, value: T) -> RepositoryResult<()>
    where
        T: Send + Sync + Clone + 'static;

    /// Store a value in the cache with custom TTL
    async fn set_with_ttl<T>(&self, key: &str, value: T, ttl: Duration) -> RepositoryResult<()>
    where
        T: Send + Sync + Clone + 'static;

    /// Get a value from the cache
    async fn get<T>(&self, key: &str) -> RepositoryResult<Option<T>>
    where
        T: Send + Sync + Clone + 'static;

    /// Check if a key exists in the cache (without affecting access stats)
    async fn exists(&self, key: &str) -> RepositoryResult<bool>;

    /// Remove a specific key from the cache
    async fn remove(&self, key: &str) -> RepositoryResult<bool>;

    /// Remove expired entries from the cache
    async fn cleanup_expired(&self) -> RepositoryResult<u64>;

    /// Get cache statistics
    async fn cache_stats(&self) -> RepositoryResult<CacheStats>;

    /// Get all keys in the cache
    async fn keys(&self) -> RepositoryResult<Vec<String>>;

    /// Get cache size (number of entries)
    async fn size(&self) -> RepositoryResult<usize>;

    /// Set default TTL for new entries
    async fn set_default_ttl(&self, ttl: Duration) -> RepositoryResult<()>;

    /// Get current default TTL
    async fn get_default_ttl(&self) -> RepositoryResult<Duration>;

    /// Extend TTL for an existing key
    async fn extend_ttl(&self, key: &str, additional_ttl: Duration) -> RepositoryResult<bool>;

    /// Get remaining TTL for a key
    async fn get_ttl(&self, key: &str) -> RepositoryResult<Option<Duration>>;
}

/// In-memory implementation of CacheRepository
#[derive(Debug)]
pub struct InMemoryCacheRepository {
    /// Cache entries stored by key
    cache: Arc<RwLock<HashMap<String, CacheEntry<Box<dyn std::any::Any + Send + Sync>>>>>,
    /// Cache statistics
    cache_stats: Arc<RwLock<CacheStats>>,
    /// Repository statistics
    repo_stats: Arc<RwLock<RepositoryStats>>,
    /// Default TTL for cache entries
    default_ttl: Arc<RwLock<Duration>>,
    /// Maximum cache size (0 = unlimited)
    max_size: usize,
}

impl InMemoryCacheRepository {
    /// Create a new in-memory cache repository
    pub fn new() -> Self {
        Self::with_config(Duration::from_secs(3600), 0) // 1 hour default TTL, unlimited size
    }

    /// Create a new in-memory cache repository with configuration
    pub fn with_config(default_ttl: Duration, max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStats::default())),
            repo_stats: Arc::new(RwLock::new(RepositoryStats::new())),
            default_ttl: Arc::new(RwLock::new(default_ttl)),
            max_size,
        }
    }

    /// Estimate memory usage of a cache entry
    fn estimate_entry_size<T: 'static>(&self, key: &str, entry: &CacheEntry<T>) -> usize {
        key.len() + std::mem::size_of::<CacheEntry<T>>() + std::mem::size_of::<T>()
    }

    /// Evict LRU entries if cache is over size limit
    async fn evict_if_needed(&self) -> RepositoryResult<()> {
        if self.max_size == 0 {
            return Ok(()); // No size limit
        }

        let mut cache = self.cache.write().await;
        let current_size = cache.len();

        if current_size <= self.max_size {
            return Ok(());
        }

        let mut cache_stats = self.cache_stats.write().await;

        // Sort entries by last access time (oldest first)
        let mut entries: Vec<(String, u64)> = cache
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_accessed))
            .collect();

        entries.sort_by_key(|(_, last_accessed)| *last_accessed);

        // Remove oldest entries until we're under the limit
        let entries_to_remove = current_size - self.max_size;
        for (key, _) in entries.into_iter().take(entries_to_remove) {
            cache.remove(&key);
            cache_stats.record_eviction();
            debug!("Evicted cache entry: {}", key);
        }

        info!("Evicted {} entries due to size limit", entries_to_remove);
        Ok(())
    }

    /// Update memory usage statistics
    async fn update_memory_stats(&self) {
        let cache = self.cache.read().await;
        let mut cache_stats = self.cache_stats.write().await;

        // Rough estimation of memory usage
        let estimated_bytes = cache
            .iter()
            .map(|(key, _)| key.len() + 256) // Rough estimate per entry
            .sum::<usize>() as u64;

        cache_stats.update_memory_usage(estimated_bytes);
    }
}

impl Default for InMemoryCacheRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Repository for InMemoryCacheRepository {
    fn name(&self) -> &'static str {
        "InMemoryCacheRepository"
    }

    async fn health_check(&self) -> RepositoryResult<bool> {
        // For in-memory cache, health check is simple
        Ok(true)
    }

    async fn clear(&self) -> RepositoryResult<()> {
        let start_time = Instant::now();

        {
            let mut cache = self.cache.write().await;
            let mut cache_stats = self.cache_stats.write().await;
            cache.clear();
            *cache_stats = CacheStats::default();
        }

        {
            let mut repo_stats = self.repo_stats.write().await;
            *repo_stats = RepositoryStats::new();
        }

        let duration_ms = start_time.elapsed().as_millis() as f64;
        {
            let mut repo_stats = self.repo_stats.write().await;
            repo_stats.record_success(duration_ms);
        }

        info!("Cleared all cache entries");
        Ok(())
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let cache = self.cache.read().await;
        let mut repo_stats = self.repo_stats.read().await.clone();

        repo_stats.total_entities = cache.len() as u64;

        // Update memory usage estimate
        let cache_stats = self.cache_stats.read().await;
        repo_stats.memory_usage_bytes = Some(cache_stats.memory_usage_bytes);

        Ok(repo_stats)
    }
}

#[async_trait]
impl CacheRepository for InMemoryCacheRepository {
    async fn set<T>(&self, key: &str, value: T) -> RepositoryResult<()>
    where
        T: Send + Sync + Clone + 'static,
    {
        let default_ttl = *self.default_ttl.read().await;
        self.set_with_ttl(key, value, default_ttl).await
    }

    async fn set_with_ttl<T>(&self, key: &str, value: T, ttl: Duration) -> RepositoryResult<()>
    where
        T: Send + Sync + Clone + 'static,
    {
        let start_time = Instant::now();

        if key.trim().is_empty() {
            let duration_ms = start_time.elapsed().as_millis() as f64;
            {
                let mut repo_stats = self.repo_stats.write().await;
                repo_stats.record_failure(duration_ms);
            }
            return Err(RepositoryError::Validation {
                field: "key".to_string(),
                message: "Cache key cannot be empty".to_string(),
            });
        }

        let entry = CacheEntry::new(value, ttl);
        let boxed_entry = CacheEntry {
            value: Box::new(entry.value) as Box<dyn std::any::Any + Send + Sync>,
            expires_at: entry.expires_at,
            created_at: entry.created_at,
            access_count: entry.access_count,
            last_accessed: entry.last_accessed,
        };

        {
            let mut cache = self.cache.write().await;
            cache.insert(key.to_string(), boxed_entry);
        }

        // Evict if needed
        self.evict_if_needed().await?;

        // Update statistics
        self.update_memory_stats().await;

        let duration_ms = start_time.elapsed().as_millis() as f64;
        {
            let mut repo_stats = self.repo_stats.write().await;
            repo_stats.record_success(duration_ms);
        }

        debug!("Cached entry with key: {} (TTL: {:?})", key, ttl);
        Ok(())
    }

    async fn get<T>(&self, key: &str) -> RepositoryResult<Option<T>>
    where
        T: Send + Sync + Clone + 'static,
    {
        let start_time = Instant::now();

        let mut cache = self.cache.write().await;
        let mut cache_stats = self.cache_stats.write().await;

        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired() {
                // Remove expired entry
                cache.remove(key);
                cache_stats.record_expiration();
                cache_stats.record_miss();
                debug!("Cache entry expired and removed: {}", key);

                let duration_ms = start_time.elapsed().as_millis() as f64;
                drop(cache);
                drop(cache_stats);
                {
                    let mut repo_stats = self.repo_stats.write().await;
                    repo_stats.record_success(duration_ms);
                }
                return Ok(None);
            }

            // Update access statistics
            entry.access_count += 1;
            entry.last_accessed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Try to downcast the value
            if let Some(typed_value) = entry.value.downcast_ref::<T>() {
                let result = typed_value.clone();
                cache_stats.record_hit();
                debug!("Cache hit for key: {}", key);

                let duration_ms = start_time.elapsed().as_millis() as f64;
                drop(cache);
                drop(cache_stats);
                {
                    let mut repo_stats = self.repo_stats.write().await;
                    repo_stats.record_success(duration_ms);
                }
                return Ok(Some(result));
            } else {
                // Type mismatch - remove the entry
                cache.remove(key);
                warn!("Type mismatch for cache key {}, removing entry", key);
            }
        }

        cache_stats.record_miss();
        debug!("Cache miss for key: {}", key);

        let duration_ms = start_time.elapsed().as_millis() as f64;
        drop(cache);
        drop(cache_stats);
        {
            let mut repo_stats = self.repo_stats.write().await;
            repo_stats.record_success(duration_ms);
        }

        Ok(None)
    }

    async fn exists(&self, key: &str) -> RepositoryResult<bool> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn remove(&self, key: &str) -> RepositoryResult<bool> {
        let start_time = Instant::now();

        let removed = {
            let mut cache = self.cache.write().await;
            cache.remove(key).is_some()
        };

        if removed {
            let mut cache_stats = self.cache_stats.write().await;
            cache_stats.record_eviction();
            debug!("Manually removed cache entry: {}", key);
        }

        let duration_ms = start_time.elapsed().as_millis() as f64;
        {
            let mut repo_stats = self.repo_stats.write().await;
            repo_stats.record_success(duration_ms);
        }

        Ok(removed)
    }

    async fn cleanup_expired(&self) -> RepositoryResult<u64> {
        let start_time = Instant::now();

        let mut removed_count = 0;
        {
            let mut cache = self.cache.write().await;
            let mut cache_stats = self.cache_stats.write().await;

            let expired_keys: Vec<String> = cache
                .iter()
                .filter(|(_, entry)| entry.is_expired())
                .map(|(key, _)| key.clone())
                .collect();

            for key in expired_keys {
                cache.remove(&key);
                cache_stats.record_expiration();
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            info!("Cleaned up {} expired cache entries", removed_count);
            self.update_memory_stats().await;
        }

        let duration_ms = start_time.elapsed().as_millis() as f64;
        {
            let mut repo_stats = self.repo_stats.write().await;
            repo_stats.record_success(duration_ms);
        }

        Ok(removed_count)
    }

    async fn cache_stats(&self) -> RepositoryResult<CacheStats> {
        Ok(self.cache_stats.read().await.clone())
    }

    async fn keys(&self) -> RepositoryResult<Vec<String>> {
        let cache = self.cache.read().await;
        let keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect();
        Ok(keys)
    }

    async fn size(&self) -> RepositoryResult<usize> {
        let cache = self.cache.read().await;
        let active_count = cache.values().filter(|entry| !entry.is_expired()).count();
        Ok(active_count)
    }

    async fn set_default_ttl(&self, ttl: Duration) -> RepositoryResult<()> {
        let mut default_ttl = self.default_ttl.write().await;
        *default_ttl = ttl;
        debug!("Updated default TTL to {:?}", ttl);
        Ok(())
    }

    async fn get_default_ttl(&self) -> RepositoryResult<Duration> {
        Ok(*self.default_ttl.read().await)
    }

    async fn extend_ttl(&self, key: &str, additional_ttl: Duration) -> RepositoryResult<bool> {
        let mut cache = self.cache.write().await;
        if let Some(entry) = cache.get_mut(key) {
            if !entry.is_expired() {
                entry.expires_at += additional_ttl.as_secs();
                debug!("Extended TTL for key {} by {:?}", key, additional_ttl);
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn get_ttl(&self, key: &str) -> RepositoryResult<Option<Duration>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                let remaining = entry.remaining_ttl();
                return Ok(Some(remaining));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_cache_operations() {
        let cache = InMemoryCacheRepository::new();

        // Test set and get
        cache.set("key1", "value1".to_string()).await.unwrap();
        let value: Option<String> = cache.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Test exists
        assert!(cache.exists("key1").await.unwrap());
        assert!(!cache.exists("nonexistent").await.unwrap());

        // Test remove
        assert!(cache.remove("key1").await.unwrap());
        let value: Option<String> = cache.get("key1").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache = InMemoryCacheRepository::new();

        // Set with short TTL
        cache
            .set_with_ttl(
                "short_lived",
                "value".to_string(),
                Duration::from_millis(100),
            )
            .await
            .unwrap();

        // Should exist immediately
        assert!(cache.exists("short_lived").await.unwrap());

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should be expired
        let value: Option<String> = cache.get("short_lived").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = InMemoryCacheRepository::new();

        cache.set("key1", "value1".to_string()).await.unwrap();

        // Hit
        let _: Option<String> = cache.get("key1").await.unwrap();

        // Miss
        let _: Option<String> = cache.get("nonexistent").await.unwrap();

        let stats = cache.cache_stats().await.unwrap();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate(), 50.0);
    }

    #[tokio::test]
    async fn test_size_limit() {
        let cache = InMemoryCacheRepository::with_config(Duration::from_secs(3600), 2);

        // Add entries up to limit
        cache.set("key1", "value1".to_string()).await.unwrap();
        cache.set("key2", "value2".to_string()).await.unwrap();

        // Add one more - should trigger eviction
        cache.set("key3", "value3".to_string()).await.unwrap();

        let size = cache.size().await.unwrap();
        assert_eq!(size, 2); // Should still be 2 due to eviction
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let cache = InMemoryCacheRepository::new();

        // Add entries with short TTL
        cache
            .set_with_ttl("expire1", "value1".to_string(), Duration::from_millis(50))
            .await
            .unwrap();
        cache
            .set_with_ttl("expire2", "value2".to_string(), Duration::from_millis(50))
            .await
            .unwrap();
        cache.set("keep", "value3".to_string()).await.unwrap();

        // Wait for expiration
        sleep(Duration::from_millis(100)).await;

        // Cleanup expired entries
        let removed = cache.cleanup_expired().await.unwrap();
        assert_eq!(removed, 2);

        // Only the non-expired entry should remain
        let size = cache.size().await.unwrap();
        assert_eq!(size, 1);
    }

    #[tokio::test]
    async fn test_ttl_operations() {
        let cache = InMemoryCacheRepository::new();

        cache
            .set_with_ttl("test", "value".to_string(), Duration::from_secs(10))
            .await
            .unwrap();

        // Check TTL
        let ttl = cache.get_ttl("test").await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap().as_secs() <= 10);

        // Extend TTL
        assert!(cache
            .extend_ttl("test", Duration::from_secs(5))
            .await
            .unwrap());

        // TTL should be longer now
        let new_ttl = cache.get_ttl("test").await.unwrap();
        assert!(new_ttl.is_some());
        assert!(new_ttl.unwrap().as_secs() > 10);
    }
}
