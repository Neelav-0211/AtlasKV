//! Codec Tests
//!
//! Tests for command and response encoding/decoding.

use std::io::Cursor;
use atlaskv::protocol::{
    Command, Response, Status,
    encode_command, decode_command,
    encode_response, decode_response,
    read_command, write_command,
    read_response, write_response,
};

// =============================================================================
// Command Encoding/Decoding Tests
// =============================================================================

#[test]
fn test_encode_decode_get() {
    let cmd = Command::Get {
        key: b"hello".to_vec(),
    };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Get { key } => assert_eq!(key, b"hello"),
        _ => panic!("Expected GET command"),
    }
}

#[test]
fn test_encode_decode_put() {
    let cmd = Command::Put {
        key: b"mykey".to_vec(),
        value: b"myvalue".to_vec(),
    };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Put { key, value } => {
            assert_eq!(key, b"mykey");
            assert_eq!(value, b"myvalue");
        }
        _ => panic!("Expected PUT command"),
    }
}

#[test]
fn test_encode_decode_delete() {
    let cmd = Command::Delete {
        key: b"todelete".to_vec(),
    };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Delete { key } => assert_eq!(key, b"todelete"),
        _ => panic!("Expected DELETE command"),
    }
}

#[test]
fn test_encode_decode_ping() {
    let cmd = Command::Ping;
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Ping => {}
        _ => panic!("Expected PING command"),
    }
}

#[test]
fn test_encode_decode_empty_key() {
    let cmd = Command::Get { key: vec![] };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Get { key } => assert!(key.is_empty()),
        _ => panic!("Expected GET command"),
    }
}

#[test]
fn test_encode_decode_empty_value() {
    let cmd = Command::Put {
        key: b"key".to_vec(),
        value: vec![],
    };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Put { key, value } => {
            assert_eq!(key, b"key");
            assert!(value.is_empty());
        }
        _ => panic!("Expected PUT command"),
    }
}

#[test]
fn test_encode_decode_binary_data() {
    // Test with binary data containing null bytes and high bytes
    let binary_key: Vec<u8> = vec![0x00, 0x01, 0xFF, 0xFE, 0x80];
    let binary_value: Vec<u8> = (0..=255).collect();

    let cmd = Command::Put {
        key: binary_key.clone(),
        value: binary_value.clone(),
    };
    let encoded = encode_command(&cmd);
    let decoded = decode_command(&encoded).unwrap();

    match decoded {
        Command::Put { key, value } => {
            assert_eq!(key, binary_key);
            assert_eq!(value, binary_value);
        }
        _ => panic!("Expected PUT command"),
    }
}

// =============================================================================
// Response Encoding/Decoding Tests
// =============================================================================

#[test]
fn test_encode_decode_response_ok() {
    let resp = Response::ok(Some(b"value".to_vec()));
    let encoded = encode_response(&resp);
    let decoded = decode_response(&encoded).unwrap();

    assert_eq!(decoded.status, Status::Ok);
    assert_eq!(decoded.payload, Some(b"value".to_vec()));
}

#[test]
fn test_encode_decode_response_ok_no_payload() {
    let resp = Response::ok(None);
    let encoded = encode_response(&resp);
    let decoded = decode_response(&encoded).unwrap();

    assert_eq!(decoded.status, Status::Ok);
    assert_eq!(decoded.payload, None);
}

#[test]
fn test_encode_decode_response_not_found() {
    let resp = Response::not_found();
    let encoded = encode_response(&resp);
    let decoded = decode_response(&encoded).unwrap();

    assert_eq!(decoded.status, Status::NotFound);
    assert_eq!(decoded.payload, None);
}

