//! MemTable Tests
//!
//! Tests verify:
//! - Basic CRUD operations
//! - Size tracking
//! - Tombstone handling
//! - Sorted iteration
//! - Clear functionality
//! - Concurrent access patterns

use atlaskv::memtable::{MemTable, MemTableEntry};

// =============================================================================
// Basic Operations Tests
// =============================================================================

#[test]
fn test_new_memtable_is_empty() {
    let memtable = MemTable::new();
    assert_eq!(memtable.entry_count(), 0);
    assert_eq!(memtable.size(), 0);
    assert!(memtable.is_empty());
}

#[test]
fn test_put_and_get() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    
    let result = memtable.get(b"key1");
    assert_eq!(result, Some(MemTableEntry::Value(b"value1".to_vec())));
}

#[test]
fn test_get_nonexistent_key() {
    let memtable = MemTable::new();
    
    let result = memtable.get(b"nonexistent");
    assert_eq!(result, None);
}

#[test]
fn test_put_multiple_entries() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.put(b"key2".to_vec(), b"value2".to_vec());
    memtable.put(b"key3".to_vec(), b"value3".to_vec());
    
    assert_eq!(memtable.entry_count(), 3);
    assert_eq!(memtable.get(b"key1"), Some(MemTableEntry::Value(b"value1".to_vec())));
    assert_eq!(memtable.get(b"key2"), Some(MemTableEntry::Value(b"value2".to_vec())));
    assert_eq!(memtable.get(b"key3"), Some(MemTableEntry::Value(b"value3".to_vec())));
}

#[test]
fn test_put_overwrites_existing() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.put(b"key1".to_vec(), b"value2".to_vec());
    
    assert_eq!(memtable.entry_count(), 1);
    assert_eq!(memtable.get(b"key1"), Some(MemTableEntry::Value(b"value2".to_vec())));
}

// =============================================================================
// Delete / Tombstone Tests
// =============================================================================

#[test]
fn test_delete_creates_tombstone() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.delete(b"key1".to_vec());
    
    assert_eq!(memtable.get(b"key1"), Some(MemTableEntry::Tombstone));
    assert_eq!(memtable.entry_count(), 1); // Tombstone still counts as entry
}

#[test]
fn test_delete_nonexistent_key() {
    let memtable = MemTable::new();
    
    memtable.delete(b"nonexistent".to_vec());
    
    assert_eq!(memtable.get(b"nonexistent"), Some(MemTableEntry::Tombstone));
    assert_eq!(memtable.entry_count(), 1);
}

#[test]
fn test_put_after_delete() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.delete(b"key1".to_vec());
    memtable.put(b"key1".to_vec(), b"value2".to_vec());
    
    assert_eq!(memtable.get(b"key1"), Some(MemTableEntry::Value(b"value2".to_vec())));
}

// =============================================================================
// Size Tracking Tests
// =============================================================================

#[test]
fn test_size_tracking_put() {
    let memtable = MemTable::new();
    
    let initial_size = memtable.size();
    assert_eq!(initial_size, 0);
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    let expected_size = b"key".len() + b"value".len();
    assert_eq!(memtable.size(), expected_size);
}

#[test]
fn test_size_tracking_multiple_puts() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.put(b"key2".to_vec(), b"value2".to_vec());
    
    let expected_size = (b"key1".len() + b"value1".len()) + 
                        (b"key2".len() + b"value2".len());
    assert_eq!(memtable.size(), expected_size);
}

#[test]
fn test_size_tracking_overwrite() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"short".to_vec());
    let size_after_first = memtable.size();
    
    memtable.put(b"key".to_vec(), b"much_longer_value".to_vec());
    let size_after_second = memtable.size();
    
    assert_eq!(size_after_first, b"key".len() + b"short".len());
    assert_eq!(size_after_second, b"key".len() + b"much_longer_value".len());
}

#[test]
fn test_size_tracking_delete() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    let size_after_put = memtable.size();
    
    memtable.delete(b"key".to_vec());
    let size_after_delete = memtable.size();
    
    assert_eq!(size_after_put, b"key".len() + b"value".len());
    assert_eq!(size_after_delete, b"key".len()); // Tombstone = just key
}

// =============================================================================
// Iteration Tests
// =============================================================================

#[test]
fn test_iter_empty() {
    let memtable = MemTable::new();
    
    let entries = memtable.iter();
    assert_eq!(entries.len(), 0);
}

