//! Tests for WAL Reader
//!
//! These tests verify:
//! - Reading entries from WAL file
//! - Iterator functionality
//! - Partial write handling
//! - Empty file handling

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use atlaskv::wal::{Operation, WalEntry, WalReader};
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_wal() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    (temp_dir, wal_path)
}

fn write_entries_to_wal(path: &PathBuf, entries: &[WalEntry]) {
    let mut file = File::create(path).unwrap();
    for entry in entries {
        let bytes = entry.serialize().unwrap();
        file.write_all(&bytes).unwrap();
    }
    file.sync_all().unwrap();
}

// =============================================================================
// Basic Reading Tests
// =============================================================================

#[test]
fn test_read_empty_file() {
    let (_temp, wal_path) = setup_temp_wal();
    File::create(&wal_path).unwrap();

    let mut reader = WalReader::open(&wal_path).unwrap();
    let entry = reader.next_entry().unwrap();

    assert!(entry.is_none());
}

#[test]
fn test_read_single_entry() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let original = WalEntry::new(
        1,
        Operation::Put {
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        },
    );
    
    write_entries_to_wal(&wal_path, &[original.clone()]);

    let mut reader = WalReader::open(&wal_path).unwrap();
    let entry = reader.next_entry().unwrap().unwrap();

    assert_eq!(entry.lsn, original.lsn);
    assert_eq!(entry.operation, original.operation);
}

#[test]
fn test_read_multiple_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let entries = vec![
        WalEntry::new(1, Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }),
        WalEntry::new(2, Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }),
        WalEntry::new(3, Operation::Delete { key: b"k1".to_vec() }),
        WalEntry::new(4, Operation::Put { key: b"k3".to_vec(), value: b"v3".to_vec() }),
    ];
    
    write_entries_to_wal(&wal_path, &entries);

    let mut reader = WalReader::open(&wal_path).unwrap();
    
    for (i, original) in entries.iter().enumerate() {
        let entry = reader.next_entry().unwrap().unwrap();
        assert_eq!(entry.lsn, original.lsn, "Entry {} LSN mismatch", i);
        assert_eq!(entry.operation, original.operation, "Entry {} operation mismatch", i);
    }

    // Should reach EOF
    assert!(reader.next_entry().unwrap().is_none());
}

// =============================================================================
// Iterator Tests
// =============================================================================

#[test]
fn test_iterator_empty_file() {
    let (_temp, wal_path) = setup_temp_wal();
    File::create(&wal_path).unwrap();

    let reader = WalReader::open(&wal_path).unwrap();
    let entries: Vec<_> = reader.entries().collect();

    assert_eq!(entries.len(), 0);
}

#[test]
fn test_iterator_multiple_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let original_entries = vec![
        WalEntry::new(1, Operation::Put { key: b"a".to_vec(), value: b"1".to_vec() }),
        WalEntry::new(2, Operation::Put { key: b"b".to_vec(), value: b"2".to_vec() }),
        WalEntry::new(3, Operation::Delete { key: b"a".to_vec() }),
    ];
    
    write_entries_to_wal(&wal_path, &original_entries);

    let reader = WalReader::open(&wal_path).unwrap();
    let read_entries: Vec<_> = reader.entries()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(read_entries.len(), 3);
    for (i, entry) in read_entries.iter().enumerate() {
        assert_eq!(entry.lsn, original_entries[i].lsn);
    }
}

#[test]
fn test_iterator_for_loop() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let entries = vec![
        WalEntry::new(1, Operation::Put { key: b"x".to_vec(), value: b"y".to_vec() }),
        WalEntry::new(2, Operation::Put { key: b"z".to_vec(), value: b"w".to_vec() }),
    ];
    
    write_entries_to_wal(&wal_path, &entries);

    let reader = WalReader::open(&wal_path).unwrap();
    let mut count = 0;
    
    for result in reader.entries() {
        let entry = result.unwrap();
        assert_eq!(entry.lsn, entries[count].lsn);
        count += 1;
    }

    assert_eq!(count, 2);
}

// =============================================================================
// Partial Write Tests
// =============================================================================

#[test]
fn test_partial_header() {
    let (_temp, wal_path) = setup_temp_wal();
    
    // Write one complete entry
    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let bytes = entry.serialize().unwrap();
    
    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&bytes).unwrap();
    
    // Write partial header (only 8 bytes)
    file.write_all(&[0u8; 8]).unwrap();
    file.sync_all().unwrap();

    let mut reader = WalReader::open(&wal_path).unwrap();
    
    // Should read first entry
    let first = reader.next_entry().unwrap();
    assert!(first.is_some());
    
    // Should stop at partial header
    let second = reader.next_entry().unwrap();
    assert!(second.is_none());
}

#[test]
fn test_partial_data() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let mut bytes = entry.serialize().unwrap();
    
    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&bytes).unwrap();
    
    // Write complete header but truncate data
    bytes.truncate(20); // Header is 16 bytes
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();

    let mut reader = WalReader::open(&wal_path).unwrap();
    
    // Should read first entry
    assert!(reader.next_entry().unwrap().is_some());
    
    // Should detect partial write
    assert!(reader.next_entry().unwrap().is_none());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_large_entry() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let large_value = vec![0xAB; 1024 * 1024]; // 1 MB
    let entry = WalEntry::new(1, Operation::Put {
        key: b"big".to_vec(),
        value: large_value.clone(),
    });
    
    write_entries_to_wal(&wal_path, &[entry.clone()]);

    let mut reader = WalReader::open(&wal_path).unwrap();
    let read_entry = reader.next_entry().unwrap().unwrap();

    if let Operation::Put { value, .. } = read_entry.operation {
        assert_eq!(value.len(), 1024 * 1024);
    } else {
        panic!("Expected Put operation");
    }
}

#[test]
fn test_delete_operation() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let entry = WalEntry::new(5, Operation::Delete { key: b"deleted_key".to_vec() });
    write_entries_to_wal(&wal_path, &[entry.clone()]);

    let mut reader = WalReader::open(&wal_path).unwrap();
    let read_entry = reader.next_entry().unwrap().unwrap();

    assert_eq!(read_entry.lsn, 5);
    match read_entry.operation {
        Operation::Delete { key } => assert_eq!(key, b"deleted_key"),
        _ => panic!("Expected Delete operation"),
    }
}
