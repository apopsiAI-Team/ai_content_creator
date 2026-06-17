use std::time::Duration;
use thiserror::Error;

/// Comprehensive error categorization for resilience framework
#[derive(Error, Debug)]
pub enum Error {
    // Configuration errors (permanent failures)
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    // I/O errors (potentially transient)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Serialization errors (usually permanent)
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    // Network errors (transient - should retry)
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Network timeout after {timeout:?}: {message}")]
    NetworkTimeout { timeout: Duration, message: String },

    #[error("Connection refused: {endpoint}")]
    ConnectionRefused { endpoint: String },

    #[error("DNS resolution failed: {hostname}")]
    DnsFailure { hostname: String },

    // Service-specific errors
    #[error("MCP protocol error: {0}")]
    Mcp(String),

    #[error("Sci-Hub service error: {code} - {message}")]
    SciHub { code: u16, message: String },

    #[error("Rate limit exceeded: retry after {retry_after:?}")]
    RateLimitExceeded { retry_after: Duration },

    // Client errors (permanent - don't retry)
    #[error("Invalid input: {field} - {reason}")]
    InvalidInput { field: String, reason: String },

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization denied: {resource}")]
    AuthorizationDenied { resource: String },

    // Server errors (transient - should retry)
    #[error("Service temporarily unavailable: {service} - {reason}")]
    ServiceUnavailable { service: String, reason: String },

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Service overloaded: {service}")]
    ServiceOverloaded { service: String },

    // Circuit breaker errors
    #[error("Circuit breaker open for service: {service}")]
    CircuitBreakerOpen { service: String },

    #[error("Circuit breaker half-open, limited requests allowed")]
    CircuitBreakerHalfOpen,

    // Resource errors
    #[error("Resource exhausted: {resource} - {current}/{limit}")]
    ResourceExhausted {
        resource: String,
        current: u64,
        limit: u64,
    },

    #[error("Timeout error: operation timed out after {timeout:?}")]
    Timeout { timeout: Duration },

    // Cache errors
    #[error("Cache error: {operation} failed - {reason}")]
    Cache { operation: String, reason: String },

    // Parse errors
    #[error("Parse error in {context}: {message}")]
    Parse { context: String, message: String },

    // General service error
    #[error("Service error: {0}")]
    Service(String),

    // Provider errors
    #[error("Provider error: {0}")]
    Provider(String),
}

/// Error categorization for retry strategies
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Permanent errors - should not retry
    Permanent,
    /// Transient errors - safe to retry
    Transient,
    /// Rate limited - retry with backoff
    RateLimited,
    /// Circuit breaker triggered - stop retrying temporarily
    CircuitBreaker,
}

impl Error {
    /// Categorize error for retry logic
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self {
            // Permanent errors - don't retry
            Self::Config(_)
            | Self::InvalidInput { .. }
            | Self::AuthenticationFailed(_)
            | Self::AuthorizationDenied { .. }
            | Self::Parse { .. }
            | Self::Serde(_) => ErrorCategory::Permanent,

            // Rate limited - retry with backoff
            Self::RateLimitExceeded { .. } => ErrorCategory::RateLimited,

            // Circuit breaker errors
            Self::CircuitBreakerOpen { .. } | Self::CircuitBreakerHalfOpen => {
                ErrorCategory::CircuitBreaker
            }

            // Transient errors - retry with exponential backoff
            Self::Http(_)
            | Self::NetworkTimeout { .. }
            | Self::ConnectionRefused { .. }
            | Self::DnsFailure { .. }
            | Self::ServiceUnavailable { .. }
            | Self::InternalServerError(_)
            | Self::ServiceOverloaded { .. }
            | Self::Timeout { .. }
            | Self::Io(_) => ErrorCategory::Transient,

            // Service specific - depends on context
            Self::SciHub { code, .. } => {
                match *code {
                    // Rate limiting (handle first to avoid unreachable pattern)
                    429 => ErrorCategory::RateLimited,
                    // 403 Forbidden for Sci-Hub should be treated as transient (mirror blocking)
                    403 => ErrorCategory::Transient,
                    // Other 4xx client errors are permanent (except 403)
                    400..=499 => ErrorCategory::Permanent,
                    // 5xx server errors are transient
                    500..=599 => ErrorCategory::Transient,
                    // Other codes treated as transient
                    _ => ErrorCategory::Transient,
                }
            }

            // Provider errors - categorize based on the error type
            Self::Provider(_) => ErrorCategory::Transient,

            // Default to transient for unknown errors
            _ => ErrorCategory::Transient,
        }
    }

    /// Check if error is retryable
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Transient | ErrorCategory::RateLimited
        )
    }

    /// Get suggested retry delay for rate limited errors
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimitExceeded { retry_after } => Some(*retry_after),
            _ => None,
        }
    }

    /// Check if error indicates a need for circuit breaker
    #[must_use]
    pub const fn should_trigger_circuit_breaker(&self) -> bool {
        matches!(
            self,
            Self::ServiceUnavailable { .. }
                | Self::InternalServerError(_)
                | Self::ServiceOverloaded { .. }
                | Self::NetworkTimeout { .. }
                | Self::ConnectionRefused { .. }
        )
    }
}

