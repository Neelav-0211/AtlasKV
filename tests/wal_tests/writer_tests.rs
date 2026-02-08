//! Tests for WAL Writer
//!
//! These tests verify:
//! - Writing entries to WAL
//! - LSN generation and sequencing
//! - Sync strategies (EveryWrite, EveryNEntries)
//! - Truncation
//! - Integration with reader

use std::path::PathBuf;
use atlaskv::config::WalSyncStrategy;
use atlaskv::wal::{Operation, WalWriter, WalReader};
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_wal() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    (temp_dir, wal_path)
}

// =============================================================================
// Basic Writing Tests
// =============================================================================

#[test]
fn test_write_single_entry() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
    let lsn = writer.append(Operation::Put {
        key: b"key1".to_vec(),
        value: b"value1".to_vec(),
    }).unwrap();

    assert_eq!(lsn, 1);
    assert_eq!(writer.current_lsn(), 2);
}

#[test]
fn test_write_multiple_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
    
    let lsn1 = writer.append(Operation::Put { key: b"a".to_vec(), value: b"1".to_vec() }).unwrap();
    let lsn2 = writer.append(Operation::Put { key: b"b".to_vec(), value: b"2".to_vec() }).unwrap();
    let lsn3 = writer.append(Operation::Delete { key: b"a".to_vec() }).unwrap();

    assert_eq!(lsn1, 1);
    assert_eq!(lsn2, 2);
    assert_eq!(lsn3, 3);
    assert_eq!(writer.current_lsn(), 4);
}

#[test]
fn test_lsn_sequential() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
    
    let mut lsns = Vec::new();
    for i in 0..100 {
        let lsn = writer.append(Operation::Put {
            key: format!("key{}", i).into_bytes(),
            value: format!("val{}", i).into_bytes(),
        }).unwrap();
        lsns.push(lsn);
    }

    // Verify LSNs are sequential
    for (i, lsn) in lsns.iter().enumerate() {
        assert_eq!(*lsn, (i + 1) as u64);
    }
}

// =============================================================================
// Sync Strategy Tests
// =============================================================================

#[test]
fn test_sync_every_write() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
    
    // Each write should sync
    writer.append(Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }).unwrap();
    assert_eq!(writer.uncommitted_count(), 0);  // Reset after sync
    
    writer.append(Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }).unwrap();
    assert_eq!(writer.uncommitted_count(), 0);  // Reset after sync
}

#[test]
fn test_sync_every_n_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryNEntries { count: 5 }).unwrap();
    
    // Write 4 entries - should not sync yet
    for i in 0..4 {
        writer.append(Operation::Put {
            key: format!("k{}", i).into_bytes(),
            value: b"v".to_vec(),
        }).unwrap();
    }
    assert_eq!(writer.uncommitted_count(), 4);
    
    // 5th entry should trigger sync
    writer.append(Operation::Put { key: b"k5".to_vec(), value: b"v".to_vec() }).unwrap();
    assert_eq!(writer.uncommitted_count(), 0);
    
    // Continue writing
    writer.append(Operation::Put { key: b"k6".to_vec(), value: b"v".to_vec() }).unwrap();
    assert_eq!(writer.uncommitted_count(), 1);
}

#[test]
fn test_manual_sync() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryNEntries { count: 100 }).unwrap();
    
    // Write entries without hitting threshold
    for i in 0..10 {
        writer.append(Operation::Put {
            key: format!("k{}", i).into_bytes(),
            value: b"v".to_vec(),
        }).unwrap();
    }
    assert_eq!(writer.uncommitted_count(), 10);
    
    // Manual sync
    writer.sync().unwrap();
    assert_eq!(writer.uncommitted_count(), 0);
}

// =============================================================================
// Write + Read Integration Tests
// =============================================================================

#[test]
fn test_write_then_read() {
    let (_temp, wal_path) = setup_temp_wal();
    
    // Write entries
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Put { key: b"key1".to_vec(), value: b"value1".to_vec() }).unwrap();
        writer.append(Operation::Put { key: b"key2".to_vec(), value: b"value2".to_vec() }).unwrap();
        writer.append(Operation::Delete { key: b"key1".to_vec() }).unwrap();
    } // Writer dropped, file closed

    // Read back
    let mut reader = WalReader::open(&wal_path).unwrap();
    
    let entry1 = reader.next_entry().unwrap().unwrap();
    assert_eq!(entry1.lsn, 1);
    assert!(matches!(entry1.operation, Operation::Put { .. }));
    
    let entry2 = reader.next_entry().unwrap().unwrap();
    assert_eq!(entry2.lsn, 2);
    
    let entry3 = reader.next_entry().unwrap().unwrap();
    assert_eq!(entry3.lsn, 3);
    assert!(matches!(entry3.operation, Operation::Delete { .. }));
    
    // EOF
    assert!(reader.next_entry().unwrap().is_none());
}

