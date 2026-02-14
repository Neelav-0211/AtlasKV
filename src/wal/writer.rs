//! WAL Writer
//!
//! Handles appending entries to the WAL file.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::Result;
use crate::config::WalSyncStrategy;
use super::{WalEntry, Operation};

/// Writes entries to the WAL file
pub struct WalWriter {
    /// Buffered file writer for performance (batches writes)
    file: BufWriter<File>,
    
    /// Next LSN to assign (auto-increments)
    current_lsn: u64,
    
    /// How aggressively to sync to disk
    sync_strategy: WalSyncStrategy,
    
    /// Count of entries written since last sync
    uncommitted_count: usize,
}

impl WalWriter {
    /// Open or create a WAL file for writing (truncates - use for fresh start)
    pub fn open(path: &Path, sync_strategy: WalSyncStrategy) -> Result<Self> {
        // Step 1: Open file in write mode, create if doesn't exist, truncate to start fresh
        let file = OpenOptions::new()
            .create(true)      // Create file if it doesn't exist
            .write(true)       // Open for writing
            .truncate(true)    // Clear existing content
            .open(path)?;

        // Step 2: Wrap in BufWriter for performance (batches writes in memory)
        let file = BufWriter::new(file);

        // Step 3: Start LSN from 1 (since we truncated)
        let current_lsn = 1;

        Ok(WalWriter {
            file,
            current_lsn,
            sync_strategy,
            uncommitted_count: 0,
        })
    }

    /// Open WAL in append mode (for use after recovery)
    ///
    /// IMPORTANT: Call this after recovery instead of open() to preserve
    /// the WAL until recovered data is flushed to disk.
    pub fn open_append(path: &Path, sync_strategy: WalSyncStrategy, next_lsn: u64) -> Result<Self> {
        // Step 1: Open file in append mode
        let file = OpenOptions::new()
            .create(true)      // Create file if it doesn't exist
            .append(true)      // Append mode - don't truncate!
            .open(path)?;

        // Step 2: Wrap in BufWriter
        let file = BufWriter::new(file);

        // Step 3: Use provided LSN (continue from where recovery left off)
        Ok(WalWriter {
            file,
            current_lsn: next_lsn,
            sync_strategy,
            uncommitted_count: 0,
        })
    }

    /// Append an entry to the WAL
    ///
    /// Returns the LSN assigned to this entry
    pub fn append(&mut self, operation: Operation) -> Result<u64> {
        // Step 1: Assign LSN and increment counter
        let lsn = self.current_lsn;
        self.current_lsn += 1;

        // Step 2: Create WAL entry with assigned LSN
        let wal_entry = WalEntry::new(lsn, operation);

        // Step 3: Serialize entry
        let bytes = wal_entry.serialize()?;

        // Step 4: Write to buffer
        self.file.write_all(&bytes)?;

        // Step 5: Increment uncommitted count
        self.uncommitted_count += 1;

        // Step 6: Sync based on strategy
        match self.sync_strategy {
            WalSyncStrategy::EveryWrite => {
                // Flush buffer and fsync immediately (most durable)
                self.sync()?;
            }
            WalSyncStrategy::EveryNEntries { count } => {
                // Check if we've reached the threshold
                if self.uncommitted_count >= count {
                    self.sync()?;
                }
            }
        }

        // Step 7: Return assigned LSN
        Ok(lsn)
    }

    /// Force sync to disk (fsync)
    ///
    /// Flushes buffer and ensures data is written to physical disk
    pub fn sync(&mut self) -> Result<()> {
        // Step 1: Flush buffer to OS
        self.file.flush()?;

        // Step 2: Get underlying file handle
        let file = self.file.get_ref();

        // Step 3: Force sync to disk (fsync syscall)
        file.sync_all()?;
// Step 4: Reset uncommitted counter
        self.uncommitted_count = 0;

        
        Ok(())
    }

    /// Get the current LSN (next LSN to be assigned)
    pub fn current_lsn(&self) -> u64 {
        self.current_lsn
    }

    /// Get the count of uncommitted entries since last sync
    pub fn uncommitted_count(&self) -> usize {
        self.uncommitted_count
    }

    /// Truncate WAL file (used after MemTable flush)
    ///
    /// Clears all entries and resets LSN to 1
    pub fn truncate(&mut self) -> Result<()> {
        // Step 1: Flush any pending writes
        self.file.flush()?;

        // Step 2: Get mutable reference to underlying file
        let file = self.file.get_mut();

        // Step 3: Truncate file to 0 bytes
        file.set_len(0)?;

        // Step 4: Seek to start (though file is empty)
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(0))?;

        // Step 5: Reset LSN counter and uncommitted count
        self.current_lsn = 1;
        self.uncommitted_count = 0;

        Ok(())
    }
}