//! Storage Module
//!
//! Persistent storage layer using SSTable-like format.
//!
//! ## Responsibilities
//! - Persist data to disk in sorted format
//! - Efficient range scans and point lookups
//! - Background compaction (future)
//! - Bloom filters for negative lookups (future)
//!
//! ## File Format (V1 - Simple)
//! ```text
//! ┌────────────────────────────────────────┐
//! │ Header                                 │
//! │ ┌──────────┬──────────┬──────────────┐ │
//! │ │Magic (4) │Version(2)│ Entry Count  │ │
//! │ └──────────┴──────────┴──────────────┘ │
//! ├────────────────────────────────────────┤
//! │ Data Block                             │
//! │ ┌────────┬────────┬─────┬───────────┐ │
//! │ │KeyLen  │ValLen  │ Key │   Value   │ │
//! │ └────────┴────────┴─────┴───────────┘ │
//! │ ... (repeated for each entry)         │
//! ├────────────────────────────────────────┤
//! │ Footer                                 │
//! │ ┌──────────────────┬─────────────────┐ │
//! │ │ Index Offset     │    CRC32        │ │
//! │ └──────────────────┴─────────────────┘ │
//! └────────────────────────────────────────┘
//! ```

mod sstable;
mod manager;

pub use sstable::{SSTable, SSTableBuilder, SSTableReader};
pub use manager::StorageManager;
