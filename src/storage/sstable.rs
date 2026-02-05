//! SSTable implementation
//!
//! Sorted String Table - immutable on-disk sorted key-value storage.

use std::path::Path;
use crate::error::Result;

/// SSTable metadata
#[derive(Debug)]
pub struct SSTable {
    // TODO: Add fields
    // - path: PathBuf
    // - entry_count: u64
    // - min_key: Vec<u8>
    // - max_key: Vec<u8>
    // - file_size: u64
}

impl SSTable {
    /// Get metadata about this SSTable
    pub fn entry_count(&self) -> u64 {
        todo!("Implement entry_count")
    }

    /// Check if a key might be in this SSTable (range check)
    pub fn might_contain(&self, _key: &[u8]) -> bool {
        todo!("Implement might_contain")
    }
}

/// Builder for creating new SSTables
pub struct SSTableBuilder {
    // TODO: Add fields
    // - path: PathBuf
    // - writer: BufWriter<File>
    // - entry_count: u64
}

impl SSTableBuilder {
    /// Create a new SSTable builder
    pub fn new(_path: &Path) -> Result<Self> {
        todo!("Implement SSTableBuilder::new")
    }

    /// Add a key-value pair (must be called in sorted key order)
    pub fn add(&mut self, _key: &[u8], _value: &[u8]) -> Result<()> {
        todo!("Implement add")
    }

    /// Add a tombstone (must be called in sorted key order)
    pub fn add_tombstone(&mut self, _key: &[u8]) -> Result<()> {
        todo!("Implement add_tombstone")
    }

    /// Finish building and return the SSTable
    pub fn finish(self) -> Result<SSTable> {
        todo!("Implement finish")
    }
}

/// Reader for SSTable files
pub struct SSTableReader {
    // TODO: Add fields
}

impl SSTableReader {
    /// Open an SSTable for reading
    pub fn open(_path: &Path) -> Result<Self> {
        todo!("Implement SSTableReader::open")
    }

    /// Get a value by key
    pub fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>> {
        todo!("Implement get")
    }

    /// Iterate over all entries
    pub fn iter(&self) -> SSTableIterator {
        todo!("Implement iter")
    }
}

/// Iterator over SSTable entries
pub struct SSTableIterator {
    // TODO: Add fields
}

impl Iterator for SSTableIterator {
    type Item = Result<(Vec<u8>, Option<Vec<u8>>)>; // None value = tombstone

    fn next(&mut self) -> Option<Self::Item> {
        todo!("Implement iterator")
    }
}
