//! Protocol Module
//!
//! Defines the wire protocol for client-server communication.
//!
//! ## Protocol Format (V1 - Simple Binary)
//!
//! ### Request Format
//! ```text
//! ┌──────────┬──────────┬─────────────────────────────┐
//! │ Cmd (1)  │ Len (4)  │         Payload             │
//! └──────────┴──────────┴─────────────────────────────┘
//! ```
//!
//! ### Commands
//! - 0x01: GET   - Payload: key
//! - 0x02: PUT   - Payload: key_len (4) + key + value
//! - 0x03: DEL   - Payload: key
//! - 0x04: PING  - Payload: empty
//!
//! ### Response Format
//! ```text
//! ┌──────────┬──────────┬─────────────────────────────┐
//! │Status(1) │ Len (4)  │         Payload             │
//! └──────────┴──────────┴─────────────────────────────┘
//! ```
//!
//! ### Status Codes
//! - 0x00: OK
//! - 0x01: NOT_FOUND
//! - 0x02: ERROR

mod command;
mod response;
mod codec;

pub use command::{Command, CommandType};
pub use response::{Response, Status};
pub use codec::{encode_command, decode_command, encode_response, decode_response};
