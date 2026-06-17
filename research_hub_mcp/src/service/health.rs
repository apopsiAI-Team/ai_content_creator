use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{info, instrument};

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
    pub timestamp: SystemTime,
    pub checks: HealthChecks,
}

/// Individual health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecks {
    pub server: bool,
    pub database: bool,
    pub external_services: bool,
    pub memory: bool,
    pub cpu: bool,
}

impl Default for HealthChecks {
    fn default() -> Self {
        Self {
            server: true,
            database: true,
            external_services: true,
            memory: true,
            cpu: true,
        }
    }
}

/// Health check service
pub struct HealthCheck {
    port: u16,
    status: Arc<RwLock<HealthStatus>>,
}

impl HealthCheck {
    /// Create a new health check service
    #[must_use]
    pub fn new(port: u16) -> Self {
        let status = HealthStatus {
            healthy: true,
            message: "Service is healthy".to_string(),
            timestamp: SystemTime::now(),
            checks: HealthChecks::default(),
        };

        Self {
            port,
            status: Arc::new(RwLock::new(status)),
        }
    }

    /// Start the health check HTTP endpoint
    #[instrument(skip(self))]
    pub async fn start(&self) -> crate::Result<()> {
        info!("Starting health check endpoint on port {}", self.port);

        let app = self.create_app();
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| crate::Error::Service(format!("Failed to bind health port: {e}")))?;

        info!("Health check endpoint listening on {}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::Error::Service(format!("Health check server error: {e}")))?;

        Ok(())
    }

    /// Create the axum application
    fn create_app(&self) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/health/startup", get(startup_handler))
            .with_state(self.status.clone())
            .layer(TraceLayer::new_for_http())
    }

    /// Get current health status
    pub async fn get_status(&self) -> HealthStatus {
        self.status.read().await.clone()
    }

    /// Set service as healthy
    pub async fn set_healthy(&self) {
        let mut status = self.status.write().await;
        status.healthy = true;
        status.message = "Service is healthy".to_string();
        status.timestamp = SystemTime::now();
    }

    /// Set service as unhealthy
    pub async fn set_unhealthy(&self, reason: &str) {
        let mut status = self.status.write().await;
        status.healthy = false;
        status.message = reason.to_string();
        status.timestamp = SystemTime::now();
    }

    /// Update individual health check
    pub async fn update_check(&self, check: HealthCheckType, healthy: bool) {
        let mut status = self.status.write().await;

        match check {
            HealthCheckType::Server => status.checks.server = healthy,
            HealthCheckType::Database => status.checks.database = healthy,
            HealthCheckType::ExternalServices => status.checks.external_services = healthy,
            HealthCheckType::Memory => status.checks.memory = healthy,
            HealthCheckType::Cpu => status.checks.cpu = healthy,
        }

        // Update overall health status
        status.healthy = status.checks.server
            && status.checks.database
            && status.checks.external_services
            && status.checks.memory
            && status.checks.cpu;

        status.timestamp = SystemTime::now();

        if status.healthy {
            status.message = "Service is healthy".to_string();
        } else {
            status.message = "One or more health checks failed".to_string();
        }
    }

    /// Perform all health checks
    pub async fn check_all(&self) -> HealthStatus {
        // Perform actual health checks here
        // For now, we'll use the stored status
        self.status.read().await.clone()
    }
}

/// Health check types
pub enum HealthCheckType {
    Server,
    Database,
    ExternalServices,
    Memory,
    Cpu,
}

/// Main health handler
async fn health_handler(
    State(status): State<Arc<RwLock<HealthStatus>>>,
) -> (StatusCode, Json<HealthStatus>) {
    let health = status.read().await.clone();

    let status_code = if health.healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(health))
}

/// Liveness probe handler (is the service alive?)
async fn liveness_handler() -> StatusCode {
    // Simple liveness check - if we can respond, we're alive
    StatusCode::OK
}

/// Readiness probe handler (is the service ready to accept requests?)
async fn readiness_handler(State(status): State<Arc<RwLock<HealthStatus>>>) -> StatusCode {
    let health = status.read().await;

    if health.healthy && health.checks.server {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// Startup probe handler (has the service started successfully?)
async fn startup_handler(
    State(status): State<Arc<RwLock<HealthStatus>>>,
) -> (StatusCode, Json<StartupStatus>) {
    let health = status.read().await;

    let startup = StartupStatus {
        started: health.checks.server,
        timestamp: health.timestamp,
    };

    let status_code = if startup.started {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(startup))
}

/// Startup status response
#[derive(Debug, Serialize, Deserialize)]
struct StartupStatus {
    started: bool,
    timestamp: SystemTime,
}

impl std::fmt::Debug for HealthCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthCheck")
            .field("port", &self.port)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_creation() {
        let health = HealthCheck::new(8090);
        let status = health.get_status().await;
        assert!(status.healthy);
        assert_eq!(status.message, "Service is healthy");
    }

    #[tokio::test]
    async fn test_set_unhealthy() {
        let health = HealthCheck::new(8090);
        health.set_unhealthy("Test failure").await;

        let status = health.get_status().await;
        assert!(!status.healthy);
        assert_eq!(status.message, "Test failure");
    }

    #[tokio::test]
    async fn test_update_check() {
        let health = HealthCheck::new(8090);
        health.update_check(HealthCheckType::Database, false).await;

        let status = health.get_status().await;
        assert!(!status.healthy);
        assert!(!status.checks.database);
    }

    #[test]
    fn test_health_checks_default() {
        let checks = HealthChecks::default();
        assert!(checks.server);
        assert!(checks.database);
        assert!(checks.external_services);
        assert!(checks.memory);
        assert!(checks.cpu);
    }
}
