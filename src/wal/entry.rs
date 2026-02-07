//! WAL Entry definitions
//!
//! Defines the structure of individual WAL log entries.

use std::{time::{ SystemTime, UNIX_EPOCH}};
use serde::{Deserialize, Serialize};

use crate::{AtlasError, Result};

/// Header size: LSN (8) + CRC (4) + Len (4) = 16 bytes
pub const HEADER_SIZE: usize = 16;

/// A single entry in the WAL
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalEntry {
    /// Log Sequence Number - monotonically increasing
    pub lsn: u64,

    /// The operation to perform
    pub operation: Operation,

    /// Timestamp (unix millis) when entry was created
    pub timestamp: u64,
}

/// Operations that can be logged
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operation {
    /// Put a key-value pair
    Put { key: Vec<u8>, value: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },
}

impl WalEntry {
    pub fn new(lsn: u64, operation: Operation) -> Self {
        WalEntry {
            lsn,
            operation,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    /// Serialize the entry to bytes with header (LSN + CRC + Len + Data)
    ///
    /// ## Format
    /// ```text
    /// [LSN: 8 bytes][CRC: 4 bytes][Len: 4 bytes][Data: variable]
    /// ```
    pub fn serialize(&self) -> Result<Vec<u8>> {
        // Step 1: Serialize the entry data using bincode
        let data = bincode::serialize(self).map_err(|e| {
            AtlasError::Serialization(format!("Failed to serialize WAL entry: {}", e))
        })?;

        let data_len = data.len() as u32;

        // Step 2: Build the buffer for CRC calculation (LSN + Len + Data)
        let mut crc_buffer = Vec::with_capacity(8 + 4 + data.len());
        crc_buffer.extend_from_slice(&self.lsn.to_le_bytes()); // LSN: 8 bytes
        crc_buffer.extend_from_slice(&data_len.to_le_bytes()); // Len: 4 bytes
        crc_buffer.extend_from_slice(&data);                   // Data: variable

        // Step 3: Compute CRC32 checksum
        let crc = crc32fast::hash(&crc_buffer);

        // Step 4: Build final output: LSN + CRC + Len + Data
        let mut output = Vec::with_capacity(HEADER_SIZE + data.len());
        output.extend_from_slice(&self.lsn.to_le_bytes());      // LSN: 8 bytes
        output.extend_from_slice(&crc.to_le_bytes());           // CRC: 4 bytes
        output.extend_from_slice(&data_len.to_le_bytes());      // Len: 4 bytes
        output.extend_from_slice(&data);                        // Data: variable

        Ok(output)
    }

    /// Deserialize an entry from bytes, validating the CRC
    ///
    /// Returns error if:
    /// - Buffer too small
    /// - CRC mismatch (corruption detected)
    /// - bincode deserialization fails
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        // Step 1: Validate minimum size
        if bytes.len() < HEADER_SIZE {
            return Err(AtlasError::WalCorruption(format!(
                "Entry too small: {} bytes, expected at least {}",
                bytes.len(),
                HEADER_SIZE
            )));
        }

        // Step 2: Parse header fields
        let lsn = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let stored_crc = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let data_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;

        // Step 3: Validate total size
        let expected_size = HEADER_SIZE + data_len;
        if bytes.len() < expected_size {
            return Err(AtlasError::WalCorruption(format!(
                "Entry truncated: {} bytes, expected {}",
                bytes.len(),
                expected_size
            )));
        }

        // Step 4: Extract data section
        let data = &bytes[HEADER_SIZE..HEADER_SIZE + data_len];

        // Step 5: Recompute CRC and validate
        let mut crc_buffer = Vec::with_capacity(8 + 4 + data_len);
        crc_buffer.extend_from_slice(&lsn.to_le_bytes());
        crc_buffer.extend_from_slice(&(data_len as u32).to_le_bytes());
        crc_buffer.extend_from_slice(data);

        let computed_crc = crc32fast::hash(&crc_buffer);

        if computed_crc != stored_crc {
            return Err(AtlasError::WalCorruption(format!(
                "CRC mismatch for LSN {}: stored={:#x}, computed={:#x}",
                lsn, stored_crc, computed_crc
            )));
        }

        // Step 6: Deserialize the data section
        let entry: WalEntry = bincode::deserialize(data).map_err(|e| {
            AtlasError::WalCorruption(format!("Failed to deserialize WAL entry: {}", e))
        })?;

        // Step 7: Sanity check - LSN in header should match LSN in data
        if entry.lsn != lsn {
            return Err(AtlasError::WalCorruption(format!(
                "LSN mismatch: header={}, data={}",
                lsn, entry.lsn
            )));
        }

        Ok(entry)
    }

    /// Get the total serialized size of this entry (without actually serializing)
    /// Only used for testing (maybe we add some marker to signify this)
    pub fn serialized_size(&self) -> Result<usize> {
        let data_size = bincode::serialized_size(self).map_err(|e| {
            AtlasError::Serialization(format!("Failed to compute size: {}", e))
        })? as usize;

        Ok(HEADER_SIZE + data_size)
    }

    /// Compute CRC32 for this entry (for external use)
    pub fn compute_crc(&self) -> Result<u32> {
        let data = bincode::serialize(self).map_err(|e| {
            AtlasError::Serialization(format!("Failed to serialize: {}", e))
        })?;

        let mut crc_buffer = Vec::with_capacity(8 + 4 + data.len());
        crc_buffer.extend_from_slice(&self.lsn.to_le_bytes());
        crc_buffer.extend_from_slice(&(data.len() as u32).to_le_bytes());
        crc_buffer.extend_from_slice(&data);

        Ok(crc32fast::hash(&crc_buffer))
    }
}