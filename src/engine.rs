//! Engine Module
//!
//! The core storage engine that coordinates all components.
//!
//! ## Responsibilities
//! - Coordinate WAL, MemTable, and Storage
//! - Handle concurrent read/write access
//! - Trigger flushes when MemTable is full
//! - Manage crash recovery on startup

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config::Config;
use crate::error::Result;
use crate::memtable::{MemTable, MemTableEntry};
use crate::protocol::Command;
use crate::storage::StorageManager;
use crate::wal::{Operation, WalRecovery, WalWriter};

/// The main storage engine
///
/// ## Concurrency Model: Single-Writer / Multiple-Reader (SWMR)
///
/// - **Writes** (put/delete/flush): Serialized by `write_lock`
///   - Only ONE write operation at a time
///   - Must acquire: write_lock → WAL → memtable → storage (write)
///
/// - **Reads** (get): Concurrent at MemTable level only
///   - No write_lock needed
///   - MemTable uses internal RwLock (many concurrent readers)
///   - StorageManager currently uses write lock for SSTable reads
///     (because SSTableReader::get needs &mut self for file seeking)
///
/// ## Future Optimization:
/// - Make SSTableReader use interior mutability (Mutex<BufReader>)
/// - Then StorageManager::get() can use read lock for true concurrent reads
/// - See future_optimizations.md for details
pub struct Engine {
    /// Engine configuration
    config: Config,

    /// Directory for all data files (SSTables)
    storage_dir: PathBuf,

    /// Write-ahead log for durability (exclusive access needed)
    wal: Mutex<WalWriter>,

    /// In-memory table for recent writes (internal RwLock)
    memtable: MemTable,

    /// Persistent storage manager (internal RwLock on sstables vec)
    storage: StorageManager,

    /// Serializes write operations (put/delete/flush)
    write_lock: Mutex<()>,
}

impl Engine {
    // =========================================================================
    // Internal Path Constants
    // =========================================================================
    const WAL_FILENAME: &'static str = "wal.log";
    const SSTABLE_DIR: &'static str = "sstables";

    /// Open or create an engine with the given config
    ///
    /// On startup:
    /// 1. Open/create data directory
    /// 2. Recover from WAL if exists
    /// 3. Load existing SSTables
    /// 4. Ready to serve requests
    pub fn open(config: Config) -> Result<Self> {
        // Step 1: Create data directory if it doesn't exist
        fs::create_dir_all(&config.data_dir)?;

        // Step 2: Compute paths (derived from data_dir, not configurable)
        let storage_dir = config.data_dir.join(Self::SSTABLE_DIR);
        let wal_path = config.data_dir.join(Self::WAL_FILENAME);

        // Step 3: Create storage directory
        fs::create_dir_all(&storage_dir)?;

        // Step 4: Open storage manager (loads existing SSTables)
        let storage = StorageManager::open(&storage_dir)?;

        // Step 5: Create memtable
        let memtable = MemTable::new();

        // Step 6: Recover from WAL if it exists and flush to make data durable
        let wal = if wal_path.exists() {
            let (entries, recovery_result) = WalRecovery::recover(&wal_path)?;

            // Log recovery stats (in production, use proper logging)
            if recovery_result.entries_recovered > 0 || recovery_result.entries_corrupted > 0 {
                eprintln!(
                    "[Engine] WAL recovery: {} entries recovered, {} corrupted, last_lsn={}",
                    recovery_result.entries_recovered,
                    recovery_result.entries_corrupted,
                    recovery_result.last_lsn
                );
            }

            // Replay entries to memtable
            for entry in entries {
                match entry.operation {
                    Operation::Put { key, value } => {
                        memtable.put(key, value);
                    }
                    Operation::Delete { key } => {
                        memtable.delete(key);
                    }
                }
            }

            // CRITICAL: Flush recovered data to SSTable immediately to make it durable
            // If we crash after this point, data is safe in SSTables
            if !memtable.is_empty() {
                eprintln!("[Engine] Flushing {} recovered entries to SSTable", memtable.entry_count());
                storage.flush(&memtable)?;
                memtable.clear();
            }

            // Now safe to truncate WAL - recovered data is durable in SSTables
            WalWriter::open(&wal_path, config.wal_sync_strategy)?
        } else {
            // No WAL to recover - start fresh
            WalWriter::open(&wal_path, config.wal_sync_strategy)?
        };

        Ok(Self {
            config,
            storage_dir,
            wal: Mutex::new(wal),
            memtable,
            storage,
            write_lock: Mutex::new(()),
        })
    }

    /// Open with a path (convenience method)
    ///
    /// Uses default config with the specified data directory
    pub fn open_path(path: &Path) -> Result<Self> {
        let mut config = Config::default();
        config.data_dir = path.to_path_buf();
        Self::open(config)
    }

