//! Bitcoin Core RPC client module for fetching mempool data

mod mock_client;
mod rpc_client;
mod traits;

pub use mock_client::MockBitcoinClient;
pub use rpc_client::{BitcoinRpcClient, BitcoinRpcConfig, RpcError};
pub use traits::{BitcoinClient, BitcoinRpc};
