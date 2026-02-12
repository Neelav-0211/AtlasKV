//! SSTable Module
//!
//! Sorted String Table - immutable on-disk sorted key-value storage.
//!
//! ## File Format
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ Header (14 bytes)                                       │
//! │   Magic: "ATKV" (4) | Version: u16 (2) | Count: u64 (8) │
//! ├─────────────────────────────────────────────────────────┤
//! │ Data Block (variable)                                   │
//! │   [KeyLen: u32][ValLen: u32][Key][Value]                │
//! │   ... repeated for each entry ...                       │
//! │   (ValLen = u32::MAX means tombstone, no value bytes)   │
//! ├─────────────────────────────────────────────────────────┤
//! │ Index Block (variable)                                  │
//! │   [KeyLen: u32][Offset: u64][Key]                       │
//! │   ... repeated for each entry ...                       │
//! ├─────────────────────────────────────────────────────────┤
//! │ Footer (16 bytes)                                       │
//! │   IndexOffset: u64 (8) | DataCRC: u32 (4) | Padding (4) │
//! └─────────────────────────────────────────────────────────┘
//! ```

mod builder;
mod iterator;
mod reader;

use std::path::PathBuf;

pub use builder::SSTableBuilder;
pub use iterator::SSTableIterator;
pub use reader::SSTableReader;

// =============================================================================
// Shared Constants (used by builder, reader, iterator)
// =============================================================================

/// Magic bytes identifying an AtlasKV SSTable file
pub(crate) const MAGIC: &[u8; 4] = b"ATKV";

/// Current SSTable format version
pub(crate) const VERSION: u16 = 1;

/// Header size: Magic (4) + Version (2) + EntryCount (8) = 14 bytes
pub(crate) const HEADER_SIZE: u64 = 14;

/// Footer size: IndexOffset (8) + DataCRC (4) + Padding (4) = 16 bytes
pub(crate) const FOOTER_SIZE: u64 = 16;

/// Sentinel value indicating a tombstone (deleted key)
pub(crate) const TOMBSTONE_MARKER: u32 = u32::MAX;

// =============================================================================
// SSTable Metadata
// =============================================================================

/// SSTable metadata — lightweight handle for closed SSTables.
///
/// NOTE: This struct is not currently used in the codebase. The StorageManager
/// keeps SSTableReader instances open (with their in-memory BTreeMap index)
/// for O(log n) lookups. This metadata struct is retained for potential future
/// use cases such as:
/// - Lazy loading of SSTable readers (trade memory for I/O)
/// - SSTable compaction metadata tracking
/// - Level-based tiering information
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SSTable {
    /// Path to the SSTable file
    pub path: PathBuf,
    /// Number of entries in this SSTable
    pub entry_count: u64,
    /// Smallest key (for range filtering)
    pub min_key: Vec<u8>,
    /// Largest key (for range filtering)
    pub max_key: Vec<u8>,
    /// File size in bytes
    pub file_size: u64,
}

impl SSTable {
    /// Get the number of entries
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Quick check if a key might be in this SSTable (range check)
    /// Returns false if key is definitely outside [min_key, max_key]
    pub fn might_contain(&self, key: &[u8]) -> bool {
        key >= self.min_key.as_slice() && key <= self.max_key.as_slice()
    }
}
