//! SSTable Reader
//!
//! Opens SSTable files and provides O(log n) key lookups via in-memory index.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::Result;
use crate::AtlasError;

use super::iterator::SSTableIterator;
use super::{FOOTER_SIZE, HEADER_SIZE, MAGIC, TOMBSTONE_MARKER, VERSION};

/// Reader for SSTable files with in-memory index for O(log n) lookups
pub struct SSTableReader {
    /// File handle for reading entries
    pub(super) file: BufReader<File>,
    /// In-memory index: key → file offset
    index: BTreeMap<Vec<u8>, u64>,
    /// Metadata
    entry_count: u64,
    /// Index block starting offset (for iteration)
    pub(super) index_offset: u64,
}

impl SSTableReader {
    /// Open an SSTable for reading
    ///
    /// Loads the entire index into memory for fast lookups.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();

        // Read and validate header
        let mut header = [0u8; HEADER_SIZE as usize];
        file.read_exact(&mut header)?;

        if &header[0..4] != MAGIC {
            return Err(AtlasError::Storage(format!(
                "Invalid SSTable magic: expected ATKV, got {:?}",
                &header[0..4]
            )));
        }

        let version = u16::from_le_bytes(header[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(AtlasError::Storage(format!(
                "Unsupported SSTable version: {}",
                version
            )));
        }

        let entry_count = u64::from_le_bytes(header[6..14].try_into().unwrap());

        // Read footer to get index offset
        file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
        let mut footer = [0u8; FOOTER_SIZE as usize];
        file.read_exact(&mut footer)?;

        let index_offset = u64::from_le_bytes(footer[0..8].try_into().unwrap());
        let _data_crc = u32::from_le_bytes(footer[8..12].try_into().unwrap());
        // Note: CRC validation could be done here for extra safety

        // Load index into memory
        let mut index = BTreeMap::new();
        file.seek(SeekFrom::Start(index_offset))?;

        // Index block size = file_size - footer_size - index_offset
        let index_block_size = file_size - FOOTER_SIZE - index_offset;
        let mut index_data = vec![0u8; index_block_size as usize];
        file.read_exact(&mut index_data)?;

        // Parse index entries: [key_len(4)][offset(8)][key]
        let mut pos = 0;
        while pos < index_data.len() {
            if pos + 4 > index_data.len() {
                break;
            }
            let key_len =
                u32::from_le_bytes(index_data[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;

            if pos + 8 > index_data.len() {
                break;
            }
            let offset = u64::from_le_bytes(index_data[pos..pos + 8].try_into().unwrap());
            pos += 8;

            if pos + key_len > index_data.len() {
                break;
            }
            let key = index_data[pos..pos + key_len].to_vec();
            pos += key_len;

            index.insert(key, offset);
        }

        // Reset file to start for reading
        file.seek(SeekFrom::Start(0))?;

        Ok(Self {
            file: BufReader::new(file),
            index,
            entry_count,
            index_offset,
        })
    }

    /// Get a value by key — O(log n) lookup via in-memory index
    ///
    /// Returns:
    /// - `Ok(Some(value))` — key found with value
    /// - `Ok(None)` — key found but is a tombstone (deleted)
    /// - `Err(KeyNotFound)` — key not in this SSTable
    pub fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // O(log n) lookup in BTreeMap
        let offset = match self.index.get(key) {
            Some(&off) => off,
            None => return Err(AtlasError::KeyNotFound),
        };

        // Seek directly to the entry
        self.file.seek(SeekFrom::Start(offset))?;

        // Read entry header
        let mut header = [0u8; 8];
        self.file.read_exact(&mut header)?;

        let key_len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
        let val_len = u32::from_le_bytes(header[4..8].try_into().unwrap());

        // Skip the key (we already know it matches)
        self.file.seek(SeekFrom::Current(key_len as i64))?;

        // Check for tombstone
        if val_len == TOMBSTONE_MARKER {
            return Ok(None);
        }

        // Read value
        let mut value = vec![0u8; val_len as usize];
        self.file.read_exact(&mut value)?;

        Ok(Some(value))
    }

    /// Get entry count
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Get the minimum key in this SSTable (for range filtering)
    pub fn min_key(&self) -> Option<&[u8]> {
        self.index.keys().next().map(|k| k.as_slice())
    }

    /// Get the maximum key in this SSTable (for range filtering)
    pub fn max_key(&self) -> Option<&[u8]> {
        self.index.keys().next_back().map(|k| k.as_slice())
    }

    /// Quick check if a key might be in this SSTable (range check)
    /// Returns false only if the key is definitely outside [min_key, max_key]
    pub fn might_contain(&self, key: &[u8]) -> bool {
        match (self.min_key(), self.max_key()) {
            (Some(min), Some(max)) => key >= min && key <= max,
            _ => false, // Empty SSTable
        }
    }

    /// Create an iterator over all entries (for compaction, debugging)
    pub fn iter(&mut self) -> Result<SSTableIterator<'_>> {
        SSTableIterator::new(&mut self.file, self.index_offset)
    }
}
