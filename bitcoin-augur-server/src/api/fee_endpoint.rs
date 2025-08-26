use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::models::transform_fee_estimate;
use crate::service::MempoolCollector;

/// GET /fees - Returns current fee estimates for all block targets
pub async fn get_fees(State(collector): State<Arc<MempoolCollector>>) -> Response {
    info!("Received request for fee estimates");

    match collector.get_latest_estimate().await {
        Some(estimate) => {
            let response = transform_fee_estimate(estimate);
            debug!(
                "Returning fee estimates with {} targets",
                response.estimates.len()
            );
            Json(response).into_response()
        }
        None => {
            warn!("No fee estimates available yet");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "No fee estimates available yet",
            )
                .into_response()
        }
    }
}

/// GET /fees/target/{num_blocks} - Returns fee estimates for a specific block target
pub async fn get_fee_for_target(
    Path(num_blocks): Path<f64>,
    State(collector): State<Arc<MempoolCollector>>,
) -> Response {
    // Validate num_blocks parameter
    if num_blocks <= 0.0 || num_blocks > 1000.0 || !num_blocks.is_finite() {
        warn!("Invalid num_blocks parameter: {}", num_blocks);
        return (
            StatusCode::BAD_REQUEST,
            "Invalid or missing number of blocks",
        )
            .into_response();
    }

    info!(
        "Received request for fee estimates targeting {} blocks",
        num_blocks
    );

    // Get estimate for specific block target
    match collector.get_estimate_for_blocks(num_blocks).await {
        Ok(estimate) => {
            let response = transform_fee_estimate(estimate);
            debug!(
                "Returning fee estimates with {} targets",
                response.estimates.len()
            );
            Json(response).into_response()
        }
        Err(err) => {
            warn!("Failed to calculate fee estimates: {}", err);

            // Check if it's a data availability issue
            if err.to_string().contains("Insufficient") {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "No fee estimates available yet",
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to calculate fee estimates",
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
#[path = "fee_endpoint_tests.rs"]
mod tests;
