use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::cli::{read_cookie_file, Cli};

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
    /// Load configuration from file and CLI arguments
    pub fn load_with_cli(cli: &Cli) -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            // Default values (these will be overridden by CLI defaults and args)
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

        // Load from config file if specified via CLI
        if let Some(ref config_file) = cli.config {
            builder = builder.add_source(File::from(Path::new(config_file)));
        } else {
            // Try to load from default config locations (optional)
            builder = builder
                .add_source(File::with_name("augur.toml").required(false))
                .add_source(File::with_name("augur.yaml").required(false))
                .add_source(File::with_name("augur.json").required(false));
        }

        // Apply CLI overrides (highest priority)
        builder = builder
            .set_override("server.host", cli.host.clone())?
            .set_override("server.port", cli.port)?
            .set_override("bitcoin_rpc.url", cli.rpc_url.clone())?
            .set_override("persistence.data_directory", cli.data_dir.clone())?
            .set_override("persistence.cleanup_days", cli.cleanup_days)?
            .set_override("collector.interval_ms", cli.interval_secs * 1000)?
            .set_override("test_mode.enabled", cli.test_mode)?
            .set_override("test_mode.use_mock_data", cli.use_mock_data)?;

        // Handle Bitcoin RPC credentials
        if let Some(ref cookie_file) = cli.rpc_cookie_file {
            // Read credentials from cookie file
            let (username, password) = read_cookie_file(cookie_file)
                .map_err(|e| ConfigError::Message(format!("Failed to read cookie file: {e}")))?;
            builder = builder
                .set_override("bitcoin_rpc.username", username)?
                .set_override("bitcoin_rpc.password", password)?;
        } else {
            // Use username/password if provided
            if let Some(ref username) = cli.rpc_username {
                builder = builder.set_override("bitcoin_rpc.username", username.clone())?;
            }
            if let Some(ref password) = cli.rpc_password {
                builder = builder.set_override("bitcoin_rpc.password", password.clone())?;
            }
        }

        builder.build()?.try_deserialize()
    }

    /// Load configuration (deprecated - for backwards compatibility only)
    pub fn load() -> Result<Self, ConfigError> {
        // Return default config when called without CLI args
        // This is only used in tests now
        Ok(Self::default())
    }

    /// Load configuration from a specific file (for testing)
    #[allow(dead_code)]
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let builder = Config::builder()
            // Default values
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("bitcoin_rpc.url", "http://localhost:8332")?
            .set_default("bitcoin_rpc.username", "")?
            .set_default("bitcoin_rpc.password", "")?
            .set_default("persistence.data_directory", "mempool_data")?
            .set_default("persistence.cleanup_days", 30)?
            .set_default("collector.interval_ms", 30000)?
            // Load from specified file
            .add_source(File::from(path.as_ref()));

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
    fn test_cli_override() {
        use clap::Parser;

        // Test that CLI args override defaults
        let cli = Cli::try_parse_from(&[
            "bitcoin-augur-server",
            "--host",
            "127.0.0.1",
            "--port",
            "9000",
            "--rpc-username",
            "testuser",
            "--rpc-password",
            "testpass",
            "--data-dir",
            "/tmp/test",
            "--interval-secs",
            "60",
        ])
        .unwrap();

        let config = AppConfig::load_with_cli(&cli).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.bitcoin_rpc.username, "testuser");
        assert_eq!(config.bitcoin_rpc.password, "testpass");
        assert_eq!(config.persistence.data_directory, "/tmp/test");
        assert_eq!(config.collector.interval_ms, 60000);
    }

    #[test]
    fn test_load_returns_default() {
        // load() should now return default config without environment access
        let config = AppConfig::load().unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.bitcoin_rpc.username, "");
        assert_eq!(config.bitcoin_rpc.password, "");
    }
}
