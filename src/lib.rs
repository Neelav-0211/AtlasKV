//! # AtlasKV
//!
//! A high-performance, distributed key-value store with:
//! - Write-Ahead Logging (WAL) for durability
//! - Crash recovery with partial write handling
//! - Single-writer/multi-reader concurrency model
//! - TCP-based client protocol
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      TCP Server                              │
//! │                  (Multiple Clients)                          │
//! └─────────────────────┬───────────────────────────────────────┘
//!                       │
//! ┌─────────────────────▼───────────────────────────────────────┐
//! │                   Command Router                             │
//! │            (Single Writer / Multi Reader)                    │
//! └─────────────────────┬───────────────────────────────────────┘
//!                       │
//!          ┌────────────┴────────────┐
//!          │                         │
//!          ▼                         ▼
//!   ┌─────────────┐          ┌─────────────┐
//!   │     WAL     │          │  MemTable   │
//!   │  (Append)   │          │  (RwLock)   │
//!   └─────────────┘          └──────┬──────┘
//!                                   │
//!                                   ▼
//!                           ┌─────────────┐
//!                           │   Storage   │
//!                           │  (SSTable)  │
//!                           └─────────────┘
//! ```

// =============================================================================
// Module Declarations
// =============================================================================

pub mod error;
pub mod config;

pub mod wal;
pub mod memtable;
pub mod storage;
pub mod network;
pub mod protocol;
pub mod engine;

// =============================================================================
// Public API Re-exports
// =============================================================================

pub use error::{AtlasError, Result};
pub use config::Config;
pub use engine::Engine;

// =============================================================================
// Version Info
// =============================================================================

/// Current version of AtlasKV
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
