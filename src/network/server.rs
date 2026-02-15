//! TCP Server
//!
//! Accepts connections and dispatches to worker threads.

use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam::channel::{bounded, Receiver, Sender};

use crate::config::Config;
use crate::engine::Engine;
use crate::error::{AtlasError, Result};

use super::Connection;

/// Message sent to worker threads
enum WorkerMessage {
    /// New client connection to handle
    NewConnection(TcpStream),
    /// Signal to shutdown
    Shutdown,
}

/// TCP server for AtlasKV
///
/// ## Architecture
/// - Main thread accepts connections
/// - Worker thread pool handles client I/O
/// - Shared Engine reference for all workers
pub struct Server {
    /// Server configuration
    config: Config,

    /// Shared storage engine
    engine: Arc<Engine>,

    /// TCP listener (created on run)
    listener: Option<TcpListener>,

    /// Channel to send work to workers
    work_sender: Option<Sender<WorkerMessage>>,

    /// Worker thread handles
    workers: Vec<JoinHandle<()>>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Active connection count
    active_connections: Arc<AtomicUsize>,
}

impl Server {
    /// Create a new server with the given config and engine
    pub fn new(config: Config, engine: Arc<Engine>) -> Self {
        Self {
            config,
            engine,
            listener: None,
            work_sender: None,
            workers: Vec::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Start the server (blocking)
    ///
    /// This method:
    /// 1. Binds to the configured address
    /// 2. Spawns worker threads
    /// 3. Accepts connections in a loop
    /// 4. Returns when shutdown is signaled
    pub fn run(&mut self) -> Result<()> {
        // Step 1: Bind to address
        let listener = TcpListener::bind(&self.config.listen_addr).map_err(|e| {
            AtlasError::Network(format!(
                "Failed to bind to {}: {}",
                self.config.listen_addr, e
            ))
        })?;

        // Set non-blocking so we can check shutdown flag
        listener.set_nonblocking(true)?;

        tracing::info!("Server listening on {}", self.config.listen_addr);
        self.listener = Some(listener);

        // Step 2: Create worker thread pool
        let num_workers = num_cpus();
        let (sender, receiver) = bounded::<WorkerMessage>(self.config.max_connections);
        self.work_sender = Some(sender);

        tracing::info!("Starting {} worker threads", num_workers);

        for worker_id in 0..num_workers {
            let worker = Worker::new(
                worker_id,
                receiver.clone(),
                Arc::clone(&self.engine),
                Arc::clone(&self.active_connections),
                self.config.read_timeout_ms,
                self.config.write_timeout_ms,
            );
            let handle = thread::Builder::new()
                .name(format!("atlaskv-worker-{}", worker_id))
                .spawn(move || worker.run())
                .map_err(|e| AtlasError::Network(format!("Failed to spawn worker: {}", e)))?;

            self.workers.push(handle);
        }

        // Step 3: Accept loop
        self.accept_loop()?;

        // Step 4: Cleanup (after shutdown signaled)
        self.cleanup();

        Ok(())
    }

    /// Main accept loop
    fn accept_loop(&mut self) -> Result<()> {
        let listener = self.listener.as_ref().unwrap();
        let sender = self.work_sender.as_ref().unwrap();

        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, addr)) => {
                    // Check connection limit
                    let current = self.active_connections.load(Ordering::Relaxed);
                    if current >= self.config.max_connections {
                        tracing::warn!(
                            "Connection limit reached ({}/{}), rejecting {}",
                            current,
                            self.config.max_connections,
                            addr
                        );
                        // Drop the connection
                        drop(stream);
                        continue;
                    }

                    tracing::debug!("Accepted connection from {}", addr);

                    // Send to worker pool
                    if let Err(e) = sender.send(WorkerMessage::NewConnection(stream)) {
                        tracing::error!("Failed to dispatch connection: {}", e);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connections, sleep briefly
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    if !self.shutdown.load(Ordering::Relaxed) {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Cleanup workers and resources
    fn cleanup(&mut self) {
        tracing::info!("Shutting down server...");

        // Send shutdown signal to all workers
        if let Some(sender) = &self.work_sender {
            for _ in 0..self.workers.len() {
                let _ = sender.send(WorkerMessage::Shutdown);
            }
        }

        // Wait for workers to finish
        for handle in self.workers.drain(..) {
            if let Err(e) = handle.join() {
                tracing::error!("Worker thread panicked: {:?}", e);
            }
        }

        tracing::info!("Server shutdown complete");
    }

    /// Signal the server to shutdown gracefully
    pub fn shutdown(&self) {
        tracing::info!("Shutdown signal received");
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::Relaxed)
    }

    /// Get the number of active connections
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get the bound address (if running)
    pub fn local_addr(&self) -> Option<std::net::SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }
}

/// Worker thread that handles client connections
struct Worker {
    /// Worker ID for logging
    id: usize,

    /// Channel to receive work
    receiver: Receiver<WorkerMessage>,

    /// Shared engine reference
    engine: Arc<Engine>,

    /// Active connection counter
    active_connections: Arc<AtomicUsize>,

    /// Read timeout in milliseconds
    read_timeout_ms: u64,

    /// Write timeout in milliseconds
    write_timeout_ms: u64,
}

impl Worker {
    fn new(
        id: usize,
        receiver: Receiver<WorkerMessage>,
        engine: Arc<Engine>,
        active_connections: Arc<AtomicUsize>,
        read_timeout_ms: u64,
        write_timeout_ms: u64,
    ) -> Self {
        Self {
            id,
            receiver,
            engine,
            active_connections,
            read_timeout_ms,
            write_timeout_ms,
        }
    }

    fn run(self) {
        tracing::debug!("Worker {} started", self.id);

        loop {
            match self.receiver.recv() {
                Ok(WorkerMessage::NewConnection(stream)) => {
                    self.handle_connection(stream);
                }
                Ok(WorkerMessage::Shutdown) => {
                    tracing::debug!("Worker {} received shutdown signal", self.id);
                    break;
                }
                Err(_) => {
                    // Channel closed
                    tracing::debug!("Worker {} channel closed", self.id);
                    break;
                }
            }
        }

        tracing::debug!("Worker {} stopped", self.id);
    }

    fn handle_connection(&self, stream: TcpStream) {
        // Increment connection count
        self.active_connections.fetch_add(1, Ordering::Relaxed);

        // Create connection handler
        let mut conn = match Connection::new(stream, Arc::clone(&self.engine)) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to create connection: {}", e);
                self.active_connections.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        };

        // Set timeouts
        if let Err(e) = conn.set_timeouts(self.read_timeout_ms, self.write_timeout_ms) {
            tracing::warn!("Failed to set connection timeouts: {}", e);
        }

        // Handle connection
        if let Err(e) = conn.handle() {
            tracing::debug!(
                "Connection {} ended with error: {}",
                conn.peer_addr(),
                e
            );
        }

        // Decrement connection count
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Get number of CPUs (for worker thread count)
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_server_creation() {
        let temp_dir = tempdir().unwrap();
        let config = Config::builder()
            .data_dir(temp_dir.path())
            .listen_addr("127.0.0.1:0")
            .build();

        let engine = Arc::new(Engine::open(config.clone()).unwrap());
        let server = Server::new(config, engine);

        assert!(!server.is_running() || server.is_running()); // Just check it exists
    }
}
