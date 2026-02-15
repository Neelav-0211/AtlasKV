//! Protocol codec
//!
//! Encoding and decoding functions for the wire protocol.
//!
//! ## Wire Format
//!
//! ### Request (Command) Format
//! ```text
//! ┌──────────┬──────────┬─────────────────────────────┐
//! │ Cmd (1)  │ Len (4)  │         Payload             │
//! └──────────┴──────────┴─────────────────────────────┘
//! ```
//!
//! ### Payload by Command Type
//! - GET:    key_len (4 bytes) + key
//! - PUT:    key_len (4 bytes) + key + value
//! - DELETE: key_len (4 bytes) + key
//! - PING:   empty
//!
//! ### Response Format
//! ```text
//! ┌──────────┬──────────┬─────────────────────────────┐
//! │Status(1) │ Len (4)  │         Payload             │
//! └──────────┴──────────┴─────────────────────────────┘
//! ```

use std::io::{Read, Write};
use crate::error::{AtlasError, Result};
use super::{Command, Response, Status};

/// Header size: 1 byte command/status + 4 bytes length
pub const HEADER_SIZE: usize = 5;

/// Maximum payload size (16 MB)
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

// =============================================================================
// Command Encoding/Decoding
// =============================================================================

/// Encode a command to bytes
///
/// Format: cmd_type (1) + payload_len (4) + payload
pub fn encode_command(command: &Command) -> Vec<u8> {
    let cmd_type = command.command_type() as u8;

    // Build payload based on command type
    let payload = match command {
        Command::Get { key } => {
            let mut payload = Vec::with_capacity(4 + key.len());
            payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
            payload.extend_from_slice(key);
            payload
        }
        Command::Put { key, value } => {
            let mut payload = Vec::with_capacity(4 + key.len() + value.len());
            payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
            payload.extend_from_slice(key);
            payload.extend_from_slice(value);
            payload
        }
        Command::Delete { key } => {
            let mut payload = Vec::with_capacity(4 + key.len());
            payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
            payload.extend_from_slice(key);
            payload
        }
        Command::Ping => Vec::new(),
    };

    // Build full message: header + payload
    let mut message = Vec::with_capacity(HEADER_SIZE + payload.len());
    message.push(cmd_type);
    message.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    message.extend_from_slice(&payload);

    message
}

/// Decode a command from bytes
///
/// Returns the command and number of bytes consumed
pub fn decode_command(bytes: &[u8]) -> Result<Command> {
    if bytes.len() < HEADER_SIZE {
        return Err(AtlasError::Protocol(format!(
            "Incomplete header: expected {} bytes, got {}",
            HEADER_SIZE,
            bytes.len()
        )));
    }

    // Parse header
    let cmd_type = bytes[0];
    let payload_len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;

    // Validate payload length
    if payload_len > MAX_PAYLOAD_SIZE as usize {
        return Err(AtlasError::Protocol(format!(
            "Payload too large: {} bytes (max {})",
            payload_len, MAX_PAYLOAD_SIZE
        )));
    }

    let total_len = HEADER_SIZE + payload_len;
    if bytes.len() < total_len {
        return Err(AtlasError::Protocol(format!(
            "Incomplete payload: expected {} bytes, got {}",
            total_len,
            bytes.len()
        )));
    }

    let payload = &bytes[HEADER_SIZE..total_len];

    // Parse command based on type
    match cmd_type {
        0x01 => decode_get_command(payload),
        0x02 => decode_put_command(payload),
        0x03 => decode_delete_command(payload),
        0x04 => decode_ping_command(payload),
        _ => Err(AtlasError::Protocol(format!(
            "Unknown command type: 0x{:02x}",
            cmd_type
        ))),
    }
}

/// Decode GET command payload
fn decode_get_command(payload: &[u8]) -> Result<Command> {
    if payload.len() < 4 {
        return Err(AtlasError::Protocol(
            "GET command: missing key length".to_string(),
        ));
    }

    let key_len = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if payload.len() < 4 + key_len {
        return Err(AtlasError::Protocol(format!(
            "GET command: incomplete key (expected {}, got {})",
            key_len,
            payload.len() - 4
        )));
    }

    let key = payload[4..4 + key_len].to_vec();
    Ok(Command::Get { key })
}

/// Decode PUT command payload
fn decode_put_command(payload: &[u8]) -> Result<Command> {
    if payload.len() < 4 {
        return Err(AtlasError::Protocol(
            "PUT command: missing key length".to_string(),
        ));
    }

    let key_len = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if payload.len() < 4 + key_len {
        return Err(AtlasError::Protocol(format!(
            "PUT command: incomplete key (expected {}, got {})",
            key_len,
            payload.len() - 4
        )));
    }

    let key = payload[4..4 + key_len].to_vec();
    let value = payload[4 + key_len..].to_vec();

    Ok(Command::Put { key, value })
}

