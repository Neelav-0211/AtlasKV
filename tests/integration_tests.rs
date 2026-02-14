//! Integration tests for AtlasKV
//!
//! Note: Most tests have been implemented in dedicated test modules:
//! - Engine tests: tests/engine_tests/
//! - MemTable tests: tests/memtable_tests/
//! - Storage tests: tests/storage_tests/
//! - WAL tests: tests/wal_tests/
//!
//! This file contains higher-level integration tests that span multiple components.

use atlaskv::config::{Config, WalSyncStrategy};
use atlaskv::Engine;
use tempfile::TempDir;

// =============================================================================
// Config Tests
// =============================================================================

#[test]
fn test_config_default() {
    let config = Config::default();

    assert_eq!(config.data_dir.to_str().unwrap(), "./atlaskv_data");
    assert_eq!(config.memtable_size_limit, 64 * 1024 * 1024); // 64 MB
    assert_eq!(config.listen_addr, "127.0.0.1:6379");
    assert_eq!(config.max_connections, 1024);
    assert_eq!(config.read_timeout_ms, 5000);
    assert_eq!(config.write_timeout_ms, 5000);
}

#[test]
fn test_config_builder() {
    let config = Config::builder()
        .data_dir("/custom/path")
        .wal_sync_strategy(WalSyncStrategy::EveryWrite)
        .memtable_size_limit(1024)
        .listen_addr("0.0.0.0:8080")
        .max_connections(100)
        .read_timeout_ms(1000)
        .write_timeout_ms(2000)
        .build();

    assert_eq!(config.data_dir.to_str().unwrap(), "/custom/path");
    assert!(matches!(config.wal_sync_strategy, WalSyncStrategy::EveryWrite));
    assert_eq!(config.memtable_size_limit, 1024);
    assert_eq!(config.listen_addr, "0.0.0.0:8080");
    assert_eq!(config.max_connections, 100);
    assert_eq!(config.read_timeout_ms, 1000);
    assert_eq!(config.write_timeout_ms, 2000);
}

#[test]
fn test_config_builder_default_values() {
    // Builder should start with default values
    let config = Config::builder().build();
    let default_config = Config::default();

    assert_eq!(config.data_dir, default_config.data_dir);
    assert_eq!(config.memtable_size_limit, default_config.memtable_size_limit);
}

// =============================================================================
// End-to-End Integration Tests
// =============================================================================

#[test]
fn test_full_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // Phase 1: Create engine, write data, flush, close gracefully
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        // Write some data
        engine.put(b"user:1", b"Alice").unwrap();
        engine.put(b"user:2", b"Bob").unwrap();
        engine.put(b"user:3", b"Charlie").unwrap();

        // Flush to SSTable
        engine.flush().unwrap();

        // Write more data (in memtable, also in WAL)
        engine.put(b"user:4", b"Diana").unwrap();
        engine.delete(b"user:2").unwrap();

        // Graceful close (flushes remaining memtable)
        engine.close().unwrap();
    }

    // Phase 2: Reopen and verify all data persisted
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        // Verify data from first SSTable
        assert_eq!(engine.get(b"user:1").unwrap(), Some(b"Alice".to_vec()));
        assert_eq!(engine.get(b"user:3").unwrap(), Some(b"Charlie".to_vec()));

        // Verify data from second SSTable (flushed on close)
        assert_eq!(engine.get(b"user:4").unwrap(), Some(b"Diana".to_vec()));
        assert_eq!(engine.get(b"user:2").unwrap(), None); // Deleted

        // Should have 2 SSTables
        assert_eq!(engine.sstable_count(), 2);
    }
}

#[test]
fn test_crash_recovery_integration() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // Phase 1: Write data, don't close gracefully (simulate crash)
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        engine.put(b"key1", b"value1").unwrap();
        engine.put(b"key2", b"value2").unwrap();
        engine.put(b"key3", b"value3").unwrap();

        // Crash! (drop without close)
        drop(engine);
    }

    // Phase 2: Recover from WAL
    {
        let config = Config::builder()
            .data_dir(&data_dir)
            .wal_sync_strategy(WalSyncStrategy::EveryWrite)
            .build();
        let engine = Engine::open(config).unwrap();

        // All data should be recovered
        assert_eq!(engine.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(engine.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(engine.get(b"key3").unwrap(), Some(b"value3".to_vec()));

        // Data was immediately flushed to SSTable during recovery
        assert_eq!(engine.sstable_count(), 1);
    }
}
