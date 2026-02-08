//! Configuration for AtlasKV
//!
//! Centralized configuration with sensible defaults.

use std::path::PathBuf;

/// Main configuration for AtlasKV instance
#[derive(Debug, Clone)]
pub struct Config {
    // -------------------------------------------------------------------------
    // Storage Configuration
    // -------------------------------------------------------------------------
    /// Directory for all data files
    pub data_dir: PathBuf,

    // -------------------------------------------------------------------------
    // WAL Configuration
    // -------------------------------------------------------------------------
    /// WAL file path (relative to data_dir)
    pub wal_path: PathBuf,

    /// Sync strategy: how often to fsync WAL
    pub wal_sync_strategy: WalSyncStrategy,

    // -------------------------------------------------------------------------
    // MemTable Configuration
    // -------------------------------------------------------------------------
    /// Max size of memtable before flush (in bytes)
    pub memtable_size_limit: usize,

    // -------------------------------------------------------------------------
    // Network Configuration
    // -------------------------------------------------------------------------
    /// TCP listen address
    pub listen_addr: String,

    /// Max concurrent client connections
    pub max_connections: usize,

    /// Connection read timeout (milliseconds)
    pub read_timeout_ms: u64,

    /// Connection write timeout (milliseconds)
    pub write_timeout_ms: u64,
}

/// WAL sync strategy
#[derive(Debug, Clone, Copy)]
pub enum WalSyncStrategy {
    /// fsync after every write (safest, slowest)
    EveryWrite,

    /// fsync after N uncommitted entries (balanced durability/performance)
    EveryNEntries { count: usize },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./atlaskv_data"),
            wal_path: PathBuf::from("wal.log"),
            wal_sync_strategy: WalSyncStrategy::EveryNEntries { count: 100 },
            memtable_size_limit: 64 * 1024 * 1024, // 64 MB
            listen_addr: "127.0.0.1:6379".to_string(),
            max_connections: 1024,
            read_timeout_ms: 5000,
            write_timeout_ms: 5000,
        }
    }
}

impl Config {
    /// Create a new config builder
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// Builder for Config
#[derive(Default)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    // TODO: Implement builder methods
    
    pub fn build(self) -> Config {
        self.config
    }
}
