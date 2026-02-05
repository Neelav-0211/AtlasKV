//! MemTable implementation
//!
//! BTreeMap-based memtable with RwLock for concurrency.

use crate::error::Result;
use super::MemTableEntry;

/// In-memory table for recent writes
pub struct MemTable {
    // TODO: Add fields
    // - data: RwLock<BTreeMap<Vec<u8>, MemTableEntry>>
    // - size: AtomicUsize (approximate size in bytes)
    // - entry_count: AtomicUsize
}

impl MemTable {
    /// Create a new empty MemTable
    pub fn new() -> Self {
        todo!("Implement MemTable::new")
    }

    /// Get a value by key (read lock)
    pub fn get(&self, _key: &[u8]) -> Result<Option<MemTableEntry>> {
        todo!("Implement get")
    }

    /// Put a key-value pair (write lock)
    pub fn put(&self, _key: Vec<u8>, _value: Vec<u8>) -> Result<()> {
        todo!("Implement put")
    }

    /// Delete a key (write lock, inserts tombstone)
    pub fn delete(&self, _key: Vec<u8>) -> Result<()> {
        todo!("Implement delete")
    }

    /// Get approximate size in bytes
    pub fn size(&self) -> usize {
        todo!("Implement size")
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        todo!("Implement entry_count")
    }

    /// Check if should flush (size > limit)
    pub fn should_flush(&self, _size_limit: usize) -> bool {
        todo!("Implement should_flush")
    }

    /// Get an iterator over all entries (for flush)
    /// Returns entries in sorted key order
    pub fn iter(&self) -> MemTableIterator {
        todo!("Implement iter")
    }

    /// Clear all entries (after successful flush)
    pub fn clear(&self) -> Result<()> {
        todo!("Implement clear")
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over MemTable entries
pub struct MemTableIterator {
    // TODO: Add fields
}

impl Iterator for MemTableIterator {
    type Item = (Vec<u8>, MemTableEntry);

    fn next(&mut self) -> Option<Self::Item> {
        todo!("Implement iterator")
    }
}
