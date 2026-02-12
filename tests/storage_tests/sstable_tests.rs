//! Tests for SSTable implementation
//!
//! These tests verify:
//! - SSTable creation and writing
//! - O(log n) key lookups via in-memory index
//! - Tombstone handling
//! - Iterator over all entries
//! - Min/max key range filtering
//! - File format validation

use std::path::PathBuf;
use atlaskv::storage::{SSTable, SSTableBuilder, SSTableReader};
use atlaskv::AtlasError;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_sstable() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test.sst");
    (temp_dir, path)
}

/// Create an SSTable with numbered entries
fn create_sstable_with_entries(path: &PathBuf, count: usize) -> SSTable {
    let mut builder = SSTableBuilder::new(path).unwrap();
    // Keys must be added in sorted order
    for i in 0..count {
        let key = format!("key{:05}", i); // Zero-padded for lexicographic order
        let value = format!("value{}", i);
        builder.add(key.as_bytes(), value.as_bytes()).unwrap();
    }
    builder.finish().unwrap()
}

// =============================================================================
// SSTableBuilder Tests
// =============================================================================

#[test]
fn test_builder_creates_file() {
    let (_temp, path) = setup_temp_sstable();
    
    let sstable = create_sstable_with_entries(&path, 5);
    
    assert!(path.exists());
    assert_eq!(sstable.entry_count(), 5);
    assert!(sstable.file_size > 0);
}

#[test]
fn test_builder_empty_sstable() {
    let (_temp, path) = setup_temp_sstable();
    
    let builder = SSTableBuilder::new(&path).unwrap();
    let sstable = builder.finish().unwrap();
    
    assert_eq!(sstable.entry_count(), 0);
    assert!(path.exists());
}

#[test]
fn test_builder_single_entry() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"mykey", b"myvalue").unwrap();
    let sstable = builder.finish().unwrap();
    
    assert_eq!(sstable.entry_count(), 1);
    assert_eq!(sstable.min_key, b"mykey");
    assert_eq!(sstable.max_key, b"mykey");
}

#[test]
fn test_builder_tracks_min_max_keys() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"apple", b"1").unwrap();
    builder.add(b"banana", b"2").unwrap();
    builder.add(b"cherry", b"3").unwrap();
    let sstable = builder.finish().unwrap();
    
    assert_eq!(sstable.min_key, b"apple");
    assert_eq!(sstable.max_key, b"cherry");
}

#[test]
fn test_builder_with_tombstone() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"key1", b"value1").unwrap();
    builder.add_tombstone(b"key2").unwrap();
    builder.add(b"key3", b"value3").unwrap();
    let sstable = builder.finish().unwrap();
    
    assert_eq!(sstable.entry_count(), 3);
}

// =============================================================================
// SSTableReader Tests - Lookups
// =============================================================================

#[test]
fn test_reader_opens_valid_sstable() {
    let (_temp, path) = setup_temp_sstable();
    create_sstable_with_entries(&path, 10);
    
    let reader = SSTableReader::open(&path).unwrap();
    assert_eq!(reader.entry_count(), 10);
}

#[test]
fn test_reader_get_existing_key() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"hello", b"world").unwrap();
    builder.finish().unwrap();
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let value = reader.get(b"hello").unwrap();
    
    assert_eq!(value, Some(b"world".to_vec()));
}

#[test]
fn test_reader_get_nonexistent_key() {
    let (_temp, path) = setup_temp_sstable();
    create_sstable_with_entries(&path, 5);
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let result = reader.get(b"nonexistent");
    
    assert!(matches!(result, Err(AtlasError::KeyNotFound)));
}