pub type Result<T> = std::result::Result<T, Error>;

// Provider error conversion
impl From<crate::client::providers::ProviderError> for Error {
    fn from(err: crate::client::providers::ProviderError) -> Self {
        match err {
            crate::client::providers::ProviderError::Network(msg) => {
                Self::Provider(format!("Network error: {msg}"))
            }
            crate::client::providers::ProviderError::Parse(msg) => Self::Parse {
                context: "provider".to_string(),
                message: msg,
            },
            crate::client::providers::ProviderError::RateLimit => Self::RateLimitExceeded {
                retry_after: Duration::from_secs(60),
            },
            crate::client::providers::ProviderError::Auth(msg) => Self::AuthenticationFailed(msg),
            crate::client::providers::ProviderError::InvalidQuery(msg) => Self::InvalidInput {
                field: "query".to_string(),
                reason: msg,
            },
            crate::client::providers::ProviderError::ServiceUnavailable(msg) => {
                Self::ServiceUnavailable {
                    service: "provider".to_string(),
                    reason: msg,
                }
            }
            crate::client::providers::ProviderError::Timeout => Self::Timeout {
                timeout: Duration::from_secs(30),
            },
            crate::client::providers::ProviderError::Other(msg) => Self::Provider(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        // Permanent errors
        assert_eq!(
            Error::InvalidInput {
                field: "test".to_string(),
                reason: "test".to_string()
            }
            .category(),
            ErrorCategory::Permanent
        );

        // Transient errors
        assert_eq!(
            Error::NetworkTimeout {
                timeout: Duration::from_secs(30),
                message: "test".to_string()
            }
            .category(),
            ErrorCategory::Transient
        );

        // Rate limited errors
        assert_eq!(
            Error::RateLimitExceeded {
                retry_after: Duration::from_secs(60)
            }
            .category(),
            ErrorCategory::RateLimited
        );

        // Sci-Hub specific errors - updated for 403 handling
        assert_eq!(
            Error::SciHub {
                code: 403,
                message: "Forbidden".to_string()
            }
            .category(),
            ErrorCategory::Transient // Changed: 403 is now transient for Sci-Hub
        );

        assert_eq!(
            Error::SciHub {
                code: 429,
                message: "Too Many Requests".to_string()
            }
            .category(),
            ErrorCategory::RateLimited
        );

        assert_eq!(
            Error::SciHub {
                code: 500,
                message: "Internal Server Error".to_string()
            }
            .category(),
            ErrorCategory::Transient
        );

        // Other 4xx errors should still be permanent
        assert_eq!(
            Error::SciHub {
                code: 404,
                message: "Not Found".to_string()
            }
            .category(),
            ErrorCategory::Permanent
        );
    }

    #[test]
    fn test_retry_logic() {
        let transient_error = Error::NetworkTimeout {
            timeout: Duration::from_secs(30),
            message: "test".to_string(),
        };
        assert!(transient_error.is_retryable());

        let permanent_error = Error::InvalidInput {
            field: "test".to_string(),
            reason: "test".to_string(),
        };
        assert!(!permanent_error.is_retryable());

        let rate_limited_error = Error::RateLimitExceeded {
            retry_after: Duration::from_secs(60),
        };
        assert!(rate_limited_error.is_retryable());
        assert_eq!(
            rate_limited_error.retry_after(),
            Some(Duration::from_secs(60))
        );

        // Test that Sci-Hub 403 errors are now retryable
        let scihub_403_error = Error::SciHub {
            code: 403,
            message: "Forbidden".to_string(),
        };
        assert!(scihub_403_error.is_retryable());
    }
}
