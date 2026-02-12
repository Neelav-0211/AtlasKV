//! SSTable Builder
//!
//! Writes sorted key-value entries to a new SSTable file.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::Result;
use crate::AtlasError;

use super::{SSTable, HEADER_SIZE, MAGIC, TOMBSTONE_MARKER, VERSION};

/// Builder for creating new SSTables from sorted entries
pub struct SSTableBuilder {
    /// Output file path
    path: std::path::PathBuf,
    /// Buffered writer for performance
    writer: BufWriter<File>,
    /// Number of entries written
    entry_count: u64,
    /// Current write position (for index)
    current_offset: u64,
    /// Index: key → file offset of entry
    index: Vec<(Vec<u8>, u64)>,
    /// Track min/max keys for metadata
    min_key: Option<Vec<u8>>,
    max_key: Option<Vec<u8>>,
    /// Running CRC hasher for data section
    data_hasher: crc32fast::Hasher,
}

impl SSTableBuilder {
    /// Create a new SSTable builder
    ///
    /// Writes header immediately; call `add()`/`add_tombstone()` in sorted order,
    /// then `finish()` to write index and footer.
    pub fn new(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        let mut writer = BufWriter::new(file);

        // Write header (entry_count placeholder, will be updated in finish)
        writer.write_all(MAGIC)?;
        writer.write_all(&VERSION.to_le_bytes())?;
        writer.write_all(&0u64.to_le_bytes())?; // Placeholder for entry count

        Ok(Self {
            path: path.to_path_buf(),
            writer,
            entry_count: 0,
            current_offset: HEADER_SIZE,
            index: Vec::new(),
            min_key: None,
            max_key: None,
            data_hasher: crc32fast::Hasher::new(),
        })
    }

    /// Add a key-value pair (must be called in sorted key order)
    pub fn add(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.write_entry(key, Some(value))
    }

    /// Add a tombstone (must be called in sorted key order)
    pub fn add_tombstone(&mut self, key: &[u8]) -> Result<()> {
        self.write_entry(key, None)
    }

    /// Internal: write an entry (value=None means tombstone)
    fn write_entry(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<()> {
        // Record offset for index
        self.index.push((key.to_vec(), self.current_offset));

        // Track min/max keys
        if self.min_key.is_none() {
            self.min_key = Some(key.to_vec());
        }
        self.max_key = Some(key.to_vec());

        // Prepare entry bytes: [key_len(4)][val_len(4)][key][value]
        let key_len = key.len() as u32;
        let val_len = match value {
            Some(v) => v.len() as u32,
            None => TOMBSTONE_MARKER,
        };

        // Write and accumulate CRC
        let key_len_bytes = key_len.to_le_bytes();
        let val_len_bytes = val_len.to_le_bytes();

        self.writer.write_all(&key_len_bytes)?;
        self.writer.write_all(&val_len_bytes)?;
        self.writer.write_all(key)?;

        self.data_hasher.update(&key_len_bytes);
        self.data_hasher.update(&val_len_bytes);
        self.data_hasher.update(key);

        // Entry size so far: 4 + 4 + key_len
        let mut entry_size: u64 = 8 + key.len() as u64;

        if let Some(v) = value {
            self.writer.write_all(v)?;
            self.data_hasher.update(v);
            entry_size += v.len() as u64;
        }

        self.current_offset += entry_size;
        self.entry_count += 1;

        Ok(())
    }

    /// Finish building: write index block, footer, and return metadata
    pub fn finish(mut self) -> Result<SSTable> {
        // Record where index block starts
        let index_offset = self.current_offset;

        // Write index block: [key_len(4)][offset(8)][key] for each entry
        for (key, offset) in &self.index {
            let key_len = key.len() as u32;
            self.writer.write_all(&key_len.to_le_bytes())?;
            self.writer.write_all(&offset.to_le_bytes())?;
            self.writer.write_all(key)?;
        }

        // Finalize CRC
        let data_crc = self.data_hasher.finalize();

        // Write footer: index_offset (8) + data_crc (4) + padding (4)
        self.writer.write_all(&index_offset.to_le_bytes())?;
        self.writer.write_all(&data_crc.to_le_bytes())?;
        self.writer.write_all(&[0u8; 4])?; // Padding for alignment

        // Flush everything
        self.writer.flush()?;

        // Seek back and update entry count in header
        let mut file = self.writer.into_inner().map_err(|e| {
            AtlasError::Storage(format!("Failed to flush SSTable: {}", e))
        })?;
        file.seek(SeekFrom::Start(6))?; // After magic + version
        file.write_all(&self.entry_count.to_le_bytes())?;
        file.sync_all()?;

        let file_size = file.metadata()?.len();

        Ok(SSTable {
            path: self.path,
            entry_count: self.entry_count,
            min_key: self.min_key.unwrap_or_default(),
            max_key: self.max_key.unwrap_or_default(),
            file_size,
        })
    }
}
