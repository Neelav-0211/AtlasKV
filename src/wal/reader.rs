//! WAL Reader
//!
//! Handles reading entries from the WAL file sequentially.
//!
//! Used during recovery to replay entries from the WAL back into the MemTable.

use std::{fs::File, io::Read, path::Path};

use crate::{error::Result, wal::HEADER_SIZE};
use super::WalEntry;

/// Reads entries from the WAL file sequentially
pub struct WalReader {
    file: File,
    position: u64,
    file_size: u64,
}

impl WalReader {
    /// Open a WAL file for reading
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();
        
        Ok(Self {
            file,
            position: 0,
            file_size,
        })
    }

    /// Read the next entry from the WAL
    ///
    /// Returns:
    /// - `Ok(Some(entry))` - Successfully read an entry
    /// - `Ok(None)` - Reached EOF or incomplete entry (safe for recovery)
    /// - `Err(...)` - I/O error or corruption detected
    pub fn next_entry(&mut self) -> Result<Option<WalEntry>> {
        // Step 1: Check EOF
        if self.position >= self.file_size {
            return Ok(None);
        }

        // Step 2: Ensure we can read full header
        if self.position + HEADER_SIZE as u64 > self.file_size {
            return Ok(None); // Partial write at EOF
        }

        // Step 3: Read header (16 bytes)
        let mut header = [0u8; HEADER_SIZE];
        self.file.read_exact(&mut header)?;

        // Step 4: Parse data length from header
        let data_len = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;

        // Step 5: Validate complete entry exists
        if self.position + HEADER_SIZE as u64 + data_len as u64 > self.file_size {
            return Ok(None); // Partial write at EOF
        }

        // Step 6: Read data section
        let mut data = vec![0u8; data_len];
        self.file.read_exact(&mut data)?;

        // Step 7: Build full buffer and deserialize (validates CRC)
        let mut full_buffer = Vec::with_capacity(HEADER_SIZE + data_len);
        full_buffer.extend_from_slice(&header);
        full_buffer.extend_from_slice(&data);

        let entry = WalEntry::deserialize(&full_buffer)?;

        // Step 8: Advance position
        self.position += (HEADER_SIZE + data_len) as u64;

        // Step 9: Return entry
        Ok(Some(entry))
    }

    /// Consume reader and return an iterator over all valid entries
    pub fn entries(self) -> WalIterator {
        WalIterator { reader: self }
    }
}

/// Iterator over WAL entries
pub struct WalIterator {
    reader: WalReader,
}

impl Iterator for WalIterator {
    type Item = Result<WalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next_entry() {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}