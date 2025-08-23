//! HTTP API endpoints for fee estimation service

mod models;
mod fee_endpoint;
mod historical;

pub use models::{FeeEstimateResponse, BlockTargetResponse, ProbabilityResponse};
pub use fee_endpoint::{get_fees, get_fee_for_target};
pub use historical::get_historical_fee;