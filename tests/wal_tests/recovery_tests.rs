//! Tests for WAL Recovery
//!
//! These tests verify:
//! - Recovery from a clean WAL (no corruption)
//! - Recovery from an empty WAL
//! - Recovery with partial writes (truncated tail)
//! - Recovery with corrupted entries (CRC mismatch)
//! - Verify mode (stats only, no entries returned)

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use atlaskv::wal::{Operation, WalEntry, WalWriter, WalRecovery};
use atlaskv::config::WalSyncStrategy;
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_temp_wal() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    (temp_dir, wal_path)
}

/// Write entries using WalWriter (produces a well-formed WAL)
fn write_entries_via_writer(path: &PathBuf, count: usize) {
    let mut writer = WalWriter::open(path, WalSyncStrategy::EveryWrite).unwrap();
    for i in 0..count {
        writer.append(Operation::Put {
            key: format!("key{}", i).into_bytes(),
            value: format!("value{}", i).into_bytes(),
        }).unwrap();
    }
}

/// Write raw serialized entries directly to a file (for crafting corruption)
fn write_raw_entries(path: &PathBuf, entries: &[WalEntry]) {
    let mut file = File::create(path).unwrap();
    for entry in entries {
        let bytes = entry.serialize().unwrap();
        file.write_all(&bytes).unwrap();
    }
    file.sync_all().unwrap();
}

// =============================================================================
// Recover: Clean WAL Tests
// =============================================================================

#[test]
fn test_recover_empty_file() {
    let (_temp, wal_path) = setup_temp_wal();
    File::create(&wal_path).unwrap();

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    assert_eq!(entries.len(), 0);
    assert_eq!(result.entries_recovered, 0);
    assert_eq!(result.entries_corrupted, 0);
    assert_eq!(result.last_lsn, 0);
    assert!(!result.was_truncated);
}

#[test]
fn test_recover_single_entry() {
    let (_temp, wal_path) = setup_temp_wal();
    write_entries_via_writer(&wal_path, 1);

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(result.entries_recovered, 1);
    assert_eq!(result.entries_corrupted, 0);
    assert_eq!(result.last_lsn, 1);
    assert!(!result.was_truncated);
}

#[test]
fn test_recover_multiple_entries() {
    let (_temp, wal_path) = setup_temp_wal();
    write_entries_via_writer(&wal_path, 10);

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    assert_eq!(entries.len(), 10);
    assert_eq!(result.entries_recovered, 10);
    assert_eq!(result.entries_corrupted, 0);
    assert_eq!(result.last_lsn, 10);
    assert!(!result.was_truncated);

    // Verify entries are in order
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.lsn, (i + 1) as u64);
    }
}

#[test]
fn test_recover_preserves_operations() {
    let (_temp, wal_path) = setup_temp_wal();

    {
        let mut writer = WalWriter::open(&wal_path, WalSyncStrategy::EveryWrite).unwrap();
        writer.append(Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() }).unwrap();
        writer.append(Operation::Delete { key: b"k1".to_vec() }).unwrap();
        writer.append(Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() }).unwrap();
    }

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    assert_eq!(result.entries_recovered, 3);
    assert!(matches!(entries[0].operation, Operation::Put { .. }));
    assert!(matches!(entries[1].operation, Operation::Delete { .. }));
    assert!(matches!(entries[2].operation, Operation::Put { .. }));
}

// =============================================================================
// Recover: Partial Write Tests (was_truncated = true)
// =============================================================================

#[test]
fn test_recover_partial_header_at_tail() {
    let (_temp, wal_path) = setup_temp_wal();

    // Write one good entry, then an incomplete header
    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let bytes = entry.serialize().unwrap();

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&bytes).unwrap();
    file.write_all(&[0u8; 8]).unwrap(); // Partial header (8 bytes < HEADER_SIZE)
    file.sync_all().unwrap();

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(result.entries_recovered, 1);
    assert_eq!(result.last_lsn, 1);
    // Trailing garbage means truncation
    assert!(result.was_truncated);
}

#[test]
fn test_recover_partial_data_at_tail() {
    let (_temp, wal_path) = setup_temp_wal();

    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let good_bytes = entry.serialize().unwrap();

    // Write good entry + a second entry with complete header but truncated data
    let entry2 = WalEntry::new(2, Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() });
    let mut bad_bytes = entry2.serialize().unwrap();
    bad_bytes.truncate(20); // Header is 16 bytes, only 4 bytes of data

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&good_bytes).unwrap();
    file.write_all(&bad_bytes).unwrap();
    file.sync_all().unwrap();

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    // Only the first entry should be recovered
    assert_eq!(entries.len(), 1);
    assert_eq!(result.entries_recovered, 1);
    assert!(result.was_truncated);
}

// =============================================================================
// Recover: Corruption Tests (CRC mismatch)
// =============================================================================