    /// Execute a command
    ///
    /// Routes commands to appropriate handlers
    pub fn execute(&self, command: Command) -> Result<Option<Vec<u8>>> {
        match command {
            Command::Get { key } => self.get(&key),
            Command::Put { key, value } => {
                self.put(&key, &value)?;
                Ok(None)
            }
            Command::Delete { key } => {
                self.delete(&key)?;
                Ok(None)
            }
            Command::Ping => Ok(Some(b"PONG".to_vec())),
        }
    }

    /// Get a value by key
    ///
    /// Search order:
    /// 1. MemTable (most recent writes)
    /// 2. SSTables (newest to oldest)
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Step 1: Check MemTable first (most recent data)
        if let Some(entry) = self.memtable.get(key) {
            return match entry {
                MemTableEntry::Value(value) => Ok(Some(value)),
                MemTableEntry::Tombstone => Ok(None), // Key was deleted
            };
        }

        // Step 2: Check SSTables (newest to oldest) - StorageManager internally locks
        self.storage.get(key)
    }

    /// Put a key-value pair
    ///
    /// Steps:
    /// 1. Acquire write lock
    /// 2. Write to WAL (durability)
    /// 3. Write to MemTable
    /// 4. Check if flush needed
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        // Acquire write lock to serialize writes
        let _write_guard = self.write_lock.lock().map_err(|e| {
            crate::AtlasError::LockPoisoned(format!("Write lock poisoned: {}", e))
        })?;

        // Step 1: Write to WAL first (durability guarantee)
        {
            let mut wal = self.wal.lock().map_err(|e| {
                crate::AtlasError::LockPoisoned(format!("WAL lock poisoned: {}", e))
            })?;

            wal.append(Operation::Put {
                key: key.to_vec(),
                value: value.to_vec(),
            })?;
        }

        // Step 2: Write to MemTable
        let new_size = self.memtable.put(key.to_vec(), value.to_vec());

        // Step 3: Check if flush is needed
        if new_size >= self.config.memtable_size_limit {
            self.flush_internal()?;
        }

        Ok(())
    }

    /// Delete a key
    ///
    /// Steps:
    /// 1. Acquire write lock
    /// 2. Write tombstone to WAL
    /// 3. Write tombstone to MemTable
    /// 4. Check if flush needed
    pub fn delete(&self, key: &[u8]) -> Result<()> {
        // Acquire write lock to serialize writes
        let _write_guard = self.write_lock.lock().map_err(|e| {
            crate::AtlasError::LockPoisoned(format!("Write lock poisoned: {}", e))
        })?;

        // Step 1: Write delete operation to WAL
        {
            let mut wal = self.wal.lock().map_err(|e| {
                crate::AtlasError::LockPoisoned(format!("WAL lock poisoned: {}", e))
            })?;

            wal.append(Operation::Delete {
                key: key.to_vec(),
            })?;
        }

        // Step 2: Write tombstone to MemTable
        let new_size = self.memtable.delete(key.to_vec());

        // Step 3: Check if flush is needed
        if new_size >= self.config.memtable_size_limit {
            self.flush_internal()?;
        }

        Ok(())
    }

    /// Flush memtable to disk (public API)
    ///
    /// Forces a flush regardless of memtable size
    pub fn flush(&self) -> Result<()> {
        let _write_guard = self.write_lock.lock().map_err(|e| {
            crate::AtlasError::LockPoisoned(format!("Write lock poisoned: {}", e))
        })?;

        self.flush_internal()
    }

    /// Internal flush implementation (called with write lock held)
    fn flush_internal(&self) -> Result<()> {
        // Skip if memtable is empty
        if self.memtable.is_empty() {
            return Ok(());
        }

        // Step 1: Flush memtable to SSTable (StorageManager internally locks)
        self.storage.flush(&self.memtable)?;

        // Step 2: Clear memtable
        self.memtable.clear();

        // Step 3: Truncate WAL (entries are now durable in SSTable)
        {
            let mut wal = self.wal.lock().map_err(|e| {
                crate::AtlasError::LockPoisoned(format!("WAL lock poisoned: {}", e))
            })?;

            wal.truncate()?;
        }

        Ok(())
    }

    /// Close the engine gracefully
    ///
    /// Flushes any pending data and syncs to disk
    pub fn close(self) -> Result<()> {
        // Flush any remaining data in memtable
        if !self.memtable.is_empty() {
            self.flush()?;
        }

        // Sync WAL to ensure all data is on disk
        {
            let mut wal = self.wal.lock().map_err(|e| {
                crate::AtlasError::LockPoisoned(format!("WAL lock poisoned: {}", e))
            })?;

            wal.sync()?;
        }

        Ok(())
    }

    // =========================================================================
    // Accessors (for testing and debugging)
    // =========================================================================

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }

    /// Get the storage directory path (where SSTables are stored)
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Get the current memtable size
    pub fn memtable_size(&self) -> usize {
        self.memtable.size()
    }

    /// Get the memtable entry count
    pub fn memtable_entry_count(&self) -> usize {
        self.memtable.entry_count()
    }

    /// Get the number of SSTables
    pub fn sstable_count(&self) -> usize {
        self.storage.sstable_count()
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}
