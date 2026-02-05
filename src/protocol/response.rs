//! Response definitions
//!
//! Represents responses to clients.

/// Response status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ok = 0x00,
    NotFound = 0x01,
    Error = 0x02,
}

/// A response to send to client
#[derive(Debug, Clone)]
pub struct Response {
    /// Status code
    pub status: Status,

    /// Optional payload (value for GET, error message for ERROR)
    pub payload: Option<Vec<u8>>,
}

impl Response {
    /// Create an OK response with optional payload
    pub fn ok(payload: Option<Vec<u8>>) -> Self {
        Self {
            status: Status::Ok,
            payload,
        }
    }

    /// Create a NOT_FOUND response
    pub fn not_found() -> Self {
        Self {
            status: Status::NotFound,
            payload: None,
        }
    }

    /// Create an ERROR response
    pub fn error(message: &str) -> Self {
        Self {
            status: Status::Error,
            payload: Some(message.as_bytes().to_vec()),
        }
    }
}
