//! Protocol codec
//!
//! Encoding and decoding functions for the wire protocol.

use crate::error::Result;
use super::{Command, Response};

/// Encode a command to bytes
pub fn encode_command(_command: &Command) -> Vec<u8> {
    todo!("Implement encode_command")
}

/// Decode a command from bytes
pub fn decode_command(_bytes: &[u8]) -> Result<Command> {
    todo!("Implement decode_command")
}

/// Encode a response to bytes
pub fn encode_response(_response: &Response) -> Vec<u8> {
    todo!("Implement encode_response")
}

/// Decode a response from bytes
pub fn decode_response(_bytes: &[u8]) -> Result<Response> {
    todo!("Implement decode_response")
}
