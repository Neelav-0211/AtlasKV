//! Tests for Engine
//!
//! These tests verify:
//! - Basic get/put/delete operations
//! - Command execution
//! - Flush to SSTable
//! - Crash recovery from WAL
//! - Concurrent access patterns
//! - Engine lifecycle (open/close)

use std::thread;

use atlaskv::config::{Config, WalSyncStrategy};
use atlaskv::engine::Engine;
use atlaskv::protocol::Command;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_engine() -> (TempDir, Engine) {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::builder()
        .data_dir(temp_dir.path())
        .wal_sync_strategy(WalSyncStrategy::EveryWrite) // Sync every write for test reliability
        .memtable_size_limit(1024 * 1024) // 1 MB
        .build();
    let engine = Engine::open(config).unwrap();
    (temp_dir, engine)
}

fn setup_temp_engine_with_small_memtable() -> (TempDir, Engine) {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::builder()
        .data_dir(temp_dir.path())
        .wal_sync_strategy(WalSyncStrategy::EveryWrite)
        .memtable_size_limit(100) // Very small to trigger flushes
        .build();
    let engine = Engine::open(config).unwrap();
    (temp_dir, engine)
}

// =============================================================================
// Basic Operations Tests
// =============================================================================

#[test]
fn test_engine_open_creates_directories() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("mydb");

    let config = Config::builder().data_dir(&data_dir).build();

    let _engine = Engine::open(config).unwrap();

    assert!(data_dir.exists());
    assert!(data_dir.join("sstables").exists());
    assert!(data_dir.join("wal.log").exists());
}

#[test]
fn test_engine_put_get() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"hello", b"world").unwrap();
    let result = engine.get(b"hello").unwrap();

    assert_eq!(result, Some(b"world".to_vec()));
}

#[test]
fn test_engine_get_nonexistent_key() {
    let (_temp, engine) = setup_temp_engine();

    let result = engine.get(b"nonexistent").unwrap();

    assert_eq!(result, None);
}

#[test]
fn test_engine_put_overwrite() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"value1").unwrap();
    engine.put(b"key", b"value2").unwrap();

    let result = engine.get(b"key").unwrap();
    assert_eq!(result, Some(b"value2".to_vec()));
}

#[test]
fn test_engine_delete() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"value").unwrap();
    assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));

    engine.delete(b"key").unwrap();
    assert_eq!(engine.get(b"key").unwrap(), None);
}

#[test]
fn test_engine_delete_nonexistent_key() {
    let (_temp, engine) = setup_temp_engine();

    // Should not error
    engine.delete(b"nonexistent").unwrap();
    assert_eq!(engine.get(b"nonexistent").unwrap(), None);
}

#[test]
fn test_engine_multiple_keys() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key1", b"value1").unwrap();
    engine.put(b"key2", b"value2").unwrap();
    engine.put(b"key3", b"value3").unwrap();

    assert_eq!(engine.get(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(engine.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    assert_eq!(engine.get(b"key3").unwrap(), Some(b"value3".to_vec()));
}

// =============================================================================
// Command Execution Tests
// =============================================================================

#[test]
fn test_engine_execute_get() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"value").unwrap();

    let result = engine
        .execute(Command::Get {
            key: b"key".to_vec(),
        })
        .unwrap();

    assert_eq!(result, Some(b"value".to_vec()));
}

#[test]
fn test_engine_execute_put() {
    let (_temp, engine) = setup_temp_engine();

    let result = engine
        .execute(Command::Put {
            key: b"key".to_vec(),
            value: b"value".to_vec(),
        })
        .unwrap();

    assert_eq!(result, None); // Put returns None
    assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
}

#[test]
fn test_engine_execute_delete() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"value").unwrap();

    let result = engine
        .execute(Command::Delete {
            key: b"key".to_vec(),
        })
        .unwrap();

    assert_eq!(result, None); // Delete returns None
    assert_eq!(engine.get(b"key").unwrap(), None);
}

