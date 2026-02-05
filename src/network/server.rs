//! TCP Server
//!
//! Accepts connections and dispatches to worker threads.

use std::sync::Arc;
use crate::error::Result;
use crate::config::Config;
use crate::engine::Engine;

/// TCP server for AtlasKV
pub struct Server {
    // TODO: Add fields
    // - config: Config
    // - engine: Arc<Engine>
    // - listener: Option<TcpListener>
    // - worker_pool: ThreadPool
    // - shutdown: AtomicBool
}

impl Server {
    /// Create a new server with the given config and engine
    pub fn new(_config: Config, _engine: Arc<Engine>) -> Self {
        todo!("Implement Server::new")
    }

    /// Start the server (blocking)
    pub fn run(&mut self) -> Result<()> {
        todo!("Implement run")
    }

    /// Signal the server to shutdown gracefully
    pub fn shutdown(&self) {
        todo!("Implement shutdown")
    }
}
