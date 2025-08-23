//! Bitcoin Core RPC client module for fetching mempool data

mod rpc_client;

pub use rpc_client::{BitcoinRpcClient, BitcoinRpcConfig, RpcError};
