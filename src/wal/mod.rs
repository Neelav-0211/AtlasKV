//! Write-Ahead Log (WAL) Module
//!
//! Provides durability guarantees through append-only logging.
//!
//! ## Responsibilities
//! - Append log entries before any mutation
//! - CRC32 checksums for corruption detection
//! - Log Sequence Numbers (LSN) for ordering
//! - Crash recovery and replay
//!
//! ## File Format
//! ```text
//! ┌─────────────────────────────────────────┐
//! │ Entry 1                                 │
//! │ ┌─────────┬─────────┬────────┬────────┐ │
//! │ │ LSN (8) │ CRC (4) │Len (4) │ Data   │ │
//! │ └─────────┴─────────┴────────┴────────┘ │
//! ├─────────────────────────────────────────┤
//! │ Entry 2                                 │
//! │ ┌─────────┬─────────┬────────┬────────┐ │
//! │ │ LSN (8) │ CRC (4) │Len (4) │ Data   │ │
//! │ └─────────┴─────────┴────────┴────────┘ │
//! └─────────────────────────────────────────┘
//! ```

mod entry;
mod writer;
mod reader;
mod recovery;

pub use entry::{WalEntry, Operation, HEADER_SIZE};
pub use writer::WalWriter;
pub use reader::WalReader;
pub use recovery::{WalRecovery, RecoveryResult};
