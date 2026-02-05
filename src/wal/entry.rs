//! WAL Entry definitions
//!
//! Defines the structure of individual WAL log entries.

use serde::{Deserialize, Serialize};

/// A single entry in the WAL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Log Sequence Number - monotonically increasing
    pub lsn: u64,

    /// The operation to perform
    pub operation: Operation,

    /// Timestamp (unix millis) when entry was created
    pub timestamp: u64,
}

/// Operations that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Put a key-value pair
    Put { key: Vec<u8>, value: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },
}

// TODO: Implement WalEntry methods
// - new(lsn, operation) -> Self
// - serialize() -> Result<Vec<u8>>
// - deserialize(bytes) -> Result<Self>
// - compute_crc() -> u32
