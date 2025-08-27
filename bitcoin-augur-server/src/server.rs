use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{info, Level};

use crate::{
    api::{get_fee_for_target, get_fees, get_historical_fee},
    service::MempoolCollector,
};

/// Create the Axum application router
pub fn create_app(collector: Arc<MempoolCollector>) -> Router {
    Router::new()
        // Fee estimation endpoints
        .route("/fees", get(get_fees))
        .route("/fees/target/:num_blocks", get(get_fee_for_target))
        .route("/historical_fee", get(get_historical_fee))
        // Health check endpoint
        .route("/health", get(health_check))
        // Add shared state
        .with_state(collector)
        // Add middleware
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Run the HTTP server
pub async fn run_server(app: Router, host: String, port: u16) -> Result<(), std::io::Error> {
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("HTTP server listening on http://{}", addr);
    info!("API endpoints:");
    info!("  GET /fees - Current fee estimates");
    info!("  GET /fees/target/{{num_blocks}} - Fee estimates for specific target");
    info!("  GET /historical_fee?timestamp={{unix_ts}} - Historical fee estimates");
    info!("  GET /health - Health check");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(std::io::Error::other)
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");

    info!("Received shutdown signal, shutting down gracefully...");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin::{BitcoinRpcClient, BitcoinRpcConfig};
    use crate::persistence::SnapshotStore;
    use axum::http::{Method, Request};
    use bitcoin_augur::FeeEstimator;
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn create_test_app() -> Router {
        let temp_dir = TempDir::new().unwrap();
        let config = BitcoinRpcConfig {
            url: "http://localhost:8332".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
        };

        let bitcoin_client = crate::bitcoin::BitcoinClient::Real(BitcoinRpcClient::new(config));
        let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
        let fee_estimator = FeeEstimator::new();

        let collector = Arc::new(MempoolCollector::new(
            bitcoin_client,
            snapshot_store,
            fee_estimator,
        ));

        create_app(collector)
    }

    #[tokio::test]
    async fn test_health_check() {
        let app = create_test_app().await;

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_fees_endpoint_exists() {
        let app = create_test_app().await;

        let request = Request::builder()
            .method(Method::GET)
            .uri("/fees")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Will return 503 (no data) but endpoint exists
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
