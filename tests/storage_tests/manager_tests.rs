//! Tests for StorageManager
//!
//! These tests verify:
//! - Opening/creating storage directories
//! - Flushing MemTable to SSTable
//! - Querying across multiple SSTables
//! - Tombstone handling across SSTables
//! - Persistence (restart and rediscover SSTables)

use std::path::PathBuf;
use atlaskv::memtable::MemTable;
use atlaskv::storage::StorageManager;
use atlaskv::AtlasError;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_storage() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();
    (temp_dir, path)
}

fn create_memtable_with_entries(entries: &[(&[u8], &[u8])]) -> MemTable {
    let memtable = MemTable::new();
    for (key, value) in entries {
        memtable.put(key.to_vec(), value.to_vec());
    }
    memtable
}

// =============================================================================
// Open/Create Tests
// =============================================================================

#[test]
fn test_open_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("new_storage");

    assert!(!path.exists());

    let _manager = StorageManager::open(&path).unwrap();

    assert!(path.exists());
    assert!(path.is_dir());
}

#[test]
fn test_open_empty_directory() {
    let (_temp, path) = setup_temp_storage();

    let manager = StorageManager::open(&path).unwrap();

    assert_eq!(manager.sstable_count(), 0);
    assert_eq!(manager.next_sstable_id(), 1);
}

#[test]
fn test_open_existing_directory() {
    let (_temp, path) = setup_temp_storage();

    // First open - create some SSTables
    {
        let mut manager = StorageManager::open(&path).unwrap();

        let memtable = create_memtable_with_entries(&[(b"k1", b"v1")]);
        manager.flush(&memtable).unwrap();

        let memtable = create_memtable_with_entries(&[(b"k2", b"v2")]);
        manager.flush(&memtable).unwrap();

        assert_eq!(manager.sstable_count(), 2);
    }

    // Second open - should discover existing SSTables
    {
        let manager = StorageManager::open(&path).unwrap();

        assert_eq!(manager.sstable_count(), 2);
        assert_eq!(manager.next_sstable_id(), 3); // Continues from max + 1
    }
}

// =============================================================================
// Flush Tests
// =============================================================================

#[test]
fn test_flush_single_memtable() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = create_memtable_with_entries(&[
        (b"apple", b"red"),
        (b"banana", b"yellow"),
        (b"cherry", b"red"),
    ]);

    let metadata = manager.flush(&memtable).unwrap();

    assert_eq!(metadata.entry_count, 3);
    assert_eq!(manager.sstable_count(), 1);
}

#[test]
fn test_flush_empty_memtable_fails() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = MemTable::new();
    let result = manager.flush(&memtable);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AtlasError::Storage(_)));
}

#[test]
fn test_flush_multiple_memtables() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    // Flush three MemTables
    for i in 0..3 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        let memtable = create_memtable_with_entries(&[(key.as_bytes(), value.as_bytes())]);
        manager.flush(&memtable).unwrap();
    }

    assert_eq!(manager.sstable_count(), 3);
}

#[test]
fn test_flush_with_tombstones() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = MemTable::new();
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.delete(b"key2".to_vec()); // Tombstone
    memtable.put(b"key3".to_vec(), b"value3".to_vec());

    let metadata = manager.flush(&memtable).unwrap();

    assert_eq!(metadata.entry_count, 3); // Includes tombstone
}

// =============================================================================
// Get Tests
// =============================================================================

#[test]
fn test_get_from_single_sstable() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = create_memtable_with_entries(&[
        (b"key1", b"value1"),
        (b"key2", b"value2"),
    ]);
    manager.flush(&memtable).unwrap();

    assert_eq!(manager.get(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(manager.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    assert_eq!(manager.get(b"key3").unwrap(), None); // Not found
}

#[test]
fn test_get_from_multiple_sstables() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    // SSTable 1: k1, k2
    let memtable = create_memtable_with_entries(&[(b"k1", b"v1"), (b"k2", b"v2")]);
    manager.flush(&memtable).unwrap();

    // SSTable 2: k3, k4
    let memtable = create_memtable_with_entries(&[(b"k3", b"v3"), (b"k4", b"v4")]);
    manager.flush(&memtable).unwrap();

    // All keys should be found
    assert_eq!(manager.get(b"k1").unwrap(), Some(b"v1".to_vec()));
    assert_eq!(manager.get(b"k2").unwrap(), Some(b"v2".to_vec()));
    assert_eq!(manager.get(b"k3").unwrap(), Some(b"v3".to_vec()));
    assert_eq!(manager.get(b"k4").unwrap(), Some(b"v4".to_vec()));
}

#[test]
fn test_get_newer_overrides_older() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    // SSTable 1: key → "old"
    let memtable = create_memtable_with_entries(&[(b"key", b"old")]);
    manager.flush(&memtable).unwrap();

    // SSTable 2: key → "new" (overwrites)
    let memtable = create_memtable_with_entries(&[(b"key", b"new")]);
    manager.flush(&memtable).unwrap();

    // Should get newer value
    assert_eq!(manager.get(b"key").unwrap(), Some(b"new".to_vec()));
}