#[test]
fn test_engine_execute_ping() {
    let (_temp, engine) = setup_temp_engine();

    let result = engine.execute(Command::Ping).unwrap();

    assert_eq!(result, Some(b"PONG".to_vec()));
}

// =============================================================================
// Flush Tests
// =============================================================================

#[test]
fn test_engine_manual_flush() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"value").unwrap();
    assert_eq!(engine.memtable_entry_count(), 1);
    assert_eq!(engine.sstable_count(), 0);

    engine.flush().unwrap();

    assert_eq!(engine.memtable_entry_count(), 0);
    assert_eq!(engine.sstable_count(), 1);

    // Data should still be accessible from SSTable
    assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
}

#[test]
fn test_engine_auto_flush_on_size_limit() {
    let (_temp, engine) = setup_temp_engine_with_small_memtable();

    // Write enough data to trigger auto-flush (memtable limit is 100 bytes)
    // Each put: key (5 bytes) + value (30+ bytes) = 35+ bytes
    // After ~3 puts we should exceed 100 bytes
    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value_that_is_definitely_long_enough_{:02}", i);
        engine.put(key.as_bytes(), value.as_bytes()).unwrap();
    }

    // Should have flushed at least once
    assert!(
        engine.sstable_count() >= 1,
        "Expected at least 1 SSTable after writing data exceeding memtable limit, got {}",
        engine.sstable_count()
    );

    // All data should still be accessible (either in memtable or SSTable)
    for i in 0..10 {
        let key = format!("key{:02}", i);
        assert!(
            engine.get(key.as_bytes()).unwrap().is_some(),
            "Key {} should exist",
            key
        );
    }
}

#[test]
fn test_engine_flush_empty_memtable() {
    let (_temp, engine) = setup_temp_engine();

    // Flushing empty memtable should be a no-op
    engine.flush().unwrap();
    assert_eq!(engine.sstable_count(), 0);
}

// =============================================================================
// Crash Recovery Tests
// =============================================================================

#[test]
fn test_engine_recovery_from_wal() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // First engine - write data, don't flush (simulating crash)
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        engine.put(b"key1", b"value1").unwrap();
        engine.put(b"key2", b"value2").unwrap();
        engine.delete(b"key1").unwrap();
        engine.put(b"key3", b"value3").unwrap();

        // Don't call close() - simulating crash
        // Data is in WAL but not flushed to SSTable
        drop(engine);
    }

    // Second engine - should recover from WAL
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        // Recovered data should be in SSTable (immediately flushed on recovery)
        assert_eq!(engine.sstable_count(), 1);

        // Verify data was recovered correctly
        assert_eq!(engine.get(b"key1").unwrap(), None); // Was deleted
        assert_eq!(engine.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(engine.get(b"key3").unwrap(), Some(b"value3".to_vec()));
    }
}

#[test]
fn test_engine_no_data_loss_after_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // Write, crash, recover, crash again, recover again
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();
        engine.put(b"key", b"value").unwrap();
        drop(engine); // Crash
    }

    // First recovery
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();
        assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
        // Crash again without writing anything new
        drop(engine);
    }

    // Second recovery - data should still be there (in SSTable from first recovery)
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();
        assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
    }
}

// =============================================================================
// Close/Lifecycle Tests
// =============================================================================

#[test]
fn test_engine_close_flushes_data() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // Write data and close gracefully
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        engine.put(b"key", b"value").unwrap();
        engine.close().unwrap(); // Graceful close
    }

    // Reopen - data should be in SSTable
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
        assert_eq!(engine.sstable_count(), 1);
    }
}

#[test]
fn test_engine_open_path_convenience() {
    let temp_dir = TempDir::new().unwrap();

    let engine = Engine::open_path(temp_dir.path()).unwrap();

    engine.put(b"key", b"value").unwrap();
    assert_eq!(engine.get(b"key").unwrap(), Some(b"value".to_vec()));
}

