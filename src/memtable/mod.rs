//! MemTable Module
//!
//! In-memory data structure for recent writes.
//!
//! ## Responsibilities
//! - Fast reads and writes in memory
//! - Single-writer/multi-reader access pattern
//! - Track size for flush triggers
//! - Ordered iteration for SSTable creation
//!
//! ## Data Structure Choice
//! Using BTreeMap wrapped in RwLock for V1:
//! - Ordered keys (required for SSTable generation)
//! - Simple and correct first, optimize later
//! - Future: Consider SkipList for better concurrent performance

mod table;

pub use table::MemTable;

/// Entry stored in the MemTable
#[derive(Debug, Clone, PartialEq)]
pub enum MemTableEntry {
    /// A live value
    Value(Vec<u8>),

    /// A tombstone (deleted key)
    Tombstone,
}