#[test]
fn test_get_tombstone_hides_older_value() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    // SSTable 1: key → "value"
    let memtable = create_memtable_with_entries(&[(b"key", b"value")]);
    manager.flush(&memtable).unwrap();

    // SSTable 2: key → TOMBSTONE
    let memtable = MemTable::new();
    memtable.delete(b"key".to_vec());
    manager.flush(&memtable).unwrap();

    // Should return None (key was deleted)
    assert_eq!(manager.get(b"key").unwrap(), None);
}

#[test]
fn test_get_not_found() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = create_memtable_with_entries(&[(b"exists", b"value")]);
    manager.flush(&memtable).unwrap();

    // Key that doesn't exist
    assert_eq!(manager.get(b"not_exists").unwrap(), None);
}

// =============================================================================
// Persistence Tests
// =============================================================================

#[test]
fn test_persistence_across_restart() {
    let (_temp, path) = setup_temp_storage();

    // Write data and close
    {
        let mut manager = StorageManager::open(&path).unwrap();
        let memtable = create_memtable_with_entries(&[
            (b"key1", b"value1"),
            (b"key2", b"value2"),
        ]);
        manager.flush(&memtable).unwrap();
    }

    // Reopen and verify data persisted
    {
        let mut manager = StorageManager::open(&path).unwrap();
        assert_eq!(manager.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(manager.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    }
}

#[test]
fn test_persistence_multiple_sstables() {
    let (_temp, path) = setup_temp_storage();

    // Create multiple SSTables
    {
        let mut manager = StorageManager::open(&path).unwrap();

        for i in 0..5 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            let memtable = create_memtable_with_entries(&[(key.as_bytes(), value.as_bytes())]);
            manager.flush(&memtable).unwrap();
        }
    }

    // Reopen and verify
    {
        let mut manager = StorageManager::open(&path).unwrap();
        assert_eq!(manager.sstable_count(), 5);

        for i in 0..5 {
            let key = format!("key{}", i);
            let expected = format!("value{}", i);
            assert_eq!(
                manager.get(key.as_bytes()).unwrap(),
                Some(expected.into_bytes())
            );
        }
    }
}

#[test]
fn test_persistence_overwrites() {
    let (_temp, path) = setup_temp_storage();

    // Write old value
    {
        let mut manager = StorageManager::open(&path).unwrap();
        let memtable = create_memtable_with_entries(&[(b"key", b"old")]);
        manager.flush(&memtable).unwrap();
    }

    // Write new value
    {
        let mut manager = StorageManager::open(&path).unwrap();
        let memtable = create_memtable_with_entries(&[(b"key", b"new")]);
        manager.flush(&memtable).unwrap();
    }

    // Reopen and verify newest value
    {
        let mut manager = StorageManager::open(&path).unwrap();
        assert_eq!(manager.get(b"key").unwrap(), Some(b"new".to_vec()));
    }
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_large_flush() {
    let (_temp, path) = setup_temp_storage();
    let mut manager = StorageManager::open(&path).unwrap();

    let memtable = MemTable::new();
    for i in 0..1000 {
        let key = format!("key{:04}", i);
        let value = format!("value{}", i);
        memtable.put(key.into_bytes(), value.into_bytes());
    }

    let metadata = manager.flush(&memtable).unwrap();
    assert_eq!(metadata.entry_count, 1000);

    // Spot check some entries
    assert_eq!(
        manager.get(b"key0500").unwrap(),
        Some(b"value500".to_vec())
    );
}

#[test]
fn test_sstable_ids_continue_after_restart() {
    let (_temp, path) = setup_temp_storage();

    // Create 3 SSTables
    {
        let mut manager = StorageManager::open(&path).unwrap();
        for _ in 0..3 {
            let memtable = create_memtable_with_entries(&[(b"k", b"v")]);
            manager.flush(&memtable).unwrap();
        }
        assert_eq!(manager.next_sstable_id(), 4);
    }

    // Reopen - next ID should continue from 4
    {
        let manager = StorageManager::open(&path).unwrap();
        assert_eq!(manager.next_sstable_id(), 4);
    }
}

#[test]
fn test_ignores_non_sstable_files() {
    let (_temp, path) = setup_temp_storage();

    // Create a valid SSTable
    {
        let mut manager = StorageManager::open(&path).unwrap();
        let memtable = create_memtable_with_entries(&[(b"k", b"v")]);
        manager.flush(&memtable).unwrap();
    }

    // Create some non-SSTable files
    std::fs::write(path.join("random.txt"), b"not an sstable").unwrap();
    std::fs::write(path.join("sstable_abc.sst"), b"bad id").unwrap();
    std::fs::write(path.join("other_000001.sst"), b"wrong prefix").unwrap();

    // Reopen - should only see the one valid SSTable
    {
        let manager = StorageManager::open(&path).unwrap();
        assert_eq!(manager.sstable_count(), 1);
    }
}
