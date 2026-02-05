//! WAL Reader
//!
//! Handles reading entries from the WAL file.

use std::path::Path;
use crate::error::Result;
use super::WalEntry;

/// Reads entries from the WAL file
pub struct WalReader {
    // TODO: Add fields
    // - file: File
    // - position: u64
}

impl WalReader {
    /// Open a WAL file for reading
    pub fn open(_path: &Path) -> Result<Self> {
        todo!("Implement WAL reader open")
    }

    /// Read the next entry from the WAL
    pub fn next_entry(&mut self) -> Result<Option<WalEntry>> {
        todo!("Implement next_entry")
    }

    /// Iterate over all valid entries
    pub fn entries(self) -> WalIterator {
        todo!("Implement entries iterator")
    }
}

/// Iterator over WAL entries
pub struct WalIterator {
    // TODO: Add fields
}

impl Iterator for WalIterator {
    type Item = Result<WalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!("Implement iterator")
    }
}
