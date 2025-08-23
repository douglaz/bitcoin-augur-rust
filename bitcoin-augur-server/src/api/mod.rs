//! HTTP API endpoints for fee estimation service

mod fee_endpoint;
mod historical;
mod models;

pub use fee_endpoint::{get_fee_for_target, get_fees};
pub use historical::get_historical_fee;
pub use models::{BlockTargetResponse, FeeEstimateResponse, ProbabilityResponse};