#[test]
fn test_recover_corrupted_entry() {
    let (_temp, wal_path) = setup_temp_wal();

    let entry1 = WalEntry::new(1, Operation::Put { key: b"k1".to_vec(), value: b"v1".to_vec() });
    let entry2 = WalEntry::new(2, Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() });

    let good_bytes = entry1.serialize().unwrap();
    let mut bad_bytes = entry2.serialize().unwrap();

    // Corrupt a data byte in the second entry (flip last byte)
    if let Some(byte) = bad_bytes.last_mut() {
        *byte ^= 0xFF;
    }

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&good_bytes).unwrap();
    file.write_all(&bad_bytes).unwrap();
    file.sync_all().unwrap();

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    // Only the first entry survives
    assert_eq!(entries.len(), 1);
    assert_eq!(result.entries_recovered, 1);
    assert_eq!(result.entries_corrupted, 1);
    assert_eq!(result.last_lsn, 1);
    assert!(result.was_truncated);
}

#[test]
fn test_recover_corruption_at_first_entry() {
    let (_temp, wal_path) = setup_temp_wal();

    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let mut bytes = entry.serialize().unwrap();

    // Corrupt the first entry
    bytes[20] ^= 0xFF;

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();

    let (entries, result) = WalRecovery::recover(&wal_path).unwrap();

    // Nothing recovered
    assert_eq!(entries.len(), 0);
    assert_eq!(result.entries_recovered, 0);
    assert_eq!(result.entries_corrupted, 1);
    assert_eq!(result.last_lsn, 0);
    assert!(result.was_truncated);
}

// =============================================================================
// Verify Tests (stats only, same logic as recover)
// =============================================================================

#[test]
fn test_verify_clean_wal() {
    let (_temp, wal_path) = setup_temp_wal();
    write_entries_via_writer(&wal_path, 5);

    let result = WalRecovery::verify(&wal_path).unwrap();

    assert_eq!(result.entries_recovered, 5);
    assert_eq!(result.entries_corrupted, 0);
    assert_eq!(result.last_lsn, 5);
    assert!(!result.was_truncated);
}

#[test]
fn test_verify_empty_wal() {
    let (_temp, wal_path) = setup_temp_wal();
    File::create(&wal_path).unwrap();

    let result = WalRecovery::verify(&wal_path).unwrap();

    assert_eq!(result.entries_recovered, 0);
    assert!(!result.was_truncated);
}

#[test]
fn test_verify_with_corruption() {
    let (_temp, wal_path) = setup_temp_wal();

    let entry1 = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let entry2 = WalEntry::new(2, Operation::Put { key: b"k2".to_vec(), value: b"v2".to_vec() });

    let good_bytes = entry1.serialize().unwrap();
    let mut bad_bytes = entry2.serialize().unwrap();
    if let Some(byte) = bad_bytes.last_mut() {
        *byte ^= 0xFF;
    }

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&good_bytes).unwrap();
    file.write_all(&bad_bytes).unwrap();
    file.sync_all().unwrap();

    let result = WalRecovery::verify(&wal_path).unwrap();

    assert_eq!(result.entries_recovered, 1);
    assert_eq!(result.entries_corrupted, 1);
    assert!(result.was_truncated);
}

#[test]
fn test_verify_with_partial_write() {
    let (_temp, wal_path) = setup_temp_wal();

    let entry = WalEntry::new(1, Operation::Put { key: b"k".to_vec(), value: b"v".to_vec() });
    let bytes = entry.serialize().unwrap();

    let mut file = File::create(&wal_path).unwrap();
    file.write_all(&bytes).unwrap();
    file.write_all(&[0u8; 5]).unwrap(); // Trailing junk
    file.sync_all().unwrap();

    let result = WalRecovery::verify(&wal_path).unwrap();

    assert_eq!(result.entries_recovered, 1);
    assert_eq!(result.entries_corrupted, 0);
    assert!(result.was_truncated);
}

// =============================================================================
// Recover + Verify Consistency Test
// =============================================================================

#[test]
fn test_recover_and_verify_agree() {
    let (_temp, wal_path) = setup_temp_wal();
    write_entries_via_writer(&wal_path, 20);

    let (entries, recover_result) = WalRecovery::recover(&wal_path).unwrap();
    let verify_result = WalRecovery::verify(&wal_path).unwrap();

    // Both should report identical stats
    assert_eq!(entries.len(), recover_result.entries_recovered as usize);
    assert_eq!(recover_result.entries_recovered, verify_result.entries_recovered);
    assert_eq!(recover_result.entries_corrupted, verify_result.entries_corrupted);
    assert_eq!(recover_result.last_lsn, verify_result.last_lsn);
    assert_eq!(recover_result.was_truncated, verify_result.was_truncated);
}