#[test]
fn test_write_read_many_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let entry_count = 1000;
    
    // Write
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryNEntries { count: 100 }).unwrap();
        for i in 0..entry_count {
            writer.append(Operation::Put {
                key: format!("key{}", i).into_bytes(),
                value: format!("value{}", i).into_bytes(),
            }).unwrap();
        }
        writer.sync().unwrap(); // Final sync
    }

    // Read
    let reader = WalReader::open(&wal_path).unwrap();
    let entries: Vec<_> = reader.entries().collect::<Result<Vec<_>, _>>().unwrap();
    
    assert_eq!(entries.len(), entry_count);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.lsn, (i + 1) as u64);
    }
}

// =============================================================================
// Truncate Tests
// =============================================================================

#[test]
fn test_truncate_resets_lsn() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
    
    // Write entries
    writer.append(Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }).unwrap();
    writer.append(Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }).unwrap();
    assert_eq!(writer.current_lsn(), 3);
    
    // Truncate
    writer.truncate().unwrap();
    assert_eq!(writer.current_lsn(), 1);
    assert_eq!(writer.uncommitted_count(), 0);
    
    // New writes start from LSN 1
    let lsn = writer.append(Operation::Put { key: b"k3".to_vec(), value: b"v3".to_vec() }).unwrap();
    assert_eq!(lsn, 1);
}

#[test]
fn test_truncate_clears_file() {
    let (_temp, wal_path) = setup_temp_wal();
    
    // Write and truncate
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }).unwrap();
        writer.append(Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }).unwrap();
        writer.truncate().unwrap();
    }

    // Read should find empty file
    let mut reader = WalReader::open(&wal_path).unwrap();
    assert!(reader.next_entry().unwrap().is_none());
}

#[test]
fn test_truncate_then_write() {
    let (_temp, wal_path) = setup_temp_wal();
    
    // Write, truncate, write again
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Put { key: b"old".to_vec(), value: b"data".to_vec() }).unwrap();
        writer.truncate().unwrap();
        writer.append(Operation::Put { key: b"new".to_vec(), value: b"data".to_vec() }).unwrap();
    }

    // Read should only see new entry
    let mut reader = WalReader::open(&wal_path).unwrap();
    let entry = reader.next_entry().unwrap().unwrap();
    
    if let Operation::Put { key, .. } = entry.operation {
        assert_eq!(key, b"new");
    } else {
        panic!("Expected Put operation");
    }
    
    assert!(reader.next_entry().unwrap().is_none());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_large_entry() {
    let (_temp, wal_path) = setup_temp_wal();
    
    let large_value = vec![0xAB; 1024 * 1024]; // 1 MB
    
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Put {
            key: b"big_key".to_vec(),
            value: large_value.clone(),
        }).unwrap();
    }

    // Read back
    let mut reader = WalReader::open(&wal_path).unwrap();
    let entry = reader.next_entry().unwrap().unwrap();
    
    if let Operation::Put { value, .. } = entry.operation {
        assert_eq!(value.len(), 1024 * 1024);
        assert_eq!(value, large_value);
    } else {
        panic!("Expected Put operation");
    }
}

#[test]
fn test_delete_operation() {
    let (_temp, wal_path) = setup_temp_wal();
    
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Delete { key: b"deleted_key".to_vec() }).unwrap();
    }

    let mut reader = WalReader::open(&wal_path).unwrap();
    let entry = reader.next_entry().unwrap().unwrap();
    
    match entry.operation {
        Operation::Delete { key } => assert_eq!(key, b"deleted_key"),
        _ => panic!("Expected Delete operation"),
    }
}

#[test]
fn test_mixed_operations() {
    let (_temp, wal_path) = setup_temp_wal();
    
    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryNEntries { count: 10 }).unwrap();
        writer.append(Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }).unwrap();
        writer.append(Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }).unwrap();
        writer.append(Operation::Delete { key: b"k1".to_vec() }).unwrap();
        writer.append(Operation::Put { key: b"k3".to_vec(), value: b"v3".to_vec() }).unwrap();
        writer.sync().unwrap();
    }

    let reader = WalReader::open(&wal_path).unwrap();
    let entries: Vec<_> = reader.entries().collect::<Result<Vec<_>, _>>().unwrap();
    
    assert_eq!(entries.len(), 4);
    assert!(matches!(entries[0].operation, Operation::Put { .. }));
    assert!(matches!(entries[1].operation, Operation::Put { .. }));
    assert!(matches!(entries[2].operation, Operation::Delete { .. }));
    assert!(matches!(entries[3].operation, Operation::Put { .. }));
}
