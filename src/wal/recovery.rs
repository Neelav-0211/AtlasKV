//! WAL Recovery
//!
//! Handles crash recovery by replaying the WAL.

use std::path::Path;
use crate::{AtlasError, error::Result, wal::WalReader};
use super::WalEntry;

/// Handles WAL recovery after crash
pub struct WalRecovery {
}

/// Result of a recovery operation
#[derive(Debug)]
pub struct RecoveryResult {
    /// Number of entries successfully recovered
    pub entries_recovered: u64,

    /// Number of corrupted entries skipped
    pub entries_corrupted: u64,

    /// Last valid LSN
    pub last_lsn: u64,

    /// Whether the WAL was truncated (partial writes removed)
    pub was_truncated: bool,
}

impl WalRecovery {
    /// Recover entries from a WAL file
    ///
    /// This will:
    /// 1. Read all valid entries
    /// 2. Detect and skip corrupted entries
    /// 3. Truncate partial writes at end
    /// 4. Return all valid entries in order
    pub fn recover(path: &Path) -> Result<(Vec<WalEntry>, RecoveryResult)> {
        let mut reader = WalReader::open(path)?;

        let mut entries: Vec<WalEntry> = Vec::new();
        let mut entries_recovered: u64 = 0;
        let mut entries_corrupted: u64 = 0;
        let mut last_lsn: u64 = 0;
        let mut was_truncated = false;

        loop {
            match reader.next_entry() {
                Ok(Some(entry)) => {
                    // Valid entry — track LSN and collect
                    last_lsn = entry.lsn;
                    entries_recovered += 1;
                    entries.push(entry);
                }
                Ok(None) => {
                    // Partial write at tail means the WAL needs truncation
                    if !reader.is_at_eof() {
                        was_truncated = true;
                    }
                    break;
                }
                Err(e) => match e {
                    // CRC mismatch — data is corrupt, stop here
                    AtlasError::WalCorruption(_) => {
                        entries_corrupted += 1;
                        was_truncated = true;
                        break;
                    }
                    // I/O errors propagate up — not a recovery concern
                    _ => return Err(e),
                },
            }
        }

        let result = RecoveryResult {
            entries_recovered,
            entries_corrupted,
            last_lsn,
            was_truncated,
        };

        Ok((entries, result))
    }

    /// Verify integrity of a WAL file without modifying it
    ///
    /// Same logic as recover() but discards the entries — only returns stats.
    pub fn verify(path: &Path) -> Result<RecoveryResult> {
        let mut reader = WalReader::open(path)?;

        let mut entries_recovered: u64 = 0;
        let mut entries_corrupted: u64 = 0;
        let mut last_lsn: u64 = 0;
        let mut was_truncated = false;

        loop {
            match reader.next_entry() {
                Ok(Some(entry)) => {
                    // Use the actual LSN from the entry, not a counter
                    last_lsn = entry.lsn;
                    entries_recovered += 1;
                }
                Ok(None) => {
                    if !reader.is_at_eof() {
                        was_truncated = true;
                    }
                    break;
                }
                Err(e) => match e {
                    AtlasError::WalCorruption(_) => {
                        entries_corrupted += 1;
                        was_truncated = true;
                        break;
                    }
                    _ => return Err(e),
                },
            }
        }

        Ok(RecoveryResult {
            entries_recovered,
            entries_corrupted,
            last_lsn,
            was_truncated,
        })
    }
}