/// Decode DELETE command payload
fn decode_delete_command(payload: &[u8]) -> Result<Command> {
    if payload.len() < 4 {
        return Err(AtlasError::Protocol(
            "DELETE command: missing key length".to_string(),
        ));
    }

    let key_len = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;

    if payload.len() < 4 + key_len {
        return Err(AtlasError::Protocol(format!(
            "DELETE command: incomplete key (expected {}, got {})",
            key_len,
            payload.len() - 4
        )));
    }

    let key = payload[4..4 + key_len].to_vec();
    Ok(Command::Delete { key })
}

/// Decode PING command payload
fn decode_ping_command(payload: &[u8]) -> Result<Command> {
    if !payload.is_empty() {
        return Err(AtlasError::Protocol(format!(
            "PING command: unexpected payload of {} bytes",
            payload.len()
        )));
    }
    Ok(Command::Ping)
}

// =============================================================================
// Response Encoding/Decoding
// =============================================================================

/// Encode a response to bytes
///
/// Format: status (1) + payload_len (4) + payload
pub fn encode_response(response: &Response) -> Vec<u8> {
    let payload = response.payload.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
    let payload_len = payload.len() as u32;

    let mut message = Vec::with_capacity(HEADER_SIZE + payload.len());
    message.push(response.status as u8);
    message.extend_from_slice(&payload_len.to_be_bytes());
    message.extend_from_slice(payload);

    message
}

/// Decode a response from bytes
pub fn decode_response(bytes: &[u8]) -> Result<Response> {
    if bytes.len() < HEADER_SIZE {
        return Err(AtlasError::Protocol(format!(
            "Incomplete response header: expected {} bytes, got {}",
            HEADER_SIZE,
            bytes.len()
        )));
    }

    // Parse header
    let status_byte = bytes[0];
    let payload_len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;

    // Validate payload length
    if payload_len > MAX_PAYLOAD_SIZE as usize {
        return Err(AtlasError::Protocol(format!(
            "Response payload too large: {} bytes (max {})",
            payload_len, MAX_PAYLOAD_SIZE
        )));
    }

    let total_len = HEADER_SIZE + payload_len;
    if bytes.len() < total_len {
        return Err(AtlasError::Protocol(format!(
            "Incomplete response payload: expected {} bytes, got {}",
            total_len,
            bytes.len()
        )));
    }

    // Parse status
    let status = match status_byte {
        0x00 => Status::Ok,
        0x01 => Status::NotFound,
        0x02 => Status::Error,
        _ => {
            return Err(AtlasError::Protocol(format!(
                "Unknown response status: 0x{:02x}",
                status_byte
            )))
        }
    };

    // Extract payload
    let payload = if payload_len > 0 {
        Some(bytes[HEADER_SIZE..total_len].to_vec())
    } else {
        None
    };

    Ok(Response { status, payload })
}

// =============================================================================
// Stream-based I/O helpers
// =============================================================================

/// Read a complete command from a stream
///
/// Blocks until a complete command is received or an error occurs
pub fn read_command<R: Read>(reader: &mut R) -> Result<Command> {
    // Read header first
    let mut header = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header)?;

    // Parse payload length
    let payload_len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

    // Validate payload length
    if payload_len > MAX_PAYLOAD_SIZE as usize {
        return Err(AtlasError::Protocol(format!(
            "Payload too large: {} bytes (max {})",
            payload_len, MAX_PAYLOAD_SIZE
        )));
    }

    // Read payload
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload)?;
    }

    // Combine and decode
    let mut full_message = Vec::with_capacity(HEADER_SIZE + payload_len);
    full_message.extend_from_slice(&header);
    full_message.extend_from_slice(&payload);

    decode_command(&full_message)
}

/// Write a command to a stream
pub fn write_command<W: Write>(writer: &mut W, command: &Command) -> Result<()> {
    let bytes = encode_command(command);
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Read a complete response from a stream
pub fn read_response<R: Read>(reader: &mut R) -> Result<Response> {
    // Read header first
    let mut header = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header)?;

    // Parse payload length
    let payload_len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

    // Validate payload length
    if payload_len > MAX_PAYLOAD_SIZE as usize {
        return Err(AtlasError::Protocol(format!(
            "Response payload too large: {} bytes (max {})",
            payload_len, MAX_PAYLOAD_SIZE
        )));
    }

    // Read payload
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload)?;
    }

    // Combine and decode
    let mut full_message = Vec::with_capacity(HEADER_SIZE + payload_len);
    full_message.extend_from_slice(&header);
    full_message.extend_from_slice(&payload);

    decode_response(&full_message)
}

/// Write a response to a stream
pub fn write_response<W: Write>(writer: &mut W, response: &Response) -> Result<()> {
    let bytes = encode_response(response);
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