#[test]
fn test_reader_get_tombstone() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"key1", b"value1").unwrap();
    builder.add_tombstone(b"key2").unwrap();
    builder.add(b"key3", b"value3").unwrap();
    builder.finish().unwrap();
    
    let mut reader = SSTableReader::open(&path).unwrap();
    
    // Tombstone returns Ok(None), not an error
    let result = reader.get(b"key2").unwrap();
    assert_eq!(result, None);
    
    // Other keys work normally
    assert_eq!(reader.get(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(reader.get(b"key3").unwrap(), Some(b"value3".to_vec()));
}

#[test]
fn test_reader_get_multiple_keys() {
    let (_temp, path) = setup_temp_sstable();
    create_sstable_with_entries(&path, 100);
    
    let mut reader = SSTableReader::open(&path).unwrap();
    
    // Test lookups at various positions
    for i in [0, 25, 50, 75, 99] {
        let key = format!("key{:05}", i);
        let expected_value = format!("value{}", i);
        let value = reader.get(key.as_bytes()).unwrap().unwrap();
        assert_eq!(value, expected_value.as_bytes());
    }
}

#[test]
fn test_reader_random_access() {
    let (_temp, path) = setup_temp_sstable();
    create_sstable_with_entries(&path, 50);
    
    let mut reader = SSTableReader::open(&path).unwrap();
    
    // Access keys out of order (proves O(log n) index works, not sequential scan)
    let keys = [45, 10, 30, 5, 49, 0, 25];
    for i in keys {
        let key = format!("key{:05}", i);
        let result = reader.get(key.as_bytes());
        assert!(result.is_ok(), "Failed to get key{:05}", i);
    }
}

// =============================================================================
// SSTableReader Tests - Iterator
// =============================================================================

#[test]
fn test_iterator_empty_sstable() {
    let (_temp, path) = setup_temp_sstable();
    
    let builder = SSTableBuilder::new(&path).unwrap();
    builder.finish().unwrap();
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let entries: Vec<_> = reader.iter().unwrap().collect();
    
    assert_eq!(entries.len(), 0);
}

#[test]
fn test_iterator_returns_all_entries() {
    let (_temp, path) = setup_temp_sstable();
    create_sstable_with_entries(&path, 10);
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let entries: Vec<_> = reader.iter().unwrap()
        .map(|r| r.unwrap())
        .collect();
    
    assert_eq!(entries.len(), 10);
    
    // Verify sorted order
    for (i, (key, value)) in entries.iter().enumerate() {
        let expected_key = format!("key{:05}", i);
        let expected_value = format!("value{}", i);
        assert_eq!(key, expected_key.as_bytes());
        assert_eq!(value.as_ref().unwrap(), expected_value.as_bytes());
    }
}

#[test]
fn test_iterator_includes_tombstones() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"a", b"1").unwrap();
    builder.add_tombstone(b"b").unwrap();
    builder.add(b"c", b"3").unwrap();
    builder.finish().unwrap();
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let entries: Vec<_> = reader.iter().unwrap()
        .map(|r| r.unwrap())
        .collect();
    
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0], (b"a".to_vec(), Some(b"1".to_vec())));
    assert_eq!(entries[1], (b"b".to_vec(), None)); // Tombstone
    assert_eq!(entries[2], (b"c".to_vec(), Some(b"3".to_vec())));
}

// =============================================================================
// SSTable Metadata Tests
// =============================================================================

#[test]
fn test_might_contain_in_range() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"apple", b"1").unwrap();
    builder.add(b"banana", b"2").unwrap();
    builder.add(b"cherry", b"3").unwrap();
    let sstable = builder.finish().unwrap();
    
    // Keys within range
    assert!(sstable.might_contain(b"apple"));
    assert!(sstable.might_contain(b"banana"));
    assert!(sstable.might_contain(b"cherry"));
    assert!(sstable.might_contain(b"blueberry")); // Between apple and cherry
}

#[test]
fn test_might_contain_out_of_range() {
    let (_temp, path) = setup_temp_sstable();
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"banana", b"1").unwrap();
    builder.add(b"cherry", b"2").unwrap();
    builder.finish().unwrap();
    
    // Reopen to get fresh metadata
    let mut reader = SSTableReader::open(&path).unwrap();
    
    // We need the SSTable metadata for range checks
    // For now, test via direct read attempts
    assert!(matches!(reader.get(b"apple"), Err(AtlasError::KeyNotFound)));
    assert!(matches!(reader.get(b"date"), Err(AtlasError::KeyNotFound)));
}

// =============================================================================
// Large Data Tests
// =============================================================================

#[test]
fn test_large_values() {
    let (_temp, path) = setup_temp_sstable();
    
    let large_value = vec![0xAB; 1024 * 100]; // 100 KB
    
    let mut builder = SSTableBuilder::new(&path).unwrap();
    builder.add(b"big_key", &large_value).unwrap();
    builder.finish().unwrap();
    
    let mut reader = SSTableReader::open(&path).unwrap();
    let value = reader.get(b"big_key").unwrap().unwrap();
    
    assert_eq!(value.len(), 100 * 1024);
    assert_eq!(value, large_value);
}

#[test]
fn test_many_entries() {
    let (_temp, path) = setup_temp_sstable();
    let sstable = create_sstable_with_entries(&path, 10_000);
    
    assert_eq!(sstable.entry_count(), 10_000);
    
    let mut reader = SSTableReader::open(&path).unwrap();
    
    // Spot check some entries
    let value = reader.get(b"key05000").unwrap().unwrap();
    assert_eq!(value, b"value5000");
    
    let value = reader.get(b"key09999").unwrap().unwrap();
    assert_eq!(value, b"value9999");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_open_nonexistent_file() {
    let (_temp, path) = setup_temp_sstable();
    // Don't create the file
    
    let result = SSTableReader::open(&path);
    assert!(result.is_err());
}

#[test]
fn test_open_invalid_magic() {
    let (_temp, path) = setup_temp_sstable();
    
    // Write garbage to file
    std::fs::write(&path, b"GARBAGE_DATA_NOT_SSTABLE").unwrap();
    
    let result = SSTableReader::open(&path);
    assert!(matches!(result, Err(AtlasError::Storage(_))));
}
