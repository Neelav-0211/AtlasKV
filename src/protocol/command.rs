//! Command definitions
//!
//! Represents commands from clients.

/// Command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandType {
    Get = 0x01,
    Put = 0x02,
    Delete = 0x03,
    Ping = 0x04,
}

/// A parsed command
#[derive(Debug, Clone)]
pub enum Command {
    /// Get a value by key
    Get { key: Vec<u8> },

    /// Put a key-value pair
    Put { key: Vec<u8>, value: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },

    /// Ping (health check)
    Ping,
}

impl Command {
    /// Get the command type
    pub fn command_type(&self) -> CommandType {
        match self {
            Command::Get { .. } => CommandType::Get,
            Command::Put { .. } => CommandType::Put,
            Command::Delete { .. } => CommandType::Delete,
            Command::Ping => CommandType::Ping,
        }
    }
}
