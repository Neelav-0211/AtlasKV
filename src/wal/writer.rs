//! WAL Writer
//!
//! Handles appending entries to the WAL file.

use std::path::Path;
use crate::error::Result;
use crate::config::WalSyncStrategy;
use super::WalEntry;

/// Writes entries to the WAL file
pub struct WalWriter {
    // TODO: Add fields
    // - file: File
    // - current_lsn: u64
    // - sync_strategy: WalSyncStrategy
    // - buffer: Option<BufWriter<File>>
}

impl WalWriter {
    /// Open or create a WAL file
    pub fn open(_path: &Path, _sync_strategy: WalSyncStrategy) -> Result<Self> {
        todo!("Implement WAL writer open")
    }

    /// Append an entry to the WAL
    pub fn append(&mut self, _entry: &WalEntry) -> Result<u64> {
        todo!("Implement WAL append")
    }

    /// Force sync to disk
    pub fn sync(&mut self) -> Result<()> {
        todo!("Implement WAL sync")
    }

    /// Get the current LSN
    pub fn current_lsn(&self) -> u64 {
        todo!("Implement current_lsn")
    }
}
