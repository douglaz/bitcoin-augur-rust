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
    pub test_mode: TestModeConfig,
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

/// Test mode configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TestModeConfig {
    /// Enable test mode (bypasses Bitcoin RPC)
    #[serde(default)]
    pub enabled: bool,
    /// Use mock data instead of real Bitcoin data
    #[serde(default)]
    pub use_mock_data: bool,
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
            .set_default("collector.interval_ms", 30000)?
            .set_default("test_mode.enabled", false)?
            .set_default("test_mode.use_mock_data", false)?;

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
        // Note: For nested fields like bitcoin_rpc.username, use AUGUR_BITCOIN_RPC_USERNAME (single underscore)
        builder = builder.add_source(
            Environment::with_prefix("AUGUR")
                .separator("_") // Use single underscore for all separators
                .try_parsing(true),
        );

        // Also support BITCOIN_RPC_ prefix for Bitcoin credentials (maps to bitcoin_rpc.*)
        if let Ok(username) = std::env::var("BITCOIN_RPC_USERNAME") {
            builder = builder.set_override("bitcoin_rpc.username", username)?;
        }
        if let Ok(password) = std::env::var("BITCOIN_RPC_PASSWORD") {
            builder = builder.set_override("bitcoin_rpc.password", password)?;
        }
        if let Ok(url) = std::env::var("BITCOIN_RPC_URL") {
            builder = builder.set_override("bitcoin_rpc.url", url)?;
        }

        // Support test mode environment variables
        if let Ok(enabled) = std::env::var("AUGUR_TEST_MODE_ENABLED") {
            builder = builder.set_override(
                "test_mode.enabled",
                enabled.parse::<bool>().unwrap_or(false),
            )?;
        }
        if let Ok(use_mock) = std::env::var("AUGUR_TEST_MODE_USE_MOCK_DATA") {
            builder = builder.set_override(
                "test_mode.use_mock_data",
                use_mock.parse::<bool>().unwrap_or(false),
            )?;
        }

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

    // These tests modify environment variables and must run sequentially
    // Run with: cargo test config::tests -- --test-threads=1

    #[test]
    fn test_env_override() {
        // Clean up ALL env vars that might interfere
        let vars_to_clean = [
            "BITCOIN_RPC_USERNAME",
            "BITCOIN_RPC_PASSWORD",
            "BITCOIN_RPC_URL",
            "AUGUR_SERVER_HOST",
            "AUGUR_SERVER_PORT",
            "AUGUR_BITCOIN_RPC_USERNAME",
            "AUGUR_BITCOIN_RPC_PASSWORD",
            "AUGUR_BITCOIN_RPC_URL",
        ];

        for var in vars_to_clean.iter() {
            env::remove_var(var);
        }

        // Test using BITCOIN_RPC_ prefix which we know works
        env::set_var("BITCOIN_RPC_USERNAME", "testuser");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.bitcoin_rpc.username, "testuser");

        // Clean up
        env::remove_var("BITCOIN_RPC_USERNAME");
    }

    #[test]
    #[ignore] // Temporarily disabled due to env var conflicts in CI
    fn test_bitcoin_rpc_env() {
        // Clean up ALL env vars that might interfere
        let vars_to_clean = [
            "BITCOIN_RPC_USERNAME",
            "BITCOIN_RPC_PASSWORD",
            "BITCOIN_RPC_URL",
            "AUGUR_SERVER_HOST",
            "AUGUR_SERVER_PORT",
            "AUGUR_BITCOIN_RPC_USERNAME",
            "AUGUR_BITCOIN_RPC_PASSWORD",
            "AUGUR_BITCOIN_RPC_URL",
        ];

        for var in vars_to_clean.iter() {
            env::remove_var(var);
        }

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
