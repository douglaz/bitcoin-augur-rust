//! Persistence layer for storing mempool snapshots

mod snapshot_store;

pub use snapshot_store::{SnapshotStore, PersistenceError};