#[test]
fn test_iter_sorted_order() {
    let memtable = MemTable::new();
    
    // Insert in random order
    memtable.put(b"cherry".to_vec(), b"3".to_vec());
    memtable.put(b"apple".to_vec(), b"1".to_vec());
    memtable.put(b"banana".to_vec(), b"2".to_vec());
    
    let entries = memtable.iter();
    
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].0, b"apple");   // Sorted!
    assert_eq!(entries[1].0, b"banana");
    assert_eq!(entries[2].0, b"cherry");
}

#[test]
fn test_iter_includes_tombstones() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.delete(b"key2".to_vec());
    memtable.put(b"key3".to_vec(), b"value3".to_vec());
    
    let entries = memtable.iter();
    
    assert_eq!(entries.len(), 3);
    assert!(matches!(entries[0].1, MemTableEntry::Value(_)));
    assert!(matches!(entries[1].1, MemTableEntry::Tombstone));
    assert!(matches!(entries[2].1, MemTableEntry::Value(_)));
}

#[test]
fn test_iter_clones_data() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    let entries = memtable.iter();
    
    // Modify memtable after getting snapshot
    memtable.put(b"key".to_vec(), b"modified".to_vec());
    
    // Snapshot should still have old value
    if let MemTableEntry::Value(v) = &entries[0].1 {
        assert_eq!(v, b"value");
    } else {
        panic!("Expected Value");
    }
}

// =============================================================================
// Clear Tests
// =============================================================================

#[test]
fn test_clear() {
    let memtable = MemTable::new();
    
    memtable.put(b"key1".to_vec(), b"value1".to_vec());
    memtable.put(b"key2".to_vec(), b"value2".to_vec());
    assert_eq!(memtable.entry_count(), 2);
    assert!(memtable.size() > 0);
    
    memtable.clear();
    
    assert_eq!(memtable.entry_count(), 0);
    assert_eq!(memtable.size(), 0);
    assert!(memtable.is_empty());
    assert_eq!(memtable.get(b"key1"), None);
}

// =============================================================================
// Should Flush Tests
// =============================================================================

#[test]
fn test_should_flush_under_limit() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    assert!(!memtable.should_flush(1000));
}

#[test]
fn test_should_flush_over_limit() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    let size = memtable.size();
    assert!(memtable.should_flush(size - 1));
    assert!(memtable.should_flush(size));
}

#[test]
fn test_should_flush_exact_limit() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    let size = memtable.size();
    assert!(memtable.should_flush(size));
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_empty_key() {
    let memtable = MemTable::new();
    
    memtable.put(vec![], b"value".to_vec());
    
    assert_eq!(memtable.get(&[]), Some(MemTableEntry::Value(b"value".to_vec())));
}

#[test]
fn test_empty_value() {
    let memtable = MemTable::new();
    
    memtable.put(b"key".to_vec(), vec![]);
    
    assert_eq!(memtable.get(b"key"), Some(MemTableEntry::Value(vec![])));
}

#[test]
fn test_large_value() {
    let memtable = MemTable::new();
    
    let large_value = vec![0xAB; 1024 * 1024]; // 1 MB
    memtable.put(b"big_key".to_vec(), large_value.clone());
    
    if let Some(MemTableEntry::Value(v)) = memtable.get(b"big_key") {
        assert_eq!(v.len(), 1024 * 1024);
        assert_eq!(v, large_value);
    } else {
        panic!("Expected Value");
    }
}

#[test]
fn test_many_entries() {
    let memtable = MemTable::new();
    
    for i in 0..1000 {
        let key = format!("key{:04}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        memtable.put(key, value);
    }
    
    assert_eq!(memtable.entry_count(), 1000);
    
    // Verify sorted order
    let entries = memtable.iter();
    for i in 0..999 {
        assert!(entries[i].0 < entries[i + 1].0);
    }
}

// =============================================================================
// Concurrent Access Tests (Basic)
// =============================================================================

#[test]
fn test_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;
    
    let memtable = Arc::new(MemTable::new());
    memtable.put(b"key".to_vec(), b"value".to_vec());
    
    let mut handles = vec![];
    
    for _ in 0..10 {
        let mt = Arc::clone(&memtable);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let result = mt.get(b"key");
                assert_eq!(result, Some(MemTableEntry::Value(b"value".to_vec())));
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_writes() {
    use std::sync::Arc;
    use std::thread;
    
    let memtable = Arc::new(MemTable::new());
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let mt = Arc::clone(&memtable);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let key = format!("key{}_{}", i, j).into_bytes();
                let value = format!("value{}_{}", i, j).into_bytes();
                mt.put(key, value);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(memtable.entry_count(), 100);
}
