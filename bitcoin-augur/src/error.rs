use thiserror::Error;

/// Main error type for the bitcoin-augur library.
#[derive(Error, Debug)]
pub enum AugurError {
    /// Invalid configuration provided.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    /// Insufficient data for fee estimation.
    #[error("Insufficient data for estimation: {0}")]
    InsufficientData(String),
    
    /// Error during calculation.
    #[error("Calculation error: {0}")]
    Calculation(String),
    
    /// Invalid input parameter.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// Date/time related error.
    #[error("Time error: {0}")]
    Time(String),
}

/// Type alias for Results in this library.
pub type Result<T> = std::result::Result<T, AugurError>;

impl AugurError {
    /// Creates an InvalidConfig error.
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }
    
    /// Creates an InsufficientData error.
    pub fn insufficient_data(msg: impl Into<String>) -> Self {
        Self::InsufficientData(msg.into())
    }
    
    /// Creates a Calculation error.
    pub fn calculation(msg: impl Into<String>) -> Self {
        Self::Calculation(msg.into())
    }
    
    /// Creates an InvalidParameter error.
    pub fn invalid_parameter(msg: impl Into<String>) -> Self {
        Self::InvalidParameter(msg.into())
    }
}