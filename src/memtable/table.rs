//! MemTable implementation
//!
//! BTreeMap-based memtable with RwLock for concurrency.
//! Uses parking_lot::RwLock which never poisons on panic.

use super::MemTableEntry;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use parking_lot::RwLock;

/// In-memory table for recent writes
pub struct MemTable {
    /// Sorted key-value store with concurrent access
    data: RwLock<BTreeMap<Vec<u8>, MemTableEntry>>,
    
    /// Approximate size in bytes (for flush trigger)
    size: AtomicUsize,
}

impl MemTable {
    /// Create a new empty MemTable
    pub fn new() -> Self {
        MemTable { 
            data: RwLock::new(BTreeMap::new()), 
            size: AtomicUsize::new(0),
        }
    }

    /// Get a value by key (read lock)
    pub fn get(&self, key: &[u8]) -> Option<MemTableEntry> {
        let data = self.data.read();
        data.get(key).cloned()
    }

    /// Put a key-value pair (write lock)
    /// Returns new total size
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> usize {
        let entry_size = key.len() + value.len();
        let mut data = self.data.write();

        let old_size = data.get(&key)
            .map(|entry| match entry {
                MemTableEntry::Value(v) => key.len() + v.len(),
                MemTableEntry::Tombstone => key.len(),
            })
            .unwrap_or(0);

        data.insert(key, MemTableEntry::Value(value));

        let size_delta = entry_size as isize - old_size as isize;
        if size_delta > 0 {
            self.size.fetch_add(size_delta as usize, Ordering::Relaxed);
        } else {
            self.size.fetch_sub((-size_delta) as usize, Ordering::Relaxed);
        }

        self.size.load(Ordering::Relaxed)
    }

    /// Delete a key (write lock, inserts tombstone)
    /// Returns new total size
    pub fn delete(&self, key: Vec<u8>) -> usize {
        let mut data = self.data.write();

        let old_size = data.get(&key)
            .map(|entry| match entry {
                MemTableEntry::Value(v) => key.len() + v.len(),
                MemTableEntry::Tombstone => key.len(),
            })
            .unwrap_or(0);

        let new_size = key.len(); // Tombstone = just key
        data.insert(key, MemTableEntry::Tombstone);

        let size_delta = new_size as isize - old_size as isize;
        if size_delta > 0 {
            self.size.fetch_add(size_delta as usize, Ordering::Relaxed);
        } else {
            self.size.fetch_sub((-size_delta) as usize, Ordering::Relaxed);
        }

        self.size.load(Ordering::Relaxed)
    }

    /// Get current size in bytes
    pub fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.data.read().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entry_count() == 0
    }

    /// Check if size exceeds limit (for flush trigger)
    pub fn should_flush(&self, size_limit: usize) -> bool {
        self.size() >= size_limit
    }

    /// Get a snapshot of all entries (for flush to SSTable)
    /// Returns entries in sorted key order
    pub fn iter(&self) -> Vec<(Vec<u8>, MemTableEntry)> {
        let data = self.data.read();
        data.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Clear all entries (after successful flush)
    pub fn clear(&self) {
        let mut data = self.data.write();
        data.clear();
        self.size.store(0, Ordering::Relaxed);
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}
