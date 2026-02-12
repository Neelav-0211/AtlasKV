//! SSTable Iterator
//!
//! Sequential iteration over all entries in an SSTable.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

use crate::error::Result;
use crate::AtlasError;

use super::{HEADER_SIZE, TOMBSTONE_MARKER};

/// Iterator over SSTable entries in sorted key order
pub struct SSTableIterator<'a> {
    file: &'a mut BufReader<File>,
    /// Stop reading when we reach this offset (start of index block)
    end_offset: u64,
    /// Current position in file
    current_offset: u64,
}

impl<'a> SSTableIterator<'a> {
    /// Create a new iterator starting from the data block
    pub(super) fn new(file: &'a mut BufReader<File>, end_offset: u64) -> Result<Self> {
        // Seek to start of data (after header)
        file.seek(SeekFrom::Start(HEADER_SIZE))?;
        Ok(Self {
            file,
            end_offset,
            current_offset: HEADER_SIZE,
        })
    }
}

impl<'a> Iterator for SSTableIterator<'a> {
    /// (key, Option<value>) — None value means tombstone
    type Item = Result<(Vec<u8>, Option<Vec<u8>>)>;

    fn next(&mut self) -> Option<Self::Item> {
        // Stop at index block
        if self.current_offset >= self.end_offset {
            return None;
        }

        // Read entry header
        let mut header = [0u8; 8];
        if let Err(e) = self.file.read_exact(&mut header) {
            return Some(Err(AtlasError::Io(e)));
        }

        let key_len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
        let val_len = u32::from_le_bytes(header[4..8].try_into().unwrap());

        // Read key
        let mut key = vec![0u8; key_len];
        if let Err(e) = self.file.read_exact(&mut key) {
            return Some(Err(AtlasError::Io(e)));
        }

        // Calculate entry size and update offset
        let mut entry_size = 8 + key_len as u64;

        // Read value (if not tombstone)
        let value = if val_len == TOMBSTONE_MARKER {
            None
        } else {
            let mut v = vec![0u8; val_len as usize];
            if let Err(e) = self.file.read_exact(&mut v) {
                return Some(Err(AtlasError::Io(e)));
            }
            entry_size += val_len as u64;
            Some(v)
        };

        self.current_offset += entry_size;

        Some(Ok((key, value)))
    }
}
