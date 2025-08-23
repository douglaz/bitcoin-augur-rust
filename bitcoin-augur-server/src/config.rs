use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Application configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub bitcoin_rpc: BitcoinRpcConfig,
    pub persistence: PersistenceConfig,
    pub collector: CollectorConfig,
}

/// HTTP server configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    /// Host to bind to (default: 0.0.0.0)
    pub host: String,
    /// Port to listen on (default: 8080)
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

/// Bitcoin RPC configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BitcoinRpcConfig {
    /// RPC URL (default: http://localhost:8332)
    pub url: String,
    /// RPC username
    pub username: String,
    /// RPC password
    pub password: String,
}

impl Default for BitcoinRpcConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8332".to_string(),
            username: String::new(),
            password: String::new(),
        }
    }
}

/// Persistence configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PersistenceConfig {
    /// Directory for storing snapshots (default: mempool_data)
    pub data_directory: String,
    /// Days to keep old snapshots (default: 30)
    pub cleanup_days: i64,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            data_directory: "mempool_data".to_string(),
            cleanup_days: 30,
        }
    }
}

/// Mempool collector configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CollectorConfig {
    /// Collection interval in milliseconds (default: 30000)
    pub interval_ms: u64,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self { interval_ms: 30000 }
    }
}

impl AppConfig {
    /// Load configuration from file and environment variables
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            // Default values
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("bitcoin_rpc.url", "http://localhost:8332")?
            .set_default("bitcoin_rpc.username", "")?
            .set_default("bitcoin_rpc.password", "")?
            .set_default("persistence.data_directory", "mempool_data")?
            .set_default("persistence.cleanup_days", 30)?
            .set_default("collector.interval_ms", 30000)?;

        // Load from config file if specified via environment variable
        if let Ok(config_file) = std::env::var("AUGUR_CONFIG_FILE") {
            builder = builder.add_source(File::from(Path::new(&config_file)));
        } else {
            // Try to load default config files
            builder = builder
                .add_source(File::with_name("config/default").required(false))
                .add_source(File::with_name("config").required(false));
        }

        // Override with environment variables (AUGUR_ prefix)
        builder = builder.add_source(
            Environment::with_prefix("AUGUR")
                .separator("_")
                .try_parsing(true),
        );

        // Also support BITCOIN_RPC_ prefix for Bitcoin credentials
        builder = builder.add_source(
            Environment::with_prefix("BITCOIN_RPC")
                .separator("_")
                .try_parsing(true),
        );

        builder.build()?.try_deserialize()
    }

    /// Load configuration from a specific file
    #[allow(dead_code)]
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            // Default values
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("bitcoin_rpc.url", "http://localhost:8332")?
            .set_default("bitcoin_rpc.username", "")?
            .set_default("bitcoin_rpc.password", "")?
            .set_default("persistence.data_directory", "mempool_data")?
            .set_default("persistence.cleanup_days", 30)?
            .set_default("collector.interval_ms", 30000)?;

        // Load from specified file
        builder = builder.add_source(File::from(path.as_ref()));

        // Still allow environment overrides
        builder = builder.add_source(
            Environment::with_prefix("AUGUR")
                .separator("_")
                .try_parsing(true),
        );

        builder.build()?.try_deserialize()
    }

    /// Convert to Bitcoin RPC config for the RPC client
    pub fn to_bitcoin_rpc_config(&self) -> crate::bitcoin::BitcoinRpcConfig {
        crate::bitcoin::BitcoinRpcConfig {
            url: self.bitcoin_rpc.url.clone(),
            username: self.bitcoin_rpc.username.clone(),
            password: self.bitcoin_rpc.password.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.bitcoin_rpc.url, "http://localhost:8332");
        assert_eq!(config.persistence.data_directory, "mempool_data");
        assert_eq!(config.collector.interval_ms, 30000);
    }

    #[test]
    fn test_env_override() {
        // Set environment variables
        env::set_var("AUGUR_SERVER_PORT", "9090");
        env::set_var("AUGUR_BITCOIN_RPC_USERNAME", "testuser");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.bitcoin_rpc.username, "testuser");

        // Clean up
        env::remove_var("AUGUR_SERVER_PORT");
        env::remove_var("AUGUR_BITCOIN_RPC_USERNAME");
    }

    #[test]
    fn test_bitcoin_rpc_env() {
        // Test BITCOIN_RPC_ prefix support
        env::set_var("BITCOIN_RPC_USERNAME", "btcuser");
        env::set_var("BITCOIN_RPC_PASSWORD", "btcpass");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.bitcoin_rpc.username, "btcuser");
        assert_eq!(config.bitcoin_rpc.password, "btcpass");

        // Clean up
        env::remove_var("BITCOIN_RPC_USERNAME");
        env::remove_var("BITCOIN_RPC_PASSWORD");
    }
}
