/// Internal modules for the bitcoin-augur library.
/// These are implementation details and should not be used directly by library consumers.
pub(crate) mod bucket_creator;
pub(crate) mod fee_calculator;
pub(crate) mod inflow_calculator;
pub(crate) mod snapshot_array;

// Re-export for internal use only
pub(crate) use bucket_creator::BUCKET_MAX;
pub(crate) use fee_calculator::FeeCalculator;
pub(crate) use inflow_calculator::InflowCalculator;
pub(crate) use snapshot_array::SnapshotArray;