// =============================================================================
// Accessor Tests
// =============================================================================

#[test]
fn test_engine_accessors() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    let config = Config::builder()
        .data_dir(&data_dir)
        .memtable_size_limit(1024)
        .build();
    let engine = Engine::open(config).unwrap();

    assert_eq!(engine.data_dir(), data_dir);
    assert_eq!(engine.storage_dir(), data_dir.join("sstables"));
    assert_eq!(engine.memtable_size(), 0);
    assert_eq!(engine.memtable_entry_count(), 0);
    assert_eq!(engine.sstable_count(), 0);
    assert_eq!(engine.config().memtable_size_limit, 1024);
}

// =============================================================================
// Concurrent Access Tests
// =============================================================================

#[test]
fn test_engine_concurrent_reads() {
    use std::sync::Arc;

    let temp_dir = TempDir::new().unwrap();
    let config = Config::builder()
        .data_dir(temp_dir.path())
        .wal_sync_strategy(WalSyncStrategy::EveryWrite)
        .build();
    let engine = Arc::new(Engine::open(config).unwrap());

    // Pre-populate data
    for i in 0..100 {
        engine
            .put(format!("key{}", i).as_bytes(), format!("value{}", i).as_bytes())
            .unwrap();
    }

    // Spawn multiple reader threads
    let mut handles = vec![];
    for _ in 0..4 {
        let engine_clone = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key{}", i);
                let expected = format!("value{}", i);
                let result = engine_clone.get(key.as_bytes()).unwrap();
                assert_eq!(result, Some(expected.into_bytes()));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_engine_concurrent_writes() {
    use std::sync::Arc;

    let temp_dir = TempDir::new().unwrap();
    let config = Config::builder()
        .data_dir(temp_dir.path())
        .wal_sync_strategy(WalSyncStrategy::EveryWrite)
        .memtable_size_limit(1024 * 1024) // Large enough to not auto-flush
        .build();
    let engine = Arc::new(Engine::open(config).unwrap());

    // Spawn multiple writer threads
    let mut handles = vec![];
    for t in 0..4 {
        let engine_clone = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..25 {
                let key = format!("thread{}_key{}", t, i);
                let value = format!("thread{}_value{}", t, i);
                engine_clone.put(key.as_bytes(), value.as_bytes()).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all writes succeeded
    for t in 0..4 {
        for i in 0..25 {
            let key = format!("thread{}_key{}", t, i);
            let expected = format!("thread{}_value{}", t, i);
            let result = engine.get(key.as_bytes()).unwrap();
            assert_eq!(result, Some(expected.into_bytes()));
        }
    }
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_engine_empty_key() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"", b"empty_key_value").unwrap();
    assert_eq!(
        engine.get(b"").unwrap(),
        Some(b"empty_key_value".to_vec())
    );
}

#[test]
fn test_engine_empty_value() {
    let (_temp, engine) = setup_temp_engine();

    engine.put(b"key", b"").unwrap();
    assert_eq!(engine.get(b"key").unwrap(), Some(b"".to_vec()));
}

#[test]
fn test_engine_large_value() {
    let (_temp, engine) = setup_temp_engine();

    let large_value = vec![0xAB; 100_000]; // 100 KB
    engine.put(b"large_key", &large_value).unwrap();

    let result = engine.get(b"large_key").unwrap();
    assert_eq!(result, Some(large_value));
}

#[test]
fn test_engine_binary_data() {
    let (_temp, engine) = setup_temp_engine();

    // Binary key and value with null bytes
    let key = b"\x00\x01\x02\xFF\xFE";
    let value = b"\xFF\x00\xAB\xCD\x00";

    engine.put(key, value).unwrap();
    assert_eq!(engine.get(key).unwrap(), Some(value.to_vec()));
}
