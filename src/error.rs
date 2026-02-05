//! Error types for AtlasKV
//!
//! Provides a unified error type for all operations.

use thiserror::Error;

/// Result type alias using AtlasError
pub type Result<T> = std::result::Result<T, AtlasError>;

/// Unified error type for AtlasKV operations
#[derive(Debug, Error)]
pub enum AtlasError {
    // -------------------------------------------------------------------------
    // I/O Errors
    // -------------------------------------------------------------------------
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // -------------------------------------------------------------------------
    // WAL Errors
    // -------------------------------------------------------------------------
    #[error("WAL corruption detected: {0}")]
    WalCorruption(String),

    #[error("WAL write failed: {0}")]
    WalWrite(String),

    // -------------------------------------------------------------------------
    // Storage Errors
    // -------------------------------------------------------------------------
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Key not found")]
    KeyNotFound,

    // -------------------------------------------------------------------------
    // Serialization Errors
    // -------------------------------------------------------------------------
    #[error("Serialization error: {0}")]
    Serialization(String),

    // -------------------------------------------------------------------------
    // Network Errors
    // -------------------------------------------------------------------------
    #[error("Network error: {0}")]
    Network(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    // -------------------------------------------------------------------------
    // Configuration Errors
    // -------------------------------------------------------------------------
    #[error("Configuration error: {0}")]
    Config(String),

    // -------------------------------------------------------------------------
    // Concurrency Errors
    // -------------------------------------------------------------------------
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
}
