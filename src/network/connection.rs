//! Connection Handler
//!
//! Handles individual client connections.

use std::io::{BufReader, BufWriter};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{AtlasError, Result};
use crate::engine::Engine;
use crate::protocol::{read_command, write_response, Response};

/// Handles a single client connection
pub struct Connection {
    /// TCP stream reader (buffered for efficiency)
    reader: BufReader<TcpStream>,

    /// TCP stream writer (buffered for efficiency)
    writer: BufWriter<TcpStream>,

    /// Reference to the storage engine
    engine: Arc<Engine>,

    /// Peer address for logging
    peer_addr: String,
}

impl Connection {
    /// Create a new connection handler
    ///
    /// Sets up buffered I/O and configures timeouts
    pub fn new(stream: TcpStream, engine: Arc<Engine>) -> Result<Self> {
        // Get peer address for logging before we split the stream
        let peer_addr = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Disable Nagle's algorithm for low latency
        stream.set_nodelay(true)?;

        // Clone stream for separate read/write handles
        let read_stream = stream.try_clone()?;
        let write_stream = stream;

        Ok(Self {
            reader: BufReader::new(read_stream),
            writer: BufWriter::new(write_stream),
            engine,
            peer_addr,
        })
    }

    /// Configure connection timeouts
    pub fn set_timeouts(&mut self, read_ms: u64, write_ms: u64) -> Result<()> {
        let read_stream = self.reader.get_ref();
        let write_stream = self.writer.get_ref();

        if read_ms > 0 {
            read_stream.set_read_timeout(Some(Duration::from_millis(read_ms)))?;
        }
        if write_ms > 0 {
            write_stream.set_write_timeout(Some(Duration::from_millis(write_ms)))?;
        }

        Ok(())
    }

    /// Handle the connection (blocking until closed)
    ///
    /// Reads commands in a loop and sends responses.
    /// Returns when the client disconnects or an error occurs.
    pub fn handle(&mut self) -> Result<()> {
        tracing::debug!("Connection established from {}", self.peer_addr);

        loop {
            // Read next command
            let command = match read_command(&mut self.reader) {
                Ok(cmd) => cmd,
                Err(AtlasError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // Client disconnected gracefully
                    tracing::debug!("Client {} disconnected", self.peer_addr);
                    return Ok(());
                }
                Err(AtlasError::Io(ref e)) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                    // Connection reset by peer
                    tracing::debug!("Connection reset by client {}", self.peer_addr);
                    return Ok(());
                }
                Err(AtlasError::Io(ref e)) if e.kind() == std::io::ErrorKind::ConnectionAborted => {
                    // Connection aborted
                    tracing::debug!("Connection aborted by client {}", self.peer_addr);
                    return Ok(());
                }
                Err(AtlasError::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Read timeout - could continue or close
                    tracing::debug!("Read timeout for client {}", self.peer_addr);
                    return Ok(());
                }
                Err(AtlasError::Io(ref e)) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Read timeout (Windows uses TimedOut instead of WouldBlock)
                    tracing::debug!("Read timeout for client {}", self.peer_addr);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Error reading from {}: {}", self.peer_addr, e);
                    // Send error response if possible
                    let _ = self.send_response(Response::error(&e.to_string()));
                    return Err(e);
                }
            };

            tracing::trace!("Received command from {}: {:?}", self.peer_addr, command);

            // Execute command
            let response = self.execute_command(command);

            // Send response
            if let Err(e) = self.send_response(response) {
                // If the client disconnected before we could send the response
                // (e.g. connection abort/reset/broken pipe), log and exit gracefully
                // rather than treating it as a server error.
                if let AtlasError::Io(ref io_err) = e {
                    match io_err.kind() {
                        std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe => {
                            tracing::debug!(
                                "Client {} disconnected before response could be sent: {}",
                                self.peer_addr, e
                            );
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                tracing::warn!("Error writing to {}: {}", self.peer_addr, e);
                return Err(e);
            }
        }
    }

    /// Execute a command and return a response
    fn execute_command(&self, command: crate::protocol::Command) -> Response {
        match self.engine.execute(command) {
            Ok(Some(value)) => Response::ok(Some(value)),
            Ok(None) => Response::ok(None),
            Err(AtlasError::KeyNotFound) => Response::not_found(),
            Err(e) => Response::error(&e.to_string()),
        }
    }

    /// Send a response to the client
    fn send_response(&mut self, response: Response) -> Result<()> {
        write_response(&mut self.writer, &response)?;
        Ok(())
    }

    /// Get the peer address string
    pub fn peer_addr(&self) -> &str {
        &self.peer_addr
    }
}
