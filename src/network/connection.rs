//! Connection Handler
//!
//! Handles individual client connections.

use std::net::TcpStream;
use std::sync::Arc;
use crate::error::Result;
use crate::engine::Engine;

/// Handles a single client connection
pub struct Connection {
    // TODO: Add fields
    // - stream: TcpStream
    // - engine: Arc<Engine>
    // - buffer: Vec<u8>
}

impl Connection {
    /// Create a new connection handler
    pub fn new(_stream: TcpStream, _engine: Arc<Engine>) -> Self {
        todo!("Implement Connection::new")
    }

    /// Handle the connection (blocking until closed)
    pub fn handle(&mut self) -> Result<()> {
        todo!("Implement handle")
    }
}
