//! Storage Manager
//!
//! Manages multiple SSTables and coordinates reads/writes.

use std::path::Path;
use crate::error::Result;
use crate::memtable::MemTable;
use super::SSTable;

/// Manages the storage layer
pub struct StorageManager {
    // TODO: Add fields
    // - data_dir: PathBuf
    // - sstables: Vec<SSTable>  // Sorted by age, newest first
    // - next_sstable_id: u64
}

impl StorageManager {
    /// Open or create storage in the given directory
    pub fn open(_path: &Path) -> Result<Self> {
        todo!("Implement StorageManager::open")
    }

    /// Get a value by key (searches all SSTables)
    pub fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>> {
        todo!("Implement get")
    }

    /// Flush a MemTable to a new SSTable
    pub fn flush(&mut self, _memtable: &MemTable) -> Result<SSTable> {
        todo!("Implement flush")
    }

    /// Get list of all SSTables
    pub fn sstables(&self) -> &[SSTable] {
        todo!("Implement sstables")
    }

    /// Compact SSTables (future - merges multiple SSTables)
    #[allow(dead_code)]
    fn compact(&mut self) -> Result<()> {
        todo!("Implement compaction")
    }
}