#[test]
fn test_encode_decode_response_error() {
    let resp = Response::error("something went wrong");
    let encoded = encode_response(&resp);
    let decoded = decode_response(&encoded).unwrap();

    assert_eq!(decoded.status, Status::Error);
    assert_eq!(decoded.payload, Some(b"something went wrong".to_vec()));
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_incomplete_header() {
    let bytes = [0x01, 0x00, 0x00]; // Only 3 bytes, need 5
    let result = decode_command(&bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Incomplete header"));
}

#[test]
fn test_incomplete_payload() {
    // Header says 10 bytes payload, but only 5 provided
    let bytes = [0x01, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x05, 0x68];
    let result = decode_command(&bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Incomplete"));
}

#[test]
fn test_unknown_command_type() {
    let bytes = [0xFF, 0x00, 0x00, 0x00, 0x00]; // Unknown cmd type
    let result = decode_command(&bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown command type"));
}

#[test]
fn test_unknown_response_status() {
    let bytes = [0xFF, 0x00, 0x00, 0x00, 0x00]; // Unknown status
    let result = decode_response(&bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown response status"));
}

#[test]
fn test_get_missing_key_length() {
    // GET command with payload too short for key length
    let bytes = [0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]; // Only 2 bytes payload
    let result = decode_command(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_ping_with_unexpected_payload() {
    // PING command should have empty payload
    let bytes = [0x04, 0x00, 0x00, 0x00, 0x05, 0x68, 0x65, 0x6C, 0x6C, 0x6F];
    let result = decode_command(&bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unexpected payload"));
}

// =============================================================================
// Stream I/O Tests
// =============================================================================

#[test]
fn test_stream_write_read_command() {
    let cmd = Command::Put {
        key: b"key".to_vec(),
        value: b"value".to_vec(),
    };

    let mut buffer = Vec::new();
    write_command(&mut buffer, &cmd).unwrap();

    let mut cursor = Cursor::new(buffer);
    let decoded = read_command(&mut cursor).unwrap();

    match decoded {
        Command::Put { key, value } => {
            assert_eq!(key, b"key");
            assert_eq!(value, b"value");
        }
        _ => panic!("Expected PUT command"),
    }
}

#[test]
fn test_stream_write_read_response() {
    let resp = Response::ok(Some(b"result".to_vec()));

    let mut buffer = Vec::new();
    write_response(&mut buffer, &resp).unwrap();

    let mut cursor = Cursor::new(buffer);
    let decoded = read_response(&mut cursor).unwrap();

    assert_eq!(decoded.status, Status::Ok);
    assert_eq!(decoded.payload, Some(b"result".to_vec()));
}

#[test]
fn test_stream_multiple_commands() {
    let commands = vec![
        Command::Ping,
        Command::Put {
            key: b"k1".to_vec(),
            value: b"v1".to_vec(),
        },
        Command::Get { key: b"k1".to_vec() },
        Command::Delete { key: b"k1".to_vec() },
    ];

    // Write all commands to buffer
    let mut buffer = Vec::new();
    for cmd in &commands {
        write_command(&mut buffer, cmd).unwrap();
    }

    // Read them back
    let mut cursor = Cursor::new(buffer);
    for expected in &commands {
        let decoded = read_command(&mut cursor).unwrap();
        assert_eq!(
            std::mem::discriminant(&decoded),
            std::mem::discriminant(expected)
        );
    }
}

#[test]
fn test_stream_multiple_responses() {
    let responses = vec![
        Response::ok(Some(b"data".to_vec())),
        Response::not_found(),
        Response::error("oops"),
        Response::ok(None),
    ];

    // Write all responses to buffer
    let mut buffer = Vec::new();
    for resp in &responses {
        write_response(&mut buffer, resp).unwrap();
    }

    // Read them back
    let mut cursor = Cursor::new(buffer);
    for expected in &responses {
        let decoded = read_response(&mut cursor).unwrap();
        assert_eq!(decoded.status, expected.status);
        assert_eq!(decoded.payload, expected.payload);
    }
}

// =============================================================================
// Wire Format Verification Tests
// =============================================================================

#[test]
fn test_wire_format_get() {
    let cmd = Command::Get {
        key: b"test".to_vec(),
    };
    let encoded = encode_command(&cmd);

    // Expected: [0x01][0x00 0x00 0x00 0x08][0x00 0x00 0x00 0x04][t e s t]
    //           cmd   payload_len(8)       key_len(4)          key
    assert_eq!(encoded[0], 0x01); // GET command
    assert_eq!(&encoded[1..5], &[0x00, 0x00, 0x00, 0x08]); // payload len = 8
    assert_eq!(&encoded[5..9], &[0x00, 0x00, 0x00, 0x04]); // key len = 4
    assert_eq!(&encoded[9..13], b"test");
}

#[test]
fn test_wire_format_response_ok() {
    let resp = Response::ok(Some(b"hi".to_vec()));
    let encoded = encode_response(&resp);

    // Expected: [0x00][0x00 0x00 0x00 0x02][h i]
    //           status payload_len(2)      payload
    assert_eq!(encoded[0], 0x00); // OK status
    assert_eq!(&encoded[1..5], &[0x00, 0x00, 0x00, 0x02]); // payload len = 2
    assert_eq!(&encoded[5..7], b"hi");
}
