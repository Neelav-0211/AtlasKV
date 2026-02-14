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
    /// Root directory for all data files (WAL, SSTables, etc.)
    /// Internal structure:
    ///   {data_dir}/
    ///     ├── wal.log          (write-ahead log)
    ///     └── sstables/        (SSTable files)
    pub data_dir: PathBuf,

    // -------------------------------------------------------------------------
    // WAL Configuration
    // -------------------------------------------------------------------------
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
    /// Set the data directory (root for all storage)
    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.data_dir = path.into();
        self
    }

    /// Set the WAL sync strategy
    pub fn wal_sync_strategy(mut self, strategy: WalSyncStrategy) -> Self {
        self.config.wal_sync_strategy = strategy;
        self
    }

    /// Set the memtable size limit (in bytes)
    pub fn memtable_size_limit(mut self, size: usize) -> Self {
        self.config.memtable_size_limit = size;
        self
    }

    /// Set the TCP listen address
    pub fn listen_addr(mut self, addr: impl Into<String>) -> Self {
        self.config.listen_addr = addr.into();
        self
    }

    /// Set the maximum number of concurrent connections
    pub fn max_connections(mut self, count: usize) -> Self {
        self.config.max_connections = count;
        self
    }

    /// Set the read timeout (in milliseconds)
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.config.read_timeout_ms = ms;
        self
    }

    /// Set the write timeout (in milliseconds)
    pub fn write_timeout_ms(mut self, ms: u64) -> Self {
        self.config.write_timeout_ms = ms;
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}
