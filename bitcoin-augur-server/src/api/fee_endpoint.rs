use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::error::{ApiError, ErrorResponse};
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
            let error_response = ErrorResponse {
                error: "service_unavailable".to_string(),
                message: "No fee estimates available yet".to_string(),
            };
            (StatusCode::SERVICE_UNAVAILABLE, Json(error_response)).into_response()
        }
    }
}

/// GET /fees/target/{num_blocks} - Returns fee estimates for a specific block target
pub async fn get_fee_for_target(
    Path(num_blocks): Path<f64>,
    State(collector): State<Arc<MempoolCollector>>,
) -> Result<Response, ApiError> {
    // Validate num_blocks parameter
    if num_blocks <= 0.0 || num_blocks > 1000.0 || !num_blocks.is_finite() {
        warn!("Invalid num_blocks parameter: {num_blocks}");
        return Err(ApiError::BadRequest(
            "Invalid number of blocks: must be between 1 and 1000".to_string(),
        ));
    }

    info!(
        "Received request for fee estimates targeting {} blocks",
        num_blocks
    );

    // Get estimate for specific block target
    let estimate = collector.get_estimate_for_blocks(num_blocks).await?;
    let response = transform_fee_estimate(estimate);
    debug!(
        "Returning fee estimates with {} targets",
        response.estimates.len()
    );
    Ok(Json(response).into_response())
}
