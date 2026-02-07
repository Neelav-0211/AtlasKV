//! Tests for WAL Entry serialization and deserialization
//!
//! These tests verify:
//! - Round-trip serialization for all operation types
//! - CRC32 corruption detection
//! - Edge cases (truncation, malformed data, large values)

use atlaskv::wal::{Operation, WalEntry, HEADER_SIZE};
use atlaskv::AtlasError;

// =============================================================================
// Serialization Round-Trip Tests
// =============================================================================

#[test]
fn test_serialize_deserialize_put() {
    let entry = WalEntry::new(
        1,
        Operation::Put {
            key: b"hello".to_vec(),
            value: b"world".to_vec(),
        },
    );

    let bytes = entry.serialize().unwrap();
    let recovered = WalEntry::deserialize(&bytes).unwrap();

    assert_eq!(entry.lsn, recovered.lsn);
    assert_eq!(entry.operation, recovered.operation);
    assert_eq!(entry.timestamp, recovered.timestamp);
}

#[test]
fn test_serialize_deserialize_delete() {
    let entry = WalEntry::new(42, Operation::Delete { key: b"mykey".to_vec() });

    let bytes = entry.serialize().unwrap();
    let recovered = WalEntry::deserialize(&bytes).unwrap();

    assert_eq!(entry, recovered);
}

#[test]
fn test_serialize_deserialize_empty_key() {
    let entry = WalEntry::new(
        100,
        Operation::Put {
            key: vec![],
            value: b"empty_key_value".to_vec(),
        },
    );

    let bytes = entry.serialize().unwrap();
    let recovered = WalEntry::deserialize(&bytes).unwrap();

    assert_eq!(entry, recovered);
}

#[test]
fn test_serialize_deserialize_empty_value() {
    let entry = WalEntry::new(
        101,
        Operation::Put {
            key: b"key_with_empty_value".to_vec(),
            value: vec![],
        },
    );

    let bytes = entry.serialize().unwrap();
    let recovered = WalEntry::deserialize(&bytes).unwrap();

    assert_eq!(entry, recovered);
}

// =============================================================================
// CRC Corruption Detection Tests
// =============================================================================

#[test]
fn test_crc_corruption_detected() {
    let entry = WalEntry::new(
        1,
        Operation::Put {
            key: b"key".to_vec(),
            value: b"value".to_vec(),
        },
    );

    let mut bytes = entry.serialize().unwrap();

    // Corrupt a byte in the data section
    if let Some(byte) = bytes.last_mut() {
        *byte ^= 0xFF;
    }

    let result = WalEntry::deserialize(&bytes);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AtlasError::WalCorruption(_)));
}

#[test]
fn test_crc_corruption_in_header_detected() {
    let entry = WalEntry::new(
        1,
        Operation::Put {
            key: b"key".to_vec(),
            value: b"value".to_vec(),
        },
    );

    let mut bytes = entry.serialize().unwrap();

    // Corrupt the CRC bytes (bytes 8-11)
    bytes[8] ^= 0xFF;

    let result = WalEntry::deserialize(&bytes);
    assert!(result.is_err());
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_truncated_entry() {
    let entry = WalEntry::new(1, Operation::Delete { key: b"key".to_vec() });
    let bytes = entry.serialize().unwrap();

    // Truncate the buffer
    let truncated = &bytes[..HEADER_SIZE + 2];
    let result = WalEntry::deserialize(truncated);

    assert!(result.is_err());
}

#[test]
fn test_header_too_small() {
    let bytes = [0u8; 10]; // Less than HEADER_SIZE
    let result = WalEntry::deserialize(&bytes);

    assert!(result.is_err());
}

#[test]
fn test_empty_buffer() {
    let bytes: [u8; 0] = [];
    let result = WalEntry::deserialize(&bytes);

    assert!(result.is_err());
}

#[test]
fn test_large_value() {
    let large_value = vec![0xAB; 1024 * 1024]; // 1 MB value
    let entry = WalEntry::new(
        999,
        Operation::Put {
            key: b"big_key".to_vec(),
            value: large_value.clone(),
        },
    );

    let bytes = entry.serialize().unwrap();
    let recovered = WalEntry::deserialize(&bytes).unwrap();

    if let Operation::Put { key, value } = recovered.operation {
        assert_eq!(key, b"big_key");
        assert_eq!(value, large_value);
    } else {
        panic!("Expected Put operation");
    }
}

// =============================================================================
// LSN Tests
// =============================================================================

#[test]
fn test_lsn_preserved() {
    for lsn in [0, 1, u64::MAX, 12345678901234] {
        let entry = WalEntry::new(lsn, Operation::Delete { key: b"key".to_vec() });
        let bytes = entry.serialize().unwrap();
        let recovered = WalEntry::deserialize(&bytes).unwrap();

        assert_eq!(recovered.lsn, lsn);
    }
}

// =============================================================================
// Serialized Size Tests
// =============================================================================

#[test]
fn test_serialized_size_matches() {
    let entry = WalEntry::new(
        1,
        Operation::Put {
            key: b"test_key".to_vec(),
            value: b"test_value".to_vec(),
        },
    );

    let expected_size = entry.serialized_size().unwrap();
    let actual_bytes = entry.serialize().unwrap();

    assert_eq!(actual_bytes.len(), expected_size);
}

#[test]
fn test_compute_crc_consistency() {
    let entry = WalEntry::new(
        42,
        Operation::Put {
            key: b"key".to_vec(),
            value: b"value".to_vec(),
        },
    );

    // CRC should be deterministic
    let crc1 = entry.compute_crc().unwrap();
    let crc2 = entry.compute_crc().unwrap();

    assert_eq!(crc1, crc2);
}
