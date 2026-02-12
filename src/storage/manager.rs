//! Storage Manager
//!
//! Manages multiple SSTables and coordinates reads/writes.
//!
//! ## Responsibilities
//! - Discover existing SSTables on startup
//! - Search SSTables newest → oldest for reads
//! - Create new SSTables from MemTable flushes
//! - Track SSTable lifecycle

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::memtable::{MemTable, MemTableEntry};
use crate::AtlasError;

use super::{SSTable, SSTableBuilder, SSTableReader};

/// Manages the storage layer
pub struct StorageManager {
    /// Directory where SSTables are stored
    data_dir: PathBuf,

    /// Open SSTable readers, ordered newest → oldest
    /// We store readers (not just metadata) to keep indexes loaded in RAM
    sstables: Vec<SSTableReader>,

    /// Next ID for creating new SSTables (monotonically increasing)
    next_sstable_id: u64,
}

impl StorageManager {
    /// Open or create storage in the given directory
    ///
    /// On startup:
    /// 1. Create directory if it doesn't exist
    /// 2. Discover existing SSTable files
    /// 3. Open readers for each (loads indexes into RAM)
    /// 4. Order by ID descending (newest first)
    pub fn open(path: &Path) -> Result<Self> {
        // Create directory if it doesn't exist
        fs::create_dir_all(path)?;

        // Discover existing SSTables
        let mut sstable_ids: Vec<u64> = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.is_file() {
                if let Some(id) = Self::parse_sstable_id(&file_path) {
                    sstable_ids.push(id);
                }
            }
        }

        // Sort newest first (highest ID first)
        sstable_ids.sort();
        sstable_ids.reverse();

        // Open readers for each SSTable
        let mut sstables = Vec::new();
        for id in &sstable_ids {
            let sstable_path = Self::sstable_path_with_dir(path, *id);
            let reader = SSTableReader::open(&sstable_path)?;
            sstables.push(reader);
        }

        // Next ID = max + 1, or 1 if no SSTables exist
        let next_id = sstable_ids.first().map(|&id| id + 1).unwrap_or(1);

        Ok(Self {
            data_dir: path.to_path_buf(),
            sstables,
            next_sstable_id: next_id,
        })
    }

    /// Get a value by key (searches all SSTables newest → oldest)
    ///
    /// Returns:
    /// - `Ok(Some(value))` — key found with value
    /// - `Ok(None)` — key not found, or found tombstone (deleted)
    pub fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Search SSTables newest → oldest
        for reader in &mut self.sstables {
            // Skip SSTable if key is outside its range (O(1) check)
            if !reader.might_contain(key) {
                continue;
            }

            // Key might be here — do the actual lookup
            match reader.get(key) {
                Ok(Some(value)) => return Ok(Some(value)), // Found!
                Ok(None) => return Ok(None),               // Tombstone = deleted
                Err(AtlasError::KeyNotFound) => continue,  // Not in this SSTable
                Err(e) => return Err(e),                   // Real error
            }
        }

        // Not found in any SSTable
        Ok(None)
    }

    /// Flush a MemTable to a new SSTable
    ///
    /// Creates a new SSTable file from the MemTable's sorted entries,
    /// opens a reader for it, and adds it to the front of the list.
    pub fn flush(&mut self, memtable: &MemTable) -> Result<SSTable> {
        // Skip if MemTable is empty
        if memtable.is_empty() {
            return Err(AtlasError::Storage(
                "Cannot flush empty MemTable".to_string(),
            ));
        }

        // Generate new SSTable ID and path
        let id = self.next_sstable_id;
        self.next_sstable_id += 1;
        let path = self.sstable_path(id);

        // Create builder and write entries (already sorted from BTreeMap)
        let mut builder = SSTableBuilder::new(&path)?;
        for (key, entry) in memtable.iter() {
            match entry {
                MemTableEntry::Value(v) => builder.add(&key, &v)?,
                MemTableEntry::Tombstone => builder.add_tombstone(&key)?,
            }
        }
        let metadata = builder.finish()?;

        // Open reader for the new SSTable
        let reader = SSTableReader::open(&path)?;

        // Insert at front (newest first)
        self.sstables.insert(0, reader);

        Ok(metadata)
    }

    /// Get the number of SSTables
    pub fn sstable_count(&self) -> usize {
        self.sstables.len()
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get the next SSTable ID (for testing/debugging)
    pub fn next_sstable_id(&self) -> u64 {
        self.next_sstable_id
    }

    // =========================================================================
    // Private Helpers
    // =========================================================================

    /// Generate the file path for an SSTable with given ID
    fn sstable_path(&self, id: u64) -> PathBuf {
        Self::sstable_path_with_dir(&self.data_dir, id)
    }

    /// Generate SSTable path given a directory and ID
    fn sstable_path_with_dir(dir: &Path, id: u64) -> PathBuf {
        dir.join(format!("sstable_{:06}.sst", id))
    }

    /// Parse SSTable ID from filename
    /// "sstable_000042.sst" → Some(42)
    fn parse_sstable_id(path: &Path) -> Option<u64> {
        let name = path.file_stem()?.to_string_lossy();
        let id_str = name.strip_prefix("sstable_")?;
        id_str.parse().ok()
    }

    /// Compact SSTables (future - merges multiple SSTables)
    #[allow(dead_code)]
    fn compact(&mut self) -> Result<()> {
        todo!("Implement compaction in V2")
    }
}
