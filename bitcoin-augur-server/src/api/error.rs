use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// API-specific error types with proper HTTP status code mapping
#[derive(Error, Debug)]
pub enum ApiError {
    /// Bad request - client error (400)
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Service unavailable - temporary issue (503)
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Internal server error - unexpected failure (500)
    #[error("Internal server error: {0}")]
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, message).into_response()
    }
}

impl From<crate::service::CollectorError> for ApiError {
    fn from(err: crate::service::CollectorError) -> Self {
        use crate::service::CollectorError;

        match err {
            // Map AugurError variants to appropriate HTTP status codes
            CollectorError::EstimationError(augur_err) => {
                match augur_err {
                    // Invalid parameters are client errors (400)
                    bitcoin_augur::AugurError::InvalidParameter(msg) => {
                        ApiError::BadRequest(msg)
                    }
                    // Insufficient data is a temporary issue (503)
                    bitcoin_augur::AugurError::InsufficientData(msg) => {
                        ApiError::ServiceUnavailable(msg)
                    }
                    // Other errors are internal server errors (500)
                    _ => ApiError::InternalError(format!("Estimation error: {augur_err}")),
                }
            }
            // RPC errors are usually temporary issues
            CollectorError::RpcError(err) => {
                ApiError::ServiceUnavailable(format!("Bitcoin RPC error: {err}"))
            }
            // Persistence errors are internal issues
            CollectorError::PersistenceError(err) => {
                ApiError::InternalError(format!("Storage error: {err}"))
            }
            // Shutdown is a service unavailable issue
            CollectorError::Shutdown => {
                ApiError::ServiceUnavailable("Service is shutting down".to_string())
            }
        }
    }
}